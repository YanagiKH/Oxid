"""Independent reversible boundary and protocol controls; no Rust execution."""
import hashlib
import importlib.util
import inspect
from pathlib import Path
import re
import tempfile
import unittest
from unittest.mock import patch

import record_vector_compatibility_observer as adapter
import verify_copy_record_vector_compatibility as gate

ROOT = Path(__file__).resolve().parents[1]


class RecordVectorObserver(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        path = ROOT / gate.RETAINED
        if hashlib.sha256(path.read_bytes()).hexdigest() != gate.RETAINED_RUNNER_SHA:
            raise AssertionError("retained observer construction changed")
        spec = importlib.util.spec_from_file_location("_record_vector_observer_retained", path)
        retained = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(retained)
        original = (ROOT / retained.U2 / retained.OBSERVER).read_bytes()
        cls.original = retained.adapt_enum_unit2_observer(
            retained.adapt_borrowed_unit2_observer(retained.adapt_unit2_observer(original)))
        spec = importlib.util.spec_from_file_location("_record_vector_protocol", ROOT / retained.U2 / "protocol.py")
        cls.protocol = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(cls.protocol)

    def test_new_three_arm_projection_reverses_every_complete_predecessor_byte(self):
        inherited = adapter.predecessor.adapt(self.original)
        self.assertEqual((len(inherited), gate.sha(inherited)),
                         (27967, "3206e4d769a790ddcec4dd3d5515c90f062dd304e5ead1fe17ff07f2681e42cf"))
        current = adapter.adapt(self.original)
        self.assertEqual(adapter.pin(current), adapter.DERIVED_PIN)
        self.assertEqual(adapter.reverse(current), inherited)
        restored = current[:-len(adapter.CONTROL_TESTS)]
        inserted = []
        for before, after in reversed(adapter.SEAMS):
            self.assertEqual(restored.count(after), 1)
            self.assertEqual(after.count(before), 1)
            extra = after.replace(before, b"", 1)
            self.assertIn(b'panic!("current Unit2 observer excludes record-vector projection")', extra)
            self.assertNotIn(b"=> record", extra)
            self.assertNotIn(b"_ =>", extra)
            inserted.append(extra)
            restored = restored.replace(after, before, 1)
        self.assertEqual(restored, inherited)
        self.assertEqual(len(inserted), 3)
        for variant in (b"AggregateTy::RecordVector(_)", b"BorrowedTy::Exact(AggregateTy::RecordVector(_))",
                        b"BorrowedTy::RecordSlice(_)"):
            self.assertTrue(any(variant in extra for extra in inserted))
        self.assertIs(adapter.descriptor()["copy_record_vector_semantics_qualified"], False)

    def test_prior_control_filters_names_and_scalar_elements_stay_exact(self):
        for prefix in ("CONTROL", "FILE_CONTROL", "INTEGER_CONTROL", "WIDTH_CONTROL", "QUAD_CONTROL",
                       "FLOAT_CONTROL", "CHARACTER_CONTROL", "VECTOR_CONTROL"):
            for suffix in ("_FILTER", "_NAMES"):
                name = prefix + suffix
                self.assertEqual(getattr(adapter, name), getattr(adapter.predecessor, name))
        self.assertEqual(adapter.SCALAR_NAMES, adapter.predecessor.SCALAR_NAMES)
        self.assertEqual(len(adapter.SCALAR_NAMES), 17)
        for name in adapter.RECORD_VECTOR_CONTROL_NAMES:
            for prefix in ("CONTROL", "FILE_CONTROL", "INTEGER_CONTROL", "WIDTH_CONTROL", "QUAD_CONTROL",
                           "FLOAT_CONTROL", "CHARACTER_CONTROL", "VECTOR_CONTROL"):
                self.assertNotIn(getattr(adapter, prefix + "_FILTER"), name)

    def test_six_controls_cover_each_public_observer_route_and_each_ordinal(self):
        text = adapter.CONTROL_TESTS.decode()
        functions = re.findall(r"#\[test\]\s*fn (current_unit2_record_vector_adapter_[A-Za-z_0-9]+)\(\) \{(.*?)\n\}", text, re.S)
        self.assertEqual(tuple("frontend::oir::unit2_observer::" + name for name, _ in functions),
                         adapter.RECORD_VECTOR_CONTROL_NAMES)
        self.assertEqual(len(functions), 6)
        self.assertEqual(tuple(re.findall(r"RecordId\((\d+)\)", text)), ("0", "7", "4095"))
        self.assertNotIn("should_panic", text)
        for _, body in functions:
            self.assertIn("for record in current_unit2_record_vector_ordinals()", body)
            self.assertIn("current_unit2_record_vector_expect_rejection", body)
        self.assertIn("value_type(ValueTy::Owned(AggregateTy::RecordVector(record)))", functions[0][1])
        self.assertIn("ParameterTy::Value(ValueTy::Owned(AggregateTy::RecordVector(record)))", functions[1][1])
        for position, kind in ((2, "Shared"), (3, "Exclusive")):
            self.assertIn("BorrowedTy::Exact(AggregateTy::RecordVector(record))", functions[position][1])
            self.assertIn("BorrowKind::" + kind, functions[position][1])
        for position, kind in ((4, "Shared"), (5, "Exclusive")):
            self.assertIn("BorrowedTy::RecordSlice(record)", functions[position][1])
            self.assertIn("BorrowKind::" + kind, functions[position][1])
        self.assertIn("catch_unwind(action).expect_err", text)
        self.assertIn('assert_eq!(message, Some("current Unit2 observer excludes record-vector projection"))', text)

    def test_input_output_repetition_and_pin_mutations_are_refused(self):
        derived = adapter.adapt(self.original)
        for body in (self.original[:-1], self.original + b"\n", derived):
            with self.subTest(input=gate.sha(body)), self.assertRaises(ValueError):
                adapter.adapt(body)
        for body in (derived[:-1], b"x" + derived, derived + b"\n"):
            with self.subTest(output=gate.sha(body)), self.assertRaises(ValueError):
                adapter.reverse(body)
        with patch.object(adapter, "DERIVED_PIN", (adapter.DERIVED_BYTES, "0" * 64)), self.assertRaises(ValueError):
            adapter.adapt(self.original)

    def test_missing_reordered_and_admitting_seams_are_refused(self):
        for seams in (adapter.SEAMS[:-1], adapter.SEAMS + adapter.SEAMS[:1]):
            with patch.object(adapter, "SEAMS", seams), self.assertRaisesRegex(ValueError, "inventory"):
                adapter.adapt(self.original)
        weakened = tuple((before, after.replace(
            b'panic!("current Unit2 observer excludes record-vector projection")', b'0'))
                         for before, after in adapter.SEAMS)
        with patch.object(adapter, "SEAMS", weakened), self.assertRaises(ValueError):
            adapter.adapt(self.original)
        with patch.object(adapter, "CONTROL_TESTS", adapter.CONTROL_TESTS.replace(b"RecordId(4095)", b"RecordId(7)")), self.assertRaises(ValueError):
            adapter.adapt(self.original)
        with patch.object(adapter, "RECORD_VECTOR_CONTROL_NAMES", adapter.RECORD_VECTOR_CONTROL_NAMES[::-1]), self.assertRaisesRegex(ValueError, "inventory"):
            adapter.adapt(self.original)

    def test_predecessor_is_authenticated_before_python_execution(self):
        body = (ROOT / gate.OBSERVER_ADAPTER).read_bytes()
        with tempfile.TemporaryDirectory() as directory:
            target = Path(directory)
            marker = target / "executed"
            (target / adapter.PARENT).write_text(f"from pathlib import Path\nPath({str(marker)!r}).touch()\n")
            module = {"__file__": str(target / "record_vector_compatibility_observer.py")}
            with self.assertRaisesRegex(ValueError, "predecessor"):
                exec(compile(body, module["__file__"], "exec"), module)
            self.assertFalse(marker.exists())

    def test_protocol_rejects_missing_extra_ignored_and_wrong_panic_results(self):
        names = list(adapter.RECORD_VECTOR_CONTROL_NAMES)
        listing = "\n".join(name + ": test" for name in names) + "\n\n6 tests, 0 benchmarks\n"
        execution = "running 6 tests\n" + "\n".join("test " + name + " ... ok" for name in names) + "\ntest result: ok. 6 passed; 0 failed; 0 ignored; 0 measured; 3 filtered out; finished in 0.01s\n"
        self.protocol.rust_listing(listing, names)
        self.assertEqual(self.protocol.rust_success(execution, names)["passed"], 6)
        for modified in (execution.replace(names[-1], "unexpected"), execution.replace(" ... ok", " ... ignored", 1),
                         execution.replace("6 passed; 0 failed; 0 ignored", "5 passed; 0 failed; 1 ignored"),
                         execution.replace(names[0] + " ...", names[0] + " - should panic ..."),
                         execution.replace("running 6 tests", "running 0 tests")):
            with self.subTest(execution=modified), self.assertRaises(self.protocol.ProtocolError):
                self.protocol.rust_success(modified, names)
        with self.assertRaises(self.protocol.ProtocolError):
            self.protocol.rust_listing(listing.replace(names[0], "unexpected"), names)

    def test_appended_runner_binds_both_binaries_sources_and_prior_census(self):
        body = inspect.getsource(gate.run_record_vector_observer_controls)
        self.assertIn('for profile in ("debug", "release")', body)
        self.assertEqual(body.count('original.check_entries(run / "source"'), 2)
        self.assertEqual(body.count('verify_current_observer(output'), 2)
        self.assertEqual(body.count('original.assert_unchanged(repo, captured)'), 2)
        self.assertEqual(body.count('sha(binary.read_bytes()) == receipt["binary_sha256"]'), 2)
        self.assertIn('"test_function_executions": previous_executions + 2 * count', body)
        self.assertNotIn("cargo", body.lower())


if __name__ == "__main__":
    unittest.main()
