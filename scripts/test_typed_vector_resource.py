"""Independent model and tamper controls; never invokes a Rust compiler."""
import ast
import hashlib
import importlib.util
import inspect
import json
from pathlib import Path
import re
import tempfile
import types
import unittest
from unittest.mock import patch

import typed_vector_resource_adapter as adapter
import verify_typed_vector_compatibility as gate

ROOT = Path(__file__).resolve().parents[1]
BINDING = ROOT / 'tests/fixtures/typed_project_source_binding_byte_storage_v1'
PACKAGE = ROOT / 'tests/fixtures/typed_project_unit2_independent'


def authenticated_module(path, digest):
    body = path.read_bytes()
    if hashlib.sha256(body).hexdigest() != digest:
        raise ValueError('changed predecessor helper')
    module = types.ModuleType('_vector_resource_predecessor')
    module.__file__ = str(path)
    exec(compile(body, str(path), 'exec'), module.__dict__)
    return module


def inherited_resource():
    retained = authenticated_module(BINDING / 'run.py',
        '584e8ee937cb2d3ef62d59d3eb69a79bfb6afce23df86de3df305f81afae7c76')
    original = (PACKAGE / retained.INDEX_RESOURCE).read_bytes()
    enum = retained.adapt_enum_index_resource(original)
    u8 = authenticated_module(BINDING / 'unit2_u8_resource.py',
        'eb7c611e986a46d8468edc57a51b7ba708dbac81eb7d0a05bde190f2a530394b')
    current, _ = u8.adapt(enum, (BINDING / 'unit2-u8-resource-authority.json').read_bytes(),
        (BINDING / 'u8-source.json').read_bytes(), types.SimpleNamespace(
            require=retained.require, digest=retained.digest, entry=retained.entry))
    return current


def struct_fields(body, name):
    match = re.search(r'struct ' + name + r'(?:<[^\n{]+>)?\s*\{([^}]+)\}', body, re.S)
    if match is None:
        raise ValueError('missing typed model ' + name)
    fields = []
    for line in match[1].splitlines():
        line = line.split('//', 1)[0].strip()
        if line:
            key, value = line.removesuffix(',').split(':', 1)
            fields.append((key.strip().removeprefix('pub '), value.strip()))
    return fields


class TypedVectorResourceTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.original = inherited_resource()

    def test_predecessor_chain_and_every_fixture_byte_are_restored(self):
        self.assertEqual((len(self.original), gate.sha(self.original)),
                         (25291, '1b717bef7b1d6ffebcab1fe245f60ef95ba2b4819bccddad90de078fcee05831'))
        current = adapter.adapt(self.original)
        self.assertEqual(adapter.reverse(current), self.original)
        self.assertTrue(current.endswith(adapter.MODEL))
        without_model = current[:-len(adapter.MODEL)]
        restored = without_model
        self.assertEqual(len(adapter.SEAMS), 2)
        for before, after in reversed(adapter.SEAMS):
            self.assertEqual(after.replace(b'+current_vector_review_fixed_growth() as u64', b''), before)
            self.assertEqual(restored.count(after), 1)
            restored = restored.replace(after, before, 1)
        self.assertEqual(restored, self.original)
        names = re.findall(rb'#\[test\]\s*fn ([A-Za-z_0-9]+)\(', self.original)
        self.assertEqual(re.findall(rb'#\[test\]\s*fn ([A-Za-z_0-9]+)\(', current), names)
        self.assertNotIn(b'#[test]', adapter.MODEL)
        retained_names = json.loads((PACKAGE / 'resource-test-names.json').read_bytes())
        self.assertEqual(len(retained_names), 21)
        self.assertEqual(adapter.descriptor()['new_rust_test_names'], [])
        for literal in [b'assert!(FIXED_SCRATCH<=4096)', b'(retained,scratch,202,Some(',
                        b'(retained,scratch,207,None)', b'(retained-1,scratch,207,Some(',
                        b'(retained,scratch-1,207,Some(', b'(retained,scratch,206,Some(',
                        b'(0,0,0,Some(', b'(retained,0,0,Some(', b'assert_eq!(alloc.attempts,0)',
                        b'assert!(alloc.trace.is_empty())', b'facts.plan().build_work,173',
                        b'"preflight visit").map(|e|e.units).sum::<u64>(),34',
                        b'for fail in 1..=16', b'ast_payload,28800']:
            self.assertIn(literal, self.original)
            self.assertEqual(without_model.count(literal), self.original.count(literal))

    def test_independent_native_64_bit_member_equations(self):
        # Native 64-bit roles, independent of any production subtotal/constant.
        # Rust helper assertions later compare both complete model layouts.
        align = lambda value, alignment: (value + alignment - 1) // alignment * alignment
        prior_admission = align(9 * 4 + 4, 8)
        current_admission = align(10 * 4 + 8, 8)
        query = 2 * 8
        source_owner, vector_headers, origin, root_id = 32, 12 * (3 * 8), 16, 4
        prior_table = align(source_owner + vector_headers + origin + prior_admission + root_id, 8)
        current_table = align(source_owner + vector_headers + origin + current_admission + root_id + 1, 8)
        header = current_table - prior_table + max(current_admission, query) - max(prior_admission, query)
        syntax_feature_roles = [3, 8, 8, 8, 2, 2 * 8]
        subtotal_roles = [8, 8, 2 * (8 + 8)]
        catalog_roles = [1 + 8] * 8 + [8] * 4
        self.assertEqual((prior_admission, current_admission, query), (40, 48, 16))
        self.assertEqual((prior_table, current_table), (384, 392))
        self.assertEqual((header, sum(syntax_feature_roles), sum(subtotal_roles), sum(catalog_roles)),
                         (16, 45, 48, 104))
        current_base = 4096 + header + sum(syntax_feature_roles) + sum(subtotal_roles) + sum(catalog_roles)
        self.assertEqual(current_base, 4309)
        self.assertEqual(current_base + 4, 4313)  # empty project's unchanged variable scratch
        self.assertEqual(current_base + 8, 4317)  # one function's unchanged variable scratch
        self.assertNotIn(b'+ 213', adapter.MODEL)
        self.assertIn(b'(16, 93, 104)', adapter.MODEL)
        for category in [header, *syntax_feature_roles, *subtotal_roles, *catalog_roles]:
            self.assertNotEqual(current_base - category, current_base)

    def test_models_cover_complete_current_fields_and_keep_the_old_header(self):
        model = adapter.MODEL.decode()
        index = (ROOT / 'src/frontend/declaration_index.rs').read_text()
        catalog = (ROOT / 'src/frontend/builtin_catalog.rs').read_text()
        sealed = (ROOT / 'src/frontend/declaration_index/sealed.rs').read_text()
        current = struct_fields(model, 'VectorReviewCurrentTable')
        actual = struct_fields(index, 'Tables')
        self.assertEqual([(name, "BuiltinAdmission<'s>" if name == 'builtins' else ty)
                          for name, ty in current], actual)
        before = struct_fields(model, 'VectorReviewPriorTable')
        expected = [(name, "VectorReviewPriorAdmission<'s>" if name == 'builtins' else ty)
                    for name, ty in current if name != 'has_element_ranges']
        self.assertEqual(before, expected)
        self.assertEqual(sum(ty.startswith('Vec<') for _, ty in current), 12)
        self.assertIn(('anchors', '[u32; 9]'), struct_fields(model, 'VectorReviewPriorAdmission'))
        self.assertIn(('anchors', '[u32; 10]'), struct_fields(sealed, 'BuiltinAdmission'))
        controls = struct_fields(catalog, 'BuiltinSetControls')
        mirrored = struct_fields(model, 'VectorReviewCatalogTransports')
        self.assertEqual([name for name, _ in mirrored], [name for name, _ in controls])
        self.assertEqual([ty for _, ty in controls].count('BuiltinSet'), 8)
        self.assertEqual([ty for _, ty in controls].count('experimental::Inventory'), 4)
        self.assertEqual([ty for _, ty in mirrored].count('[u8; 1 + 8]'), 8)
        self.assertEqual([ty for _, ty in mirrored].count('[u8; 8]'), 4)
        self.assertIn('size_of::<[Option<usize>; 2]>()', model)
        self.assertIn('std::cmp::max(new_admission, query) - std::cmp::max(old_admission, query)', model)
        self.assertNotIn('PreVectorTables', model)
        self.assertNotIn('PreVectorBuiltinAdmission', model)

    def test_any_mutation_double_application_or_weakened_model_is_rejected(self):
        current = adapter.adapt(self.original)
        for changed in [self.original + b'\n', self.original[:-1], current,
                        self.original.replace(b'assert!(FIXED_SCRATCH<=4096)', b'assert!(FIXED_SCRATCH<=8192)')]:
            with self.assertRaises(ValueError):
                adapter.adapt(changed)
        for changed in [current + b'\n', current[:-1], self.original]:
            with self.assertRaises(ValueError):
                adapter.reverse(changed)
        for changed in [adapter.SEAMS[:-1], adapter.SEAMS + adapter.SEAMS[:1]]:
            with patch.object(adapter, 'SEAMS', changed), self.assertRaises(ValueError):
                adapter.adapt(self.original)
        for old, new in [(b'anchors: [u32; 10]', b'anchors: [u32; 9]'),
                         (b'+ size_of::<[Option<usize>; 2]>()', b'+ 0'),
                         (b'inventory_caller_result: [u8; 8]', b'inventory_caller_result: [u8; 0]'),
                         (b'assert_eq!(FIXED_SCRATCH, 4096);', b'assert!(FIXED_SCRATCH <= 8192);')]:
            with patch.object(adapter, 'MODEL', adapter.MODEL.replace(old, new)), self.assertRaises(ValueError):
                adapter.adapt(self.original)
        seams = tuple((before, after.replace(b'current_vector_review_fixed_growth() as u64', b'0'))
                      for before, after in adapter.SEAMS)
        with patch.object(adapter, 'SEAMS', seams), self.assertRaises(ValueError):
            adapter.adapt(self.original)

    def test_controller_authenticates_helper_before_execution(self):
        actual = (ROOT / gate.INDEX_RESOURCE_ADAPTER).read_bytes()
        self.assertEqual((len(actual), gate.sha(actual)),
                         (gate.INDEX_RESOURCE_ADAPTER_BYTES, gate.INDEX_RESOURCE_ADAPTER_SHA))
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            helper = root / gate.INDEX_RESOURCE_ADAPTER
            helper.parent.mkdir(parents=True)
            for body in [actual + b'\n', b'raise RuntimeError("untrusted helper executed")\n',
                         actual.replace(b'legacy_fixed_bank": 4096', b'legacy_fixed_bank": 8192')]:
                helper.write_bytes(body)
                with self.assertRaisesRegex(ValueError, 'changed pinned'):
                    gate.authenticate_index_resource_adapter(root)
            helper.unlink()
            target = root / 'target.py'
            target.write_bytes(actual)
            helper.symlink_to(target)
            with self.assertRaisesRegex(ValueError, 'symlink'):
                gate.authenticate_index_resource_adapter(root)

    def test_current_capture_changes_only_the_named_resource_before_parent_assembly(self):
        parser_original = (PACKAGE / 'archive/resource/parser-resource-review-tests.rs').read_bytes()
        old = b'mode:SourceMode::ProjectCandidate,tokens,'
        new = b'mode:SourceMode::ProjectCandidate,project_recovery:false,arrays:ArraySyntaxPolicy::Closed,enums:EnumSyntaxPolicy::Closed,std_imports:StdImportPolicy::Closed,storage:enums::SyntaxStorage::default(),tokens,'
        self.assertEqual(parser_original.count(old), 1)
        parser_original = parser_original.replace(old, new, 1)
        captured = {'resource': parser_original, 'u8_index_resource': self.original,
                    'authority': {'unchanged': 'historical source authority'}}
        import copy
        before = copy.deepcopy(captured)
        expected = adapter.adapt(self.original)
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            path = root / adapter.RESOURCE
            path.parent.mkdir(parents=True)
            path.write_bytes(expected)
            sentinel = object()
            with patch.object(gate.library, 'prepare_with_parser_resource', return_value=(sentinel, {'resource_package_root': directory})) as prepare:
                result, seam = gate.prepare_current_observer('original', 'output', captured, 'observer', adapter)
            self.assertIs(result, sentinel)
            args = prepare.call_args.args
            self.assertEqual(args[:2], ('original', 'output'))
            self.assertEqual(args[2], dict(captured, u8_index_resource=expected))
            self.assertEqual(args[3], 'observer')
            self.assertEqual(args[4], gate.adapt_parser_resource(gate.library.adapt_parser_resource(parser_original)))
            self.assertEqual(captured, before)
            self.assertEqual(seam['typed_vector_index_resource_adapter'], adapter.descriptor())
            path.write_bytes(self.original)
            with patch.object(gate.library, 'prepare_with_parser_resource', return_value=(sentinel, {'resource_package_root': directory})), self.assertRaisesRegex(ValueError, 'prepared index resource'):
                gate.prepare_current_observer('original', 'output', captured, 'observer', adapter)
        source = inspect.getsource(gate.run)
        self.assertIn('authenticate_index_resource_adapter(actual_repo)[1] == index_adapter_body', source)
        self.assertIn('identity(INDEX_RESOURCE_ADAPTER, index_adapter_body)', source)
        self.assertEqual(gate.PREDECESSOR_POLICY_SHA, 'f23fa37e76036e633ff56a9902bf01efd4682750f2d71f2dded88a757d06ba75')
        self.assertEqual(len(gate.PREDECESSOR_POLICIES), 48)


if __name__ == '__main__':
    unittest.main()
