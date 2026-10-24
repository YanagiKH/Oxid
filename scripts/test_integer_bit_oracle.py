"""Keep bit-pattern expectations independent and authenticate the oracle chain."""
import hashlib
from pathlib import Path
import tempfile
import unittest
import verify_integer_bits as adapter


class IntegerBitOracle(unittest.TestCase):
    def test_bit_patterns_shift_bounds_and_signed_right_shift(self):
        for ty, width, signed in ((t, *shape) for t, shape in adapter.predecessor.TYPES.items()):
            self.assertEqual(adapter.expected(ty, "not", 0, 0), (-1 if signed else 2**width-1, None))
            self.assertEqual(adapter.expected(ty, "shl", 1, width), (None, "E0611"))
            self.assertEqual(adapter.expected(ty, "shr", 1, -1), (None, "E0611"))
            self.assertIn((0, width), adapter.cases(ty, "shl"))
        self.assertEqual(adapter.expected("u8", "shl", 255, 1), (254, None))
        self.assertEqual(adapter.expected("i8", "shr", -128, 7), (-1, None))
        self.assertEqual(adapter.expected("i128", "shl", 1, 127), (-(2**127), None))
        self.assertEqual(adapter.expected("u128", "xor", 2**128-1, 1), (2**128-2, None))

    def test_generated_programs_use_named_receivers_and_exact_operator_origins(self):
        for ty in adapter.predecessor.TYPES:
            for operation, marker in adapter.OPERATIONS.items():
                text, result, origin = adapter.source(ty, operation)
                self.assertEqual(result, ty)
                self.assertNotIn("number(2).", text)
                self.assertEqual(text[origin["start"]:origin["end"]], marker)
                self.assertEqual(text[:origin["start"]].count("\n")+1, origin["line"])
                if operation in ("shl", "shr"):
                    self.assertIn("fn shift_amount(argument:i32)->i32", text)
                    self.assertNotIn("fn count(", text)
            if ty in ("i32", "u8"):
                self.assertIn("fn number(argument:i32)->i32", text)
                self.assertNotIn("i64", text.split("fn apply(")[0])

    def test_parent_authenticates_before_python_execution(self):
        controller = Path(adapter.__file__).read_bytes()
        parent = Path(adapter.__file__).with_name(adapter.PARENT)
        self.assertEqual(hashlib.sha256(parent.read_bytes()).hexdigest(), adapter.PARENT_SHA)
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            for body in (parent.read_bytes()+b"\n", b'raise RuntimeError("untrusted executed")\n'):
                (root / adapter.PARENT).write_bytes(body)
                with self.assertRaisesRegex(ValueError, "changed pinned"):
                    exec(compile(controller, str(root / "adapter.py"), "exec"), {"__file__": str(root / "adapter.py")})
