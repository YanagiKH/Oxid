#!/usr/bin/env python3
"""Pure source/schema/protocol controls, never process qualification.

All transcripts and layouts below are deliberately synthetic test values. No
compiler, observer, or probe executable is launched by this test module.
"""
import ast
import copy
import io
import json
from pathlib import Path
import re
import sys
import types
import unittest
from unittest.mock import patch

sys.dont_write_bytecode = True
import lexer_phase as phase
import lexer_phase_controls as controls


NONCE = 'a' * 32
DIGEST = 'b' * 64
ROOT = Path('/synthetic-session')


def sidecar(*, role='observed', rows=1, failed=False, result='ok', case_id='synthetic', modes=None):
    trace = [{'seq': i + 1, 'kind': 'lexer token tape', 'length': 4 << i,
              'element_bytes': 32, 'success': not (failed and i == rows - 1)} for i in range(rows)]
    if role == 'not_observed':
        trace = []
    return {'schema': phase.SCHEMA, 'nonce': NONCE, 'case_id': case_id, 'role': role,
            'runtime_os': 'linux', 'runtime_architecture': 'x86_64', 'pointer_width': 64,
            'token_bytes': 32, 'lex_attempts': 1, 'lex_result': result, 'phase': 'finished',
            'closed': True, 'mode_indices': [0, 1] if modes is None else modes,
            'event_limit': 16, 'event_count': len(trace), 'fixed_row_bytes': 8,
            'fixed_rows_bytes': 128, 'occupied_row_bytes': len(trace) * 8, 'ledger_bytes': 248,
            'row_byte_limit': 128, 'serialized_byte_limit': 1912,
            'reserve_failed': None if role == 'not_observed' else failed, 'rows': trace}


def synthetic_layout():
    # G=16 is only a deliberately invented in-memory schema fixture. It is not
    # a measurement, an ABI approval, or a claim about the actual stdout lock.
    values = (8, 4, 128, 4, 248, 4, 256, 8, 16, 8, 16, 8, 32, 8,
              72, 8, 120, 8, 88, 8, 16, 8, 48, 8, 2544)
    return dict(zip(controls.LAYOUT_KEYS, values))


def binaries():
    return {(profile, role): {'path': 'build/' + profile + '-' + role, 'bytes': 100,
                              'sha256': format(i + 1, '064x')}
            for i, (profile, role) in enumerate((p, r) for p in controls.PROFILES for r in controls.ROLES)}


def request(probe='inactive_noop', profile='debug', role='observed', number=1):
    return {'schema': controls.REQUEST_SCHEMA, 'nonce': format(number, '032x'), 'probe': probe,
            'profile': profile, 'role': role, 'session_sha256': '1' * 64,
            'authority_sha256': '2' * 64, 'driver_sha256': '3' * 64,
            'binary': binaries()[(profile, role)], 'entrypoint': controls.ENTRYPOINT,
            'expected': controls.expected(probe, profile, role)}


def transcript(req):
    want = req['expected']
    prefix = (controls.LAYOUT_PREFIX if want['status'] == 'measured' else
              controls.ARMED_PREFIX if want['status'] == 'rejected_as_expected' else controls.OK_PREFIX)
    marker = prefix + req['nonce'].encode() + b' ' + req['probe'].encode() + b' ' + want['boundary'].encode()
    if want['status'] == 'measured':
        marker += b' ' + controls.canonical_bytes(synthetic_layout()).rstrip(b'\n')
    summary = (b'test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 1 filtered out; finished in 0.00s\n'
               if want['exit_code'] else
               b'test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 1 filtered out; finished in 0.00s\n')
    stdout = b'\nrunning 1 test\ntest synthetic ... ' + marker + b'\n\n' + summary
    stderr = b''
    if want['exit_code']:
        message = (b'allocator event lacks actual parser position' if want['boundary'] == 'parser.position'
                   else b'UNIT4_PHASE_V1:' + want['boundary'].encode())
        stderr = b"thread 'synthetic' panicked at synthetic.rs:1:1:\n" + message + b'\n'
    return stdout, stderr


def receipt(req, stdout, stderr, started=1):
    directory = controls.process_dir(req)
    status, boundary, count, layout, failure = controls.validate_probe_output(
        req, stdout, stderr, req['expected']['exit_code'], approved_layout=synthetic_layout())
    return {'schema': controls.RECEIPT_SCHEMA,
            **{name: req[name] for name in ('nonce', 'probe', 'profile', 'role')},
            'request': controls.identity(ROOT, directory + '/request.json', controls.canonical_bytes(req)),
            'status': status, 'exit_code': req['expected']['exit_code'], 'boundary': boundary,
            'started_ns': started, 'finished_ns': started + 1, 'timed_out': False,
            'stream_limit_exceeded': False, **controls.input_hashes(*controls.inputs(ROOT, req)),
            'stdout': controls.identity(ROOT, directory + '/stdout', stdout),
            'stderr': controls.identity(ROOT, directory + '/stderr', stderr),
            'marker_count': count, 'layout': layout, 'failure': failure}


def protocol_fixture():
    """Return an in-memory synthetic 217-file transport; never write evidence."""
    data, records = {}, []
    for number, (probe, profile, role) in enumerate(controls.roster(), 1):
        req = request(probe, profile, role, number)
        stdout, stderr = transcript(req)
        row = receipt(req, stdout, stderr, number * 2)
        directory = controls.process_dir(req)
        for name, raw in (('request.json', controls.canonical_bytes(req)),
                          ('receipt.json', controls.canonical_bytes(row)), ('stdout', stdout), ('stderr', stderr)):
            data[str(ROOT / directory / name)] = raw
        records.append(controls.identity(ROOT, directory + '/receipt.json', controls.canonical_bytes(row)))
    index = {'schema': controls.INDEX_SCHEMA, 'session_sha256': '1' * 64,
             'authority_sha256': '2' * 64, 'driver_sha256': '3' * 64,
             'counts': dict(controls.COUNTS), 'receipts': records}
    data[str(ROOT / controls.ROOT_NAME / 'probe-index.json')] = controls.canonical_bytes(index)
    return data


def verify_fixture(data):
    inventory = {name: {'path': name, 'bytes': len(raw), 'sha256': controls.sha256(raw)}
                 for name, raw in data.items()}
    return controls.verify(ROOT, session_sha256='1' * 64, authority_sha256='2' * 64,
                           driver_sha256='3' * 64, binaries=binaries(),
                           approved_layouts={row['sha256']: synthetic_layout() for row in binaries().values()},
                           resolve=lambda record: data[record['path']], inventory=inventory)


