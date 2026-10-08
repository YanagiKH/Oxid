"""Fast v2 evidence controls; no Rust or native producer build is required."""
import copy
import json
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch

import qualify_hir_producers_v2 as qualification


def write_json(path, data):
    path.write_text(json.dumps(data) + '\n')


def evidence_fixture(root):
    """Synthetic evidence for validator tests, never qualification evidence."""
    build = root / 'v2-build'
    (build / 'sources').mkdir(parents=True)
    (build / 'bundle').mkdir()
    manifest = json.dumps({'schema_version': 2, 'sources': {'one': {}}}).encode()
    (build / 'sources/source-manifest.json').write_bytes(manifest)
    expected = dict(source_head='a' * 40, source_tree='b' * 40,
                    compiler_sha256='c' * 64, harness_sha256='d' * 64,
                    builder_sha256='e' * 64,
                    source_manifest_sha256=qualification.builder.digest(manifest))
    hashes = {}
    for role in ('parser', 'static'):
        data = ('synthetic-' + role).encode()
        (build / 'bundle' / role).write_bytes(data)
        hashes[role] = qualification.builder.digest(data)
    (build / 'bundle/manifest.txt').write_text('OXID-HIR-PRODUCERS-2\n' + ''.join(
        role + ' ' + hashes[role] + '\n' for role in ('parser', 'static')))
    write_json(build / 'build-evidence.json', dict(
        schema_version=2, compiler_sha256=expected['compiler_sha256'],
        source_manifest_sha256=expected['source_manifest_sha256'],
        executable_sha256=hashes, source_count=1))
    receipts = []
    for name, status in qualification.EXPECTED_RECEIPTS:
        receipt = dict(name=name, status=status, argv=['synthetic-control', name])
        for stream in ('stdout', 'stderr'):
            data = b''
            (root / (name + '.' + stream)).write_bytes(data)
            receipt[stream + '_sha256'] = qualification.builder.digest(data)
        receipts.append(receipt)
    write_json(root / 'receipts.json', receipts)
    summary = dict(expected, schema_version=2, status='passed', receipts=58,
                   receipts_sha256=qualification.builder.digest((root / 'receipts.json').read_bytes()),
                   build_evidence_sha256=qualification.builder.digest((build / 'build-evidence.json').read_bytes()),
                   executable_sha256=hashes)
    write_json(root / 'summary.json', summary)
    return expected, summary, receipts


