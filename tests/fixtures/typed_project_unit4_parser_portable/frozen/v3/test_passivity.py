#!/usr/bin/env python3
"""Synthetic checks for the newly required fresh ordinary passivity receipts."""
import copy
import json
import os
from pathlib import Path
import sys
import tempfile
import time
import unittest

import portable as p
from synthetic_receipts import build_fixture


def append_passivity_fixture(f):
    root, a, toolchain, profile = f['root'], f['A'], f['TOOLCHAIN'], f['profile']
    out = root / ('build-control-' + profile); out.mkdir(); (out / 'home').mkdir()
    p.cargo_cache(f['CACHE'], out / 'cargo-home', a)
    view = p.create_executable_view(out / 'rust-bin', toolchain, a)
    env = p.minimal_env(out / 'home', out / 'cargo-home', toolchain, view)
    source = root / 'control-source'; result = out / 'result'
    def save(path, value):
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(value if isinstance(value, bytes) else (json.dumps(value, sort_keys=True) + '\n').encode())
        return p.identity(path)
    started = time.time_ns()
    binary = result / 'target' / a['recipe']['target'] / profile / 'deps/oxid-c0ffee'
    body = bytearray(Path(f['build']['binary']['path']).read_bytes()); body[-1] = 1
    binary_id = save(binary, bytes(body)); finished = time.time_ns()
    cargo = copy.deepcopy(f['cargo'])
    cargo.update(package_id='path+' + source.as_uri() + '#oxid@0.9.0', manifest_path=str(source / 'Cargo.toml'),
                 filenames=[str(binary)], executable=str(binary))
    cargo['target']['src_path'] = str(source / 'src/cli.rs')
    build = copy.deepcopy(f['build'])
    build.update(control=True, cwd=str(source), argv=[str(view / 'cargo'), *a['recipe']['cargo_tail'], *(['--release'] if profile == 'release' else [])],
                 overlay_manifest=p.identity(source / 'overlay-manifest.json'), binary=binary_id,
                 stdout=save(result / 'stdout.jsonl', (json.dumps(cargo) + '\n{"reason":"build-finished","success":true}\n').encode()),
                 stderr=save(result / 'stderr.txt', b''))
    invocation = {'argv': [sys.executable, '-B', str(root / 'helpers/build.py'), '--overlay', str(source / 'overlay-manifest.json'),
                           '--profile', profile, '--output', str(result)], 'cwd': str(root), 'environment': env,
                  'started_ns': started, 'finished_ns': finished, 'exit_code': 0, 'host': f['session']['host'],
                  'stdout': save(out / 'driver.stdout', b'SYNTHETIC control build; zero compilations\n'),
                  'stderr': save(out / 'driver.stderr', b''), 'tools': p.process_tools(env)}
    envelope = {'schema': 'oxid-unit4-portable-parser-build-v1', 'session': p.identity(f['session_path']), 'profile': profile,
                'control': True, 'toolchain': str(toolchain), 'executable_view': str(view), 'rust_sysroot': p.verify_sysroot(view, toolchain, env),
                'invocation': save(out / 'invocation.json', invocation), 'receipt': save(result / 'build-receipt.json', build)}
    save(out / 'portable-build.json', envelope)
    p.verify_build(root, f['session_path'], profile, a, control=True)
    out = root / ('passivity-' + profile); result = out / 'result'; results = []
    started = time.time_ns()
    for name, source_text in a['recipe']['passivity']['cases']:
        source_id = save(result / (name + '.ox'), source_text.encode()); receipts = []
        for index, actual_build in enumerate((f['build'], build)):
            work = result / (name + ('-control' if index else '-instrumented'))
            nonce = p.sha((profile + name + str(index)).encode())[:32]
            raw = copy.deepcopy(f['raw'])
            raw.update(case_id=name, source_utf8=source_text, display_path=name + '.ox', nonce=nonce)
            raw['observations'] = [copy.deepcopy(raw['observations'][0]), copy.deepcopy(raw['observations'][0])]
            for mode_index, mode in enumerate(a['recipe']['passivity']['modes']):
                raw['observations'][mode_index].update(mode=mode, mode_execution_index=mode_index)
            receipts.append({'argv': [actual_build['binary']['path'], a['recipe']['entrypoint'], '--exact', '--ignored', '--nocapture', '--test-threads=1'],
                             'exit_code': 0, 'raw': save(work / 'raw.json', raw),
                             'stdout': save(work / 'stdout.txt', ('UNIT4_EXECUTED ' + nonce + '\ntest result: ok. 1 passed; 0 failed; 0 ignored;\n').encode()),
                             'stderr': save(work / 'stderr.txt', b'')})
        results.append({'case': name, 'source': source_id, 'pairs': 2, 'equal_fields': a['recipe']['passivity']['compare_fields'], 'receipts': receipts})
    report = {'schema': 'oxid-unit4-parser-passivity-v1', 'status': 'pass',
              'scope': 'three hand-prescribed ordinary controls in both modes; no seam equivalence or full-corpus assertion',
              'instrumented': f['portable_build']['receipt'], 'control': envelope['receipt'],
              'authority_checkpoint': p.identity(f['session_path']), 'results': results}
    env = p.minimal_env(root / 'home')
    invocation = {'argv': [sys.executable, '-B', str(root / 'helpers/passivity.py'), '--instrumented', str(root / ('build-' + profile) / 'result/build-receipt.json'),
                           '--control', str(root / ('build-control-' + profile) / 'result/build-receipt.json'), '--checkpoint', str(f['session_path']), '--output', str(result)],
                  'cwd': str(root), 'environment': env, 'started_ns': started, 'finished_ns': time.time_ns(), 'exit_code': 0,
                  'host': f['session']['host'], 'tools': p.process_tools(env),
                  'stdout': save(out / 'driver.stdout', b'SYNTHETIC ordinary passivity; zero candidate executions\n'), 'stderr': save(out / 'driver.stderr', b'')}
    bound = {'schema': 'oxid-unit4-portable-parser-passivity-v1', 'session': p.identity(f['session_path']), 'profile': profile,
             'invocation': save(out / 'invocation.json', invocation), 'report': save(result / 'report.json', report)}
    save(out / 'portable-passivity.json', bound)
    return result / 'report.json'


