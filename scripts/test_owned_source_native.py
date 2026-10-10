#!/usr/bin/env python3
"""Temporary-fixture controls for the bounded source-native runner."""
import io
import json
import os
from pathlib import Path
import sys
import tempfile
import time
import unittest
from types import SimpleNamespace

import verify_owned_source_native as runner


class RunnerControls(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)

    def command(self, name, code):
        return runner.Command(name, (sys.executable, '-c', code), self.root, {})

    def run_commands(self, commands, **kwargs):
        return runner.run_commands(commands, self.root / 'logs', output=io.StringIO(), **kwargs)

    def test_missing_duplicate_and_empty_inventory_rejected(self):
        for actual in ([], ['first'], ['first', 'first'], ['first', 'second', 'extra']):
            with self.subTest(actual=actual), self.assertRaises(ValueError):
                runner.require_inventory(actual, ['first', 'second'], 'cases')
        runner.require_inventory(['first', 'second'], ['first', 'second'], 'cases')

    def test_fresh_evidence_rejects_even_an_empty_existing_directory(self):
        self.root.joinpath('evidence').mkdir()
        with self.assertRaises(FileExistsError):
            runner.fresh_directory(self.root / 'evidence')

    def test_stale_process_logs_are_not_reused(self):
        self.root.joinpath('logs').mkdir()
        with self.assertRaises(FileExistsError):
            self.run_commands([self.command('never', 'raise SystemExit(0)')])

    def test_zero_case_success_is_impossible(self):
        with self.assertRaises(ValueError):
            self.run_commands([])

    def test_failure_propagates_preserves_logs_and_skips_pending(self):
        commands = [self.command('failed', "print('first failure', flush=True); raise SystemExit(7)"),
                    self.command('pending', "open('unexpected', 'w').write('ran')")]
        with self.assertRaises(runner.RunnerFailure) as caught:
            self.run_commands(commands, jobs=1)
        self.assertEqual(caught.exception.code, 7)
        report = json.loads((self.root / 'logs/process-manifest.json').read_text())
        self.assertEqual([r['status'] for r in report['commands']], ['FAIL', 'NOT RUN'])
        self.assertIn('first failure', (self.root / 'logs/00-failed.stdout').read_text())
        self.assertFalse((self.root / 'unexpected').exists())

    def test_timeout_cleans_descendant_process_group(self):
        marker = self.root / 'escaped'
        descendant = "import pathlib,time;time.sleep(.6);pathlib.Path(" + repr(str(marker)) + ").write_text('escaped')"
        code = "import subprocess,sys,time;subprocess.Popen([sys.executable,'-c'," + repr(descendant) + "]);time.sleep(5)"
        started = time.monotonic()
        with self.assertRaises(runner.RunnerFailure) as caught:
            self.run_commands([self.command('timeout', code)], timeout=.15)
        self.assertEqual(caught.exception.code, 124)
        self.assertLess(time.monotonic() - started, 3)
        time.sleep(.7)
        self.assertFalse(marker.exists())

    def test_successful_worker_with_running_descendants_is_rejected(self):
        code = "import subprocess,sys;subprocess.Popen([sys.executable,'-c','import time;time.sleep(10)'])"
        with self.assertRaises(runner.RunnerFailure):
            self.run_commands([self.command('leak', code)])

    def test_failure_cancels_parallel_worker(self):
        marker = self.root / 'parallel-escaped'
        fail = self.command('failure', 'import time;time.sleep(.15);raise SystemExit(9)')
        slow = self.command('slow', "import time;from pathlib import Path;time.sleep(.6);Path('parallel-escaped').write_text('escaped')")
        with self.assertRaises(runner.RunnerFailure) as caught:
            self.run_commands([fail, slow], jobs=2)
        self.assertEqual(caught.exception.code, 9)
        time.sleep(.7)
        self.assertFalse(marker.exists())
        report = json.loads((self.root / 'logs/process-manifest.json').read_text())
        self.assertEqual([row['status'] for row in report['commands']], ['FAIL', 'CANCELLED'])

    def test_signal_exit_is_a_failure(self):
        code = 'import os,signal;os.kill(os.getpid(),signal.SIGTERM)'
        with self.assertRaises(runner.RunnerFailure) as caught:
            self.run_commands([self.command('signal', code)])
        self.assertEqual(caught.exception.code, 128 + 15)

    def test_source_identity_drift_fails_after_successful_process(self):
        source = self.root / 'input.rs'
        source.write_text('original')
        identities = {'input.rs': runner.digest(source)}
        command = self.command('mutate', "from pathlib import Path;Path('input.rs').write_text('changed')")
        with self.assertRaises(runner.RunnerFailure):
            self.run_commands([command], check=lambda: runner.verify_hashes(self.root, identities))

    def test_missing_identity_is_not_ignored(self):
        with self.assertRaises((ValueError, OSError)):
            runner.verify_hashes(self.root, {'absent': '0' * 64})

    def test_two_worker_limit_and_sequential_equivalence(self):
        intervals = []
        for jobs in (1, 2):
            path = self.root / str(jobs)
            path.mkdir()
            code = "import time;print(time.monotonic(), flush=True);time.sleep(.1);print(time.monotonic(), flush=True)"
            commands = [runner.Command('job' + str(i), (sys.executable, '-c', code), path, {}) for i in range(5)]
            results = runner.run_commands(commands, path / 'logs', jobs=jobs, output=io.StringIO())
            self.assertEqual(len(results), 5)
            stamps = [list(map(float, Path(row['stdout']).read_text().split())) for row in results]
            events = sorted([(start, 1) for start, end in stamps] + [(end, -1) for start, end in stamps])
            current = maximum = 0
            for _, delta in events:
                current += delta
                maximum = max(maximum, current)
            self.assertLessEqual(maximum, jobs)
            intervals.append(maximum)
        self.assertEqual(intervals, [1, 2])

    def test_invalid_parallelism_timeout_and_duplicate_jobs_rejected(self):
        command = self.command('one', 'pass')
        for kwargs in ({'jobs': 0}, {'jobs': 3}, {'timeout': 0}, {'timeout': float('nan')}):
            with self.subTest(kwargs=kwargs), self.assertRaises(ValueError):
                self.run_commands([command], **kwargs)
        with self.assertRaises(ValueError):
            self.run_commands([command, command])

    def test_exact_cargo_binary_inventory_rejects_zero_or_multiple(self):
        binary = self.root / 'test-binary'
        binary.write_bytes(b'fixture')
        row = {'reason': 'compiler-artifact', 'target': {'name': 'oxid'},
               'profile': {'test': True}, 'executable': str(binary)}
        path = self.root / 'build.jsonl'
        for rows in ([], [row, row]):
            path.write_text(''.join(json.dumps(r) + '\n' for r in rows))
            with self.assertRaises(ValueError):
                runner.built_test_binary(path)
        path.write_text(json.dumps(row) + '\n')
        self.assertEqual(runner.built_test_binary(path), binary)

    def test_zero_filtered_rust_tests_cannot_count_as_execution(self):
        path = self.root / 'test.stdout'
        result = {'name': 'fake-collector', 'stdout': str(path)}
        path.write_text('test result: ok. 0 passed; 0 failed; 0 ignored; 10 filtered out\n')
        with self.assertRaises(ValueError):
            runner.require_one_test(result)
        path.write_text('test result: ok. 1 passed; 0 failed; 0 ignored; 10 filtered out\n')
        runner.require_one_test(result)

    def test_freeze_uses_complete_independent_model_bytes(self):
        repo = Path(runner.__file__).resolve().parents[1]
        ids = ('guarded-overflow-after-write', 'guarded-replace-self', 'guarded-replace-relay',
               'guarded-owned-result-assign', 'guarded-empty-between-scalars', 'guarded-scalar-store',
               'batch-overflow-after-earlier-write', 'batch')
        fixture, manifest = runner.freeze_sources(repo, self.root, SimpleNamespace(NATIVE_IDS=ids))
        self.assertEqual(len(manifest['files']), 8)
        self.assertEqual(sum(row['budget_count'] for row in manifest['files']), 1932)
        self.assertEqual(manifest['specification_sha256'], runner.digest(fixture))
        for row in manifest['files']:
            source = self.root / 'frozen' / (row['id'] + '.ox')
            expectation = self.root / 'frozen' / (row['id'] + '.json')
            self.assertEqual(runner.digest(source), row['source_sha256'])
            self.assertEqual(runner.digest(expectation), row['expectation_sha256'])
        with self.assertRaises(FileExistsError):
            runner.freeze_sources(repo, self.root, SimpleNamespace(NATIVE_IDS=ids))
        with self.assertRaises(ValueError):
            runner.freeze_sources(repo, self.root, SimpleNamespace(NATIVE_IDS=ids[:-1]))

    def test_partial_collector_receipts_cannot_create_catalog(self):
        directory = self.root / 'case'
        directory.mkdir()
        frozen = self.root / 'frozen'
        frozen.mkdir()
        (directory / 'COMPLETE').write_text('free-form PASS')
        for root in (directory, frozen):
            (root / 'case.ox').write_text('original source')
        item = {'id': 'case', 'fuel': 3, 'source_sha256': runner.digest(frozen / 'case.ox')}
        for rows in ([], [{'name': 'default', 'budget': 1000000}],
                     [{'name': 'budget-0', 'budget': 0}] * 12):
            (directory / 'receipts.jsonl').write_text(''.join(json.dumps(r) + '\n' for r in rows))
            with self.assertRaises(ValueError):
                runner.collect_catalog(directory, frozen, item, {}, {}, 'fixture')
        self.assertFalse((directory / 'catalog.json').exists())

    def test_added_source_input_changes_snapshot_inventory(self):
        for name in ('Cargo.toml', 'Cargo.lock', 'build.rs'):
            (self.root / name).write_text('fixture')
        (self.root / 'src').mkdir()
        (self.root / 'src/main.rs').write_text('initial')
        before = runner.source_inventory(self.root)
        (self.root / 'src/extra.rs').write_text('added after build')
        self.assertNotEqual(before, runner.source_inventory(self.root))

    def test_source_inventory_rejects_symlink_directories_and_scan_roots(self):
        for name in ('Cargo.toml', 'Cargo.lock', 'build.rs'):
            (self.root / name).write_text('fixture')
        external = self.root / 'external'
        external.mkdir()
        (external / 'mod.rs').write_text('pub fn indirect() {}')
        source = self.root / 'src'
        source.mkdir()
        (source / 'main.rs').write_text('mod indirect;')
        (source / 'indirect').symlink_to(external, target_is_directory=True)
        with self.assertRaises(ValueError):
            runner.source_inventory(self.root)
        (source / 'indirect').unlink()
        (self.root / 'native').symlink_to(external, target_is_directory=True)
        with self.assertRaises(ValueError):
            runner.source_inventory(self.root)
        (self.root / 'native').unlink()
        (self.root / 'tests').mkdir()
        (external / 'owned_source').mkdir()
        (self.root / 'tests/fixtures').symlink_to(external, target_is_directory=True)
        with self.assertRaises(ValueError):
            runner.source_inventory(self.root)

    def test_current_reviewed_fixture_outside_legacy_roots_is_retained(self):
        for name in ('Cargo.toml', 'Cargo.lock', 'build.rs',
                     'tests/fixtures/typed_project_source_binding/current-source.json',
                     'scripts/verify_bounded_enum_native.py'):
            path = self.root / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text('fixture')
        name = 'tests/fixtures/current-extra/body.txt'
        path = self.root / name
        path.parent.mkdir(parents=True)
        path.write_bytes(b'complete compile-time fixture')
        row = {'path': name, 'bytes': path.stat().st_size, 'sha256': runner.digest(path)}
        reviewed = {'files': [row]}
        actual = runner.source_inventory(self.root, reviewed)
        self.assertEqual(actual[name], row['sha256'])
        self.assertIn('tests/fixtures/typed_project_source_binding/current-source.json', actual)
        path.write_bytes(b'changed fixture')
        with self.assertRaisesRegex(ValueError, 'reviewed input changed'):
            runner.source_inventory(self.root, reviewed)
        path.unlink()
        path.symlink_to(self.root / 'Cargo.toml')
        with self.assertRaisesRegex(ValueError, 'reject symlink'):
            runner.source_inventory(self.root, reviewed)

    def test_relocated_batch_fixture_is_in_source_inventory(self):
        for name in ('Cargo.toml', 'Cargo.lock', 'build.rs'):
            (self.root / name).write_text('fixture')
        batch = self.root / 'fixtures/owned_source/batch.ox'
        batch.parent.mkdir(parents=True)
        batch.write_text('initial Batch source')
        before = runner.source_inventory(self.root)
        self.assertEqual(before['fixtures/owned_source/batch.ox'], runner.digest(batch))
        batch.write_text('changed Batch source')
        self.assertNotEqual(before, runner.source_inventory(self.root))


if __name__ == '__main__':
    unittest.main()