class SidecarSchemaControls(unittest.TestCase):
    def reject(self, value):
        with self.assertRaises(phase.Reject):
            phase.validate_sidecar(phase.canonical_bytes(value))

    def test_valid_closed_roles_results_and_mode_sequences(self):
        for role in phase.ROLES:
            for result in ('ok', 'err'):
                for modes in ([0], [0, 1]):
                    value = sidecar(role=role, result=result, modes=modes)
                    self.assertEqual(phase.validate_sidecar(phase.canonical_bytes(value)), value)
        self.assertEqual(phase.validate_sidecar(phase.canonical_bytes(sidecar(rows=0, result='err')))['rows'], [])

    def test_all_top_and_row_keys_are_closed_and_ordered(self):
        base = sidecar()
        for key in base:
            wrong = copy.deepcopy(base); del wrong[key]
            with self.subTest(missing=key): self.reject(wrong)
        for key in base['rows'][0]:
            wrong = copy.deepcopy(base); del wrong['rows'][0][key]
            with self.subTest(row_missing=key): self.reject(wrong)
        wrong = copy.deepcopy(base); wrong['extra'] = None; self.reject(wrong)
        wrong = copy.deepcopy(base); wrong['rows'][0]['cursor'] = 0; self.reject(wrong)
        self.reject(dict(reversed(list(base.items()))))
        wrong = copy.deepcopy(base); wrong['rows'][0] = dict(reversed(list(wrong['rows'][0].items()))); self.reject(wrong)

    def test_duplicate_keys_and_alternate_encodings_reject(self):
        raw = phase.canonical_bytes(sidecar())
        variants = [raw.replace(b'"closed":true', b'"closed":true,"closed":true'),
                    raw.replace(b'"seq":1', b'"seq":1,"seq":1'), raw[:-1], raw + b'\n',
                    raw.replace(b'\n', b'\r\n'), b' ' + raw, raw.replace(b':', b': ', 1),
                    raw.replace(b'"synthetic"', b'"\\u0073ynthetic"'),
                    raw.replace(b'"pointer_width":64', b'"pointer_width":64.0'),
                    raw.replace(b'"pointer_width":64', b'"pointer_width":6.4e1'),
                    raw.replace(b'"pointer_width":64', b'"pointer_width":NaN'),
                    raw.replace(b'"pointer_width":64', b'"pointer_width":064')]
        for variant in variants:
            with self.subTest(raw=variant[:60]), self.assertRaises(phase.Reject):
                phase.validate_sidecar(variant)

    def test_every_unsigned_integer_rejects_boolean_negative_float_string(self):
        base = sidecar()
        fields = [key for key, value in base.items() if type(value) is int]
        for key in fields:
            for value in (True, -1, 1.0, '1', None):
                wrong = copy.deepcopy(base); wrong[key] = value
                with self.subTest(key=key, value=value): self.reject(wrong)
        for key in ('seq', 'length', 'element_bytes'):
            for value in (True, -1, 1.0, '1', None):
                wrong = copy.deepcopy(base); wrong['rows'][0][key] = value
                with self.subTest(row_key=key, value=value): self.reject(wrong)

    def test_nonce_case_abi_phase_and_association(self):
        invalid = {'nonce': ('a' * 31, 'a' * 33, 'A' * 32, 'g' * 32),
                   'case_id': ('', 'a' * 65, 'é', 'a"', 'a\\', 'a\n', '-first'),
                   'runtime_os': ('darwin',), 'runtime_architecture': ('aarch64',),
                   'pointer_width': (32,), 'token_bytes': (16,), 'ledger_bytes': (256,),
                   'phase': ('lexed',), 'closed': (False, 1), 'lex_attempts': (0, 2),
                   'mode_indices': ([], [1], [0, 0], [0, 1, 2], [False], [0.0]),
                   'role': ('control',), 'reserve_failed': (0, None)}
        for key, values in invalid.items():
            for value in values:
                wrong = sidecar(); wrong[key] = value
                with self.subTest(key=key, value=value): self.reject(wrong)
        raw = phase.canonical_bytes(sidecar())
        for kwargs in ({'nonce': 'b' * 32}, {'case_id': 'other'}, {'role': 'not_observed'},
                       {'mode_indices': [0]}, {'lex_result': 'err'}):
            with self.subTest(kwargs=kwargs), self.assertRaises(phase.Reject):
                phase.validate_sidecar(raw, **kwargs)

    def test_terminal_rows_order_bounds_and_control_claims(self):
        self.reject(sidecar(rows=0))
        self.reject(sidecar(rows=2, failed=True))
        phase.validate_sidecar(phase.canonical_bytes(sidecar(rows=2, failed=True, result='err')))
        for key, value in (('seq', 2), ('length', 8), ('length', 3), ('length', 131073),
                           ('element_bytes', 8), ('success', 1), ('kind', 'parser nodes')):
            wrong = sidecar(); wrong['rows'][0][key] = value; self.reject(wrong)
        for mutation in ('reorder', 'duplicate', 'after_failure', 'wrong_count', 'wrong_charge'):
            wrong = sidecar(rows=3, result='err')
            if mutation == 'reorder': wrong['rows'].reverse()
            elif mutation == 'duplicate': wrong['rows'][1] = copy.deepcopy(wrong['rows'][0])
            elif mutation == 'after_failure': wrong['rows'][0]['success'] = False; wrong['reserve_failed'] = True
            elif mutation == 'wrong_count': wrong['event_count'] = 2
            else: wrong['occupied_row_bytes'] = 25
            with self.subTest(mutation=mutation): self.reject(wrong)
        skipped = sidecar(rows=2); skipped['rows'][1]['length'] = 32
        phase.validate_sidecar(phase.canonical_bytes(skipped))
        wrong = sidecar(role='not_observed'); wrong['reserve_failed'] = False; self.reject(wrong)
        wrong = sidecar(); wrong['role'] = 'not_observed'; wrong['reserve_failed'] = None; self.reject(wrong)

    def test_independent_exact_serialized_width_proof(self):
        maximum = sidecar(rows=16, result='err', case_id='a' * 64)
        raw = phase.canonical_bytes(maximum)
        self.assertEqual(len(raw), 1912)
        self.assertEqual([len(phase.canonical_bytes(row)) - 1 for row in maximum['rows']],
                         [80, 80, 81, 81, 81, 82, 82, 82, 83, 84, 84, 84, 85, 85, 85, 86])
        self.assertEqual(len(phase.canonical_bytes(sidecar(rows=16, result='err', failed=True, case_id='a' * 64))), 1912)
        self.assertEqual(len(phase.canonical_bytes(sidecar(role='not_observed', result='err', case_id='a' * 64))), 572)
        self.assertEqual(len(phase.FRAME_PREFIX) + 32 + 1 + len(raw), 1966)
        self.assertEqual(571 + 1325 + 15 + 1, 1912)
        self.assertEqual(571 + 15 * 86 + 1 + 14 + 1, 1877)
        self.assertEqual(574 + 248 * 906 + 247, 225509)
        self.assertEqual(6 * 574 + 524 * 906 + 518, 478706)
        self.assertEqual(510 * 1912 + 14 * 572, 983128)
        self.assertEqual(510 * 1966 + 14 * 626, 1011424)
        self.assertEqual(983128 + 1011424 + 478706, 2473258)

    def test_raw_prefix_bounded_admission(self):
        prefix = b'{"schema":"oxid-unit4-parser-raw-v1","nonce":"' + NONCE.encode() + b'","case_id":"synthetic","source_utf8":'
        self.assertEqual(phase.validate_raw_prefix(prefix + b'"source may include \\" quotes"}'), (NONCE, 'synthetic'))
        for length in range(len(prefix)):
            with self.subTest(length=length), self.assertRaises(phase.Reject):
                phase.validate_raw_prefix(prefix[:length])
        for wrong in (prefix.replace(b'nonce', b'noncE'), prefix.replace(b'case_id', b'case'),
                      prefix.replace(b'"source_utf8":', b'"other":'),
                      prefix.replace(b'synthetic', b'a' * 65), prefix.replace(b'synthetic', b''),
                      prefix.replace(b'synthetic', b'\\u0073ynthetic'), prefix.replace(b'synthetic', b'a\\b'),
                      prefix.replace(NONCE.encode(), b'A' * 32)):
            with self.assertRaises(phase.Reject): phase.validate_raw_prefix(wrong)

    def test_witness_exact_bytes_unique_bounded_and_before_completion(self):
        raw = phase.canonical_bytes(sidecar())
        frame = phase.FRAME_PREFIX + NONCE.encode() + b' ' + raw
        marker = b'UNIT4_EXECUTED ' + NONCE.encode() + b'\n'
        stdout = b'test synthetic ... ' + frame + marker + b'ok\n'
        self.assertEqual(phase.extract_witness(stdout, nonce=NONCE), raw)
        bad = (marker, frame + frame + marker, marker + frame, frame[:-1],
               frame + marker + marker, frame.replace(NONCE.encode(), b'b' * 32, 1) + marker,
               frame[:-1] + b'x\n' + marker, frame[:-1] + b' ' * 2000 + b'\n' + marker)
        for value in bad:
            with self.subTest(value=value[:40]), self.assertRaises(phase.Reject):
                phase.extract_witness(value, nonce=NONCE)
        # The validator is only a conjunct: no API can claim helper/process
        # success from a witness. Original success admission is caller-owned.
        self.assertIn('first admit the original successful helper/process receipt', phase.extract_witness.__doc__)


