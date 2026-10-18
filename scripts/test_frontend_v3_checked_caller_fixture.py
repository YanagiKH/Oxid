"""Fixture-only Linux development controls; never V3 qualification."""
import contextlib
import io
import json
import os
from pathlib import Path
import signal
import subprocess
import sys
import tempfile
import time
import unittest
from unittest.mock import patch

FIXTURE = Path(__file__).resolve().parents[1] / 'tests/qualification/frontend_v3_checked_caller_fixture_v2'
WATCHDOG_SECONDS = 60
EXPECTED_IDS = (
    'test_caller.CallerControls.test_all_finalization_failures_do_not_skip_actual_cleanup_or_closure',
    'test_caller.CallerControls.test_anchor_resources_close_before_cleanup_even_when_close_reports_error',
    'test_caller.CallerControls.test_cancellation_same_object_after_cleanup_and_closed',
    'test_caller.CallerControls.test_cleanup_parent_open_failure_retains_original_and_leftover_path',
    'test_caller.CallerControls.test_escaped_bound_method_revoked_during_all_finalization_and_after_return',
    'test_caller.CallerControls.test_failure_before_scope_establishment_keeps_independent_scope',
    'test_caller.CallerControls.test_first_cancellation_identity_wins_over_later_cancellation',
    'test_caller.CallerControls.test_fixed_outcome_required',
    'test_caller.CallerControls.test_fixture_subclass_rejects_without_owner_cleanup',
    'test_caller.CallerControls.test_full_outer_unconsumed_body_mutation_is_detected',
    'test_caller.CallerControls.test_initial_hardlink_rejects_before_consumers',
    'test_caller.CallerControls.test_invalid_owner_never_deletes',
    'test_caller.CallerControls.test_malformed_absence_keeps_independent_source_scope',
    'test_caller.CallerControls.test_membership_and_known_absence_mutations_detected',
    'test_caller.CallerControls.test_native_adapter_exact_membership_and_no_fallback',
    'test_caller.CallerControls.test_nested_directory_replacement_never_reads_replacement_body',
    'test_caller.CallerControls.test_ownership_close_failure_still_closes_remaining_anchors',
    'test_caller.CallerControls.test_ownership_partial_anchor_failure_closes_acquired_descriptors',
    'test_caller.CallerControls.test_ownership_physical_ancestor_aliases_reject_before_cleanup',
    'test_caller.CallerControls.test_ownership_shared_ordinary_ancestors_pass_and_close_anchors',
    'test_caller.CallerControls.test_partial_evidence_initial_failure_still_attempts_every_final_body',
    'test_caller.CallerControls.test_partial_initial_failure_still_seals_both_scopes',
    'test_caller.CallerControls.test_preservation_cancellation_with_ordered_other_failures',
    'test_caller.CallerControls.test_raw_failure_preserves_source_outcome',
    'test_caller.CallerControls.test_replaced_owned_root_untouched_and_cleanup_once_last',
    'test_caller.CallerControls.test_source_failure_skips_raw_and_preserves_original',
    'test_caller.CallerControls.test_success_one_actual_view_same_dispatch_and_no_core_fallback',
    'test_caller.CallerControls.test_unregistered_binding_rejects_before_probe',
    'test_caller.CallerControls.test_view_construction_failure_zero_admitted_views_with_full_finalization',
    'test_caller.PhysicalReaderControls.test_empty_file_reads_only_one_eof_sentinel',
    'test_caller.PhysicalReaderControls.test_missing_platform_features_reject',
    'test_caller.PhysicalReaderControls.test_nlink_required_at_all_four_observations',
    'test_caller.PhysicalReaderControls.test_partial_anchor_traversal_closes_every_opened_fd',
    'test_caller.PhysicalReaderControls.test_read_failures_still_attempt_fd_name_ancestor_checks_and_close',
    'test_caller.PhysicalReaderControls.test_same_fd_and_expected_plus_one_bound',
    'test_caller.PhysicalReaderControls.test_size_mismatch_refuses_before_open_or_body',
    'test_caller.PhysicalReaderControls.test_symlink_and_fifo_rejected_before_body_read',
    'test_caller.SupportPackagingControls.test_altered_dependency_rejected_before_compile',
    'test_caller.SupportPackagingControls.test_exact_captured_bytes_compiled_once_from_fixture_directory',
    'test_caller.SupportPackagingControls.test_file_directory_symlinks_and_hardlinks_rejected',
    'test_caller.SupportPackagingControls.test_missing_dependency_has_no_execution_or_fallback',
    'test_caller.SupportPackagingControls.test_noncanonical_spellings_rejected_before_filesystem_access',
)


