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
import subprocess
import tempfile
import types
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


@unittest.skipIf(sys.flags.optimize, "production facade rejects optimized Python")
class Unit2AccountingTransport(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.captured = current.preflight(ROOT, PACKAGE)
        _, _, cls.api = current.capture_retained(PACKAGE)

    def runtime(self, **overrides):
        runtime = types.SimpleNamespace(**vars(self.api))
        for name, value in overrides.items():
            setattr(runtime, name, value)
        return current.install(runtime, PACKAGE)

    def test_exact_separate_schemas_and_independent_nested_copies(self):
        old = self.captured[current.RETAINED_ACCOUNTING_KEY]
        new = self.captured[current.CURRENT_ACCOUNTING_KEY]
        self.assertEqual(len(current.RETAINED_ACCOUNTING_FIELDS), 11)
        self.assertEqual(len(current.CURRENT_ACCOUNTING_FIELDS), 12)
        self.assertEqual(set(old), current.RETAINED_ACCOUNTING_FIELDS)
        self.assertEqual(set(new), current.CURRENT_ACCOUNTING_FIELDS)
        self.assertEqual(new['predecessor_accounting_source_binding'], old)
        self.assertEqual(old['current_source']['sha256'], s.PREDECESSOR_SHA)
        self.assertEqual(new['current_source']['sha256'], SEAL['source_manifest']['sha256'])
        self.assertEqual(new['version'], 'unit2-lexer-reservation-identical-accounting-source-v1')
        expected = current.validate_unit2_accounting(self.captured, PACKAGE)
        expected['predecessor_accounting_source_binding']['source_dependencies'][0]['sha256'] = '0' * 64
        self.assertNotEqual(expected['predecessor_accounting_source_binding'], old)
        self.assertEqual(expected['source_dependencies'], old['source_dependencies'])
        self.assertEqual(current.validate_unit2_accounting(self.captured, PACKAGE), new)

    def test_changed_missing_extra_duplicate_reordered_dependencies_reject(self):
        for key in ('u8_index_resource_authority', current.RETAINED_ACCOUNTING_KEY,
                    current.CURRENT_ACCOUNTING_KEY):
            for mutation in ('changed', 'missing', 'extra', 'duplicate', 'reordered'):
                captured = copy.deepcopy(self.captured)
                rows = captured[key]['source_dependencies']
                if mutation == 'changed': rows[0]['sha256'] = '0' * 64
                elif mutation == 'missing': rows.pop()
                elif mutation == 'extra': rows.append({**rows[0], 'path': 'src/unapproved.rs'})
                elif mutation == 'duplicate': rows.append(copy.deepcopy(rows[0]))
                else: rows.reverse()
                with self.subTest(key=key, mutation=mutation), self.assertRaises(a.Reject):
                    current.validate_unit2_accounting(captured, PACKAGE)

    def test_each_restored_and_current_accounting_body_mismatch_rejects(self):
        for key in ('byte_storage_inputs', 'inputs'):
            for name in s.ACCOUNTING_PATHS:
                captured = copy.deepcopy(self.captured)
                captured[key][name] += b'\n'
                with self.subTest(key=key, name=name), self.assertRaises(a.Reject):
                    current.validate_unit2_accounting(captured, PACKAGE)

    def test_wrong_current_retained_manifests_and_member_copies_reject(self):
        for name in ('current-source.json', s.CURRENT_NAME, 'u8-source.json',
                     'unit2-u8-resource-authority.json', 'unit2_u8_resource.py'):
            captured = copy.deepcopy(self.captured)
            captured['package_bytes'][name] = self.api.encoded(captured['byte_storage_source'])
            with self.subTest(name=name), self.assertRaises(a.Reject):
                current.validate_unit2_accounting(captured, PACKAGE)
        for key in ('current', 'lexer_reservation_source', 'byte_storage_source'):
            for field in ('reviewed_source_head', 'source_only_tree'):
                captured = copy.deepcopy(self.captured)
                captured[key][field] = '0' * 40
                with self.subTest(key=key, field=field), self.assertRaises(a.Reject):
                    current.validate_unit2_accounting(captured, PACKAGE)

    def test_forged_associations_reject_before_immutable_preparation(self):
        prepare = mock.Mock(side_effect=AssertionError('materialization ran'))
        runtime = self.runtime(prepare_unit2=prepare)
        for key in (current.RETAINED_ACCOUNTING_KEY, current.CURRENT_ACCOUNTING_KEY):
            for mutation in ('missing', 'extra', 'changed'):
                captured = copy.deepcopy(self.captured)
                if mutation == 'missing': del captured[key]['version']
                elif mutation == 'extra': captured[key]['unapproved'] = True
                else: captured[key]['current_source']['sha256'] = '0' * 64
                with self.subTest(key=key, mutation=mutation), self.assertRaises(a.Reject):
                    runtime.prepare_unit2(Path('must-not-be-created'), captured)
        prepare.assert_not_called()

    def test_preparation_checks_old_seam_and_copies_both_receipts(self):
        old = self.captured[current.RETAINED_ACCOUNTING_KEY]
        prepare = mock.Mock(return_value={current.RETAINED_ACCOUNTING_KEY: old})
        runtime = self.runtime(prepare_unit2=prepare)
        seam = runtime.prepare_unit2(Path('unused-mock-output'), self.captured)
        self.assertEqual(seam[current.RETAINED_ACCOUNTING_KEY], old)
        self.assertEqual(seam[current.CURRENT_ACCOUNTING_KEY], self.captured[current.CURRENT_ACCOUNTING_KEY])
        seam[current.RETAINED_ACCOUNTING_KEY]['source_dependencies'][0]['sha256'] = '0' * 64
        seam[current.CURRENT_ACCOUNTING_KEY]['predecessor_accounting_source_binding']['version'] = 'changed'
        current.validate_unit2_accounting(self.captured, PACKAGE)
        prepare.return_value = {current.RETAINED_ACCOUNTING_KEY: {'version': 'wrong'}}
        with self.assertRaisesRegex(a.Reject, 'changed prepared historical'):
            runtime.prepare_unit2(Path('unused-mock-output'), self.captured)

    def test_plan_terminal_propagation_and_failed_read_clears_previous_capture(self):
        writer = mock.Mock()
        runtime = self.runtime(write_json=writer)
        with mock.patch.object(current, 'preflight', return_value=self.captured):
            runtime.preflight(ROOT)
        for status in ('planned', 'prepared-current-unit2', 'passed-current-unit2'):
            value = {'schema': 'oxid-current-archive-binding-v1', 'status': status,
                     current.RETAINED_ACCOUNTING_KEY: copy.deepcopy(self.captured[current.RETAINED_ACCOUNTING_KEY])}
            runtime.write_json(Path('unused-mock-output'), value)
            emitted = writer.call_args.args[1]
            self.assertNotIn(current.CURRENT_ACCOUNTING_KEY, value)
            self.assertEqual(emitted[current.CURRENT_ACCOUNTING_KEY], self.captured[current.CURRENT_ACCOUNTING_KEY])
            self.assertEqual(emitted[current.RETAINED_ACCOUNTING_KEY], self.captured[current.RETAINED_ACCOUNTING_KEY])
            emitted[current.CURRENT_ACCOUNTING_KEY]['predecessor_accounting_source_binding']['version'] = 'changed'
            current.validate_unit2_accounting(self.captured, PACKAGE)
        with mock.patch.object(current, 'preflight', side_effect=a.Reject('later admission failed')):
            with self.assertRaisesRegex(a.Reject, 'later admission failed'):
                runtime.preflight(ROOT)
        failure = {'schema': 'oxid-current-archive-binding-v1', 'status': 'failed'}
        runtime.write_json(Path('unused-mock-output'), failure)
        self.assertEqual(writer.call_args.args[1], failure)
        self.assertNotIn(current.CURRENT_ACCOUNTING_KEY, writer.call_args.args[1])
        self.assertNotIn('lexer_reservation_source_sha256', writer.call_args.args[1])

    def test_forged_seam_rejects_before_immutable_verification(self):
        verify = mock.Mock(side_effect=AssertionError('immutable verifier ran'))
        runtime = self.runtime(verify_unit2_result=verify)
        seam = {key: copy.deepcopy(self.captured[key])
                for key in (current.RETAINED_ACCOUNTING_KEY, current.CURRENT_ACCOUNTING_KEY)}
        seam[current.CURRENT_ACCOUNTING_KEY]['current_source']['sha256'] = s.PREDECESSOR_SHA
        with self.assertRaisesRegex(a.Reject, 'changed materialized accounting'):
            runtime.verify_unit2_result(Path('unused'), self.captured, seam, True)
        verify.assert_not_called()

    def test_prepare_only_materializes_and_verifies_active_compiler_bytes(self):
        with tempfile.TemporaryDirectory(prefix='oxid-unit2-accounting-prepare-') as directory:
            root = Path(directory)
            output = root / 'output'
            absent_compiler = root / 'compiler-must-not-run'
            argv = [sys.executable, '-B', str(PACKAGE / 'run.py'), 'run-unit2',
                    '--repo', str(ROOT), '--output', str(output), '--prepare-only',
                    '--cargo', str(absent_compiler), '--rustc', str(absent_compiler)]
            process = subprocess.run(argv, capture_output=True, text=True, timeout=120)
            self.assertEqual(process.returncode, 0, process.stdout + process.stderr)
            self.assertFalse(absent_compiler.exists())
            expected = self.captured[current.CURRENT_ACCOUNTING_KEY]
            identities = {}
            for name in ('plan.json', 'prepared.json', 'resource-seam.json'):
                raw = (output / name).read_bytes()
                value = json.loads(raw)
                self.assertEqual(value[current.CURRENT_ACCOUNTING_KEY], expected)
                self.assertEqual(value[current.RETAINED_ACCOUNTING_KEY], self.captured[current.RETAINED_ACCOUNTING_KEY])
                identities[name] = a.binding(name, raw)
            self.assertFalse((output / 'result.json').exists())
            for name in ('unit2/prepared.json', 'unit2/run/prepared.json'):
                raw = (output / name).read_bytes()
                value = json.loads(raw)
                self.assertEqual(value['compiler_executions'], 0)
                self.assertEqual(value['source_inputs_sha256'], expected['current_source']['sha256'])
                identities[name] = a.binding(name, raw)
            selected = (output / 'unit2/derived-package/source-inputs.json').read_bytes()
            self.assertEqual(selected, self.captured['package_bytes']['current-source.json'])
            lexer = 'src/frontend/lexer.rs'
            source = output / 'unit2/run/source'
            self.assertEqual((source / lexer).read_bytes(), self.captured['inputs'][lexer])
            self.assertNotEqual((source / lexer).read_bytes(), self.captured['byte_storage_inputs'][lexer])
            for row in expected['source_dependencies']:
                self.assertEqual(a.binding(row['path'], (source / row['path']).read_bytes()), row)
            seam = json.loads((output / 'resource-seam.json').read_bytes())
            runtime = self.runtime()
            result = runtime.verify_unit2_result(output, self.captured, seam, True)
            self.assertEqual(result[current.CURRENT_ACCOUNTING_KEY], expected)
            for name in (lexer, *s.ACCOUNTING_PATHS):
                path = source / name
                raw = path.read_bytes()
                path.write_bytes(raw + b'\n')
                try:
                    with self.subTest(name=name), self.assertRaises(a.Reject):
                        runtime.verify_unit2_result(output, self.captured, seam, True)
                finally:
                    path.write_bytes(raw)
            print('unit2 preparation-only identities:', json.dumps({
                'compiler_executions': 0, 'current_source': expected['current_source'],
                'assembled_lexer': a.binding(lexer, (source / lexer).read_bytes()),
                'predecessor_lexer': a.binding(lexer, self.captured['byte_storage_inputs'][lexer]),
                'accounting_dependencies': expected['source_dependencies'],
                'artifacts': identities}, sort_keys=True))


if __name__ == '__main__':
    unittest.main()
