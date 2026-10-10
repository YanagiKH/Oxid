#!/usr/bin/env python3
"""Exact source-only current token/reserve controls, no semantic qualification."""
import sys
sys.dont_write_bytecode = True
import ast
import copy
import json
from pathlib import Path
import unittest
from unittest.mock import patch
import portable as p


class CurrentLexer(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.a = p.authority()
        cls.inputs = {r['path']: (p.REPOSITORY / r['path']).read_bytes()
                      for r in cls.a['current_source']['files']}
        cls.helper = p.lexer_module(cls.a['current'])
        cls.hooks = cls.helper.adapter(p.lexer_api(), cls.a['current'])

    def test_complete_outer_inverse_and_wrong_stage(self):
        prior = p.restore_lexer_source(self.a['current'], self.inputs)
        expected = p.read(p.REPOSITORY / self.a['current']['lexer_reservation']['source_predecessor']['path'])
        self.assertEqual(len(prior), 376)
        self.assertEqual([{'path': n, 'bytes': len(b), 'sha256': p.sha(b)} for n, b in sorted(prior.items())], expected['files'])
        with self.assertRaises(p.Rejected):
            p.restore_lexer_source(self.a['current'], prior)
        for change in ('missing', 'extra', 'altered'):
            damaged = dict(self.inputs)
            if change == 'missing': damaged.pop('src/frontend/lexer.rs')
            elif change == 'extra': damaged['src/unapproved.rs'] = b''
            else: damaged['src/frontend/lexer.rs'] += b'\n'
            with self.subTest(change=change), self.assertRaises(p.Rejected):
                p.restore_lexer_source(self.a['current'], damaged)

    def test_coherent_manifest_row_rejects_before_preflight_or_materialization(self):
        current = copy.deepcopy(self.a['current_source'])
        name = 'src/frontend/lexer.rs'
        raw = self.inputs[name] + b'\n'
        next(r for r in current['files'] if r['path'] == name).update(bytes=len(raw), sha256=p.sha(raw))
        # The source API is not even requested for forged incoming metadata.
        with patch.object(p, 'u8_binding_api', side_effect=AssertionError('preflight/materialization reached')):
            with self.assertRaisesRegex(p.Rejected, 'complete admitted lexer source'):
                p.validate_u8_transition(self.a['current'], current, self.a)

    def test_successful_append_hook_and_exact_inverse(self):
        name = 'src/frontend/lexer.rs'
        raw = self.inputs[name]
        derived = p.compose_current_lexer(self.a, raw)
        hook = b'crate::frontend::parser::unit4_observer::lex_token(*tokens.last().unwrap());'
        self.assertEqual(derived.count(hook), 1)
        self.assertIn(b'    tokens.push(token);\n    ' + hook + b'\n    Ok(())', derived)
        self.assertEqual(self.hooks.inverse_exact(derived, self.hooks.TOKEN_SEAMS), raw)
        control = next(r for r in self.a['current']['current_control_derived_files'] if r['path'] == name)
        self.assertEqual(control, {'path': name, 'bytes': len(raw), 'sha256': p.sha(raw)})
        with self.assertRaises((p.Rejected, self.hooks.Reject)):
            p.compose_current_lexer(self.a, derived)

    def test_reserve_hook_and_control_are_preserved(self):
        name = 'src/frontend/project/budget.rs'
        raw = self.inputs[name]
        derived = p.compose_array_instrumentation(self.a, name, raw)
        self.assertEqual(derived.count(b'::reserve(kind, length, element_bytes, success);'), 1)
        self.assertEqual(self.hooks.inverse_exact(derived, self.hooks.BUDGET_SEAMS), raw)
        control = next(r for r in self.a['current']['current_control_derived_files'] if r['path'] == name)
        self.assertEqual(control, {'path': name, 'bytes': len(raw), 'sha256': p.sha(raw)})

    def test_stale_and_uninstrumented_current_hook_maps_reject(self):
        prior = p.lexer_predecessor_active(self.a['current'])
        for name in ('src/frontend/lexer.rs', 'src/frontend/project/budget.rs'):
            for source in ('current_base_files', 'current_derived_files'):
                wrong = copy.deepcopy(self.a)
                rows = self.a['current']['current_base_files'] if source == 'current_base_files' else prior[source]
                replacement = next(r for r in rows if r['path'] == name)
                next(r for r in wrong['current']['current_derived_files'] if r['path'] == name).update(replacement)
                with self.subTest(name=name, source=source), self.assertRaises((p.Rejected, self.hooks.Reject)):
                    self.helper.compose(p.lexer_api(), wrong, name, self.inputs[name])

    def test_historical_division_composer_is_byte_identical(self):
        path = 'tests/qualification/unit4_parser_current/portable.py'
        saved = Path(p.__file__).with_name('byte_storage_portable_v1.py').read_bytes()
        self.assertEqual(p.sha(saved), '2efcbd2f11ee0b9fee041c19b362459b15f7d843b16bc55f8743a938c58210cd')
        old = saved.decode()
        current = Path(p.__file__).read_text()
        def definition(source):
            node = next(n for n in ast.parse(source).body if isinstance(n, ast.FunctionDef) and n.name == 'compose_division_lexer')
            return ast.get_source_segment(source, node)
        self.assertEqual(definition(old), definition(current))


if __name__ == '__main__':
    unittest.main()