class PassivityControls(unittest.TestCase):
    def test_six_pairs_and_coherently_rehashed_negative_controls(self):
        records = []
        with tempfile.TemporaryDirectory(prefix='oxid-passivity-controls-') as temp:
            f = build_fixture(Path(temp) / 'session')
            report_path = append_passivity_fixture(f)
            root, a = f['root'], f['A']; result = report_path.parent
            got = p.verify_passivity(root, f['session_path'], 'debug', a, set())
            self.assertEqual(got['pairs'], 6)
            records.append({'name': 'synthetic_six_pairs_admitted', 'status': 'pass'})
            saved = {path: path.read_bytes() for path in (root / 'passivity-debug').rglob('*') if path.is_file()}
            def restore():
                for path, raw in saved.items(): path.write_bytes(raw)
                for path in result.rglob('*'):
                    if path.is_file() and path not in saved: path.unlink()
            def rewrite(path, fn):
                value = p.read(path); fn(value); p.write(path, value)
            def check(name, mutate, reuse=None):
                restore(); mutate()
                report = p.read(report_path)
                for row in report['results']:
                    row['source'] = p.identity(row['source']['path'])
                    for receipt in row['receipts']:
                        for key in ('stdout', 'stderr', 'raw'):
                            if Path(receipt[key]['path']).exists(): receipt[key] = p.identity(receipt[key]['path'])
                p.write(report_path, report)
                bound = root / 'passivity-debug/portable-passivity.json'
                rewrite(bound, lambda v: v.__setitem__('report', p.identity(report_path)))
                try: p.verify_passivity(root, f['session_path'], 'debug', a, set() if reuse is None else reuse)
                except (ValueError, OSError, KeyError, TypeError, IndexError) as error:
                    records.append({'name': name, 'status': 'pass', 'rejection': str(error)})
                else:
                    records.append({'name': name, 'status': 'fail'}); self.fail('invalid passivity receipt admitted: ' + name)
            raw = result / 'original-control/raw.json'
            check('ordinary_field_diff', lambda: rewrite(raw, lambda v: v['observations'][0].__setitem__('result', 'parse_error')))
            check('missing_ordinary_case', lambda: rewrite(report_path, lambda v: v['results'].pop()))
            check('missing_parser_mode', lambda: rewrite(raw, lambda v: v['observations'].pop()))
            check('wrong_source_same_path', lambda: (result / 'original.ox').write_bytes(b'changed source'))
            check('wrong_raw_source', lambda: rewrite(raw, lambda v: v.__setitem__('source_utf8', 'changed source')))
            check('zero_execution_witness', lambda: (result / 'original-control/stdout.txt').write_bytes(b'test result: ok. 0 passed; 0 failed; 0 ignored;\n'))
            check('wrong_abi', lambda: rewrite(raw, lambda v: v['observations'][0].__setitem__('pointer_width', 32)))
            check('missing_raw_evidence', lambda: raw.unlink())
            check('extra_evidence', lambda: (result / 'EXTRA').write_bytes(b'extra'))
            nonce = p.sha(b'debugoriginal0')[:32]
            check('nonce_reused_by_other_gate', lambda: None, reuse={nonce})
        if os.environ.get('UNIT4_PORTABLE_PASSIVITY_REPORT'):
            p.write(os.environ['UNIT4_PORTABLE_PASSIVITY_REPORT'], {'schema': 'oxid-portable-passivity-synthetic-controls-v2',
                    'status': 'pass', 'records': records, 'compiler_build_executions': 0, 'candidate_executions': 0,
                    'native_and_rust_identity_probes': True})


if __name__ == '__main__':
    unittest.main(verbosity=2)
