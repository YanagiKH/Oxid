#!/usr/bin/env python3
"""Fail-closed Unit2 accounting successor controls (no compiler modification)."""
import importlib.util
import json
from pathlib import Path
import unittest
import tempfile
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[3]
PACKAGE = ROOT / 'tests/fixtures/typed_project_source_binding'

def load(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module

binding = load('unit2_binding', PACKAGE / 'run.py')
helper_path = PACKAGE / 'unit2_u8_resource.py'
helper = load('unit2_successor', helper_path)

class CurrentResource(unittest.TestCase):
    def setUp(self):
        self.frozen = (ROOT / binding.U2 / binding.INDEX_RESOURCE).read_bytes()
        self.old = binding.adapt_enum_index_resource(self.frozen)
        self.authority = helper_path.with_name('unit2-u8-resource-authority.json').read_bytes()
        # The immutable accounting adapter authenticates its original u8 source.
        # Current execution is admitted separately through identical dependencies.
        self.source = (PACKAGE / 'u8-source.json').read_bytes()
        self.current_source = (PACKAGE / 'current-source.json').read_bytes()
    def adapt(self, old=None, authority=None, source=None):
        return helper.adapt(self.old if old is None else old, self.authority if authority is None else authority,
                            self.source if source is None else source, binding)
    def test_retained_accounting_source_is_distinct_from_current_execution(self):
        current = json.loads(self.current_source)
        retained = json.loads(self.source)
        authority = json.loads(self.authority)
        self.assertEqual(binding.digest(self.source), helper.SOURCE_SHA)
        self.assertEqual(binding.digest(self.source), current['u8_source_sha256'])
        self.assertEqual(retained['reviewed_source_head'], authority['reviewed_source_head'])
        self.assertNotEqual(current['reviewed_source_head'], retained['reviewed_source_head'])
        self.assertNotEqual(binding.digest(self.current_source), helper.SOURCE_SHA)
        before = {row['path']: row for row in retained['files']}
        cross_host = json.loads((PACKAGE / 'u8-cross-host-source.json').read_bytes())
        after = {row['path']: row for row in cross_host['files']}
        byte_current = {row['path']: row for row in current['files']}
        self.assertEqual(set(before), set(after))
        self.assertEqual({name for name in before if before[name] != after[name]},
                         {'src/frontend/project.rs', 'src/frontend/declaration_index/u8_integration_tests.rs'})
        self.assertTrue(authority['source_dependencies'])
        for row in authority['source_dependencies']:
            self.assertEqual(before[row['path']], row)
            self.assertEqual(after[row['path']], row)
            self.assertEqual(byte_current[row['path']], row)
        with self.assertRaisesRegex(binding.BindingError, 'wrong Unit2 u8 current source'):
            self.adapt(source=self.current_source)

    def test_exact_derived_and_inverse(self):
        result, authority = self.adapt()
        self.assertEqual(binding.digest(result), authority['derived']['sha256'])
        self.assertEqual(authority['logical_resource_tests'], 21)
        for old, new in reversed(helper.SEAMS):
            result = result.replace(new, old, 1)
        self.assertEqual(result, self.old)
        self.assertEqual(binding.digest(self.frozen), binding.ENUM_INDEX_RESOURCE_ORIGINAL_SHA)
    def test_exact_two_named_function_scope(self):
        result, authority = self.adapt()
        self.assertEqual(len(helper.SEAMS), 2)
        for (old, new), name in zip(helper.SEAMS, authority['changed_controls']):
            self.assertTrue(old.startswith(('fn '+name+'()').encode()))
            self.assertTrue(new.startswith(('fn '+name+'()').encode()))
        self.assertEqual(result.count(b'#[test]'), self.old.count(b'#[test]'))
        self.assertIn(b'(retained,scratch,202,Some(', result)
        self.assertIn(b'(retained,scratch,207,None)', result)
        self.assertIn(b'(retained,scratch,206,Some(', result)
        self.assertIn(b'for budget in 0..=68u64', result)
        self.assertIn(b'if budget<43', result)
        self.assertIn(b'suffix[..paid]', result)
    def test_predecessor_tail_truncation_and_reapplication_rejected(self):
        result, _ = self.adapt()
        for bad in [self.old+b'\n', self.old[:-1], result, self.frozen]:
            with self.subTest(sha=binding.digest(bad)), self.assertRaises(Exception): self.adapt(old=bad)
    def test_coherent_descriptor_or_source_mutation_rejected(self):
        for key in ['derived', 'derivation', 'source_dependencies', 'preserved_test_names']:
            data = json.loads(self.authority); data[key] = []
            with self.subTest(key=key), self.assertRaises(Exception): self.adapt(authority=json.dumps(data).encode())
        data=json.loads(self.source);data['reviewed_source_head']='0'*40
        with self.assertRaises(Exception): self.adapt(source=json.dumps(data).encode())
    def test_current_materialization_consumes_separately_named_successor(self):
        captured = binding.preflight(ROOT)
        result, authority = self.adapt()
        self.assertEqual(captured['index_resource'], self.old)
        self.assertEqual(captured['u8_index_resource'], result)
        self.assertEqual(captured['u8_index_resource_authority'], authority)
        transfer = captured['u8_accounting_source_binding']
        self.assertEqual(transfer['current_source'], binding.entry('current-source.json', self.current_source))
        self.assertEqual(transfer['retained_accounting_source'], binding.entry('u8-source.json', self.source))
        self.assertEqual(transfer['reviewed_source_head'], json.loads(self.current_source)['reviewed_source_head'])
        self.assertEqual(transfer['retained_reviewed_source_head'], json.loads(self.source)['reviewed_source_head'])
        self.assertEqual(transfer['source_dependencies'], authority['source_dependencies'])
        with tempfile.TemporaryDirectory() as root:
            seam = binding.prepare_unit2(Path(root), captured)
            self.assertEqual((Path(seam['resource_package_root']) / binding.INDEX_RESOURCE).read_bytes(), result)
            self.assertEqual(seam['index_resource_adapter'], captured['index_resource_authority'])
            self.assertEqual(seam['u8_index_resource_adapter'], authority)
            self.assertEqual(seam['u8_accounting_source_binding'], transfer)

    def test_missing_extra_or_changed_seam_rejected(self):
        for seams in [helper.SEAMS[:1], helper.SEAMS+helper.SEAMS[:1],
                      ((helper.SEAMS[0][0],helper.SEAMS[0][1]+b'\n'),helper.SEAMS[1])]:
            with patch.object(helper,'SEAMS',seams), self.assertRaises(Exception): self.adapt()

if __name__ == '__main__': unittest.main()
