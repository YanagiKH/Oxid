"""Pinned successor controls for the narrow scalar parser/static wrappers."""
import hashlib
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

import build_typed_parser_observer as parser
import build_typed_static_observer as static
import legacy_scalar_observer_local_references as parent
import legacy_scalar_observer_vectors as adapter

ROOT = Path(__file__).resolve().parents[1]


class VectorScalarAdapterTests(unittest.TestCase):
    def test_every_old_wrapper_and_schema_is_recovered_exactly(self):
        self.assertEqual(adapter.PINS, parent.PINS)
        self.assertEqual(adapter.PARENT_SCHEMA, parent.SCHEMA)
        self.assertEqual(parent.SCHEMA, "legacy-scalar-observer-local-reference-compatibility-11")
        for key, expected in adapter.DERIVED_PINS.items():
            kind, name = key.split('/')
            original = (ROOT / f'tests/fixtures/bounded_typed_{kind}/observer' / name).read_bytes()
            before = parent.adapt_wrapper(kind, name, original)
            after = adapter.adapt_wrapper(kind, name, original)
            self.assertEqual(hashlib.sha256(after).hexdigest(), expected)
            restored = adapter.reverse_wrapper(kind, name, after)
            self.assertEqual(restored, before)
            self.assertEqual(hashlib.sha256(restored).hexdigest(), parent.DERIVED_PINS[key])
            if name != 'frontend_mod.rs':
                self.assertEqual(after, before)
        parser_original = (ROOT / 'tests/fixtures/bounded_typed_parser/observer/frontend_mod.rs').read_bytes()
        static_original = (ROOT / 'tests/fixtures/bounded_typed_static/observer/frontend_mod.rs').read_bytes()
        self.assertTrue(adapter.adapt_wrapper('static', 'frontend_mod.rs', static_original).startswith(
            adapter.adapt_wrapper('parser', 'frontend_mod.rs', parser_original)))

    def test_only_the_explicit_named_slice_refusal_is_inserted(self):
        self.assertEqual(len(adapter.EDITS), 1)
        before, after = adapter.EDITS[0]
        self.assertEqual(after.replace(before, b'', 1),
                         b'        TypeSyntaxKind::GenericSliceReference { .. } => "named_slice_reference_type",\n')
        self.assertNotIn(b'return None', after)
        self.assertNotIn(b'_ =>', after)
        for kind, builder in [('parser', parser), ('static', static)]:
            self.assertEqual(builder.SOURCE_FILES.count('vector_builtins.rs'), 1)
            self.assertEqual(builder.SCHEMA, adapter.SCHEMA)
            for name, _ in builder.WRAPPER_FILES:
                original = (ROOT / f'tests/fixtures/bounded_typed_{kind}/observer' / name).read_bytes()
                self.assertEqual(builder.adapt_wrapper(name, original), adapter.adapt_wrapper(kind, name, original))

    def test_unknown_input_double_adaptation_and_weakened_refusals_fail(self):
        original = (ROOT / 'tests/fixtures/bounded_typed_parser/observer/frontend_mod.rs').read_bytes()
        derived = adapter.adapt_wrapper('parser', 'frontend_mod.rs', original)
        for body in [original + b'\n', derived]:
            with self.assertRaises(ValueError):
                adapter.adapt_wrapper('parser', 'frontend_mod.rs', body)
        for body in [derived + b'\n', derived.replace(b'named_slice_reference_type', b'accepted')]:
            with self.assertRaises(ValueError):
                adapter.reverse_wrapper('parser', 'frontend_mod.rs', body)
        weakened = tuple((before, after.replace(b'"named_slice_reference_type"', b'return None'))
                         for before, after in adapter.EDITS)
        with patch.object(adapter, 'EDITS', weakened), self.assertRaises(ValueError):
            adapter.adapt_wrapper('parser', 'frontend_mod.rs', original)
        with patch.object(adapter, 'EDITS', ()), self.assertRaises(ValueError):
            adapter.adapt_wrapper('parser', 'frontend_mod.rs', original)

    def test_parent_is_authenticated_before_execution(self):
        controller = Path(adapter.__file__).read_bytes()
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            for body in [(ROOT / 'scripts' / adapter.PARENT).read_bytes() + b'\n',
                         b'raise RuntimeError("untrusted parent executed")\n']:
                (root / adapter.PARENT).write_bytes(body)
                with self.assertRaisesRegex(ValueError, 'changed pinned'):
                    exec(compile(controller, str(root / 'adapter.py'), 'exec'),
                         {'__file__': str(root / 'adapter.py')})


if __name__ == '__main__':
    unittest.main()
