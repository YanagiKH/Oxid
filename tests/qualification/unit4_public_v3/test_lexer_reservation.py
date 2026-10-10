#!/usr/bin/env python3
"""Source-only current lifecycle controls; never invokes a compiler."""
import sys
sys.dont_write_bytecode = True
import json
from pathlib import Path
import unittest
import lexer_reservation_lifecycle as m


class CurrentLifecycle(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.patch = (m.HERE / 'observer-lexer-reservation-v2.patch').read_bytes()
        cls.proof = json.loads((m.HERE / 'lexer-reservation-lifecycle-v2.json').read_bytes())
        cls.inputs = {r['path']: (m.REPOSITORY / r['path']).read_bytes() for r in cls.proof['base_files']}
        cls.derived, _ = m.admit(m.REPOSITORY, cls.patch)

    def test_complete_maps_and_exact_inverse(self):
        self.assertEqual((len(self.inputs), len(self.derived)), (376, 377))
        self.assertEqual(m.rows(self.inputs), self.proof['base_files'])
        self.assertEqual(m.rows(self.derived, True), self.proof['observer_files'])
        self.assertEqual(m.exact_patch(self.derived, self.patch, True), self.inputs)
        self.assertEqual(m.exact_patch(self.inputs, self.patch), self.derived)
        with self.assertRaises(m.Rejected):
            m.exact_patch(self.inputs, self.patch, True)
        with self.assertRaises(m.Rejected):
            m.exact_patch(self.derived, self.patch)

    def test_nonlexer_insertions_unchanged(self):
        old = m.inserted_roster((m.HERE / 'observer-u8-v1.patch').read_bytes())
        new = m.inserted_roster(self.patch)
        self.assertEqual({r['path']: r['inserted'] for r in old if r['path'] != m.LEXER},
                         {r['path']: r['inserted'] for r in new if r['path'] != m.LEXER})

    def test_each_exact_hunk_context_is_checked(self):
        count = 0
        for name, added, hunks in m.sections(self.patch):
            if added:
                continue
            for starts, counts, before, after, lines in hunks:
                damaged = dict(self.inputs)
                raw = damaged[name].splitlines(keepends=True)
                offset = starts[0] - (counts[0] != 0)
                raw[offset] += b'changed exact context\n'
                damaged[name] = b''.join(raw)
                with self.subTest(name=name, offset=offset), self.assertRaises(m.Rejected):
                    m.exact_patch(damaged, self.patch)
                count += 1
        self.assertEqual(count, 16)

    def test_reordered_and_duplicate_patch_sections_rejected(self):
        sections = self.patch.split(b'--- ')
        duplicate = self.patch + b'--- ' + sections[1]
        with self.assertRaises(m.Rejected):
            m.exact_patch(self.inputs, duplicate)
        reordered = b''.join(b'--- ' + s for s in reversed(sections[1:]))
        # Exact sealed order is separately authenticated, even if disjoint
        # text sections could otherwise commute.
        with self.assertRaises(m.Rejected):
            m.admit(m.REPOSITORY, reordered)

    def test_missing_duplicate_and_wrapper_hooks_reject(self):
        api = m.adapter()
        raw = self.inputs[m.LEXER]
        derived = self.derived[m.LEXER]
        hook = b'    crate::frontend::lifecycle_observer::event("lex_attempt", source.path());\n'
        self.assertEqual(derived.count(hook), 1)
        self.assertEqual(api.inverse_exact(derived, api.LIFECYCLE_SEAMS), raw)
        for bad in (derived.replace(hook, b'', 1), derived.replace(hook, hook * 2, 1),
                    derived + hook):
            altered = dict(self.derived); altered[m.LEXER] = bad
            with self.subTest(digest=m.sha(bad)), self.assertRaises(m.Rejected):
                m.need(m.exact_patch(altered, self.patch, True) == self.inputs,
                       'complete inverse identity differs')
        core = derived.split(api.CORE_START, 1)[1].split(api.CORE_END.replace(
            b'    Ok(tokens)\n', b'    ' + api.LIFECYCLE_OBSERVER + b'::event("lex_complete", source.path());\n    Ok(tokens)\n'), 1)[0]
        self.assertIn(hook, core)
        self.assertNotIn(hook, derived.split(api.CORE_START, 1)[0])

    def test_stale_predecessor_and_tampered_patch_reject(self):
        for bad in ((m.HERE / 'observer-u8-v1.patch').read_bytes(), self.patch + b'\n',
                    self.patch.replace(b'lex_attempt', b'lex_missing')):
            with self.subTest(digest=m.sha(bad)), self.assertRaises(m.Rejected):
                m.admit(m.REPOSITORY, bad)


if __name__ == '__main__':
    unittest.main()
