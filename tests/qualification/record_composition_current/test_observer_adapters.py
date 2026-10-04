#!/usr/bin/env python3
"""Identity/reversal controls for the bounded archived-observer derivations."""
import ast
import hashlib
import json
from pathlib import Path
import re
import subprocess
import tempfile
import unittest

import observer_adapters as a
import current_hooks as hooks
import runtime_overlay as runtime

HERE = Path(__file__).resolve().parent
REPO = HERE.parents[2]


def originals():
    return {row['path']: (REPO / row['path']).read_bytes()
            for row in runtime.CURRENT_FILES if row['before']['bytes']}


def runtime_inputs():
    return (originals(), (REPO / a.IDENTITIES['prepare']['original']['path']).read_bytes(),
            (REPO / a.IDENTITIES['raw_walker']['archived_container']['path']).read_bytes())


class ObserverAdapters(unittest.TestCase):
    def cases(self):
        patch = runtime_inputs()[2]
        added = a.archived_observer_additions(patch)
        return [
            ('typing', a.derive_typing_observer, a.reverse_typing_observer,
             (REPO / a.IDENTITIES['typing']['original']['path']).read_bytes()),
            ('prepare', a.derive_observer_prepare, a.reverse_observer_prepare,
             runtime_inputs()[1]),
            ('store', a.derive_scalar_store_instrumentation, a.reverse_scalar_store_instrumentation,
             (REPO / a.IDENTITIES['store']['original']['path']).read_bytes()),
            ('raw_walker', a.derive_raw_walker, a.reverse_raw_walker,
             added['src/frontend/oir/owned/unit3_raw_owned.rs']),
            ('journal', a.derive_journal, a.reverse_journal,
             added['src/frontend/unit3_observer.rs']),
        ]

    def test_exact_derived_identity_and_reversal(self):
        for name, derive, reverse, old in self.cases():
            with self.subTest(name=name):
                derived = derive(old)
                self.assertEqual(a.digest(derived), a.IDENTITIES[name]['derived']['sha256'])
                self.assertEqual(reverse(derived), old)

    def test_changed_original_rejected(self):
        for name, derive, _, old in self.cases():
            with self.subTest(name=name), self.assertRaises(ValueError): derive(old + b'\n')

    def test_double_adaptation_rejected(self):
        for name, derive, _, old in self.cases():
            with self.subTest(name=name), self.assertRaises(ValueError): derive(derive(old))

    def test_changed_derived_rejected_on_reversal(self):
        for name, derive, reverse, old in self.cases():
            with self.subTest(name=name), self.assertRaises(ValueError): reverse(derive(old) + b'\n')

    def test_identity_document_is_exact(self):
        self.assertEqual(json.loads((HERE / 'observer-adapter-identities.json').read_text())['adapters'], a.IDENTITIES)
        self.assertEqual(json.loads((HERE / 'current-hook-identities.json').read_text()), hooks.IDENTITIES)

    def test_archived_overlay_mutation_rejected(self):
        with self.assertRaises(ValueError): a.archived_observer_additions(runtime_inputs()[2] + b'\n')

    def test_current_hook_identities_reversal_and_replay_rejection(self):
        for path in hooks.IDENTITIES:
            with self.subTest(path=path):
                old = (REPO / path).read_bytes(); new = hooks.derive(path, old)
                self.assertEqual(hooks.reverse(path, new), old)
                with self.assertRaises(ValueError): hooks.derive(path, new)
                with self.assertRaises(ValueError): hooks.derive(path, old + b'\n')
                with self.assertRaises(ValueError): hooks.reverse(path, new + b'\n')

    def test_runtime_derivation_and_current_identity_rejection(self):
        inputs, recipe, patch = runtime_inputs()
        got = runtime.derive_runtime_overlay(inputs, recipe, patch)
        self.assertEqual(set(got), {row['path'] for row in runtime.CURRENT_FILES})
        changed = dict(inputs); changed['src/frontend/oir/owned/execute.rs'] += b'\n'
        with self.assertRaises(ValueError): runtime.derive_runtime_overlay(changed, recipe, patch)
        with self.assertRaises(ValueError): runtime.derive_runtime_overlay({}, recipe, patch)

    def test_runtime_patch_apply_and_reverse_matches_derivation(self):
        inputs, recipe, patch = runtime_inputs(); expected = runtime.derive_runtime_overlay(inputs, recipe, patch)
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            for name, data in inputs.items():
                path = root / name; path.parent.mkdir(parents=True, exist_ok=True); path.write_bytes(data)
            patch_path = HERE / 'unit3-observer-current-v1.patch'
            subprocess.run(['git', 'apply', '--check', str(patch_path)], cwd=root, check=True, capture_output=True)
            subprocess.run(['git', 'apply', str(patch_path)], cwd=root, check=True, capture_output=True)
            self.assertEqual({name: (root / name).read_bytes() for name in expected}, expected)
            subprocess.run(['git', 'apply', '-R', str(patch_path)], cwd=root, check=True, capture_output=True)
            self.assertEqual({str(path.relative_to(root)): path.read_bytes() for path in root.rglob('*') if path.is_file()}, inputs)

    def test_store_patch_changes_only_exact_scalar_store(self):
        old = (REPO / a.IDENTITIES['store']['original']['path']).read_bytes()
        new = a.derive_scalar_store_instrumentation(old)
        before, after = a.STORE_SEAMS[0]
        self.assertEqual(old.replace(before, b'<store_field>', 1), new.replace(after, b'<store_field>', 1))
        self.assertLess(after.index(b'        )?;'), after.index(b'::event("payload_write"'))
        self.assertEqual(after.count(b'::event("payload_write"'), 1)

    def test_event_hooks_follow_exact_existing_pushes(self):
        old = (REPO / hooks.EVENT_PATH).read_bytes(); new = hooks.derive(hooks.EVENT_PATH, old)
        self.assertEqual(new.count(b'::event("existing_owned_event"'), 2)
        before, after = hooks.SEAMS[hooks.EVENT_PATH][0]
        self.assertEqual(old.replace(before, b'<record_event>'), new.replace(after, b'<record_event>'))
        for segment in after.split(b'self.events.push(event);')[1:]:
            self.assertTrue(segment.lstrip().startswith(b'crate::frontend::unit3_observer::event('))

    def test_historical_store_recipe_gap_and_migrated_recipe(self):
        old = runtime_inputs()[1]
        def transform(recipe):
            ns = {'re': re, 'J': 'crate::frontend::unit3_observer'}
            for node in ast.parse(recipe).body:
                if isinstance(node, ast.FunctionDef) and node.name in ('replace', 'insert_before', 'insert_after', 'hook', 'owned_runtime'):
                    exec(compile(ast.Module([node], []), '<pinned-recipe-test>', 'exec'), ns)
            return ns['owned_runtime']((REPO / a.IDENTITIES['store']['original']['path']).read_text())
        # This is a reproduction-only assertion in the archived recipe. The
        # adapter identities and all actual acceptance checks also run under -O.
        if __debug__:
            with self.assertRaises(AssertionError): transform(old)
        self.assertIn('::event("payload_write"', transform(a.derive_observer_prepare(old)))

    def test_typing_overlay_closed_membership(self):
        source = {path: (REPO / path).read_bytes() for path in hooks.TYPING_PATHS}
        observer = (REPO / a.IDENTITIES['typing']['original']['path']).read_bytes()
        got = hooks.derive_typing_overlay(source, observer)
        self.assertEqual(set(got), set(hooks.TYPING_PATHS) | {'src/frontend/oir/owned/source/array_typing_observer.rs'})
        with self.assertRaises(ValueError): hooks.derive_typing_overlay({}, observer)


if __name__ == '__main__': unittest.main()
