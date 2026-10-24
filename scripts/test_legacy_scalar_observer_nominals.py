"""Retained scalar observers refuse new nominal nodes without changing old wires."""
import hashlib
from pathlib import Path
import unittest
from unittest.mock import patch

import build_typed_parser_observer as parser
import build_typed_static_observer as static
import legacy_scalar_observer_nominals as adapter

ROOT = Path(__file__).resolve().parents[1]


class NominalObserverAdapterTests(unittest.TestCase):
    def original(self, kind, name):
        family = "bounded_typed_parser" if kind == "parser" else "bounded_typed_static"
        return (ROOT / "tests/fixtures" / family / "observer" / name).read_bytes()

    def test_parent_is_the_unchanged_authenticated_character_adapter(self):
        body = (ROOT / "scripts" / adapter.PARENT).read_bytes()
        self.assertEqual(hashlib.sha256(body).hexdigest(), adapter.PARENT_SHA)
        self.assertEqual(adapter.PINS, adapter.predecessor.PINS)
        for name, pin in adapter.PINS.items():
            kind, name = name.split("/", 1)
            self.assertEqual(hashlib.sha256(self.original(kind, name)).hexdigest(), pin)

    def test_all_wrapper_outputs_are_pinned_and_old_projection_bytes_recover_exactly(self):
        for key, expected in adapter.DERIVED_PINS.items():
            kind, name = key.split("/", 1)
            original = self.original(kind, name)
            result = adapter.adapt_wrapper(kind, name, original)
            self.assertEqual(hashlib.sha256(result).hexdigest(), expected)
            if name == "frontend_mod.rs":
                for before, after in reversed(adapter.EDITS):
                    self.assertEqual(result.count(after), 1)
                    result = result.replace(after, before, 1)
            self.assertEqual(result, adapter.predecessor.adapt_wrapper(kind, name, original))

    def test_changed_original_and_wrong_wrapper_domains_are_rejected(self):
        for key in adapter.PINS:
            kind, name = key.split("/", 1)
            with self.assertRaises(ValueError):
                adapter.adapt_wrapper(kind, name, self.original(kind, name) + b"\n")
        with self.assertRaises(ValueError):
            adapter.adapt_wrapper("other", "main.rs", b"")

    def test_missing_exclusions_and_coherent_substitution_changes_are_rejected(self):
        original = self.original("parser", "frontend_mod.rs")
        with patch.object(adapter, "EDITS", adapter.EDITS[:-1]), self.assertRaises(ValueError):
            adapter.adapt_wrapper("parser", "frontend_mod.rs", original)
        changed = tuple((before, after.replace(b'"generic_variant"', b'"qualified_value"'))
                        for before, after in adapter.EDITS)
        with patch.object(adapter, "EDITS", changed), self.assertRaises(ValueError):
            adapter.adapt_wrapper("parser", "frontend_mod.rs", original)

    def test_both_live_builders_copy_the_complete_nominal_module_and_use_the_new_boundary(self):
        name = "declaration_index/generics/nominal.rs"
        for kind, builder in (("parser", parser), ("static", static)):
            self.assertEqual(builder.SOURCE_FILES.count(name), 1)
            import legacy_scalar_observer_record_patterns as current
            self.assertEqual(builder.SCHEMA, current.SCHEMA)
            for filename, _ in builder.WRAPPER_FILES:
                original = self.original(kind, filename)
                result = builder.adapt_wrapper(filename, original)
                if filename == "frontend_mod.rs":
                    for before, after in reversed(current.EDITS):
                        self.assertEqual(result.count(after), 1)
                        result = result.replace(after, before, 1)
                self.assertEqual(result, adapter.adapt_wrapper(kind, filename, original))


if __name__ == "__main__":
    unittest.main()