def need(ok, message):
    if not ok:
        raise AssertionError('fixture-only wrapper: ' + message)


def exact_ids(ids):
    return (type(ids) is list and all(type(item) is str for item in ids)
            and len(ids) == len(EXPECTED_IDS) == len(set(ids)) and set(ids) == set(EXPECTED_IDS))


def unique_object(pairs):
    result = {}
    for key, value in pairs:
        need(key not in result, 'duplicate summary key')
        result[key] = value
    return result


def validate_summary(raw, mode):
    need(type(raw) is str and len(raw) <= 131072, 'invalid summary size/type')
    try:
        value = json.loads(raw, object_pairs_hook=unique_object)
    except (ValueError, TypeError) as error:
        raise AssertionError('fixture-only wrapper: malformed/missing summary') from error
    expected = {'schema', 'fixture_only', 'qualified', 'optimize', 'discovered',
                'started', 'stopped', 'succeeded', 'tests_run', 'failures', 'errors',
                'skips', 'expected_failures', 'unexpected_successes'}
    need(type(value) is dict and set(value) == expected, 'summary fields differ')
    need(value['schema'] == 'oxid-v3-fixture-hosted-controls-v1' and
         value['fixture_only'] is True and value['qualified'] is False, 'summary scope differs')
    need(type(value['optimize']) is int and value['optimize'] == mode, 'optimization differs')
    for name in ('discovered', 'started', 'stopped', 'succeeded'):
        need(exact_ids(value[name]), name + ' test IDs differ/duplicate/missing')
    need(type(value['tests_run']) is int and value['tests_run'] == len(EXPECTED_IDS), 'test count differs')
    for name in ('failures', 'errors', 'skips', 'expected_failures', 'unexpected_successes'):
        need(type(value[name]) is int and value[name] == 0, name + ' must be zero')
    return value


def flatten(suite):
    for item in suite:
        if isinstance(item, unittest.TestSuite):
            yield from flatten(item)
        else:
            yield item


class RecordedResult(unittest.TextTestResult):
    def __init__(self, *args, **kwargs):
        super().__init__(*args, **kwargs)
        self.started, self.stopped, self.succeeded = [], [], []

    def startTest(self, test):
        self.started.append(test.id())
        super().startTest(test)

    def stopTest(self, test):
        self.stopped.append(test.id())
        super().stopTest(test)

    def addSuccess(self, test):
        self.succeeded.append(test.id())
        super().addSuccess(test)


def child(mode):
    need(sys.platform == 'linux' and sys.version_info >= (3, 11), 'Linux/Python 3.11+ required')
    need(sys.flags.optimize == mode, 'child optimization differs')
    sys.path.insert(0, str(FIXTURE))
    with contextlib.redirect_stdout(sys.stderr):
        loader = unittest.TestLoader()
        suite = loader.loadTestsFromName('test_caller')
        ids = [test.id() for test in flatten(suite)]
        need(not loader.errors and exact_ids(ids), 'discovered roster differs')
        result = unittest.TextTestRunner(verbosity=2, resultclass=RecordedResult).run(suite)
    summary = dict(schema='oxid-v3-fixture-hosted-controls-v1', fixture_only=True, qualified=False,
                   optimize=sys.flags.optimize, discovered=ids, started=result.started,
                   stopped=result.stopped, succeeded=result.succeeded, tests_run=result.testsRun,
                   failures=len(result.failures), errors=len(result.errors), skips=len(result.skipped),
                   expected_failures=len(result.expectedFailures), unexpected_successes=len(result.unexpectedSuccesses))
    raw = json.dumps(summary, sort_keys=True)
    print(raw, flush=True)
    validate_summary(raw, mode)
    return 0


