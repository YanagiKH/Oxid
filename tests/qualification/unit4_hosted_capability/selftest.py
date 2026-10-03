#!/usr/bin/env python3
"""Source-only hosted-controller inventory controls; no credentials/actions."""
import sys
sys.dont_write_bytecode = True
import copy
import os
import tempfile
import time
from pathlib import Path
import unittest
from hosted import replacement_map
from unittest import mock
import types
import json
import hosted

class Reject(Exception):
    pass

def need(value, message):
    if not value:
        raise Reject(message)

class ExactJoinControls(unittest.TestCase):
    def test_exact_twelve_domain(self):
        keys = ['public/' + str(i) for i in range(6)] + ['lifecycle/' + str(i) for i in range(6)]
        rows = [{'key': key, 'status': 'pass', 'executed': True} for key in keys]
        self.assertEqual(set(replacement_map(keys, rows, need)), set(keys))
        self.assertEqual(replacement_map([], [], need), {})
        variants = [[], rows[1:], rows + [rows[0]], rows + [{'key': 'extra', 'status': 'pass', 'executed': True}]]
        for field, value in [('status', 'unsupported-capability'), ('status', 'fail'), ('executed', False), ('key', 'other-profile')]:
            variant = copy.deepcopy(rows)
            variant[0][field] = value
            variants.append(variant)
        for variant in variants:
            with self.assertRaises(Reject): replacement_map(keys, variant, need)
        with self.assertRaises(Reject): replacement_map([], rows, need)

class ReadonlyClosureControls(unittest.TestCase):
    def test_manifest_sources_cannot_bypass_ledger(self):
        adapter = Path(__file__).resolve().parent.parent / 'unit4_public_v3'
        c, r, _, _, _ = hosted.modules(adapter)
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            ordinary = root / 'ordinary'
            observer = root / 'observer'
            cfgs = []
            for location in (ordinary, observer):
                location.mkdir()
                source = location / 'Cargo.lock'
                source.write_bytes(b'original source bytes')
                manifest = location / 'manifest.json'
                c.save(manifest, {'files': [{**c.binding(source), 'path': 'Cargo.lock'}]})
                binary = location / 'binary'
                binary.write_bytes(b'controlled fixture; never executed')
                stream = location / 'stdout'
                stream.write_bytes(b'controlled build output')
                recipe = location / 'recipe.json'
                c.save(recipe, {'streams': {'stdout': c.binding(stream)}})
                cfg = location / 'config.json'
                c.save(cfg, {'source_root': str(location), 'source_manifest': c.binding(manifest),
                            'binaries': {'debug': c.binding(binary)}, 'build_receipts': {'debug': c.binding(recipe)}})
                cfgs.append(c.binding(cfg))
            packages = root / 'packages'
            packages.mkdir()
            inputs = {'adapter': str(adapter), 'controller': str(Path(__file__).resolve().parent),
                      'contract_package': str(packages), 'amendment_root': None, 'trap_binding': str(root / 'trap.json')}
            for name in ('README.md', 'authorities.tar.gz', 'publication-summary.json', 'test_transport.py', 'transport-manifest.json', 'transport.py'):
                (packages / name).write_bytes(b'controlled input fixture')
            c.save(inputs['trap_binding'], {'source': c.binding(ordinary / 'Cargo.lock'), 'binary': c.binding(ordinary / 'binary')})
            expected = hosted.copy_input_inventory(inputs, *cfgs, c, lambda path: path)
            for missing in (ordinary / 'Cargo.lock', observer / 'Cargo.lock', ordinary / 'stdout', packages / 'transport.py'):
                declared = set(expected) - {str(missing)}
                def physical(path):
                    c.need(path in declared, 'unlisted readonly transport read: ' + path)
                    return path
                with self.assertRaises(c.Reject):
                    hosted.copy_input_inventory(inputs, *cfgs, c, physical)
                self.assertTrue(missing.is_file())

