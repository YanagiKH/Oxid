#!/usr/bin/env python3
"""Live builds and negative controls for the frozen scalar observer successor."""
import copy
import hashlib
import json
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch

import build_typed_parser_observer as parser
import build_typed_static_observer as static
import legacy_scalar_observer_u8 as adapter
import verify_bounded_typed_parser as gate

ROOT = Path(__file__).resolve().parents[1]


class AdapterTests(unittest.TestCase):
    def test_exact_pinned_inputs_outputs_and_reverse(self):
        for key, expected in adapter.DERIVED_PINS.items():
            kind, name = key.split('/')
            original = (ROOT / f'tests/fixtures/bounded_typed_{kind}/observer' / name).read_bytes()
            derived = adapter.adapt_wrapper(kind, name, original)
            self.assertEqual(hashlib.sha256(derived).hexdigest(), expected)
            with self.assertRaises(ValueError):
                adapter.adapt_wrapper(kind, name, original + b'\n')
        with self.assertRaises(ValueError):
            adapter.adapt_wrapper('other', 'main.rs', b'')

    def test_coherent_edit_mutation_is_not_authorized_by_reversibility(self):
        original = (ROOT / 'tests/fixtures/bounded_typed_parser/observer/frontend_mod.rs').read_bytes()
        edits = tuple((before, after.replace('"conversion"', '"other"')) for before, after in adapter.AST_EDITS)
        with patch.object(adapter, 'AST_EDITS', edits), self.assertRaises(ValueError):
            adapter.adapt_wrapper('parser', 'frontend_mod.rs', original)


class LiveObserverTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.temporary = tempfile.TemporaryDirectory()
        cls.root = Path(cls.temporary.name)
        cls.binaries = {}
        for kind, builder in [('parser', parser), ('static', static)]:
            result = builder.build(cls.root / kind)
            cls.binaries[kind] = Path(result['observer'])

    @classmethod
    def tearDownClass(cls):
        cls.temporary.cleanup()

    def observe(self, kind, source, *args):
        result = subprocess.run([str(self.binaries[kind]), *args], input=source.encode(), capture_output=True, timeout=10)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(result.stderr, b'')
        return json.loads(result.stdout)

    def test_conversions_are_outside_both_domains(self):
        for source in ('fn f(x:i32)->u8{return x.to_u8_checked();}',
                       'fn f(x:u8)->i32{return x.to_i32();}'):
            for kind, args in [('parser', ()), ('static', ()), ('static', ('--resolve-only',))]:
                value = self.observe(kind, source, *args)
                self.assertEqual((value['status'], value['family']), ('outside_subset', 'conversion'))
                self.assertFalse({'ast', 'typed_hir', 'resolved_hir'} & value.keys())

    def test_static_u8_type_positions_are_outside_legacy_schema(self):
        for source in ('fn f(x:u8)->(){return;}', 'fn f()->u8{return 0;}',
                       'fn f()->(){let x:u8=0;return;}',
                       'fn f()->(){if true {let x:u8=0;}return;}'):
            self.assertEqual(self.observe('parser', source)['status'], 'ok')
            for args in [(), ('--resolve-only',)]:
                value = self.observe('static', source, *args)
                self.assertEqual((value['status'], value['family']), ('outside_subset', 'u8_type'))
                self.assertFalse({'ast', 'typed_hir', 'resolved_hir'} & value.keys())

    def test_ordinary_u8_identifiers_and_unknown_type_route_preserved(self):
        for source in ('fn u8(x:i32)->i32{return x;}',
                       'fn f(u8:i32)->i32{return u8;}',
                       'fn f()->i32{let u8=1;return u8;}',
                       'fn f()->i32{/* u8 */return 1;}'):
            self.assertEqual(self.observe('parser', source)['status'], 'ok')
            self.assertEqual(self.observe('static', source)['status'], 'ok')
        value = self.observe('static', 'fn f(x:unknown)->(){return;}')
        self.assertEqual((value['status'], value['route']), ('public_route_required', 'owned'))

    def test_manifest_selection_and_coherently_rehashed_mutations_rejected(self):
        for kind, builder in [('parser', parser), ('static', static)]:
            binary = self.binaries[kind]
            path = binary.parent / 'source-manifest.json'
            original = path.read_bytes()
            baseline = json.loads(original)
            gate.observer_identity(binary, builder, kind)
            mutations = []
            for section in ['files', 'wrappers']:
                for operation in ['omit', 'duplicate']:
                    m = copy.deepcopy(baseline)
                    m[section] = m[section][1:] if operation == 'omit' else m[section] + m[section][:1]
                    mutations.append(m)
            m = copy.deepcopy(baseline)
            m['wrapper_adaptation'] = 'unapproved'
            mutations.append(m)
            try:
                for m in mutations:
                    path.write_text(json.dumps(m))
                    with self.assertRaises(ValueError):
                        gate.observer_identity(binary, builder, kind)
                m = copy.deepcopy(baseline)
                entry = next(x for x in m['wrappers'] if x['copied_path'] == 'frontend/mod.rs')
                copied = binary.parent / entry['copied_path']
                before = copied.read_bytes()
                try:
                    copied.write_bytes(before + b'\n')
                    entry['sha256'] = gate.digest(copied)
                    path.write_text(json.dumps(m))
                    with self.assertRaises(ValueError):
                        gate.observer_identity(binary, builder, kind)
                finally:
                    copied.write_bytes(before)
            finally:
                path.write_bytes(original)

    def test_missing_each_added_module_fails_actual_rustc(self):
        for kind, builder in [('parser', parser), ('static', static)]:
            for index, missing in enumerate(['parser/conversions.rs', 'declaration_index/u8_reservation.rs']):
                output = self.root / f'{kind}-omit-{index}'
                with patch.object(builder, 'SOURCE_FILES', tuple(x for x in builder.SOURCE_FILES if x != missing)):
                    with self.assertRaisesRegex(ValueError, 'rustc failed'):
                        builder.build(output)
                self.assertIn(missing.split('/')[-1].removesuffix('.rs').encode(), (output / 'build.stderr').read_bytes())
                self.assertNotEqual(json.loads((output / 'build-evidence.json').read_bytes())['exit_code'], 0)


if __name__ == '__main__':
    unittest.main()