class V2EvidenceControls(unittest.TestCase):
    def test_complete_evidence_is_admitted(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            expected, summary, _ = evidence_fixture(root)
            self.assertEqual(qualification.validate_summary(root, expected), summary)
            self.assertEqual(len(qualification.EXPECTED_RECEIPTS), 58)
            self.assertEqual(sum(status != 0 for _, status in qualification.EXPECTED_RECEIPTS), 15)

    def test_summary_requires_exact_recipe_and_every_input_identity(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            expected, original, _ = evidence_fixture(root)
            cases = [(key, 'wrong') for key in expected]
            cases += [('receipts', 57), ('receipts', 59), ('receipts', '58'),
                      ('status', 'failed'), ('schema_version', True),
                      ('receipts_sha256', 'bad'), ('build_evidence_sha256', 'bad'),
                      ('executable_sha256', {})]
            for key, value in cases:
                with self.subTest(key=key, value=value):
                    write_json(root / 'summary.json', dict(original, **{key: value}))
                    with self.assertRaises(RuntimeError):
                        qualification.validate_summary(root, expected)

    def test_receipt_duplicates_wrong_order_status_and_streams_are_rejected(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            _, _, original = evidence_fixture(root)
            wrong_order = copy.deepcopy(original)
            wrong_order[0], wrong_order[1] = wrong_order[1], wrong_order[0]
            duplicate = copy.deepcopy(original)
            duplicate[1] = duplicate[0]
            bad_status = copy.deepcopy(original)
            bad_status[0]['status'] = False
            bad_command = copy.deepcopy(original)
            bad_command[0]['argv'] = []
            bad_stream = copy.deepcopy(original)
            bad_stream[0]['stdout_sha256'] = 'bad'
            for receipts in (original[:-1], original + original[:1], wrong_order,
                             duplicate, bad_status, bad_command, bad_stream):
                with self.subTest(receipts=receipts[:2]):
                    write_json(root / 'receipts.json', receipts)
                    with self.assertRaises(RuntimeError):
                        qualification.validate_receipts(root)
            write_json(root / 'receipts.json', original)
            (root / 'leading-129-parser.stdout').write_bytes(b'changed')
            with self.assertRaisesRegex(RuntimeError, 'stream identity'):
                qualification.validate_receipts(root)

    def test_build_compiler_manifest_count_and_binary_identities_are_rejected(self):
        for key, value in (('compiler_sha256', 'bad'), ('source_manifest_sha256', 'bad'),
                           ('source_count', True), ('source_count', 2), ('executable_sha256', {})):
            with self.subTest(key=key), tempfile.TemporaryDirectory() as tmp:
                root = Path(tmp)
                expected, _, _ = evidence_fixture(root)
                path = root / 'v2-build/build-evidence.json'
                build = json.loads(path.read_text())
                write_json(path, dict(build, **{key: value}))
                with self.assertRaises(RuntimeError):
                    qualification.validate_build(root, expected)
        for relative in ('v2-build/bundle/parser', 'v2-build/bundle/manifest.txt',
                         'v2-build/sources/source-manifest.json'):
            with self.subTest(relative=relative), tempfile.TemporaryDirectory() as tmp:
                root = Path(tmp)
                expected, _, _ = evidence_fixture(root)
                (root / relative).write_bytes(b'changed')
                with self.assertRaises(RuntimeError):
                    qualification.validate_build(root, expected)

    def test_malformed_json_cannot_pass(self):
        for relative in ('summary.json', 'receipts.json', 'v2-build/build-evidence.json'):
            with self.subTest(relative=relative), tempfile.TemporaryDirectory() as tmp:
                root = Path(tmp)
                expected, _, _ = evidence_fixture(root)
                (root / relative).write_text('{invalid')
                with self.assertRaises((RuntimeError, ValueError)):
                    qualification.validate_summary(root, expected)

    def test_input_change_prevents_success_summary(self):
        with tempfile.TemporaryDirectory() as tmp:
            run = qualification.Run.__new__(qualification.Run)
            run.root = Path(tmp)
            run.compiler = run.root / 'compiler'
            run.identities = {'compiler_sha256': 'before'}
            with patch.object(qualification, 'input_identities', return_value={'compiler_sha256': 'after'}):
                with self.assertRaisesRegex(RuntimeError, 'inputs changed'):
                    run.finish()
            self.assertFalse((run.root / 'summary.json').exists())

    def test_existing_evidence_is_never_overwritten(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            executable = root / 'compiler'
            executable.write_bytes(b'keep')
            with self.assertRaises(FileExistsError):
                qualification.Run(executable, root, root)
            self.assertEqual(executable.read_bytes(), b'keep')

    def test_failed_timed_out_and_missing_commands_keep_partial_evidence(self):
        cases = [(subprocess.CompletedProcess(['fake'], 7, b'partial', b'error'), 7),
                 (subprocess.TimeoutExpired(['fake'], 1, output=b'partial', stderr=b'error'), 'outer-timeout'),
                 (FileNotFoundError('missing-helper'), 'spawn-failed')]
        for result, status in cases:
            with self.subTest(status=status), tempfile.TemporaryDirectory() as tmp:
                run = qualification.Run.__new__(qualification.Run)
                run.root = Path(tmp)
                run.env, run.receipts = {}, []
                kwargs = {'side_effect': result} if isinstance(result, Exception) else {'return_value': result}
                with patch.object(qualification.subprocess, 'run', **kwargs):
                    with self.assertRaises(RuntimeError):
                        run.call('failure', ['fake'])
                receipt = json.loads((run.root / 'receipts.json').read_text())[0]
                self.assertEqual(receipt['status'], status)
                self.assertEqual(receipt['stdout_sha256'], qualification.builder.digest((run.root / 'failure.stdout').read_bytes()))
                self.assertEqual(receipt['stderr_sha256'], qualification.builder.digest((run.root / 'failure.stderr').read_bytes()))
                self.assertFalse((run.root / 'summary.json').exists())

    def test_producer_execution_requires_complete_identity_and_success(self):
        source, ast, opa, wire = b'source', b'ast', b'parser-wire', b'static-wire'
        hashes = {'parser': 'p' * 64, 'static': 's' * 64}
        rows = []
        for role, data, output in zip(('parser', 'static'), (source, ast), (opa, wire)):
            rows.append(dict(kind='hir-producer', role=role, executable_sha256=hashes[role],
                             input_sha256=qualification.builder.digest(data), input_written=len(data),
                             captured_stdout_sha256=qualification.builder.digest(output), captured_stdout_bytes=len(output),
                             captured_stderr_bytes=0, spawned=True, leader_reaped=True, stop='exited', exit_status=0, signal=None))
        rows.append({'success': True})

        def validate(records):
            result = subprocess.CompletedProcess([], 0, b'\n'.join(json.dumps(row).encode() for row in records), b'')
            qualification.validate_execution(result, source, ast, opa, wire, hashes)

        validate(rows)
        for key, value in (('role', 'static'), ('executable_sha256', 'bad'), ('input_sha256', 'bad'),
                           ('captured_stdout_sha256', 'bad'), ('input_written', 0), ('captured_stdout_bytes', 0),
                           ('captured_stderr_bytes', False), ('spawned', 1), ('leader_reaped', False),
                           ('stop', 'deadline'), ('exit_status', False), ('signal', 9)):
            with self.subTest(key=key):
                changed = copy.deepcopy(rows)
                changed[0][key] = value
                with self.assertRaises(RuntimeError):
                    validate(changed)
        for records in ([], [None], rows[1:], rows[:-1], rows + [{'success': False}]):
            with self.subTest(records=records), self.assertRaises(RuntimeError):
                validate(records)


if __name__ == '__main__':
    unittest.main()