class ProbeProtocolControls(unittest.TestCase):
    def test_closed_catalogue_counts_order_and_twelve_key_environment(self):
        self.assertEqual(len(controls.CATALOGUE), 24)
        self.assertEqual(len(controls.roster()), 54)
        statuses = [controls.expected(*entry)['status'] for entry in controls.roster()]
        self.assertEqual({key: statuses.count(key) for key in controls.COUNTS}, controls.COUNTS)
        self.assertEqual(controls.roster()[:4], [('layout', p, r) for p in controls.PROFILES for r in controls.ROLES])
        argv, cwd, env = controls.inputs(ROOT, request())
        self.assertEqual(argv[1:], [controls.ENTRYPOINT, '--exact', '--ignored', '--nocapture', '--test-threads=1'])
        self.assertEqual(cwd, str(ROOT))
        self.assertEqual(len(env), 12)
        self.assertEqual(set(env), {'HOME', 'PATH', 'LANG', 'LC_ALL', 'PYTHONDONTWRITEBYTECODE',
                         'PYTHONNOUSERSITE', 'UNIT4_PHASE_PROBE', 'UNIT4_PHASE_NONCE', 'UNIT4_PHASE_ROLE',
                         'RUST_BACKTRACE', 'RUST_LIB_BACKTRACE', 'RUST_TEST_THREADS'})
        self.assertEqual(env['PATH'], '/usr/bin:/bin')
        self.assertNotIn('UNIT4_REJECT_KIND', env)
        self.assertNotIn('UNIT4_REJECT_OCCURRENCE', env)

    def test_all_request_keys_types_paths_expected_and_bounds(self):
        req = request()
        raw = controls.canonical_bytes(req)
        self.assertEqual(controls.validate_request(raw), req)
        for key in req:
            wrong = copy.deepcopy(req); del wrong[key]
            with self.subTest(missing=key), self.assertRaises(controls.Reject):
                controls.validate_request(controls.canonical_bytes(wrong))
        changes = [('extra', 1), ('nonce', 'A' * 32), ('probe', 'anything'), ('profile', 'bench'),
                   ('role', 'control'), ('entrypoint', 'observe_request'), ('session_sha256', 'a' * 63)]
        for key, value in changes:
            wrong = copy.deepcopy(req); wrong[key] = value
            with self.subTest(key=key), self.assertRaises(controls.Reject):
                controls.validate_request(controls.canonical_bytes(wrong))
        for path in ('/absolute', '../other', 'a/./b', 'a//b', 'a\\b', 'é', 'x' * 129):
            wrong = copy.deepcopy(req); wrong['binary']['path'] = path
            with self.subTest(path=path), self.assertRaises(controls.Reject):
                controls.validate_request(controls.canonical_bytes(wrong))
        for value in (True, -1, 1.5, 1 << 64):
            wrong = copy.deepcopy(req); wrong['binary']['bytes'] = value
            with self.assertRaises(controls.Reject): controls.validate_request(controls.canonical_bytes(wrong))
        wrong = copy.deepcopy(req); wrong['expected']['exit_code'] = False
        with self.assertRaises(controls.Reject): controls.validate_request(controls.canonical_bytes(wrong))
        for raw_wrong in (raw + b' ', raw.replace(b':', b': ', 1), raw.replace(b'"exit_code":0', b'"exit_code":0,"exit_code":0'), b'x' * 2049):
            with self.assertRaises(controls.Reject): controls.validate_request(raw_wrong)
        with self.assertRaises(controls.Reject): controls.expected('lex_ok_seal', 'debug', 'not_observed')

    def test_complete_catalogue_preflight_no_duplicates_omissions_or_wrong_binding(self):
        rows = [request(*entry, number=i + 1) for i, entry in enumerate(controls.roster())]
        kwargs = dict(session_sha256='1' * 64, authority_sha256='2' * 64,
                      driver_sha256='3' * 64, binaries=binaries())
        controls.validate_catalogue(rows, **kwargs)
        for mutation in ('missing', 'extra', 'duplicate', 'nonce', 'order', 'binary', 'expected', 'session'):
            wrong = copy.deepcopy(rows)
            if mutation == 'missing': wrong.pop()
            elif mutation == 'extra': wrong.append(copy.deepcopy(wrong[0]))
            elif mutation == 'duplicate': wrong[-1] = copy.deepcopy(wrong[0])
            elif mutation == 'nonce': wrong[-1]['nonce'] = wrong[0]['nonce']
            elif mutation == 'order': wrong.reverse()
            elif mutation == 'binary': wrong[0]['binary'] = wrong[1]['binary']
            elif mutation == 'expected': wrong[0]['expected']['status'] = 'passed'
            else: wrong[0]['session_sha256'] = 'f' * 64
            with self.subTest(mutation=mutation), self.assertRaises(controls.Reject):
                controls.validate_catalogue(wrong, **kwargs)

    def test_layout_closed_25_fields_including_all_alignments(self):
        layout = synthetic_layout()
        self.assertEqual(controls.validate_layout(layout, layout), layout)
        self.assertEqual(len(layout), 25)
        for key in layout:
            wrong = dict(layout); del wrong[key]
            with self.subTest(key=key), self.assertRaises(controls.Reject): controls.validate_layout(wrong)
        for key, value in [('rows_align', 0), ('token_align', 3), ('frame_bytes', 49),
                           ('named_total_bytes', 2496), ('capture_bytes', 80), ('row_bytes', True)]:
            wrong = dict(layout); wrong[key] = value
            with self.subTest(key=key), self.assertRaises(controls.Reject): controls.validate_layout(wrong)
        other = dict(layout); other.update(stdout_lock_bytes=24, frame_bytes=56, named_total_bytes=2552)
        controls.validate_layout(other)
        with self.assertRaises(controls.Reject): controls.validate_layout(other, layout)

    def test_every_catalogue_outcome_has_actual_expected_status_and_boundary(self):
        for i, entry in enumerate(controls.roster(), 1):
            req = request(*entry, number=i)
            stdout, stderr = transcript(req)
            status, boundary, count, layout, failure = controls.validate_probe_output(
                req, stdout, stderr, req['expected']['exit_code'], approved_layout=synthetic_layout())
            self.assertEqual((status, boundary, count, failure),
                             (req['expected']['status'], req['expected']['boundary'], 1, None))
            self.assertEqual(layout is not None, req['probe'] == 'layout')

    def test_rejection_requires_actual_panic_status_stderr_and_no_success_claim(self):
        req = request('event_cap'); stdout, stderr = transcript(req)
        for bad_out, bad_err, code in [(stdout, stderr, 0), (stdout, stderr, -9),
                                      (stdout, b'', 101), (stdout, stderr * 2, 101),
                                      (stdout, stderr.replace(b'cap.events', b'other'), 101),
                                      (stdout, stderr.replace(b'cap.events', b'cap.events_more'), 101),
                                      (stdout.replace(b'ARMED', b'OK'), stderr, 101),
                                      (stdout + controls.OK_PREFIX + b'fake\n', stderr, 101),
                                      (stdout + b'UNIT4_EXECUTED fake\n', stderr, 101),
                                      (stdout, stderr + b'UNIT4_LEXER_PHASE_V1 fake\n', 101),
                                      (stdout.replace(b'0 passed; 1 failed;', b'1 passed; 0 failed;'), stderr, 101)]:
            with self.subTest(code=code, stderr=bad_err[-50:]):
                self.assertEqual(controls.validate_probe_output(req, bad_out, bad_err, code)[0], 'failed')
        req = request('parser_missing_position'); stdout, stderr = transcript(req)
        self.assertIn(b'allocator event lacks actual parser position', stderr)
        self.assertEqual(controls.validate_probe_output(req, stdout, stderr, 101)[0], 'rejected_as_expected')

    def test_markers_are_exact_unique_complete_and_domain_separated(self):
        req = request(); stdout, stderr = transcript(req)
        for value in (stdout.replace(req['nonce'].encode(), b'a' * 32),
                      stdout.replace(b'inactive\n', b'inactive extra\n'),
                      stdout.replace(controls.OK_PREFIX, b'UNIT4_PHASE_PROBE_UNKNOWN_V1 '),
                      stdout + controls.OK_PREFIX + b'other\n',
                      stdout.replace(b'inactive\n', b'inactive\r\n'),
                      stdout.split(b'\n\n')[0], stdout + b'test result: ok. 1 passed; 0 failed; 0 ignored;\n'):
            self.assertEqual(controls.validate_probe_output(req, value, stderr, 0)[0], 'failed')
        for word in (b'UNIT4_EXECUTED', b'UNIT4_LEXER_PHASE_V1', controls.RESERVED_PREFIX):
            self.assertEqual(controls.validate_probe_output(req, stdout, word, 0)[0], 'failed')
        with self.assertRaises(controls.Reject): controls.validate_probe_output(req, b'x' * 4097, b'', 0)

    def test_receipt_every_key_closed_and_actual_stream_input_bindings(self):
        req = request(); stdout, stderr = transcript(req); row = receipt(req, stdout, stderr)
        controls.validate_receipt(controls.canonical_bytes(row), req, stdout, stderr, ROOT,
                                  approved_layout=synthetic_layout())
        for key in row:
            wrong = copy.deepcopy(row); del wrong[key]
            with self.subTest(key=key), self.assertRaises(controls.Reject):
                controls.validate_receipt(controls.canonical_bytes(wrong), req, stdout, stderr, ROOT,
                                          approved_layout=synthetic_layout())
        mutations = [('argv_sha256', 'f' * 64), ('cwd_sha256', 'f' * 64), ('environment_sha256', 'f' * 64),
                     ('started_ns', True), ('finished_ns', 0), ('exit_code', False), ('marker_count', 0),
                     ('layout', synthetic_layout()), ('timed_out', True), ('stream_limit_exceeded', True),
                     ('boundary', 'other'), ('failure', 'exit'), ('status', [])]
        for key, value in mutations:
            wrong = copy.deepcopy(row); wrong[key] = value
            with self.subTest(key=key), self.assertRaises(controls.Reject):
                controls.validate_receipt(controls.canonical_bytes(wrong), req, stdout, stderr, ROOT,
                                          approved_layout=synthetic_layout())
        with self.assertRaises(controls.Reject):
            controls.validate_receipt(controls.canonical_bytes(row), req, stdout + b'x', stderr, ROOT,
                                      approved_layout=synthetic_layout())

    def test_full_transport_exact_217_files_and_synthetic_closed_counts(self):
        data = protocol_fixture()
        self.assertEqual(len(data), 217)
        self.assertEqual(verify_fixture(data)['counts'], controls.COUNTS)
        for mutation in ('missing', 'extra', 'stale_stdout', 'receipt_rehash', 'wrong_count', 'wrong_index_path'):
            wrong = dict(data)
            path = str(ROOT / controls.ROOT_NAME / 'debug-observed-layout/stdout')
            if mutation == 'missing': del wrong[path]
            elif mutation == 'extra': wrong[str(ROOT / controls.ROOT_NAME / 'extra')] = b''
            elif mutation == 'stale_stdout': wrong[path] = wrong[path].replace(b'00000000000000000000000000000001', b'a' * 32)
            else:
                index_path = str(ROOT / controls.ROOT_NAME / 'probe-index.json')
                index = controls.decode(wrong[index_path], controls.INDEX_LIMIT)
                if mutation == 'wrong_count': index['counts']['passed'] = 19
                elif mutation == 'wrong_index_path': index['receipts'][0]['path'] = index['receipts'][1]['path']
                else:
                    receipt_path = str(ROOT / index['receipts'][0]['path'])
                    changed = controls.decode(wrong[receipt_path], controls.RECEIPT_LIMIT)
                    changed['environment_sha256'] = 'f' * 64
                    wrong[receipt_path] = controls.canonical_bytes(changed)
                    index['receipts'][0] = controls.identity(ROOT, index['receipts'][0]['path'], wrong[receipt_path])
                wrong[index_path] = controls.canonical_bytes(index)
            with self.subTest(mutation=mutation), self.assertRaises(controls.Reject): verify_fixture(wrong)

    def test_independent_protocol_field_width_and_retained_caps(self):
        max_layout = {key: 65535 for key in controls.LAYOUT_KEYS}
        self.assertEqual(len(controls.canonical_bytes(max_layout)), 546)
        self.assertEqual(54 * (2048 + 4096 + 4096 + 4096) + 16384, 790528)
        self.assertEqual(2473258 + 790528, 3263786)
        self.assertEqual((controls.REQUEST_LIMIT, controls.LAYOUT_LIMIT, controls.RECEIPT_LIMIT,
                          controls.STREAM_LIMIT, controls.INDEX_LIMIT, controls.TIMEOUT_SECONDS),
                         (2048, 1024, 4096, 4096, 16384, 30))
        req = request('parser_missing_position')
        req['probe'] = 'x' * 24; req['nonce'] = 'f' * 32
        req['binary'] = {'path': 'x' * 128, 'bytes': (1 << 64) - 1, 'sha256': 'f' * 64}
        req['expected']['boundary'] = 'x' * 24
        self.assertLessEqual(len(controls.canonical_bytes(req)), 856)
        identities = [{'path': 'x' * 128, 'bytes': (1 << 64) - 1, 'sha256': 'f' * 64} for _ in range(54)]
        index = {'schema': controls.INDEX_SCHEMA, 'session_sha256': 'f' * 64,
                 'authority_sha256': 'f' * 64, 'driver_sha256': 'f' * 64,
                 'counts': dict(controls.COUNTS), 'receipts': identities}
        self.assertLessEqual(len(controls.canonical_bytes(index)), 13609)
        measured = request('layout'); out, err = transcript(measured)
        maximal = receipt(measured, out, err)
        maximal.update(nonce='f' * 32, probe='x' * 24, profile='release', role='not_observed',
                       status='rejected_as_expected', exit_code=-255, boundary='x' * 24,
                       started_ns=(1 << 64) - 1, finished_ns=(1 << 64) - 1,
                       marker_count=256, layout=max_layout, failure='rejection_text')
        for key in ('request', 'stdout', 'stderr'):
            maximal[key] = {'path': 'x' * 128, 'bytes': (1 << 64) - 1, 'sha256': 'f' * 64}
        # Field-wise width arithmetic, not a semantically admissible receipt.
        self.assertEqual(len(controls.canonical_bytes(maximal)), 1990)

    def test_request_receipt_index_nested_duplicate_extra_and_type_controls(self):
        req = request('layout'); stdout, stderr = transcript(req); row = receipt(req, stdout, stderr)
        for name in ('request', 'stdout', 'stderr'):
            for key in ('path', 'bytes', 'sha256'):
                wrong = copy.deepcopy(row); del wrong[name][key]
                with self.subTest(identity=name, key=key), self.assertRaises(controls.Reject):
                    controls.validate_receipt(controls.canonical_bytes(wrong), req, stdout, stderr, ROOT,
                                              approved_layout=synthetic_layout())
            wrong = copy.deepcopy(row); wrong[name]['extra'] = 1
            with self.assertRaises(controls.Reject):
                controls.validate_receipt(controls.canonical_bytes(wrong), req, stdout, stderr, ROOT,
                                          approved_layout=synthetic_layout())
        raw = controls.canonical_bytes(row)
        for variant in (raw.replace(b'"marker_count":1', b'"marker_count":1,"marker_count":1'),
                        raw.replace(b'"row_bytes":8', b'"row_bytes":8,"row_bytes":8'),
                        raw.replace(b'"finished_ns":2', b'"finished_ns":2.0'),
                        raw.replace(b'"exit_code":0', b'"exit_code":NaN'), b'x' * 4097):
            with self.assertRaises(controls.Reject):
                controls.validate_receipt(variant, req, stdout, stderr, ROOT, approved_layout=synthetic_layout())
        fixture = protocol_fixture(); index_path = str(ROOT / controls.ROOT_NAME / 'probe-index.json')
        index = controls.decode(fixture[index_path], controls.INDEX_LIMIT)
        for key in index:
            bad = copy.deepcopy(index); del bad[key]
            data = dict(fixture); data[index_path] = controls.canonical_bytes(bad)
            with self.subTest(index_key=key), self.assertRaises(controls.Reject): verify_fixture(data)
        for key in ('measured', 'passed', 'rejected_as_expected'):
            bad = copy.deepcopy(index); bad['counts'][key] = float(bad['counts'][key])
            data = dict(fixture); data[index_path] = controls.canonical_bytes(bad)
            with self.assertRaises(controls.Reject): verify_fixture(data)


