"""Reject observer drift while keeping all preceding projections exact."""
import hashlib
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

import legacy_scalar_observer_byte_strings as adapter

ROOT = Path(__file__).resolve().parents[1]


class ByteStringObserverAdapterTests(unittest.TestCase):
    def test_exact_parent_and_reverse_projection_for_every_wrapper(self):
        for key, digest in adapter.DERIVED_PINS.items():
            kind, name = key.split("/")
            original = (ROOT / f"tests/fixtures/bounded_typed_{kind}/observer" / name).read_bytes()
            before = adapter.predecessor.adapt_wrapper(kind, name, original)
            after = adapter.adapt_wrapper(kind, name, original)
            self.assertEqual(hashlib.sha256(after).hexdigest(), digest)
            if name == "frontend_mod.rs":
                for removed, added in reversed(adapter.EDITS):
                    self.assertEqual(after.count(added), 1)
                    after = after.replace(added, removed, 1)
            self.assertEqual(after, before)
            with self.assertRaises(ValueError):
                adapter.adapt_wrapper(kind, name, original + b"\n")

    def test_coherent_projection_changes_and_missing_seams_are_rejected(self):
        original = (ROOT / "tests/fixtures/bounded_typed_parser/observer/frontend_mod.rs").read_bytes()
        edits = tuple((before, after.replace(b'"byte_string"', b'"other"'))
                      for before, after in adapter.EDITS)
        with patch.object(adapter, "EDITS", edits), self.assertRaisesRegex(ValueError, "pinned"):
            adapter.adapt_wrapper("parser", "frontend_mod.rs", original)
        with patch.object(adapter, "EDITS", adapter.EDITS[:-1]), self.assertRaisesRegex(ValueError, "count"):
            adapter.adapt_wrapper("parser", "frontend_mod.rs", original)

    def test_parent_bytes_authenticate_before_python_execution(self):
        controller = Path(adapter.__file__).read_bytes()
        parent = ROOT / "scripts" / adapter.PARENT
        self.assertEqual(hashlib.sha256(parent.read_bytes()).hexdigest(), adapter.PARENT_SHA)
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            for body in (parent.read_bytes() + b"\n", b'raise RuntimeError("untrusted code executed")\n'):
                (root / adapter.PARENT).write_bytes(body)
                with self.assertRaisesRegex(ValueError, "changed pinned"):
                    exec(compile(controller, str(root / "adapter.py"), "exec"),
                         {"__file__": str(root / "adapter.py"), "__name__": "_adapter_control"})


if __name__ == "__main__":
    unittest.main()
