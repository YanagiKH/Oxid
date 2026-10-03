#!/usr/bin/env python3
"""Deterministic source-only adapter controls. Never invokes the candidate."""
import sys
sys.dont_write_bytecode = True
import copy
import json
import os
import sys
import tempfile
import time
import threading
from unittest import mock
from pathlib import Path
import unittest
from contracts import Reject, host, receipt_identity, check_inventory, sha, EXCLUDED_REASON, REMOTE_REASON
from compare import valid_origins, location, diagnostic_vector, policy_bounds, envelope, lifecycle

def process_gone_or_zombie(info):
    try:
        return info.read_text().split()[2] == 'Z'
    except FileNotFoundError:
        return True

class ComparatorTests(unittest.TestCase):
    def test_source_inventory_uses_exact_path_key_order(self):
        from runtime import source_map
        rows = [{'path': 'm1', 'kind': 'directory'}, {'path': 'm1/m2.ox', 'kind': 'file', 'bytes': 1, 'sha256': 'nested'}, {'path': 'm1.ox', 'kind': 'file', 'bytes': 2, 'sha256': 'parent'}, {'path': 'root.ox', 'kind': 'file', 'bytes': 3, 'sha256': 'root'}]
        got = source_map(rows)
        self.assertEqual([r['path'] for r in got], ['m1.ox', 'm1/m2.ox', 'root.ox'])
        self.assertEqual({r['path']: (r['bytes'], r['sha256']) for r in got}, {'m1.ox': (2, 'parent'), 'm1/m2.ox': (1, 'nested'), 'root.ox': (3, 'root')})

    def test_original_manifest_platform_newline_transport_is_narrow(self):
        from predecessors import original_manifest_transport
        expected = b'{\n  "value": 1\n}\n'
        windows = expected.replace(b'\n', b'\r\n')
        self.assertEqual(original_manifest_transport(windows, expected, 'nt'), expected)
        self.assertEqual(original_manifest_transport(expected, expected, 'posix'), expected)
        for raw, platform_name in ((windows, 'posix'), (windows.replace(b'1', b'2'), 'nt'), (windows + b'\r', 'nt')):
            with self.assertRaises(Reject): original_manifest_transport(raw, expected, platform_name)

    def test_aliases_and_architecture_are_exact(self):
        self.assertEqual(host('Darwin', 'aarch64'), 'macOS arm64')
        self.assertEqual(host('Windows', 'AMD64'), 'Windows x86_64')
        self.assertNotEqual(host('Linux', 'arm64'), 'Linux x86_64')

    def test_location_end_fields_are_required_when_frozen(self):
        actual = {'file_id': 0, 'path': 'm.ox', 'start': 0, 'end': 2, 'line': 1, 'column': 1, 'end_line': 1, 'end_column': 3}
        expected = {'file': 0, 'path': 'm.ox', 'start': 0, 'end': 2, 'line': 1, 'scalar_column': 1, 'end_line': 1, 'end_scalar_column': 3}
        location(actual, expected)
        altered = dict(actual, end_column=2)
        with self.assertRaises(Reject): location(altered, expected)
        expected.pop('end_line'); expected.pop('end_scalar_column')
        location(altered, expected)

    def test_literal_message_is_not_model_prose(self):
        got = {'code': 'E0100', 'stage': 'parse', 'primary': None, 'secondary': [], 'message': 'known'}
        want = {'code': 'E0100', 'stage': 'parse', 'location': None, 'related_locations': [], 'model_message': 'not literal'}
        diagnostic_vector([got], [want])
        want['literal_message'] = 'different'
        with self.assertRaises(Reject): diagnostic_vector([got], [want])

    def test_utf8_byte_bounds(self):
        policy = {'applies_to': [{'code': 'E0400', 'stage': 'source-project'}], 'message_utf8_bytes_max': 1024, 'secondary_label_count_max': 2, 'secondary_label_message_utf8_bytes_max': 256, 'note_count_max': 2, 'note_utf8_bytes_max': 256}
        got = {'code': 'E0400', 'stage': 'source-project', 'message': 'é' * 512, 'secondary': [], 'notes': []}
        policy_bounds([got], policy)
        got['message'] += 'é'
        with self.assertRaises(Reject): policy_bounds([got], policy)

    def test_partial_rows_still_validate_every_source_origin(self):
        sources = {'main.ox': 'é\nfn'.encode()}
        origin = {'file_id': 0, 'path': 'main.ox', 'start': 3, 'end': 5, 'line': 2, 'column': 1, 'end_line': 2, 'end_column': 3}
        diagnostic = {'primary': origin, 'secondary': [], 'notes': []}
        valid_origins([diagnostic], sources)
        with self.assertRaises(Reject):
            valid_origins([{**diagnostic, 'secondary': [{'span': None, 'message': 'unbound'}]}], sources)
        for field, value in [('start', 1), ('path', 'absent.ox'), ('end', 99), ('end_column', 2), ('file_id', 2)]:
            bad = copy.deepcopy(diagnostic)
            bad['primary'][field] = value
            with self.assertRaises((Reject, UnicodeDecodeError)): valid_origins([bad], sources)

    def test_failed_predicate_cannot_become_aggregate_success(self):
        from run import aggregate
        row = {'key': 'K', 'contract_identity': 'v3', 'group': 'literal', 'case_id': 'x', 'observation_index': 0, 'profile': 'debug', 'host': 'Linux x86_64', 'execution_host': 'Linux x86_64', 'required_hosts': ['Linux x86_64'], 'scope': 'execute', 'format': 'json', 'argv_template': ['{oxid}', 'check'], 'required_capabilities': []}
        got = {**receipt_identity(row), 'executed': True, 'status': 'fail'}
        with self.assertRaises(Reject): aggregate([row], [got])

    def test_unavailable_required_capability_is_incomplete(self):
        from run import aggregate
        row = {'key': 'K', 'contract_identity': 'v3', 'group': 'literal', 'case_id': 'x', 'observation_index': 0, 'profile': 'debug', 'host': 'Linux x86_64', 'execution_host': 'Linux x86_64', 'required_hosts': ['Linux x86_64'], 'scope': 'execute', 'format': 'json', 'argv_template': ['{oxid}', 'check'], 'required_capabilities': []}
        passed = {**receipt_identity(row), 'executed': True, 'status': 'pass'}
        capability_row = {**row, 'key': 'C', 'case_id': 'child-unreadable', 'required_capabilities': ['posix_regular_file_mode_denies_open_for_effective_identity']}
        from contracts import CAPABILITY_REASON
        gap = {**receipt_identity(capability_row), 'executed': False, 'status': 'unsupported-capability', 'reason': CAPABILITY_REASON,
               'capability_proof': {'read_open_succeeded': True, 'mode_before_candidate': 0, 'source_restored': True}}
        got = aggregate([row, capability_row], [passed, gap])
        self.assertEqual(got['status'], 'INCOMPLETE_LOCAL_CAPABILITY')
        self.assertEqual(got['executed'], 1)
        self.assertEqual(got['unsupported_capability'], 1)
        self.assertFalse(got['global_host_qualification'])
        false_pass = dict(gap, status='pass')
        with self.assertRaises(Reject): aggregate([row, capability_row], [passed, false_pass])

    def test_inventory_mutations_fail_closed(self):
        row = {'key': 'K', 'contract_identity': 'v3', 'group': 'literal', 'case_id': 'x', 'observation_index': 0, 'profile': 'debug', 'host': 'Linux x86_64', 'execution_host': 'Linux x86_64', 'required_hosts': ['Linux x86_64'], 'scope': 'execute', 'format': 'json', 'argv_template': ['{oxid}', 'check'], 'required_capabilities': []}
        good = {**receipt_identity(row), 'executed': True, 'status': 'pass'}
        check_inventory([row], [good])
        variants = [[], [good, good], [{**good, 'key': 'other'}], [{**good, 'profile': 'release'}], [{**good, 'host': 'macOS arm64'}], [{**good, 'executed': False}], [{**good, 'status': 'unsupported-capability', 'capability_proof': {}}]]
        for variant in variants:
            with self.assertRaises(Reject): check_inventory([row], variant)

