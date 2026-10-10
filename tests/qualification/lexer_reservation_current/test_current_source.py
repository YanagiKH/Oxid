#!/usr/bin/env python3
"""Source-only outer admission controls; compiler execution is never evidence here."""
import sys
sys.dont_write_bytecode = True
import copy
import hashlib
import importlib.util
import json
from pathlib import Path
import re
import shutil
import tempfile
import unittest
from unittest import mock

import adapters as a
import current
import source_transition as s
from seal import SEAL

ROOT = Path(__file__).resolve().parents[3]
PACKAGE = ROOT / 'tests/fixtures/typed_project_source_binding'


class CurrentSource(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.saved, cls.old, cls.api = current.capture_retained(PACKAGE)
        cls.raw = {p.name: p.read_bytes() for p in PACKAGE.iterdir() if p.is_file()}
        cls.result = s.admit(ROOT, cls.raw, SEAL, cls.api)
        cls.inputs = cls.result['inputs']
        cls.restored = cls.result['predecessor_inputs']
        cls.paths = cls.result['transition_touched']
        cls.patch = cls.raw[s.PATCH_NAME]
        cls.directory = tempfile.TemporaryDirectory(prefix='oxid-lexer-source-controls-')
        cls.repo = Path(cls.directory.name) / 'source'
        cls.api.materialize(cls.repo, cls.inputs)

    @classmethod
    def tearDownClass(cls):
        cls.directory.cleanup()

    @unittest.skipIf(sys.flags.optimize, "retained full preflight explicitly rejects optimized Python")
    def test_complete_current_predecessor_and_archived_chain(self):
        result = current.preflight(ROOT, PACKAGE)
        self.assertEqual((len(result['inputs']), len(result['byte_storage_inputs']),
                          len(result['archived'])), (376, 376, 117))
        self.assertEqual(result['byte_storage_inputs'], self.restored)
        self.assertEqual(sum(n.startswith(('src/', 'native/')) for n in self.inputs), 288)
        self.assertEqual(len(result['lexer_reservation_authority']['compile_time_fixture_inputs']), 78)
        self.assertEqual(len(result['lexer_reservation_authority']['compile_time_include_directives']), 136)
        self.assertEqual(len(self.paths), 14)
        for n in s.ACCOUNTING_PATHS:
            self.assertEqual(self.inputs[n], self.restored[n])

    def test_exact_inverse_wrong_stage_and_double_inverse(self):
        restored, touched = self.api.apply_inverse_patch(self.inputs, self.patch,
            a.sha(self.patch), len(self.patch), tuple(self.paths))
        self.assertEqual(restored, self.restored)
        self.assertEqual(touched, self.paths)
        with self.assertRaises(self.api.BindingError):
            self.api.apply_inverse_patch(restored, self.patch, a.sha(self.patch),
                                         len(self.patch), tuple(self.paths))

    def test_every_changed_hunk_context_rejects(self):
        count = 0
        name = None
        for line in self.patch.splitlines(keepends=True):
            if line.startswith(b'diff --git '):
                name = line.split()[2][2:].decode()
            if not line.startswith(b'@@ '):
                continue
            match = re.match(rb'@@ -\d+(?:,\d+)? \+(\d+)(?:,(\d+))? @@', line)
            offset = max(int(match[1]) - 1, 0)
            data = self.inputs[name].splitlines(keepends=True)
            data[offset] = b'// deliberately changed exact current hunk context\n'
            damaged = {**self.inputs, name: b''.join(data)}
            with self.subTest(path=name, offset=offset), self.assertRaises(self.api.BindingError):
                self.api.apply_inverse_patch(damaged, self.patch, a.sha(self.patch),
                                             len(self.patch), tuple(self.paths))
            count += 1
        self.assertGreater(count, 14)
        print('exact current hunk controls:', count)

    def test_patch_order_duplicate_and_section_deletion(self):
        pieces = [b'diff --git ' + p for p in self.patch.split(b'diff --git ')[1:]]
        for bad in (b''.join(reversed(pieces)), self.patch + pieces[0], b''.join(pieces[1:])):
            with self.assertRaises(self.api.BindingError):
                self.api.apply_inverse_patch(self.inputs, bad, a.sha(bad), len(bad), tuple(self.paths))

    def test_every_selected_body_and_immutable_fixture_rejects_mutation(self):
        for name in self.inputs:
            path = self.repo / name
            old = path.read_bytes()
            path.write_bytes(old + b'\n')
            try:
                with self.subTest(path=name), self.assertRaises(a.Reject):
                    s.read_inputs(self.repo, self.result['current']['files'])
            finally:
                path.write_bytes(old)

    def test_missing_extra_empty_directory_mode_and_symlink(self):
        path = self.repo / 'src/frontend/lexer.rs'
        old = path.read_bytes()
        path.unlink()
        try:
            with self.assertRaises((a.Reject, FileNotFoundError)):
                s.admit(self.repo, self.raw, SEAL, self.api)
        finally:
            path.write_bytes(old)
        for name, directory in (('src/unapproved.txt', False), ('native/__pycache__', True)):
            added = self.repo / name
            added.mkdir() if directory else added.write_bytes(b'extra')
            try:
                with self.assertRaises(a.Reject):
                    s.admit(self.repo, self.raw, SEAL, self.api)
            finally:
                added.rmdir() if directory else added.unlink()
        path.chmod(0o755)
        try:
            with self.assertRaises(a.Reject):
                s.admit(self.repo, self.raw, SEAL, self.api)
        finally:
            path.chmod(0o644)
        target = self.repo / 'native/typed_preview.c'
        path.unlink()
        try:
            path.symlink_to(target)
        except OSError:
            path.write_bytes(old)
            self.skipTest('host cannot create a symlink')
        try:
            with self.assertRaises(a.Reject):
                s.admit(self.repo, self.raw, SEAL, self.api)
        finally:
            path.unlink(); path.write_bytes(old)

    def test_manifest_authority_patch_and_helper_tampering(self):
        for name in (s.CURRENT_NAME, s.AUTHORITY_NAME, s.PATCH_NAME, s.PREDECESSOR_NAME):
            changed = {**self.raw, name: self.raw[name] + b'\n'}
            with self.subTest(member=name), self.assertRaises(a.Reject):
                s.admit(self.repo, changed, SEAL, self.api)
        for name in ('run.py', 'u8_cross_host.py', 'byte_storage.py', 'package-manifest.json'):
            changed = {**self.old, name: self.old[name] + b'\n'}
            with self.subTest(helper=name), self.assertRaises(a.Reject):
                s.retained_public_api(self.saved, changed)

    def test_coherent_include_authority_changes_still_reject(self):
        # Caller-supplied manifests are not a seal; even here, coherent authority
        # resealing cannot change the lexically derived immutable include roster.
        for operation in ('remove', 'duplicate', 'reorder', 'change'):
            authority = copy.deepcopy(self.result['authority'])
            rows = authority['compile_time_include_directives']
            if operation == 'remove': rows.pop()
            if operation == 'duplicate': rows.append(rows[0])
            if operation == 'reorder': rows.reverse()
            if operation == 'change': rows[0]['expression'] += ' '
            raw = (json.dumps(authority, indent=2, sort_keys=True) + '\n').encode()
            seal = {**SEAL, 'authority': a.binding(s.AUTHORITY_NAME, raw)}
            with self.subTest(operation=operation), self.assertRaises(a.Reject):
                s.admit(self.repo, {**self.raw, s.AUTHORITY_NAME: raw}, seal, self.api)

    def test_failure_precedes_materialization_or_compiler_invocation(self):
        path = self.repo / 'src/frontend/lexer.rs'
        old = path.read_bytes(); path.write_bytes(old + b'\n')
        try:
            with mock.patch('tempfile.TemporaryDirectory', side_effect=AssertionError('unexpected materialization')), \
                 mock.patch('subprocess.run', side_effect=AssertionError('unexpected compiler execution')):
                with self.assertRaises(a.Reject):
                    current.preflight(self.repo, PACKAGE)
        finally:
            path.write_bytes(old)

    @unittest.skipIf(sys.flags.optimize, "production facade rejects optimized Python")
    def test_active_facade_preservation_guard_uses_current_package(self):
        path = PACKAGE / 'run.py'
        spec = importlib.util.spec_from_file_location('current_facade_regression', path)
        facade = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(facade)
        self.assertEqual(facade.assert_unchanged.__defaults__, (PACKAGE,))
        admitted = facade.preflight(ROOT)
        self.assertNotEqual(admitted['inputs']['src/frontend/lexer.rs'],
                            admitted['byte_storage_inputs']['src/frontend/lexer.rs'])
        facade.assert_unchanged(ROOT, admitted)

    @unittest.skipIf(sys.flags.optimize, "production facade rejects optimized Python")
    def test_active_facade_rejects_changed_source_before_materialization(self):
        spec = importlib.util.spec_from_file_location('current_facade_negative_regression', PACKAGE / 'run.py')
        facade = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(facade)
        self.assertIs(facade.preflight, facade._runtime.preflight)
        self.assertIs(facade.preflight.__globals__, vars(facade._modules['current.py']))
        self.assertEqual(facade.preflight.__defaults__, (PACKAGE,))
        rejected = facade._modules['adapters.py'].Reject
        path = self.repo / 'src/frontend/lexer.rs'
        old = path.read_bytes()
        path.write_bytes(old + b'\n')
        try:
            with mock.patch('tempfile.TemporaryDirectory', side_effect=AssertionError('unexpected materialization')), \
                 mock.patch('subprocess.run', side_effect=AssertionError('unexpected compiler execution')):
                with self.assertRaisesRegex(rejected, 'unapproved complete body: src/frontend/lexer.rs'):
                    facade.preflight(self.repo)
        finally:
            path.write_bytes(old)

    def test_all_89_predecessor_package_members_are_authenticated(self):
        self.assertEqual(len(self.old), 89)
        self.assertEqual(a.sha(self.old['package-manifest.json']), s.RETAINED_PACKAGE_SHA)
        self.assertEqual(a.sha(self.old['run.py']), s.RETAINED_RUNNER_SHA)
        self.assertEqual(a.sha(self.old['current-source.json']), s.PREDECESSOR_SHA)

    def test_trusted_dispatcher_rejects_coherent_helper_tampering_before_loading(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            package = root / 'tests/fixtures/typed_project_source_binding'
            helpers = root / 'tests/qualification/lexer_reservation_current'
            package.mkdir(parents=True); helpers.mkdir(parents=True)
            wrapper = (PACKAGE / 'run.py').read_bytes()
            (package / 'run.py').write_bytes(wrapper)
            names = ('adapters.py', 'source_transition.py', 'seal.py', 'current.py')
            originals = {name: (Path(__file__).parent / name).read_bytes() for name in names}
            for name, body in originals.items(): (helpers / name).write_bytes(body)
            for name in names:
                (helpers / name).write_bytes(b"raise AssertionError('untrusted code executed')\n")
                try:
                    module = {'__file__': str(package / 'run.py'), '__name__': 'tampered_facade'}
                    with self.subTest(helper=name), self.assertRaisesRegex(ValueError, 'changed pinned lexer dispatcher helper'):
                        exec(compile(wrapper, str(package / 'run.py'), 'exec'), module)
                finally:
                    (helpers / name).write_bytes(originals[name])

    def test_closed_package_rejects_empty_directories_and_modes(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory); body = root / 'one.py'; body.write_bytes(b'one')
            current.closed_package(root, {'one.py'})
            extra = root / '__pycache__'; extra.mkdir()
            with self.assertRaises(a.Reject): current.closed_package(root, {'one.py'})
            extra.rmdir(); body.chmod(0o755)
            with self.assertRaises(a.Reject): current.closed_package(root, {'one.py'})


if __name__ == '__main__':
    unittest.main()
