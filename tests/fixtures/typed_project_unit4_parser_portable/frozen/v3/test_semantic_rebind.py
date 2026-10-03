#!/usr/bin/env python3
"""Strict portable v8b/effective-authority binding; no candidate execution."""
import ast
import copy
import json
import os
from pathlib import Path
import shutil
import tempfile
import unittest

import portable as p


class SemanticRebindControls(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.a = p.authority(); cls.c, cls.proof = p.comparator()
        cls.contract_dir = Path(os.environ['UNIT4_PORTABLE_CONTRACT'])
        cls.base = cls.c.load_contract(cls.contract_dir)
        cls.authority = p.effective_authority(cls.a)

    def test_exact_v8b_transform_retains_all_other_functions_and_case_tail(self):
        source = (p.HERE / 'frozen/comparator/comparator.py').read_text()
        self.assertEqual(p.sha(source.encode()), '7c40e4782bee8082dc41534227348c26f952f3b870904cda9e71862b0be42a6b')
        start = source.index(p.PREFIX_START, source.index('def execution_manifest(path, contract, approval):\n'))
        end = source.index(p.PREFIX_END, start)
        derived = source[:start] + '    manifest, host, build, profile = portable_admission(path, contract, approval)\n' + source[end:]
        self.assertEqual(p.sha(derived.encode()), self.proof['derived_sha256'])
        before, after = ast.parse(source), ast.parse(derived)
        original = {n.name: n for n in before.body if isinstance(n, (ast.FunctionDef, ast.ClassDef))}
        transformed = {n.name: n for n in after.body if isinstance(n, (ast.FunctionDef, ast.ClassDef))}
        self.assertEqual(original.keys(), transformed.keys())
        for name in original.keys() - {'execution_manifest'}:
            with self.subTest(definition=name):
                self.assertEqual(ast.dump(original[name], include_attributes=False), ast.dump(transformed[name], include_attributes=False))
        fn = original['execution_manifest']
        index = next(i for i, n in enumerate(fn.body) if isinstance(n, ast.Assign) and any(isinstance(t, ast.Name) and t.id == 'cases' for t in n.targets))
        self.assertEqual([ast.dump(n, include_attributes=False) for n in fn.body[index:]],
                         [ast.dump(n, include_attributes=False) for n in transformed['execution_manifest'].body[1:]])
        for required in ('events', 'expected_predicates', 'required_first_admission', 'admit_contract_amendment', 'apply_coordinate_amendment', 'compare_effective_rows'):
            self.assertIn(required, original)

    def test_base_execution_identity_and_strict_effective_comparison_identity(self):
        base_bytes = self.c.canonical(self.base)
        effective, receipt = self.c.admit_contract_amendment(self.base, self.contract_dir, self.authority)
        self.assertEqual(self.c.canonical(self.base), base_bytes)
        self.assertEqual(self.c.sha(self.c.canonical(effective)), 'c2d4f8db28f7815b3e17ca13fec9a7da70f0923dd052656edb339bc0cd7740cd')
        self.assertEqual(len(self.c.changed_leaves(self.base, effective)), 4)
        self.assertEqual(receipt, self.c.effective_receipt())
        result = self.c.compare_effective_rows(effective, receipt, [], {})
        self.assertEqual(result['status'], 'fail')
        self.assertEqual(result['expected_observations'], 638)
        self.assertEqual(result['execution_contract']['contract_decoded_sha256'], self.a['contract_sha256'])
        self.assertEqual(result['execution_contract']['package_freeze_sha256'], self.a['freeze_sha256'])
        for wrong in (self.base, {'effective_contract_identity': receipt['effective_contract_identity']}):
            with self.assertRaises(self.c.Rejected): self.c.compare_effective_rows(wrong, receipt, [], {})
        altered = dict(receipt, effective_parser_document_canonical_sha256=self.a['contract_sha256'])
        with self.assertRaises(self.c.Rejected): self.c.compare_effective_rows(effective, altered, [], {})

    def test_no_already_applied_truncated_or_mutated_base(self):
        effective, _ = self.c.admit_contract_amendment(self.base, self.contract_dir, self.authority)
        cases = [effective, {'cases': self.base['cases']}, copy.deepcopy(self.base)]
        cases[-1]['cases'][0]['source']['path'] = 'unreviewed'
        for value in cases:
            with self.subTest(keys=sorted(value)), self.assertRaises(self.c.Rejected):
                self.c.admit_contract_amendment(value, self.contract_dir, self.authority)

    def test_complete_authority_and_coherent_same_path_tampering(self):
        with tempfile.TemporaryDirectory(prefix='oxid-effective-authority-') as temp:
            target = Path(temp)
            shutil.copytree(p.HERE / 'frozen/amendment', target / 'amendment')
            shutil.copyfile(p.HERE / 'reviews/amendment-review.json', target / 'review.json')
            relocated = {key: p.identity(target / suffix) for key, suffix in {
                'checkpoint': 'amendment/AMENDMENT-CHECKPOINT-v1.json', 'descriptor': 'amendment/artifacts/effective-contract.json',
                'amendment': 'amendment/artifacts/amendment.json', 'review': 'review.json'}.items()}
            effective, receipt = self.c.admit_contract_amendment(self.base, self.contract_dir, relocated)
            self.assertEqual(self.c.sha(self.c.canonical(effective)), self.c.EFFECTIVE_DOCUMENT_SHA)
            self.assertEqual(receipt, self.c.effective_receipt())
            for key in relocated:
                missing = dict(relocated); missing.pop(key)
                with self.subTest(missing=key), self.assertRaises(self.c.Rejected): self.c.amendment_artifacts(missing)
                path = Path(relocated[key]['path']); raw = path.read_bytes()
                path.write_bytes(raw + b'\n'); changed = dict(relocated); changed[key] = p.identity(path)
                with self.subTest(coherent_hash_change=key), self.assertRaises(self.c.Rejected): self.c.amendment_artifacts(changed)
                path.write_bytes(raw)
            changed = dict(relocated, extra=relocated['review'])
            with self.assertRaises(self.c.Rejected): self.c.amendment_artifacts(changed)

    def test_portable_authority_cannot_substitute_amendment(self):
        original = copy.deepcopy(self.a)
        for key in original['effective_contract_authority']:
            a = copy.deepcopy(original); a['effective_contract_authority'][key]['sha256'] = '0' * 64
            with self.subTest(artifact=key), self.assertRaises(p.Rejected): p.effective_authority(a)
        a = copy.deepcopy(original); a['effective_contract_authority']['review']['path'] = '../foreign-review.json'
        with self.assertRaises(p.Rejected): p.effective_authority(a)


if __name__ == '__main__':
    unittest.main(verbosity=2)
