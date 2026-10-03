"""Deterministic cancellation races; no Rust builds or corpus execution."""
import contextlib
import hashlib
import io
import json
import os
from pathlib import Path
import subprocess
import sys
import tarfile
import tempfile
import unittest
from unittest import mock

import collect_unit3_ci_diagnostics as compact
import preserve_unit3_ci_evidence as evidence


class EvidenceTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.base = Path(self.temporary.name)
        self.root = self.base / 'typed-project-unit3'
        self.package = self.base / 'package'
        self.root.mkdir()
        for name in evidence.PACKAGE_INPUTS:
            self.write(self.package / name, b'{"identity":"fixture"}\n')
        self.write(self.root / 'source-plan.json', b'{"planned_rows":304}\n')
        self.write(self.root / 'inputs/sources/origin-unicode-crlf-owned_overflow.unavailable/main.ox',
                   b'// unicode \xc3\xa9\r\nfn main() {}\r\n')
        self.victim = self.root / 'inputs/sources/origin-unicode-crlf-owned_overflow.unavailable/main.ox'
        self.name = self.victim.relative_to(self.root).as_posix()

    @staticmethod
    def write(path, payload):
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(payload)

    def run_preserve(self, outcome='success', observe=lambda phase, name: None):
        return evidence.preserve(self.root, self.package, outcome, _observe=observe)

    def assert_archive(self, status):
        archive_path = self.base / evidence.ARCHIVE
        checksum = self.base / 'typed-project-unit3-evidence.sha256'
        self.assertEqual(checksum.read_text(), hashlib.sha256(archive_path.read_bytes()).hexdigest()
                         + '  ' + evidence.ARCHIVE + '\n')
        with tarfile.open(archive_path) as archive:
            document = json.load(archive.extractfile(evidence.INDEX))
            self.assertEqual(document, json.loads((self.root / evidence.INDEX).read_bytes()))
            self.assertEqual(document['archive_status'], status)
            names = [row['path'] for row in document['files']]
            self.assertEqual(archive.getnames(), names + [evidence.INDEX])
            self.assertEqual(len(names), len(set(names)))
            for row in document['files']:
                member = archive.getmember(row['path'])
                if row.get('type') == 'symlink':
                    self.assertTrue(member.issym())
                    self.assertEqual(member.linkname, row['target'])
                else:
                    payload = archive.extractfile(member).read()
                    self.assertEqual(len(payload), row['bytes'])
                    self.assertEqual(hashlib.sha256(payload).hexdigest(), row['sha256'])
            self.assertEqual(document['retained_bytes'], sum(row.get('bytes', 0) for row in document['files']))
        return document

    def test_stable_success_preserves_membership_bytes_modes_and_package_identity(self):
        self.write(self.root / 'source-target/debug/deps/oxid-required', b'\x7fELFrequired test binary')
        self.write(self.root / 'mutations-v2/target/release/deps/oxid-required', b'\x7fELFanother binary')
        (self.root / 'source-target/debug/deps/oxid-required').chmod(0o755)
        self.write(self.root / 'source-target/debug/deps/oxid-not-elf', b'build metadata')
        self.write(self.root / 'source-target/debug/deps/dependency', b'\x7fELFcache')
        (self.root / 'source-target/debug/deps/oxid-alias').symlink_to('oxid-required')
        self.write(self.root / 'raw/observations.json.gz', b'raw compressed fixture')
        self.run_preserve()
        document = self.assert_archive('complete')
        self.assertEqual({row['path'] for row in document['files']}, {
            self.name, 'source-plan.json', 'raw/observations.json.gz',
            'source-target/debug/deps/oxid-required', 'mutations-v2/target/release/deps/oxid-required'})
        self.assertEqual(document['preservation_issues'], [])
        for name in evidence.PACKAGE_INPUTS:
            payload = (self.package / name).read_bytes()
            self.assertEqual(document['package_inputs'][name],
                             {'bytes': len(payload), 'sha256': hashlib.sha256(payload).hexdigest()})
        with tarfile.open(self.base / evidence.ARCHIVE) as archive:
            self.assertEqual(archive.getmember('source-target/debug/deps/oxid-required').mode, 0o755)

    def test_cancelled_and_failed_disappearance_and_rename_at_each_phase(self):
        # Separate invocations prevent a preceding archive from masking a later fault.
        for outcome in ('cancelled', 'failure'):
            for phase in ('stat', 'read', 'archive'):
                for operation in ('unlink', 'rename'):
                    with self.subTest(outcome=outcome, phase=phase, operation=operation):
                        fixture = EvidenceTests()
                        fixture.setUp()
                        self.addCleanup(fixture.doCleanups)
                        fired = []

                        def fault(at, name):
                            if at == phase and name == fixture.name and not fired:
                                fired.append(True)
                                if operation == 'unlink':
                                    fixture.victim.unlink()
                                else:
                                    fixture.victim.parent.rename(fixture.victim.parent.with_suffix(''))

                        fixture.run_preserve(outcome, fault)
                        document = fixture.assert_archive('partial')
                        self.assertTrue(fired)
                        self.assertTrue(any(row['path'] == fixture.name for row in document['preservation_issues']))
                        self.assertIn('source-plan.json', [row['path'] for row in document['files']])
                        if phase == 'archive':
                            self.assertIn(fixture.name, [row['path'] for row in document['files']])
                        self.assertTrue(document['partial_reasons'])

    def test_success_refuses_same_size_mutation_at_read_archive_and_final_verification(self):
        for phase in ('read', 'after-read', 'archive', 'verify'):
            with self.subTest(phase=phase):
                fixture = EvidenceTests()
                fixture.setUp()
                self.addCleanup(fixture.doCleanups)
                before = fixture.victim.stat()
                fired = []

                def fault(at, name):
                    if at == phase and (name == fixture.name or phase == 'verify') and not fired:
                        fired.append(True)
                        fixture.victim.write_bytes(b'x' * before.st_size)
                        # Size and mtime alone cannot hide the changed source.
                        os.utime(fixture.victim, ns=(before.st_atime_ns, before.st_mtime_ns))

                with self.assertRaises(evidence.IncompleteEvidence):
                    fixture.run_preserve('success', fault)
                document = fixture.assert_archive('partial')
                self.assertTrue(fired)
                self.assertTrue(any(row['path'] == fixture.name for row in document['preservation_issues']))

    def test_success_refuses_disappearance_and_preserves_read_snapshot(self):
        def fault(phase, name):
            if phase == 'archive' and name == self.name:
                self.victim.unlink()
        with self.assertRaises(evidence.IncompleteEvidence):
            self.run_preserve('success', fault)
        document = self.assert_archive('partial')
        self.assertIn(self.name, [row['path'] for row in document['files']])

    def test_success_refuses_new_evidence_after_inventory(self):
        def fault(phase, name):
            if phase == 'verify':
                self.write(self.root / 'late-result.json', b'{}')
        with self.assertRaises(evidence.IncompleteEvidence):
            self.run_preserve('success', fault)
        document = self.assert_archive('partial')
        self.assertTrue(any(row['path'] == 'late-result.json' for row in document['preservation_issues']))

    def test_broken_absolute_and_directory_symlinks_are_retained_without_following(self):
        outside = self.base / 'outside'
        self.write(outside / 'secret', b'do not include')
        (self.root / 'directory-link').symlink_to(outside, target_is_directory=True)
        (self.root / 'broken-link').symlink_to('missing')
        (self.root / 'absolute-link').symlink_to(outside / 'secret')
        self.run_preserve()
        document = self.assert_archive('complete')
        links = {row['path']: row['target'] for row in document['files'] if row.get('type') == 'symlink'}
        self.assertEqual(links, {'directory-link': str(outside), 'broken-link': 'missing',
                                 'absolute-link': str(outside / 'secret')})
        self.assertFalse(any('secret' in row['path'] for row in document['files']))

    def test_ancestor_replaced_with_symlink_never_reads_external_bytes(self):
        original = self.victim.read_bytes()
        outside = self.base / 'outside'
        self.write(outside / 'main.ox', b'external bytes')

        def fault(phase, name):
            if phase == 'read' and name == self.name:
                parent = self.victim.parent
                parent.rename(self.base / 'moved')
                parent.symlink_to(outside, target_is_directory=True)
        self.run_preserve('cancelled', fault)
        self.assert_archive('partial')
        with tarfile.open(self.base / evidence.ARCHIVE) as archive:
            self.assertEqual(archive.extractfile(self.name).read(), original)

    def test_growing_file_is_bounded_and_records_instability(self):
        def fault(phase, name):
            if phase == 'read' and name == self.name:
                with self.victim.open('ab') as output:
                    output.write(b'new bytes')
        self.run_preserve('failure', fault)
        document = self.assert_archive('partial')
        self.assertNotIn(self.name, [row['path'] for row in document['files']])
        self.assertTrue(any('during read' in row['detail'] for row in document['preservation_issues']))

    def test_missing_package_identity_fails_success_and_is_explicit_for_failure(self):
        (self.package / evidence.PACKAGE_INPUTS[0]).unlink()
        with self.assertRaises(evidence.IncompleteEvidence):
            self.run_preserve()
        document = self.assert_archive('partial')
        self.assertTrue(any(row['scope'] == 'package_inputs' and row['path'] == evidence.PACKAGE_INPUTS[0]
                            for row in document['preservation_issues']))

    def test_unsuccessful_run_is_always_labeled_partial_even_when_files_are_stable(self):
        self.run_preserve('cancelled')
        document = self.assert_archive('partial')
        self.assertEqual(document['preservation_issues'], [])
        self.assertEqual(document['partial_reasons'], ['qualification did not succeed'])

    def test_unknown_outcome_cannot_claim_complete(self):
        with self.assertRaises(evidence.IncompleteEvidence):
            self.run_preserve('unknown')
        self.assert_archive('partial')

    def test_unrelated_io_and_permission_errors_propagate(self):
        for error in (PermissionError('denied'), OSError(5, 'I/O failure')):
            with self.subTest(error=error):
                with mock.patch.object(evidence.os, 'stat', side_effect=error):
                    with self.assertRaises(type(error)):
                        self.run_preserve('cancelled')
                self.assertFalse((self.base / evidence.ARCHIVE).exists())

    def test_unrelated_snapshot_storage_errors_propagate(self):
        with mock.patch.object(evidence.tempfile, 'TemporaryFile',
                               side_effect=FileNotFoundError('snapshot storage missing')):
            with self.assertRaises(FileNotFoundError):
                self.run_preserve('cancelled')
        self.assertFalse((self.base / evidence.ARCHIVE).exists())

    def test_excluded_cache_changes_do_not_invalidate_selected_evidence(self):
        path = self.root / 'target/debug/deps/oxid-metadata'
        self.write(path, b'not ELF')

        def fault(phase, name):
            if phase == 'verify':
                path.write_bytes(b'changed regenerable metadata')
        self.run_preserve('success', fault)
        document = self.assert_archive('complete')
        self.assertNotIn(path.relative_to(self.root).as_posix(),
                         [row['path'] for row in document['files']])

    def test_cache_candidate_selection_race_is_recorded(self):
        path = self.root / 'target/debug/deps/oxid-required'
        self.write(path, b'\x7fELFrequired test binary')
        real_open = evidence.source_open
        fired = []

        def racing_open(name, flags, parent=None):
            if name == path.name and not fired:
                fired.append(True)
                path.unlink()
            return real_open(name, flags, parent)

        with mock.patch.object(evidence, 'source_open', side_effect=racing_open):
            self.run_preserve('cancelled')
        document = self.assert_archive('partial')
        self.assertTrue(any(row['path'] == path.relative_to(self.root).as_posix()
                            for row in document['preservation_issues']))

    def test_archive_failure_is_not_swallowed_and_compact_survives_independently(self):
        for error in (FileNotFoundError('archive storage missing'), OSError(28, 'disk full')):
            with self.subTest(error=error):
                with mock.patch.object(evidence.tarfile.TarFile, 'addfile', side_effect=error):
                    with self.assertRaises(type(error)):
                        self.run_preserve('failure')
                self.assertFalse((self.base / evidence.ARCHIVE).exists())
        destination = self.base / 'compact.tar.gz'
        with contextlib.redirect_stdout(io.StringIO()):
            compact.compact(self.root, destination, 'failure')
        with tarfile.open(destination) as archive:
            self.assertEqual(archive.extractfile('source-plan.json').read(), (self.root / 'source-plan.json').read_bytes())
            summary = json.load(archive.extractfile('compact-index.json'))
            self.assertFalse(summary['binary_byte_replay'])
            index_row = next(row for row in summary['files'] if row['path'] == evidence.INDEX)
            self.assertFalse(index_row['included'])

    def test_repeated_preservation_cannot_overwrite_existing_artifacts(self):
        self.run_preserve()
        before = (self.base / evidence.ARCHIVE).read_bytes()
        with self.assertRaises(FileExistsError):
            self.run_preserve()
        self.assertEqual((self.base / evidence.ARCHIVE).read_bytes(), before)

    def test_workflow_keeps_compact_independent_and_before_full_preservation(self):
        workflow = (Path(__file__).resolve().parents[1] / '.github/workflows/ci.yml').read_text()
        start = workflow.index('      - name: Prepare bounded Unit3 diagnostic receipts')
        archive = workflow.index('      - name: Index and preserve Unit3 results')
        upload = workflow.index('      - name: Retain compact Unit3 diagnostic receipts')
        self.assertLess(start, upload)
        self.assertLess(upload, archive)
        for at in (start, upload, archive):
            block = workflow[at:workflow.index('\n      - name:', at + 1)]
            self.assertIn('if: always()', block)
        self.assertIn('run: python3 -B scripts/preserve_unit3_ci_evidence.py', workflow)

    def test_cli_uses_runner_temp_and_actual_qualification_outcome(self):
        result = subprocess.run([sys.executable, '-B', str(Path(evidence.__file__).resolve())],
                                cwd=Path(__file__).resolve().parents[1],
                                env={**os.environ, 'RUNNER_TEMP': str(self.base),
                                     'UNIT3_STEP_OUTCOME': 'failure'},
                                capture_output=True, text=True, timeout=10)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(json.loads(result.stdout)['archive_status'], 'partial')
        self.assert_archive('partial')


if __name__ == '__main__':
    unittest.main()
