"""Byte oracle adaptation keeps preceding checks and authenticates its library."""
import hashlib
from pathlib import Path
import tempfile
import unittest
import verify_byte_integers as adapter

ROOT = Path(__file__).resolve().parents[1]


class ByteOracle(unittest.TestCase):
    def test_byte_subjects_use_narrow_cells_and_direct_byte_results(self):
        for operation in adapter.operations("u8"):
            text, result, origin = adapter.source("u8", operation)
            self.assertIn("fn number(argument:i32)->i32", text)
            self.assertNotIn("u64", text.split("fn apply(")[0])
            if operation in adapter.predecessor.ARITHMETIC or operation == "identity":
                self.assertEqual(result, "u8")
            if origin:
                self.assertEqual(text[:origin["start"]].count("\n") + 1, origin["line"])
                self.assertEqual(len(text[:origin["start"]].rsplit("\n", 1)[-1]) + 1, origin["column"])
                self.assertGreater(origin["end"], origin["start"])
        self.assertIn("to_u8", adapter.operations("u8"))

    def test_parent_is_authenticated_before_execution(self):
        controller = Path(adapter.__file__).read_bytes()
        parent = ROOT / "scripts" / adapter.PARENT
        self.assertEqual(hashlib.sha256(parent.read_bytes()).hexdigest(), adapter.PARENT_SHA)
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            for body in (parent.read_bytes() + b"\n", b'raise RuntimeError("untrusted executed")\n'):
                (root / adapter.PARENT).write_bytes(body)
                with self.assertRaisesRegex(ValueError, "changed pinned"):
                    exec(compile(controller, str(root / "adapter.py"), "exec"),
                         {"__file__": str(root / "adapter.py"), "__name__": "_control"})


if __name__ == "__main__":
    unittest.main()