class ProcessControls(unittest.TestCase):
    def test_complete_capture_and_stream_limit(self):
        from runtime import process
        with tempfile.TemporaryDirectory() as temp:
            good = process([sys.executable, '-c', "print('ok')"], temp, os.environ, timeout=5, limit=64)
            self.assertEqual(good['stdout'], 'ok\n')
            self.assertEqual(good['status'], 0)
            bad = process([sys.executable, '-c', "import sys; sys.stdout.write('x'*1000000)"], temp, os.environ, timeout=5, limit=64)
            self.assertTrue(bad['stream_limit_exceeded'])
            self.assertEqual(len(bad['stdout']), 64)

    def test_timeout_when_child_closes_output(self):
        from runtime import process
        with tempfile.TemporaryDirectory() as temp:
            got = process([sys.executable, '-c', "import os,time; os.close(1); os.close(2); time.sleep(30)"], temp, os.environ, timeout=0.2)
            self.assertTrue(got['timed_out'])

    def test_reader_error_is_reported_and_cleanup_is_bounded(self):
        import runtime
        read = os.read
        def injected(fd, count):
            if threading.current_thread().name.startswith('unit4-pipe-reader-'):
                raise OSError(5, 'injected pipe read error')
            return read(fd, count)
        with tempfile.TemporaryDirectory() as temp:
            started = time.monotonic()
            with mock.patch('runtime.os.read', side_effect=injected):
                with self.assertRaisesRegex(OSError, 'reader failure'):
                    runtime.process([sys.executable, '-c', 'import time; time.sleep(30)'], temp, os.environ, timeout=2)
            self.assertLess(time.monotonic() - started, 5)

    def test_success_cleans_closed_pipe_descendant(self):
        from runtime import process
        with tempfile.TemporaryDirectory() as temp:
            script = "import subprocess,sys; p=subprocess.Popen([sys.executable,'-c','import time; time.sleep(30)'],stdout=subprocess.DEVNULL,stderr=subprocess.DEVNULL); print(p.pid,flush=True)"
            got = process([sys.executable, '-c', script], temp, os.environ, timeout=3)
            self.assertEqual(got['status'], 0)
            self.assertFalse(got['timed_out'])
            pid = int(got['stdout'].strip())
            if os.name != 'nt' and Path('/proc').exists():
                info = Path('/proc') / str(pid) / 'stat'
                deadline = time.monotonic() + 2
                while not process_gone_or_zombie(info) and time.monotonic() < deadline:
                    time.sleep(0.02)
                self.assertTrue(process_gone_or_zombie(info))

    def test_timeout_cleans_descendant_pipe(self):
        from runtime import process
        with tempfile.TemporaryDirectory() as temp:
            script = "import subprocess,sys,time; p=subprocess.Popen([sys.executable,'-c','import time; time.sleep(30)']); print(p.pid,flush=True); time.sleep(30)"
            started = time.monotonic()
            got = process([sys.executable, '-c', script], temp, os.environ, timeout=0.3, limit=64)
            self.assertTrue(got['timed_out'])
            self.assertLess(time.monotonic() - started, 5)
            pid = int(got['stdout'].strip())
            if os.name != 'nt' and Path('/proc').exists():
                info = Path('/proc') / str(pid) / 'stat'
                self.assertTrue(process_gone_or_zombie(info))

if __name__ == '__main__':
    unittest.main()
