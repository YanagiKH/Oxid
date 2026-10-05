"""Exact six-case current contract derivation; no candidate observations used."""
import base64
import copy
import hashlib
import json
from pathlib import Path
import unittest
from unittest.mock import patch

import portable as p
import record_composition_amendment as amendment


class CurrentParserDiagnosticAmendmentTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.a = p.authority()
        cls.c, cls.proof = p.comparator()
        # The complete predecessor document is reconstructed from immutable
        # packaged transport; no compiler invocation or observed JSON is read.
        transport = p.REPOSITORY / 'tests/fixtures/typed_project_unit4_contracts'
        import tempfile
        cls.temporary = tempfile.TemporaryDirectory()
        cls.root = Path(cls.temporary.name)
        import importlib.util
        spec = importlib.util.spec_from_file_location('record_parser_transport', transport / 'transport.py')
        module = importlib.util.module_from_spec(spec); spec.loader.exec_module(module)
        manifest = module.materialize(transport, cls.root / 'contracts')
        relative = manifest['active_contracts']['parser']['logical_path']
        member = next(row for row in manifest['members'] if row['logical_path'] == relative)
        cls.contract_root = (cls.root / 'contracts' / member['archive_path']).parent
        cls.contract = cls.c.load_contract(cls.contract_root)
        cls.effective, cls.receipt = cls.c.admit_contract_amendment(
            cls.contract, cls.contract_root, p.effective_authority(cls.a))
        cls.data = json.loads(Path(amendment.__file__).with_name('record-composition-diagnostics-v1.json').read_bytes())

    @classmethod
    def tearDownClass(cls):
        cls.temporary.cleanup()

    def test_exact_six_predicates_and_complete_reversal(self):
        before = copy.deepcopy(self.effective)
        current, receipt = amendment.apply(self.effective, self.receipt)
        self.assertEqual(self.effective, before)
        changed = []
        for old, new in zip(before['cases'], current['cases']):
            if old != new:
                changed.append(old['id'])
                self.assertEqual({k: v for k, v in old.items() if k != 'expected'},
                                 {k: v for k, v in new.items() if k != 'expected'})
                self.assertEqual({k: v for k, v in old['expected'].items() if k != 'diagnostics_exact'},
                                 {k: v for k, v in new['expected'].items() if k != 'diagnostics_exact'})
                self.assertNotIn('relation_to_original', old['expected'])
        self.assertEqual(set(changed), set(amendment.CASE_IDS))
        self.assertEqual(len(changed), 6)
        self.assertEqual(len(current['cases']), 248)
        self.assertEqual(sum(1 + ('relation_to_original' in x['expected']) for x in current['cases']), 319)
        self.assertEqual(receipt['predecessor_effective_contract_receipt'], self.receipt)
        self.assertEqual(receipt['ordered_amendment_sha256'], [*self.receipt['ordered_amendment_sha256'], amendment.DATA_SHA])
        self.assertEqual(self.proof, p.read(p.FROZEN / 'derivation-proof.json'))
        self.assertEqual(self.proof['semantic_predicate_handlers_unchanged'], 22)

    def test_all_diagnostic_spans_are_derived_from_frozen_utf8_source(self):
        for row in self.data['cases']:
            source = base64.b64decode(row['source']['base64'], validate=True)
            self.assertEqual(hashlib.sha256(source).hexdigest(), row['source']['sha256'])
            new = row['new_diagnostics_exact']
            self.assertEqual(new[-2:], row['old_diagnostics_exact'][-2:])
            self.assertEqual(len(new), 2 if '-qualified-' in row['id'] else 3)
            for diagnostic in new:
                word = b'self' if 'absolute item paths' in diagnostic['message'] else b'pub'
                at = source.index(word); primary = diagnostic['primary']
                self.assertEqual((primary['start'], primary['end']), (at, at + len(word)))
                prefix = source[:at]
                self.assertEqual(primary['line'], prefix.count(b'\n') + 1)
                self.assertEqual(primary['column'], len(prefix.rsplit(b'\n', 1)[-1].decode('utf8')) + 1)
                self.assertEqual(diagnostic['code'], 'E0101')
                self.assertEqual(diagnostic['stage'], 'parse')
            self.assertEqual(row['unchanged_expected'], {'recognized_final': True, 'result': 'parse_error'})

    def test_data_tampering_rejects_before_derivation(self):
        for mutate in (lambda x: x['cases'].pop(), lambda x: x['case_ids'].reverse(),
                       lambda x: x['cases'][0]['new_diagnostics_exact'][0].update(code='E9999'),
                       lambda x: x['cases'][0]['source'].update(sha256='0' * 64),
                       lambda x: x.update(current_document_canonical_sha256='0' * 64)):
            value = copy.deepcopy(self.data); mutate(value)
            with self.assertRaisesRegex(ValueError, 'unapproved composition parser amendment'):
                amendment.apply(self.effective, self.receipt, json.dumps(value).encode())

    def test_predecessor_tampering_or_double_application_rejects(self):
        for change in ('source', 'expected', 'extra', 'missing'):
            value = copy.deepcopy(self.effective)
            if change == 'source': value['cases'][0]['source']['bytes'] += 1
            elif change == 'expected': value['cases'][0]['expected']['result'] = 'accepted'
            elif change == 'extra': value['cases'].append(copy.deepcopy(value['cases'][0]))
            else: value['cases'].pop()
            with self.assertRaisesRegex(ValueError, 'predecessor document differs'):
                amendment.apply(value, self.receipt)
        current, receipt = amendment.apply(self.effective, self.receipt)
        with self.assertRaisesRegex(ValueError, 'predecessor document differs'):
            amendment.apply(current, receipt)
        with self.assertRaisesRegex(ValueError, 'predecessor receipt differs'):
            amendment.apply(self.effective, {**self.receipt, 'extra': 'unapproved'})

    def test_current_entrypoint_requires_exact_module_and_data(self):
        current, receipt = p.current_parser_contract(self.a, self.effective, self.receipt)
        self.assertEqual((current, receipt), amendment.apply(self.effective, self.receipt))
        with patch.object(p, 'verify_map', side_effect=p.Rejected('changed pinned artifact')):
            with self.assertRaisesRegex(p.Rejected, 'changed pinned artifact'):
                p.current_parser_contract(self.a, self.effective, self.receipt)


if __name__ == '__main__':
    unittest.main()