class GroupControls(unittest.TestCase):
    def test_nested_timeout_and_stream_limit_are_confined(self):
        adapter = Path(__file__).resolve().parent.parent / 'unit4_public_v3'
        sys.path.insert(0, str(adapter))
        import runtime
        controller = Path(__file__).resolve().parent
        parent_group = os.getpgrp()
        cases = [('outer-timeout', 0.35, 30, 4096), ('inner-timeout', 5, 0.25, 4096), ('inner-stream-limit', 5, 5, 16)]
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            for name, outer_timeout, inner_timeout, limit in cases:
                marker = root / (name + '.pid')
                inner = "import os,sys,time; open(sys.argv[1],'w').write(str(os.getpid())); " + ("sys.stdout.write('x'*1000000);sys.stdout.flush();" if name == 'inner-stream-limit' else "") + "time.sleep(30)"
                worker = "import sys;sys.dont_write_bytecode=True;sys.path.insert(0,sys.argv[1]);sys.path.insert(0,sys.argv[2]);import hosted,runtime;hosted.install_worker_group_policy(runtime,lambda x,m: x or (_ for _ in ()).throw(RuntimeError(m)));runtime.process([sys.executable,'-c',sys.argv[3],sys.argv[4]],sys.argv[5],__import__('os').environ,timeout=float(sys.argv[6]),limit=int(sys.argv[7]))"
                result = runtime.process([sys.executable, '-c', worker, str(adapter), str(controller), inner, str(marker), str(root), str(inner_timeout), str(limit)], root, os.environ, timeout=outer_timeout)
                self.assertNotEqual(result['status'], 0, name)
                self.assertTrue(marker.exists(), name)
                pid = int(marker.read_text())
                status = Path('/proc') / str(pid) / 'stat'
                deadline = time.monotonic() + 2
                def gone():
                    try: return status.read_text().split()[2] == 'Z'
                    except FileNotFoundError: return True
                while not gone() and time.monotonic() < deadline: time.sleep(0.02)
                self.assertTrue(gone(), name)
                self.assertEqual(os.getpgrp(), parent_group)

class RootBranchControls(unittest.TestCase):
    def test_root_launch_flags_and_failure_are_bounded(self):
        adapter = Path(__file__).resolve().parent.parent / 'unit4_public_v3'
        c, r, _, _, _ = hosted.modules(adapter)
        uid_before = os.geteuid()
        for preflight_status in (0, 20):
            with tempfile.TemporaryDirectory() as directory:
                root = Path(directory)
                main_root = root / 'main'
                main_root.mkdir()
                c.save(main_root / 'comparison.json', {'all_contract_sections_collected': True, 'status': 'INCOMPLETE'})
                child = root / 'fresh-child'
                def fresh(*args, **kwargs):
                    child.mkdir()
                    return str(child)
                calls = []
                ownership = []
                def fake_process(argv, cwd, env, **kwargs):
                    calls.append(argv)
                    is_probe = 'probe' in argv
                    return {'argv': argv, 'cwd': str(cwd), 'pid': 9001, 'status': preflight_status if is_probe else 71,
                            'stdout': json.dumps({'status': 'READ_ACCESS_VERIFIED' if preflight_status == 0 else 'READ_ACCESS_DENIED'}),
                            'stderr': '', 'timed_out': False, 'stream_limit_exceeded': False}
                args = types.SimpleNamespace(adapter=str(adapter),out=str(root/'result'),main_root=str(main_root),
                    existing_user='nobody',ordinary_binding=str(root/'ordinary.json'),observer_binding=str(root/'observer.json'),
                    expected_head='f'*40,contract_package=str(root/'package'),amendment_root=None,copy_if_unreadable=False)
                with mock.patch.object(hosted.os, 'geteuid', return_value=0), mock.patch.object(hosted.tempfile, 'mkdtemp', side_effect=fresh), \
                     mock.patch.object(hosted.os, 'chown', side_effect=lambda path,uid,gid: ownership.append((Path(path),uid,gid))), \
                     mock.patch.object(hosted, 'read_rows', side_effect=lambda path,section: [{'status':'unsupported-capability'}]*6 if section in ('public','lifecycle') else []), \
                     mock.patch.object(r, 'setup_no_tool_traps', return_value=({},{})), mock.patch.object(r, 'process', side_effect=fake_process), \
                     mock.patch.object(hosted.shutil, 'which', return_value='/usr/bin/setpriv'):
                    with self.assertRaises(c.Reject): hosted.orchestrate(args)
                self.assertTrue(calls)
                self.assertTrue(all('--clear-groups' in argv and '--no-new-privs' in argv for argv in calls))
                self.assertEqual(len(calls), 2 if preflight_status == 0 else 1)
                self.assertTrue(all(path == child or child in path.parents for path,uid,gid in ownership))
                self.assertFalse((root/'result/comparison.json').exists())
        self.assertEqual(os.geteuid(), uid_before)

if __name__ == '__main__':
    unittest.main()
