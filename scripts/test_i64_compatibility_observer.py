"""File observer additions cannot change preceding observation bytes."""
import hashlib
import importlib.util
from pathlib import Path
import re
import tempfile
import unittest
from unittest.mock import patch
import i64_compatibility_observer as adapter

ROOT = Path(__file__).resolve().parents[1]

class IntegerObserverTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        path = ROOT / "tests/fixtures/typed_project_source_binding_byte_storage_v1/run.py"
        assert hashlib.sha256(path.read_bytes()).hexdigest() == "584e8ee937cb2d3ef62d59d3eb69a79bfb6afce23df86de3df305f81afae7c76"
        spec = importlib.util.spec_from_file_location("_file_observer_retained", path)
        retained = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(retained)
        original = (ROOT / retained.U2 / retained.OBSERVER).read_bytes()
        cls.original = retained.adapt_enum_unit2_observer(retained.adapt_borrowed_unit2_observer(retained.adapt_unit2_observer(original)))

    def test_whole_predecessor_is_preserved_by_reverse_projection(self):
        before = adapter.predecessor.adapt(self.original)
        after = adapter.adapt(self.original)
        self.assertEqual((len(after), hashlib.sha256(after).hexdigest()), (adapter.DERIVED_BYTES, adapter.DERIVED_SHA))
        self.assertTrue(after.endswith(adapter.CONTROL_TESTS))
        restored = after[:-len(adapter.CONTROL_TESTS)]
        for old, new in reversed(adapter.SEAMS):
            self.assertEqual(restored.count(new), 1)
            restored = restored.replace(new, old, 1)
        self.assertEqual(restored, before)
        for prefix, count in ((b"aggregate", 6), (b"byte_buffer", 3), (b"native_file", 3), (b"i64", 2)):
            names = re.findall(rb"^fn (current_unit2_" + prefix + rb"_adapter_[A-Za-z_0-9]+)\(", after, re.M)
            self.assertEqual(len(names), count)
        self.assertEqual(adapter.CONTROL_NAMES, adapter.predecessor.CONTROL_NAMES)
        self.assertEqual(adapter.CONTROL_FILTER, adapter.predecessor.CONTROL_FILTER)
        self.assertEqual(tuple("frontend::oir::unit2_observer::" + name.decode() for name in re.findall(rb"^fn ([A-Za-z_0-9]+)\(", adapter.CONTROL_TESTS, re.M)), adapter.INTEGER_CONTROL_NAMES)

    def test_input_drift_readaptation_and_successor_pin_drift_are_rejected(self):
        for body in (self.original[:-1], self.original + b"\n", adapter.adapt(self.original)):
            with self.subTest(length=len(body)), self.assertRaises(ValueError):
                adapter.adapt(body)
        with patch.object(adapter, "DERIVED_SHA", "0" * 64), self.assertRaisesRegex(ValueError, "derived"):
            adapter.adapt(self.original)
        with patch.object(adapter, "SEAMS", adapter.SEAMS[:-1]), self.assertRaisesRegex(ValueError, "count"):
            adapter.adapt(self.original)

    def test_parent_authenticates_before_python_execution(self):
        controller = Path(adapter.__file__).read_bytes()
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            for body in ((ROOT / "scripts" / adapter.PARENT).read_bytes() + b"\n", b'raise RuntimeError("untrusted executed")\n'):
                (root / adapter.PARENT).write_bytes(body)
                with self.assertRaisesRegex(ValueError, "changed pinned"):
                    exec(compile(controller, str(root / "adapter.py"), "exec"), {"__file__": str(root / "adapter.py")})

if __name__ == "__main__":
    unittest.main()
