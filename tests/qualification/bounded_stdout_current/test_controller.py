"""Bounded controller refusal controls; no Rust or LLVM builds."""
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from types import SimpleNamespace
from unittest import mock

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(ROOT / 'scripts'))
import verify_bounded_stdout_native as gate

c = gate.c
controls = gate.controls


class SelectionTests(unittest.TestCase):
    def test_discovery_rejects_zero_duplicate_missing_and_extra(self):
        name = controls.RAW
        good = (name + ': test\n\n1 test, 0 benchmarks\n').encode()
        controls.admit_listing(good, [name])
        for bad in (b'0 tests, 0 benchmarks\n', good + good,
                    good.replace(b'1 test,', b'2 tests,'), good.replace(name.encode(), b'wrong')):
            with self.subTest(bad=bad), self.assertRaises(ValueError):
                controls.admit_listing(bad, [name])
        for names in ([], [name, name]):
            with self.assertRaises(ValueError):
                controls.admit_listing(good, names)

    def test_direct_exit_children_cannot_pass_as_inert_or_zero_tests(self):
        name = controls.RAW
        good = ('\nrunning 1 test\ntest ' + name + ' ... ').encode()
        controls.admit_selected_harness(good, name)
        for bad in (b'', good.replace(b'1 test', b'0 tests'), good + b'ok\n',
                    good.replace(name.encode(), controls.SOURCE.encode())):
            with self.assertRaises(ValueError):
                controls.admit_selected_harness(bad, name)

    def test_printing_selected_test_uses_capture_and_strict_completion(self):
        name = 'frontend::oir::owned::execute::output_tests::output_reference_carriers_keep_the_existing_frame_reservations'
        argv = controls.selection_command('/unit', name)
        self.assertEqual(argv, ['/unit', name, '--exact', '--test-threads=1', '--color=never'])
        # A successful printing test under ordinary libtest capture has exactly
        # this completion. Leaked measurements must still fail strict admission.
        captured = (f'\nrunning 1 test\ntest {name} ... ok\n\n'
                    'test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 1521 filtered out; finished in 0.00s\n').encode()
        gate.enum_gate.admit_execution(captured, name)
        leaked = captured.replace(b' ... ok', b' ... output reference carriers: Frame=272\nok')
        with self.assertRaises(ValueError):
            gate.enum_gate.admit_execution(leaked, name)
        for _, parent in controls.PARENTS:
            self.assertEqual(controls.selection_command('/unit', parent),
                             ['/unit', parent, '--exact', '--test-threads=1', '--color=never', '--ignored', '--nocapture'])
        # Effectful direct-exit children retain nocapture and remain separate
        # from ordinary selected-test execution.
        self.assertIn('--nocapture', c.test_command('/unit', controls.RAW))

    def test_all_effect_selectors_and_preloads_are_removed_before_inert_run(self):
        env = {'PATH': 'ok', 'HOME': 'h', 'LD_PRELOAD': 'bad', 'LD_AUDIT': 'bad',
               'OXID_RAW_STDOUT_CASE': 'abc', 'OXID_SOURCE_STDOUT_PATH': 'bad',
               'OXID_RAW_STDIN_MODE': 'bad', 'OXID_PRIVATE_SOURCE_STDIN': 'bad',
               'OXID_SETUP_EXE_INO': 'bad', 'OXID_FAULT_STDOUT_INO': 'bad',
               'OXID_OUTPUT_FAULT': 'bad', 'OXID_PATH': 'bad'}
        self.assertEqual(controls.clean_env(env), {'PATH': 'ok', 'HOME': 'h'})
        selected = controls.selected_env(env, 'raw', 'abc', 83)
        self.assertEqual(selected, {'PATH': 'ok', 'HOME': 'h', 'OXID_RAW_STDOUT_CASE': 'abc', 'OXID_RAW_STDOUT_FUEL': '83'})

    def test_independent_rosters_and_corrected_projection_are_explicit(self):
        roster = json.loads(c.read_file(controls.HELPERS / 'roster.json'))
        self.assertEqual([len(rows) for rows in roster['modules'].values()], [9, 6, 10, 13, 8, 7])
        self.assertEqual(len(roster['named']), 16)
        self.assertEqual((len(controls.RAW_CASES), len(controls.FUEL_CASES), len(controls.SOURCE_CASES),
                          len(controls.REAL_CASES), len(controls.INJECTED_CASES)), (10, 28, 11, 5, 8))
        self.assertIn(('projected', 179, b'ABC', 0), controls.FUEL_CASES)
        self.assertNotIn(181, [fuel for case, fuel, _, _ in controls.FUEL_CASES if case == 'projected'])
        self.assertEqual(list(controls.HELPERS.glob('*.ox')), [])
        self.assertIn(b'179 is the expectation', c.read_file(controls.HELPERS / 'fuel-oracle.md'))

    def test_partial_effects_and_stderr_cannot_be_normalized(self):
        controls.admit_effect(subprocess.CompletedProcess([], 74, b'ABC', b'e'), 74, b'ABC', 'one')
        for status, output, error in ((0, b'ABC', b'e'), (74, b'', b'e'), (74, b'ABC', b'error')):
            with self.assertRaises(ValueError):
                controls.admit_effect(subprocess.CompletedProcess([], status, output, error), 74, b'ABC', 'one')
        with self.assertRaises(ValueError):
            controls.admit_effect(subprocess.CompletedProcess([], 0, b'ABC0\n', b''), 0, b'ABC')


class EvidenceTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix='bounded-stdout-controls-')
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)

    def test_source_head_tree_event_and_complete_manifest_receipt_are_required(self):
        manifest = {'path': '/repo/current-source.json', 'bytes': 12, 'sha256': 'a' * 64}
        reviewed = [{'path': '/repo/src/a.rs', 'bytes': 1, 'sha256': 'b' * 64}]
        previous = {'head': 'head', 'tree': 'tree', 'event_sha': 'event',
                    'reviewed_source_manifest': manifest, 'reviewed_inputs': reviewed}
        gate.admit_source_receipt(previous, 'head', 'tree', 'event', manifest, reviewed)
        for field, value in (('head', 'old'), ('tree', 'other'), ('event_sha', 'old'),
                             ('reviewed_source_manifest', {}), ('reviewed_inputs', [])):
            bad = dict(previous, **{field: value})
            with self.subTest(field=field), self.assertRaises(ValueError):
                gate.admit_source_receipt(bad, 'head', 'tree', 'event', manifest, reviewed)

    def test_current_source_closure_and_source_seal_are_reused(self):
        self.assertEqual(gate.REVIEWED_SOURCE_SHA256, gate.stdin_gate.REVIEWED_SOURCE_SHA256)
        manifest = gate.stdin_gate.source_manifest(ROOT, gate.REVIEWED_SOURCE_SHA256)
        core = [row['path'] for row in manifest['files'] if row['path'].startswith(('src/', 'native/'))
                or row['path'] in ('Cargo.toml', 'Cargo.lock', 'build.rs')]
        self.assertEqual((len(manifest['files']), len(core)), (345, 260))
        listing = b'\0'.join(name.encode() for name in core) + b'\0'
        self.assertEqual(len(gate.stdin_gate.source_identity(ROOT, manifest, listing)), 345)
        for bad in (listing + b'src/extra.rs\0', listing + b'src/cli.rs\0', listing.split(b'\0', 1)[1]):
            with self.assertRaises(ValueError):
                gate.stdin_gate.source_identity(ROOT, manifest, bad)
        with self.assertRaises(ValueError):
            gate.stdin_gate.source_manifest(ROOT, '0' * 64)

    def test_stale_and_rehashed_source_seals_reject_before_git_or_native_execution(self):
        current = gate.stdin_gate.source_manifest(ROOT, gate.REVIEWED_SOURCE_SHA256)
        wrong = dict(current, reviewed_source_head="c15465acb90e9f8bb18f5291a8931f5d5bbc6edb")
        forged = hashlib.sha256((json.dumps(wrong, sort_keys=True, indent=2) + "\n").encode()).hexdigest()
        for digest in ("35e7e43cb1ef5de8be0c1a78d9e5ac70b1a2caf2efe1e37ba05b7445916c2e29", forged):
            args = SimpleNamespace(repo=self.root, llvm_bin=self.root, build_evidence=self.root,
                                   target_dir=self.root, source_manifest_sha256=digest)
            with self.subTest(digest=digest), \
                 mock.patch.object(gate.platform, "system", return_value="Linux"), \
                 mock.patch.object(gate.platform, "machine", return_value="x86_64"), \
                 mock.patch.dict(os.environ, {"RUSTFLAGS": "", "CARGO_ENCODED_RUSTFLAGS": ""}), \
                 mock.patch.object(gate.enum_gate, "git_command", side_effect=AssertionError("Git must not run")), \
                 mock.patch.object(c, "run", side_effect=AssertionError("native process must not run")), \
                 self.assertRaisesRegex(ValueError, "unreviewed stdout source seal"):
                gate.verify(args, self.root)

    def build_evidence(self):
        repo, target, evidence = self.root / 'repo', self.root / 'target', self.root / 'evidence'
        previous = c.fresh(evidence / 'debug')
        identities = {'profile': 'debug'}
        for kind, unit in (('unit', True), ('cli', False)):
            binary = target / ('debug/deps/oxid-0123456789abcdef' if unit else 'debug/oxid')
            binary.parent.mkdir(parents=True, exist_ok=True)
            binary.write_bytes(b'\x7fELFfixture-' + kind.encode())
            identities['unit' if unit else 'production_cli'] = c.identity(binary)
            row = {'reason': 'compiler-artifact', 'target': {'name': 'oxid', 'kind': ['bin'], 'src_path': str(repo / 'src/cli.rs')},
                   'profile': {'test': unit, 'opt_level': '0', 'debug_assertions': True, 'overflow_checks': True, 'debuginfo': 2},
                   'executable': str(binary)}
            (previous / (kind + '-build.stdout')).write_text(json.dumps(row) + '\n{"reason":"build-finished","success":true}\n')
            (previous / (kind + '-build.stderr')).write_bytes(b'')
            args = ['cargo', 'test', '--locked', '--bin', 'oxid', '--no-run', '--message-format=json'] if unit else ['cargo', 'build', '--locked', '--bin', 'oxid', '--message-format=json']
            (previous / (kind + '-build.json')).write_text(json.dumps({'status': 0, 'cwd': str(repo), 'argv': args}))
        (previous / 'binaries.json').write_text(json.dumps(identities))
        return repo, target, evidence

    def test_prebuilt_receipts_reject_modified_binary_and_failed_command(self):
        repo, target, evidence = self.build_evidence()
        found = gate.stdin_gate.reused_binaries(evidence, c.fresh(self.root / 'accepted'), repo, target, 'debug')
        self.assertEqual(found['cli'], target / 'debug/oxid')
        found['cli'].write_bytes(b'\x7fELFchanged')
        with self.assertRaisesRegex(ValueError, 'identity changed'):
            gate.stdin_gate.reused_binaries(evidence, c.fresh(self.root / 'modified'), repo, target, 'debug')
        found['cli'].write_bytes(b'\x7fELFfixture-cli')
        receipt = evidence / 'debug/unit-build.json'
        data = json.loads(receipt.read_text())
        for field, bad in (('status', 1), ('cwd', '/wrong'), ('argv', data['argv'] + ['--release'])):
            receipt.write_text(json.dumps(dict(data, **{field: bad})))
            with self.subTest(field=field), self.assertRaises(ValueError):
                gate.stdin_gate.reused_binaries(evidence, c.fresh(self.root / field), repo, target, 'debug')

    def test_cargo_profile_and_ambiguous_artifact_refusals(self):
        repo, target, evidence = self.build_evidence()
        path = evidence / 'debug/unit-build.stdout'
        raw = path.read_bytes()
        for data, profile in ((raw, 'release'), (raw + raw, 'debug'),
                              (raw.replace(b'"success":true', b'"success":false'), 'debug'),
                              (raw.replace(b'"debug_assertions": true', b'"debug_assertions": false'), 'debug'),
                              (raw.replace(b'"test": true', b'"test": false'), 'debug')):
            with self.subTest(profile=profile), self.assertRaises(ValueError):
                gate.stdin_gate.admit_build_artifact(data, repo, target, profile, True)

    def test_captures_preserve_program_bytes_and_separate_harness(self):
        child = controls.RAW
        opening = '\nrunning 1 test\ntest ' + child + ' ... '
        script = 'import os;os.write(1,' + repr(opening.encode()) + ');os.dup2(int(os.environ["OXID_RAW_STDOUT_FD"]),1);os.write(1,b"ABC");os.write(2,b"e");os._exit(74)'
        result, _ = controls.capture(self.root / 'capture', [str(Path(sys.executable).resolve()), '-c', script], c.ELF_ENV, child=child)
        self.assertEqual((result.returncode, result.stdout, result.stderr), (74, b'ABC', b'e'))
        self.assertEqual((self.root / 'capture/harness.stdout').read_text(), opening)

    def test_actual_nonblocking_pipe_has_exactly_one_free_byte(self):
        script = 'import os\nn=0\ntry:\n for b in b"ABC":\n  os.write(1,bytes([b]));n+=1\nexcept OSError: os._exit(n+2)'
        result, _ = controls.capture(self.root / 'pipe', [str(Path(sys.executable).resolve()), '-c', script], c.ELF_ENV, mode='partial-output')
        self.assertEqual((result.returncode, result.stdout, result.stderr), (3, b'A', b''))
        receipt = json.loads((self.root / 'pipe/receipt.json').read_text())
        self.assertEqual(receipt['kind'], 'real-process')

    def test_timeout_keeps_terminal_receipt_and_raw_streams(self):
        with self.assertRaisesRegex(ValueError, 'process failure'):
            controls.capture(self.root / 'timeout', [str(Path(sys.executable).resolve()), '-c', 'import time;time.sleep(5)'], c.ELF_ENV, timeout=0.03)
        receipt = json.loads((self.root / 'timeout/receipt.json').read_text())
        self.assertEqual(receipt['status'], -9)
        self.assertIn('timed out', receipt['error'])
        self.assertTrue((self.root / 'timeout/stdout').is_file())
        self.assertTrue((self.root / 'timeout/stderr').is_file())


if __name__ == '__main__':
    unittest.main()
