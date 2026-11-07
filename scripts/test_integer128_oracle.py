"""The wide oracle preserves exact values, source origins and parent authority."""
import hashlib
import json
from pathlib import Path
import tempfile
from types import SimpleNamespace
import unittest
import verify_integer128 as adapter


class Integer128Oracle(unittest.TestCase):
    def test_runtime_parsers_cover_the_complete_decimal_domain(self):
        for ty in ("i128", "u128"):
            for operation in adapter.operations(ty):
                text, result, origin = adapter.source(ty, operation)
                self.assertIn(f"fn number(argument:i32)->{ty}", text)
                self.assertIn('b"' + "." * 41 + '"', text)
                if origin is not None:
                    token = operation + "_checked" if operation.startswith("to_") else (adapter.predecessor.ARITHMETIC | adapter.predecessor.COMPARISONS).get(operation, "-")
                    self.assertEqual(text[origin["start"]:origin["end"]], token)
                    self.assertEqual(text[:origin["start"]].count("\n") + 1, origin["line"])
                if operation in adapter.predecessor.COMPARISONS:
                    self.assertEqual(result, "bool")
            self.assertIn(2**64, adapter.boundary_values(ty))
        self.assertIn(2**127, adapter.boundary_values("u128"))
        self.assertIn(-(2**127), adapter.boundary_values("i128"))
        self.assertEqual(adapter.predecessor.expected("i128", "div", -7, 3), (-2, None))
        self.assertEqual(adapter.predecessor.expected("i128", "rem", -7, 3), (-1, None))
        self.assertEqual(adapter.predecessor.expected("i128", "rem", -(2**127), -1), (None, "E0604"))

    def test_json_requires_exact_decimal_strings_for_128_bit_values(self):
        for ty, value in (("u128", 2**128 - 1), ("i128", -(2**127))):
            def result(encoded):
                row = {"kind": "run-summary", "success": True, "result": {"type": ty, "value": encoded}}
                return SimpleNamespace(returncode=0, stdout=json.dumps(row).encode()+b"\n", stderr=b"")
            adapter.predecessor.verify(result(str(value)), ty, value, None, None, True)
            for wrong in (value, float(value), str(value + 1)):
                with self.assertRaisesRegex(ValueError, "inexact"):
                    adapter.predecessor.verify(result(wrong), ty, value, None, None, True)

    def test_parent_authenticates_before_execution(self):
        controller = Path(adapter.__file__).read_bytes()
        parent = Path(adapter.__file__).with_name(adapter.PARENT)
        self.assertEqual(hashlib.sha256(parent.read_bytes()).hexdigest(), adapter.PARENT_SHA)
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            for body in (parent.read_bytes()+b"\n", b'raise RuntimeError("untrusted executed")\n'):
                (root / adapter.PARENT).write_bytes(body)
                with self.assertRaisesRegex(ValueError, "changed pinned"):
                    exec(compile(controller, str(root / "adapter.py"), "exec"), {"__file__": str(root / "adapter.py")})
