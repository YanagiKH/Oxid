"""Current CLI local-reference expectations preserve frozen historical facts."""
import copy
import hashlib
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

import verify_owned_source as h


class LocalReferenceCliAmendment(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.temporary = tempfile.TemporaryDirectory()
        cls.addClassCleanup(cls.temporary.cleanup)
        cls.root = Path(cls.temporary.name)
        cls.data = json.loads(h.LOCAL_REFERENCE_CLI_AMENDMENT_PATH.read_bytes())
        wanted = {row['id'] for row in cls.data['cases']}
        cls.items = {}
        for case in h.model.corpus():
            if case.identifier not in wanted:
                continue
            item = h.model.case_expectation(case)
            source = item.pop('source')
            (cls.root / (case.identifier+'.ox')).write_bytes(source.encode())
            (cls.root / (case.identifier+'.json')).write_text(json.dumps(item, indent=2, sort_keys=True)+'\n')
            cls.items[case.identifier] = item
        if set(cls.items) != wanted:
            raise AssertionError('amendment cases differ from the historical model')

    def test_successor_preserves_all_seven_predecessor_rows_and_their_scope(self):
        parent = json.loads(h.PROJECTED_CLI_AMENDMENT_PATH.read_bytes())
        self.assertEqual(self.data['cases'][:-1], parent['cases'])
        self.assertEqual(len(self.data['cases']), 8)
        self.assertEqual(self.data['predecessor_amendment_sha256'], h.digest(h.PROJECTED_CLI_AMENDMENT_PATH))
        self.assertEqual(self.data['scope'], 'current-production-cli-only')
        self.assertIs(self.data['full_qualification'], False)

    def test_named_source_only_case_has_one_main_and_literal_zero_result(self):
        old = copy.deepcopy(self.items['excluded-reference-local'])
        before = copy.deepcopy(old)
        self.assertNotIn('function_count', old)
        self.assertEqual(old['source_sha256'], '24b02b9ca6edbb3b46d335a7dc0163cf6067ca4fa4864e9b29d7a8f2468bd6b6')
        loaded = h.load_cli_amendment(h.LOCAL_REFERENCE_CLI_AMENDMENT_ID, self.root, mode='cli')
        view = h.apply_cli_amendment(old, loaded)
        self.assertEqual(view['function_count'], 1)
        self.assertEqual(view['expected'], {'status':'accept', 'result':0, 'result_type':'i32', 'runtime_failure':None})
        self.assertEqual(old, before)
        self.assertEqual(view['origins'], old['origins'])
        self.assertEqual(view['note'], old['note'])

    def test_historical_selection_and_implicit_selection_keep_the_original_rejection(self):
        old = self.items['excluded-reference-local']
        parent = h.load_cli_amendment(h.PROJECTED_CLI_AMENDMENT_ID, self.root, mode='cli')
        self.assertEqual(h.apply_cli_amendment(old, parent), old)
        self.assertIs(h.apply_cli_amendment(old, None), old)
        for mode in ('production', 'model'):
            with self.assertRaisesRegex(ValueError, 'explicit --mode cli'):
                h.load_cli_amendment(h.LOCAL_REFERENCE_CLI_AMENDMENT_ID, self.root, mode=mode)

    def test_function_count_conversion_rejects_false_or_inapplicable_metadata(self):
        old = self.items['excluded-reference-local']
        for value in (True, False, 0, -1, 1.0, '1', None):
            data = copy.deepcopy(self.data)
            data['cases'][-1]['effective_function_count'] = value
            with self.subTest(value=value), self.assertRaisesRegex(ValueError, 'function count override'):
                h.apply_cli_amendment(old, data)
        data = copy.deepcopy(self.data)
        data['cases'][-1]['effective_expected'] = old['expected']
        with self.assertRaisesRegex(ValueError, 'function count override'):
            h.apply_cli_amendment(old, data)

    def test_tampered_successor_and_frozen_source_are_rejected(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / 'changed.json'
            path.write_bytes(h.LOCAL_REFERENCE_CLI_AMENDMENT_PATH.read_bytes()+b'\n')
            with patch.object(h, 'LOCAL_REFERENCE_CLI_AMENDMENT_PATH', path):
                with self.assertRaisesRegex(ValueError, 'sealed version'):
                    h.load_cli_amendment(h.LOCAL_REFERENCE_CLI_AMENDMENT_ID, self.root, mode='cli')
        source = self.root / 'excluded-reference-local.ox'
        original = source.read_bytes()
        try:
            source.write_bytes(original+b'\n')
            with self.assertRaisesRegex(ValueError, 'source identity mismatch'):
                h.load_cli_amendment(h.LOCAL_REFERENCE_CLI_AMENDMENT_ID, self.root, mode='cli')
        finally:
            source.write_bytes(original)


if __name__ == '__main__':
    unittest.main(verbosity=2)