class EnvelopeSchemaControls(unittest.TestCase):
    @staticmethod
    def fixture(scope='passivity', profile='debug'):
        count = {'collection': 248, 'passivity': 6, 'u8_controls': 8}[scope]
        records = []
        for index in range(count):
            role = ('observed' if scope == 'collection' else
                    controls.ROLES[index % 2] if scope == 'passivity' else controls.ROLES[index // 4])
            case = ('case-' + str(index) if scope == 'collection' else
                    ('original', 'project', 'malformed')[index // 2] if scope == 'passivity' else
                    ('narrow', 'widen', 'trivia', 'numeric')[index % 4])
            path = ('collect-' + profile + '/result/' + case if scope == 'collection' else
                    'passivity-' + profile + '/result/' + case + ('-instrumented' if index % 2 == 0 else '-control')
                    if scope == 'passivity' else
                    'u8-policy-controls/' + profile + ('-observer-' if index < 4 else '-control-') + case)
            records.append({'case_id': case, 'nonce': format(index + 1, '032x'), 'role': role,
                            'process_index': index + (9 if scope == 'u8_controls' and profile == 'release' else 0),
                            **{key: DIGEST for key in ('process_receipt_sha256', 'binary_sha256', 'source_sha256',
                                                      'raw_sha256', 'stdout_sha256', 'stderr_sha256')},
                            'sidecar': {'path': path + '/lexer-phase.json',
                                        'bytes': 571, 'sha256': DIGEST}})
        parent = {'path': {'collection': 'collect-' + profile + '/result/execution-manifest.json',
                           'passivity': 'passivity-' + profile + '/result/report.json',
                           'u8_controls': 'u8-policy-controls/receipt.json'}[scope], 'bytes': 1000, 'sha256': DIGEST}
        value = {'schema': phase.ENVELOPE_SCHEMA, 'scope': scope, 'profile': profile,
                 'session_sha256': '1' * 64, 'authority_sha256': '2' * 64, 'adapter_sha256': '3' * 64,
                 'parent_receipt': parent, 'record_count': count, 'records': records}
        kwargs = {key: value[key] for key in ('scope', 'profile', 'session_sha256', 'authority_sha256',
                                             'adapter_sha256', 'parent_receipt', 'records')}
        return value, copy.deepcopy(kwargs)

    def test_exact_six_envelopes_and_524_ordered_processes(self):
        total = 0
        for scope in ('collection', 'passivity', 'u8_controls'):
            for profile in controls.PROFILES:
                value, kwargs = self.fixture(scope, profile)
                self.assertEqual(phase.validate_envelope(phase.canonical_bytes(value), **kwargs), value)
                total += value['record_count']
        self.assertEqual(total, 524)

    def test_missing_extra_duplicate_reordered_fields_and_processes(self):
        value, kwargs = self.fixture()
        for key in value:
            wrong = copy.deepcopy(value); del wrong[key]
            with self.subTest(key=key), self.assertRaises(phase.Reject):
                phase.validate_envelope(phase.canonical_bytes(wrong), **kwargs)
        for key in value['records'][0]:
            wrong = copy.deepcopy(value); del wrong['records'][0][key]
            with self.subTest(row_key=key), self.assertRaises(phase.Reject):
                phase.validate_envelope(phase.canonical_bytes(wrong), **kwargs)
        mutations = ('extra', 'row_extra', 'row_missing', 'row_duplicate', 'row_reorder', 'key_reorder')
        for mutation in mutations:
            wrong = copy.deepcopy(value)
            if mutation == 'extra': wrong['extra'] = None
            elif mutation == 'row_extra': wrong['records'].append(wrong['records'][0])
            elif mutation == 'row_missing': wrong['records'].pop()
            elif mutation == 'row_duplicate': wrong['records'][-1] = wrong['records'][0]
            elif mutation == 'row_reorder': wrong['records'].reverse()
            else: wrong = dict(reversed(list(wrong.items())))
            with self.subTest(mutation=mutation), self.assertRaises(phase.Reject):
                phase.validate_envelope(phase.canonical_bytes(wrong), **kwargs)
        raw = phase.canonical_bytes(value)
        with self.assertRaises(phase.Reject):
            phase.validate_envelope(raw.replace(b'"record_count":6', b'"record_count":6,"record_count":6'), **kwargs)

    def test_authentication_kept_fixed_rejects_coherent_local_record_changes(self):
        value, kwargs = self.fixture()
        changes = [('process_index', 1), ('process_index', False), ('role', 'not_observed'),
                   ('binary_sha256', 'f' * 64), ('source_sha256', 'f' * 64), ('stdout_sha256', 'f' * 64),
                   ('process_receipt_sha256', 'f' * 64), ('raw_sha256', 'f' * 64), ('nonce', 'f' * 32)]
        for key, changed in changes:
            wrong = copy.deepcopy(value); wrong['records'][0][key] = changed
            with self.subTest(key=key), self.assertRaises(phase.Reject):
                phase.validate_envelope(phase.canonical_bytes(wrong), **kwargs)
        for path in ('other/lexer-phase.json', '../lexer-phase.json', '/lexer-phase.json',
                     'a//lexer-phase.json', 'a/./lexer-phase.json', 'a\\lexer-phase.json'):
            wrong = copy.deepcopy(value); wrong['records'][0]['sidecar']['path'] = path
            wrong['records'][0]['sidecar']['sha256'] = 'f' * 64
            with self.subTest(path=path), self.assertRaises(phase.Reject):
                phase.validate_envelope(phase.canonical_bytes(wrong), **kwargs)
        for key, changed in [('profile', 'release'), ('scope', 'collection'), ('session_sha256', 'f' * 64),
                             ('authority_sha256', 'f' * 64), ('adapter_sha256', 'f' * 64), ('record_count', True)]:
            wrong = copy.deepcopy(value); wrong[key] = changed
            with self.subTest(key=key), self.assertRaises(phase.Reject):
                phase.validate_envelope(phase.canonical_bytes(wrong), **kwargs)

    def test_parent_identity_sidecar_caps_and_unsigned_types(self):
        value, kwargs = self.fixture()
        for key, changed in [('path', 'passivity-release/result/report.json'), ('bytes', -1),
                             ('bytes', True), ('bytes', 1 << 64), ('sha256', 'A' * 64)]:
            wrong = copy.deepcopy(value); wrong['parent_receipt'][key] = changed
            with self.assertRaises(phase.Reject): phase.validate_envelope(phase.canonical_bytes(wrong), **kwargs)
        for index, size in ((0, 0), (0, 1913), (1, 573), (1, True)):
            wrong = copy.deepcopy(value); wrong['records'][index]['sidecar']['bytes'] = size
            with self.assertRaises(phase.Reject): phase.validate_envelope(phase.canonical_bytes(wrong), **kwargs)
        with self.assertRaises(phase.Reject): phase.validate_envelope(b'x' * 225510, **kwargs)


class CaptureSourceControls(unittest.TestCase):
    class FakePipe(io.BytesIO):
        def __init__(self, fd):
            super().__init__(); self.fd = fd
        def fileno(self): return self.fd

    class FakeChild:
        def __init__(self):
            self.stdout = CaptureSourceControls.FakePipe(100)
            self.stderr = CaptureSourceControls.FakePipe(101)
            self.pid = 99999999
            self.returncode = None
            self.exit_code = 0
        def wait(self, timeout=None):
            self.returncode = self.exit_code
            return self.returncode

    class FakeSelector:
        def __init__(self): self.mapping = {}
        def register(self, stream, events, name):
            self.mapping[stream.fileno()] = types.SimpleNamespace(fd=stream.fileno(), fileobj=stream, data=name)
        def unregister(self, stream): del self.mapping[stream.fileno()]
        def get_map(self): return self.mapping
        def select(self, timeout): return [(value, 1) for value in list(self.mapping.values())]
        def close(self): pass

    def test_bounded_reader_uses_remaining_plus_one_and_never_retains_detection(self):
        child = self.FakeChild(); reads = []
        chunks = {100: [b'x' * 4096, b'!'], 101: [b'err', b'']}
        def read(fd, count):
            reads.append((fd, count))
            return chunks[fd].pop(0)
        with patch.object(controls.subprocess, 'Popen', return_value=child) as launch, \
             patch.object(controls.selectors, 'DefaultSelector', self.FakeSelector), \
             patch.object(controls.os, 'set_blocking'), patch.object(controls.os, 'read', side_effect=read), \
             patch.object(controls.os, 'killpg') as kill:
            result = controls.bounded_capture(['synthetic'], '/synthetic', {'SYNTHETIC': '1'})
        self.assertEqual(result['stdout'], b'x' * 4096)
        self.assertEqual(result['stderr'], b'err')
        self.assertNotIn(b'!', result['stdout'])
        self.assertEqual(reads, [(100, 4097), (101, 4097), (100, 1)])
        self.assertEqual(result['failure'], 'stream_limit')
        self.assertTrue(result['stream_limit_exceeded'])
        kill.assert_called_once_with(child.pid, controls.signal.SIGKILL)
        self.assertTrue(launch.call_args.kwargs['start_new_session'])

    def test_spawn_failure_keeps_no_invented_exit_or_transcript(self):
        with patch.object(controls.subprocess, 'Popen', side_effect=OSError('synthetic failure')):
            result = controls.bounded_capture(['synthetic'], '/synthetic', {})
        self.assertEqual(result['failure'], 'spawn')
        self.assertIsNone(result['exit_code'])
        self.assertEqual((result['stdout'], result['stderr']), (b'', b''))
        self.assertLessEqual(result['started_ns'], result['finished_ns'])

    def test_complete_independent_stream_capture_drains_until_both_eof(self):
        child = self.FakeChild(); reads = []
        chunks = {100: [b'a' * 2000, b'b' * 2096, b''], 101: [b'e' * 4096, b'']}
        def read(fd, count):
            reads.append((fd, count)); return chunks[fd].pop(0)
        with patch.object(controls.subprocess, 'Popen', return_value=child), \
             patch.object(controls.selectors, 'DefaultSelector', self.FakeSelector), \
             patch.object(controls.os, 'set_blocking'), patch.object(controls.os, 'read', side_effect=read), \
             patch.object(controls.os, 'killpg') as kill:
            result = controls.bounded_capture(['synthetic'], '/synthetic', {})
        self.assertEqual(result['stdout'], b'a' * 2000 + b'b' * 2096)
        self.assertEqual(result['stderr'], b'e' * 4096)
        self.assertEqual(reads, [(100, 4097), (101, 4097), (100, 2097), (101, 1), (100, 1)])
        self.assertIsNone(result['failure'])
        self.assertEqual(result['exit_code'], 0)
        kill.assert_not_called()

    def test_timeout_kills_reaps_and_never_admits_expected_rejection(self):
        child = self.FakeChild(); child.exit_code = -9
        with patch.object(controls.subprocess, 'Popen', return_value=child), \
             patch.object(controls.selectors, 'DefaultSelector', self.FakeSelector), \
             patch.object(controls.os, 'set_blocking'), \
             patch.object(controls.time, 'monotonic', side_effect=[0, 31]), \
             patch.object(controls.os, 'killpg') as kill:
            result = controls.bounded_capture(['synthetic'], '/synthetic', {})
        self.assertEqual(result['failure'], 'timeout')
        self.assertTrue(result['timed_out'])
        self.assertEqual(result['exit_code'], -9)
        kill.assert_called_once()

    def test_no_ambient_environment_unbounded_communicate_or_subset_cli(self):
        source = Path(controls.__file__).read_text()
        tree = ast.parse(source)
        self.assertNotIn('os.environ', source)
        self.assertNotIn('.communicate(', source)
        self.assertNotIn('argparse', source)
        self.assertIn('os.read(key.fd, allowance + 1)', source)
        self.assertEqual(sum(isinstance(node, ast.Call) and isinstance(node.func, ast.Attribute)
                             and node.func.attr == 'Popen' for node in ast.walk(tree)), 1)


class SourceInverseControls(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        import portable
        cls.p = portable
        # Authenticate the complete outer authority before projecting its exact
        # immutable pre-phase object into the retained public composers.
        current_raw = (portable.HERE / 'authority.json').read_bytes()
        portable.same(portable.sha(current_raw), portable.AUTHORITY_SHA, 'immutable current authority bytes')
        cls.authority = portable.phase_previous(portable.authority())
        cls.observer = portable.compose_observer_initializer(cls.authority)
        cls.lexer = portable.compose_current_lexer(cls.authority,
            (portable.REPOSITORY / 'src/frontend/lexer.rs').read_bytes())
        cls.budget = portable.compose_array_instrumentation(cls.authority, 'src/frontend/project/budget.rs',
            (portable.REPOSITORY / 'src/frontend/project/budget.rs').read_bytes())

    def test_complete_three_body_inverse_and_both_observer_roles(self):
        for name in ('lexer', 'budget'):
            original = getattr(self, name)
            compose, invert = getattr(phase, 'compose_' + name), getattr(phase, 'invert_' + name)
            derived = compose(original)
            self.assertEqual(invert(derived), original)
            for wrong in (original + b'\n', original[:-1], derived, original * 2):
                with self.subTest(name=name), self.assertRaises(phase.Reject): compose(wrong)
            with self.assertRaises(phase.Reject): invert(derived + b'\n')
        for role in phase.ROLES:
            derived = phase.compose_observer(self.observer, role)
            self.assertEqual(phase.invert_observer(derived, role), self.observer)
            for wrong in (self.observer + b'\n', self.observer[:-1], derived, self.observer * 2):
                with self.subTest(role=role), self.assertRaises(phase.Reject): phase.compose_observer(wrong, role)
            with self.assertRaises(phase.Reject): phase.invert_observer(derived, phase.ROLES[role == 'observed'])

    def test_retained_function_bodies_and_original_position_ordinal_raw_write(self):
        derived = phase.compose_observer(self.observer, 'observed')
        for function in ('reserve', 'lex_token', 'reset', 'run_mode'):
            pattern = rb'(?m)^(?:pub(?:\([^)]*\))? )?fn ' + function.encode() + rb'\([\s\S]*?^}\n'
            before = re.search(pattern, self.observer)
            after = re.search(pattern, derived)
            self.assertIsNotNone(before, function)
            self.assertIsNotNone(after, function)
            self.assertEqual(before.group(), after.group(), function)
        self.assertIn(b'position.expect("allocator event lacks actual parser position")', derived)
        self.assertIn(b'UNIT4_RESERVE_FAIL_AT', derived)
        self.assertIn(b'std::fs::write(env("UNIT4_RAW_OUTPUT"), out).expect("raw observation write");', derived)
        self.assertIn(b'UNIT4_EXECUTED', derived)
        self.assertEqual(derived.count(b'#[ignore]'), self.observer.count(b'#[ignore]') + 1)

    def test_control_lexer_budget_remain_hook_free_and_hook_sites_preserve_cardinality(self):
        a, p = self.authority, self.p
        for name in ('src/frontend/lexer.rs', 'src/frontend/project/budget.rs'):
            raw = (p.REPOSITORY / name).read_bytes()
            control_row = next(row for row in a['current']['current_control_derived_files'] if row['path'] == name)
            self.assertEqual(control_row, {'path': name, 'bytes': len(raw), 'sha256': p.sha(raw)})
            self.assertNotIn(b'unit4_observer::', raw)
        lexer = phase.compose_lexer(self.lexer)
        hook = b'crate::frontend::parser::unit4_observer::lexer_phase::lex_token(*tokens.last().unwrap());'
        self.assertEqual(lexer.count(hook), 1)
        self.assertIn(b'tokens.push(token);\n    ' + hook + b'\n    Ok(())', lexer)
        budget = phase.compose_budget(self.budget)
        self.assertEqual(budget.count(b'lexer_phase::reserve(kind, length, element_bytes, success);'), 1)

    def test_private_probe_state_and_no_ordinary_environment_dependencies(self):
        module = phase.rust_module('observed')
        ordinary, probes = module.split(b'    fn probe_state(', 1)
        self.assertEqual(set(re.findall(rb'std::env::var\("([A-Z0-9_]+)"\)', probes)),
                         {b'UNIT4_PHASE_PROBE', b'UNIT4_PHASE_NONCE', b'UNIT4_PHASE_ROLE'})
        self.assertNotRegex(probes, rb'\b(?:reset|run_mode|observe_request)\s*\(')
        self.assertNotRegex(ordinary, rb'\bprobe_[a-z_]+\s*\(')
        self.assertNotIn(b'UNIT4_REJECT_', probes)
        self.assertNotIn(b'UNIT4_SOURCE', probes)
        self.assertNotIn(b'UNIT4_TOKEN_LIMIT', probes)
        self.assertIn(b'enabled: !CONTROL_ROLE, tokens, position: None,', probes)
        self.assertIn(b'reject_kind: String::new(), reject_occurrence: 0,', probes)
        self.assertIn(b'..State::default()', probes)
        self.assertIn(b'let tokens = probe_lex("a").expect("actual parser fixture lex");', probes)
        self.assertIn(b'probe_state(tokens);', probes)
        self.assertNotIn(b'catch_unwind', module)
        self.assertNotIn(b'Token {', probes)
        probe_ids = set(re.findall(rb'^            "([a-z_]+)"(?: if CONTROL_ROLE)? =>', probes, re.M))
        self.assertEqual(probe_ids, {row[0].encode() for row in controls.CATALOGUE})

    def test_fixed_carriers_no_ordinary_unbounded_serialization_or_shadow_ledger(self):
        module = phase.rust_module('observed')
        ordinary, probes = module.split(b'    fn probe_state(', 1)
        self.assertIn(b'#[repr(C)]', ordinary)
        self.assertIn(b'rows: [Row; 16]', ordinary)
        self.assertIn(b'nonce: [u8; 32]', ordinary)
        self.assertIn(b'case_id: [u8; 64]', ordinary)
        self.assertIn(b'decimal: [u8; 20]', ordinary)
        self.assertNotIn(b'Option<Row>', ordinary)
        self.assertNotIn(b'Vec::new()', ordinary)
        for expression in (b'format!(', b'.to_string(', b'.clone(', b'String::new(', b'println!('):
            self.assertNotIn(expression, ordinary)
        self.assertEqual(ordinary.count(b'let mut destination = [0u8; CAP];'), 1)
        self.assertEqual(len(re.findall(rb's\.ledger\.serialized_length\s*=(?!=)', ordinary)), 1)
        self.assertIn(b's.used == usize::from(s.ledger.serialized_length)', ordinary)
        self.assertIn(b's.used = COUNT_ONLY;', ordinary)
        self.assertIn(b'assert_eq!(probe_render(&cell, MAX_RAW, &mut undersized), Err(()));', probes)
        self.assertIn(b'assert!(undersized.iter().all(|byte| *byte == 0xa5));', probes)
        after_close = ordinary.split(b'    pub(in crate::frontend) fn begin_mode(', 1)[1]
        self.assertNotRegex(after_close, rb'\.(?:rows|row_count|occupied_row_bytes|token_count|failed|lex_result)\s*=(?!=)')

    def test_layout_report_measures_every_typed_alignment(self):
        module = phase.rust_module('observed')
        layout = module.split(b'    fn probe_layout(', 1)[1].split(b'    #[test]', 1)[0]
        keys = re.findall(rb'\\"([a-z_]+)\\":', layout)
        self.assertEqual(keys, [name.encode() for name in controls.LAYOUT_KEYS])
        for name in (b'Row', b'[Row; 16]', b'Ledger', b'RefCell<Ledger>', b"Ref<'static, Ledger>",
                     b"RefMut<'static, Ledger>", b'Token', b"CaptureCarriers<'static, 'static>",
                     b"SerializeCarriers<'static, Ref<'static, Ledger>, CAP>",
                     b"TokenForwardCarriers<'static>", b"StdoutLock<'static>", b"FrameCarriers<'static>"):
            self.assertIn(b'align_of::<' + name + b'>()', layout)
            self.assertIn(b'size_of::<' + name + b'>()', layout)
        self.assertLess(layout.rfind(b'assert_eq!'), layout.find(b'println!'))
        self.assertIn(b'+ size_of::<Token>()', layout)


if __name__ == '__main__':
    unittest.main()