def run_child(mode):
    need(type(mode) is int and mode in (0, 1), 'invalid child mode')
    command = [sys.executable, '-I', '-B', *(['-O'] if mode else []),
               str(Path(__file__).resolve()), '--fixture-child', str(mode)]
    process = subprocess.Popen(command, cwd=FIXTURE, stdout=subprocess.PIPE,
                               stderr=subprocess.PIPE, text=True)
    try:
        stdout, stderr = process.communicate(timeout=WATCHDOG_SECONDS)
    except subprocess.TimeoutExpired as error:
        process.kill()
        stdout, stderr = process.communicate()
        raise AssertionError(f'fixture-only child exceeded {WATCHDOG_SECONDS}-second watchdog\n' + stderr) from error
    except BaseException:
        process.kill()
        process.communicate()
        raise
    sys.stderr.write(stderr)
    need(process.returncode == 0, 'child failed with exit ' + str(process.returncode))
    summary = validate_summary(stdout, mode)
    print('fixture-only; qualified=False; optimize=' + str(mode) + '; executed=42; passed=42; skipped=0')
    return summary


class HostedFixtureControls(unittest.TestCase):
    def setUp(self):
        if sys.platform != 'linux':
            self.skipTest('Unsupported Linux-only development fixture; not qualification')
        need(sys.version_info >= (3, 11), 'Linux requires Python 3.11+')

    def test_normal(self):
        run_child(0)

    def test_optimized(self):
        run_child(1)


class WrapperProtocolControls(unittest.TestCase):
    def good(self):
        return dict(schema='oxid-v3-fixture-hosted-controls-v1', fixture_only=True, qualified=False,
                    optimize=0, discovered=list(EXPECTED_IDS), started=list(EXPECTED_IDS),
                    stopped=list(EXPECTED_IDS), succeeded=list(EXPECTED_IDS), tests_run=42,
                    failures=0, errors=0, skips=0, expected_failures=0, unexpected_successes=0)

    def test_exact_summary_and_bad_counts_ids_modes(self):
        self.assertEqual(validate_summary(json.dumps(self.good()), 0), self.good())
        for field in ('discovered', 'started', 'stopped', 'succeeded'):
            for kind in ('missing', 'replaced', 'duplicate'):
                with self.subTest(field=field, kind=kind):
                    value = self.good()
                    if kind == 'missing': value[field].pop()
                    elif kind == 'replaced': value[field][0] = 'test_caller.replacement'
                    else: value[field][0] = value[field][1]
                    with self.assertRaises(AssertionError): validate_summary(json.dumps(value), 0)
        for field, changed in (('tests_run', 41), ('tests_run', True), ('optimize', 1), ('qualified', True)):
            value = self.good(); value[field] = changed
            with self.subTest(field=field, changed=changed), self.assertRaises(AssertionError):
                validate_summary(json.dumps(value), 0)

    def test_skips_failures_errors_and_expected_results_refuse(self):
        for field in ('skips', 'failures', 'errors', 'expected_failures', 'unexpected_successes'):
            value = self.good(); value[field] = 1
            with self.subTest(field=field), self.assertRaises(AssertionError):
                validate_summary(json.dumps(value), 0)

    def test_malformed_missing_and_duplicate_summary_refuse(self):
        for raw in ('', 'not json', '{}', '[]', '{} {}', '{"schema":1,"schema":2}'):
            with self.subTest(raw=raw), self.assertRaises(AssertionError): validate_summary(raw, 0)

    def test_child_exit_and_invalid_summary_propagate(self):
        for code, raw in ((1, json.dumps(self.good())), (0, ''), (0, json.dumps({**self.good(), 'skips':1}))):
            with self.subTest(code=code, raw=raw), patch.object(subprocess, 'Popen') as opened:
                process = opened.return_value
                process.returncode = code; process.communicate.return_value = (raw, 'diagnostic')
                with contextlib.redirect_stderr(io.StringIO()), self.assertRaises(AssertionError): run_child(0)
                command = opened.call_args.args[0]
                self.assertEqual(command[:3], [sys.executable, '-I', '-B'])
                self.assertNotIn('-O', command)
                process.communicate.assert_called_once_with(timeout=WATCHDOG_SECONDS)

    def test_timeout_kills_and_reaps_child(self):
        with patch.object(subprocess, 'Popen') as opened:
            process = opened.return_value
            process.communicate.side_effect = [subprocess.TimeoutExpired('fixture', 60), ('', 'timeout diagnostic')]
            with self.assertRaisesRegex(AssertionError, '60-second watchdog'): run_child(1)
            self.assertEqual(opened.call_args.args[0][:4], [sys.executable, '-I', '-B', '-O'])
            process.kill.assert_called_once_with()
            self.assertEqual(process.communicate.call_count, 2)
            self.assertEqual(process.method_calls, [unittest.mock.call.communicate(timeout=60),
                                                   unittest.mock.call.kill(), unittest.mock.call.communicate()])

    @unittest.skipUnless(sys.platform == 'linux', 'Linux-only direct-child watchdog control; not qualification')
    def test_real_timeout_terminates_and_reaps_direct_child(self):
        actual_popen = subprocess.Popen
        children = []
        readiness_seconds = 10
        def retire(child):
            try:
                if child.poll() is None:
                    child.kill()
                child.wait()
            finally:
                for pipe in (child.stdout, child.stderr):
                    if pipe is not None:
                        pipe.close()
        with tempfile.TemporaryDirectory(prefix='oxid-watchdog-') as directory:
            helper = Path(directory) / 'sleep_helper.py'
            ready = Path(directory) / 'ready'
            helper.write_text('import sys, time\nfrom pathlib import Path\n'
                              'print("trusted sleep helper ready", file=sys.stderr, flush=True)\n'
                              'Path(sys.argv[1]).write_text("ready")\n'
                              'while True:\n    time.sleep(1)\n')
            def launch_helper(command, **kwargs):
                child = actual_popen([sys.executable, '-I', '-B', str(helper), str(ready)], **kwargs)
                children.append(child)
                try:
                    deadline = time.monotonic() + readiness_seconds
                    while not ready.is_file():
                        need(child.poll() is None, 'sleep helper exited before readiness')
                        need(time.monotonic() < deadline, 'sleep helper readiness exceeded 10 seconds')
                        time.sleep(0.01)
                    need(child.poll() is None, 'sleep helper exited at readiness')
                    return child
                except BaseException:
                    retire(child)
                    raise
            try:
                with patch.object(subprocess, 'Popen', side_effect=launch_helper), \
                        patch(__name__ + '.WATCHDOG_SECONDS', 1), \
                        self.assertRaisesRegex(AssertionError, '1-second watchdog') as caught:
                    run_child(0)
                self.assertIsInstance(caught.exception.__cause__, subprocess.TimeoutExpired)
                self.assertEqual(caught.exception.__cause__.timeout, 1)
                self.assertIn('trusted sleep helper ready', str(caught.exception))
                self.assertEqual(len(children), 1)
                child = children[0]
                self.assertEqual(child.returncode, -signal.SIGKILL)
                self.assertEqual(child.poll(), -signal.SIGKILL)
                with self.assertRaises(ChildProcessError):
                    os.waitpid(child.pid, os.WNOHANG)
                self.assertTrue(child.stdout.closed and child.stderr.closed)
                self.assertEqual(WATCHDOG_SECONDS, 60)
            finally:
                for child in children:
                    retire(child)

    def test_failures_reach_outer_unittest_result(self):
        with patch.object(sys, 'platform', 'linux'), patch(__name__ + '.run_child', side_effect=AssertionError('child refused')):
            result = unittest.TestResult()
            unittest.defaultTestLoader.loadTestsFromTestCase(HostedFixtureControls).run(result)
        self.assertEqual(result.testsRun, 2)
        self.assertEqual(len(result.failures), 2)
        self.assertFalse(result.wasSuccessful())

    def test_nonlinux_explicit_outer_skips(self):
        with patch.object(sys, 'platform', 'win32'), patch(__name__ + '.run_child') as run:
            result = unittest.TestResult()
            unittest.defaultTestLoader.loadTestsFromTestCase(HostedFixtureControls).run(result)
        self.assertEqual(run.call_count, 0)
        self.assertEqual(len(result.skipped), 2)
        self.assertTrue(all('not qualification' in reason for _, reason in result.skipped))


if __name__ == '__main__':
    if len(sys.argv) == 3 and sys.argv[1] == '--fixture-child' and sys.argv[2] in ('0', '1'):
        raise SystemExit(child(int(sys.argv[2])))
    unittest.main(verbosity=2)
