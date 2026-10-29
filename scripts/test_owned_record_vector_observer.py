"""Source/protocol controls for the observer successor; no Rust execution here."""
from contextlib import contextmanager
import hashlib
import importlib.util
import inspect
import json
from pathlib import Path
import re
import tempfile
import types
import unittest
from unittest.mock import Mock, patch

import owned_record_vector_compatibility_observer as adapter
import verify_owned_record_vector_compatibility as gate

ROOT = Path(__file__).resolve().parents[1]


class OwnedRecordVectorObserver(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        path = ROOT / gate.RETAINED
        if hashlib.sha256(path.read_bytes()).hexdigest() != gate.RETAINED_RUNNER_SHA:
            raise AssertionError("retained observer construction changed")
        spec = importlib.util.spec_from_file_location("_owned_vector_observer_retained", path)
        retained = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(retained)
        original = (ROOT / retained.U2 / retained.OBSERVER).read_bytes()
        cls.original = retained.adapt_enum_unit2_observer(
            retained.adapt_borrowed_unit2_observer(retained.adapt_unit2_observer(original)))
        cls.protocol_body = (ROOT / retained.U2 / "protocol.py").read_bytes()
        cls.protocol = types.ModuleType("_owned_vector_observer_protocol")
        exec(compile(cls.protocol_body, "protocol.py", "exec"), cls.protocol.__dict__)

    def test_two_explicit_exclusions_reverse_the_complete_v12_body(self):
        inherited = adapter.predecessor.adapt(self.original)
        self.assertEqual(adapter.pin(inherited),
                         (30768, "96b8c45c257740a56b550ee24287bede07d3ca4ecd728ff5d40903cefc8b84d0"))
        current = adapter.adapt(self.original)
        self.assertEqual(adapter.pin(current),
                         (32684, "8e65804b8486ec2fda4bca21875c3c296f0b722bd6c40041ee1144b4c44e6042"))
        self.assertEqual(adapter.reverse(current), inherited)
        expected = (
            b'        AggregateTy::OwnedRecordVector(_) => panic!("current Unit2 observer excludes owned-record-vector projection"),\n',
            b'        BorrowedTy::Exact(AggregateTy::OwnedRecordVector(_)) => panic!("current Unit2 observer excludes owned-record-vector projection"),\n',
        )
        restored = current[:-len(adapter.CONTROL_TESTS)]
        for (before, after), extra in zip(adapter.SEAMS, expected, strict=True):
            self.assertEqual(after, before + extra)
            self.assertEqual(restored.count(after), 1)
            self.assertNotIn(b"_ =>", extra)
            restored = restored.replace(after, before, 1)
        self.assertEqual(restored, inherited)
        self.assertIs(adapter.descriptor()["owned_record_vector_semantics_qualified"], False)

    def test_all_prior_filters_names_and_scalar_domains_stay_identical(self):
        for prefix in ("CONTROL", "FILE_CONTROL", "INTEGER_CONTROL", "WIDTH_CONTROL", "QUAD_CONTROL",
                       "FLOAT_CONTROL", "CHARACTER_CONTROL", "VECTOR_CONTROL", "RECORD_VECTOR_CONTROL"):
            for suffix in ("_FILTER", "_NAMES"):
                name = prefix + suffix
                self.assertEqual(getattr(adapter, name), getattr(adapter.predecessor, name))
            for name in adapter.OWNED_RECORD_VECTOR_CONTROL_NAMES:
                self.assertNotIn(getattr(adapter, prefix + "_FILTER"), name)
        self.assertEqual(adapter.SCALAR_NAMES, adapter.predecessor.SCALAR_NAMES)
        self.assertEqual(len(adapter.SCALAR_NAMES), 17)

    def test_three_real_rust_controls_check_each_route_and_record_ordinal(self):
        text = adapter.CONTROL_TESTS.decode("ascii")
        functions = re.findall(r"#\[test\]\s*fn (current_unit2_owned_record_vector_adapter_[A-Za-z_0-9]+)\(\) \{(.*?)\n\}", text, re.S)
        self.assertEqual(tuple("frontend::oir::unit2_observer::" + name for name, _ in functions),
                         adapter.OWNED_RECORD_VECTOR_CONTROL_NAMES)
        self.assertEqual(len(functions), 3)
        self.assertEqual(tuple(re.findall(r"RecordId\((\d+)\)", text)), ("0", "7", "4095"))
        self.assertNotIn("should_panic", text)
        for _, body in functions:
            self.assertIn("for record in current_unit2_owned_record_vector_ordinals()", body)
            self.assertIn("current_unit2_owned_record_vector_expect_rejection", body)
        self.assertIn("ParameterTy::Value(ValueTy::Owned(AggregateTy::OwnedRecordVector(record)))", functions[0][1])
        self.assertIn(b"ParameterTy::Value(value) => value_type(value)", adapter.reverse(adapter.adapt(self.original)))
        for position, kind in ((1, "Shared"), (2, "Exclusive")):
            self.assertIn("BorrowedTy::Exact(AggregateTy::OwnedRecordVector(record))", functions[position][1])
            self.assertIn("BorrowKind::" + kind, functions[position][1])
        self.assertIn("catch_unwind(action).expect_err", text)
        self.assertIn('assert_eq!(message, Some("current Unit2 observer excludes owned-record-vector projection"))', text)

    def test_input_output_repetition_inventory_and_hash_drift_are_refused(self):
        derived = adapter.adapt(self.original)
        for body in (self.original[:-1], self.original + b"\n", derived):
            with self.subTest(input=adapter.pin(body)), self.assertRaises(ValueError):
                adapter.adapt(body)
        for body in (derived[:-1], b"x" + derived, derived + b"\n"):
            with self.subTest(output=adapter.pin(body)), self.assertRaises(ValueError):
                adapter.reverse(body)
        for seams in (adapter.SEAMS[:-1], adapter.SEAMS + adapter.SEAMS[:1]):
            with patch.object(adapter, "SEAMS", seams), self.assertRaisesRegex(ValueError, "inventory"):
                adapter.adapt(self.original)
        with patch.object(adapter, "DERIVED_PIN", (adapter.DERIVED_BYTES, "0" * 64)), self.assertRaises(ValueError):
            adapter.adapt(self.original)
        with patch.object(adapter, "OWNED_RECORD_VECTOR_CONTROL_NAMES", adapter.OWNED_RECORD_VECTOR_CONTROL_NAMES[::-1]), self.assertRaises(ValueError):
            adapter.adapt(self.original)

    def test_coherently_repinned_output_cannot_change_any_old_body_byte(self):
        derived = adapter.adapt(self.original)
        changed = derived.replace(b"fn q(value: &str)", b"fn q(value: &mut str)", 1)
        self.assertNotEqual(changed, derived)
        with patch.object(adapter, "DERIVED_PIN", adapter.pin(changed)), self.assertRaisesRegex(ValueError, "predecessor bytes"):
            adapter.reverse(changed)
        changed = derived.replace(b"RecordId(4095)", b"RecordId(4094)", 1)
        with patch.object(adapter, "DERIVED_PIN", adapter.pin(changed)), self.assertRaises(ValueError):
            adapter.reverse(changed)

    def test_outer_adapter_pin_refuses_coherent_source_changes_before_execution(self):
        body = (ROOT / gate.OWNED_VECTOR_OBSERVER_ADAPTER).read_bytes()
        self.assertEqual(adapter.pin(body),
                         (7268, "ac49c38f23b0d3a5929fe3d52edae1cb85341c72b16249522e30dc155e1fa8f3"))
        self.assertEqual(adapter.pin(body), gate.OWNED_VECTOR_OBSERVER_ADAPTER_PIN)
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            marker = root / "executed"
            changed = body.replace(adapter.DERIVED_SHA.encode(), b"0" * 64)
            changed += ("\nPath(" + repr(str(marker)) + ").write_bytes(b'executed')\n").encode()
            path = root / gate.OWNED_VECTOR_OBSERVER_ADAPTER
            path.parent.mkdir()
            path.write_bytes(changed)
            with patch.object(gate, "authenticate_observer_adapter", return_value=(adapter.predecessor, b"parent")), \
                    self.assertRaisesRegex(ValueError, "changed pinned owned-vector observer adapter"):
                gate.authenticate_owned_vector_observer_adapter(root)
            self.assertFalse(marker.exists())

    def test_predecessor_pin_is_checked_before_loading_python(self):
        body = (ROOT / gate.OWNED_VECTOR_OBSERVER_ADAPTER).read_bytes()
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            marker = root / "parent-executed"
            (root / adapter.PARENT).write_text("from pathlib import Path\nPath(" + repr(str(marker)) + ").touch()\n")
            module = {"__file__": str(root / "owned_record_vector_compatibility_observer.py")}
            with self.assertRaisesRegex(ValueError, "predecessor"):
                exec(compile(body, module["__file__"], "exec"), module)
            self.assertFalse(marker.exists())

    def test_protocol_rejects_missing_extra_ignored_wrong_panic_and_zero_runs(self):
        names = list(adapter.OWNED_RECORD_VECTOR_CONTROL_NAMES)
        listing = "\n".join(name + ": test" for name in names) + "\n\n3 tests, 0 benchmarks\n"
        execution = "running 3 tests\n" + "\n".join("test " + name + " ... ok" for name in names) + "\ntest result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 1 filtered out; finished in 0.01s\n"
        self.protocol.rust_listing(listing, names)
        self.assertEqual(self.protocol.rust_success(execution, names)["passed"], 3)
        for changed in (execution.replace(names[-1], "unexpected"), execution.replace(" ... ok", " ... ignored", 1),
                        execution.replace("3 passed; 0 failed; 0 ignored", "2 passed; 0 failed; 1 ignored"),
                        execution.replace(names[0] + " ...", names[0] + " - should panic ..."),
                        execution.replace("running 3 tests", "running 0 tests")):
            with self.subTest(execution=changed), self.assertRaises(self.protocol.ProtocolError):
                self.protocol.rust_success(changed, names)
        with self.assertRaises(self.protocol.ProtocolError):
            self.protocol.rust_listing(listing.replace(names[0], "unexpected"), names)

    @contextmanager
    def control_fixture(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            output = root / "output"
            run = output / "unit2/run"
            source = run / "source"
            (source / "src/frontend/oir").mkdir(parents=True)
            observer = adapter.adapt(self.original)
            (source / "src/frontend/oir/unit2_observer.rs").write_bytes(observer)
            protocol = root / "protocol"
            protocol.mkdir()
            (protocol / "protocol.py").write_bytes(self.protocol_body)
            (run / "assembly.json").write_text(json.dumps({"files": []}))
            binaries = {}
            for profile in ("debug", "release"):
                binary = run / "target" / profile / "observer"
                binary.parent.mkdir(parents=True)
                binary.write_bytes(b"existing-paid-binary-" + profile.encode())
                binaries[profile] = binary
                (run / (profile + "-receipt.json")).write_text(json.dumps({
                    "binary": str(binary), "binary_sha256": gate.sha(binary.read_bytes())}))
            captured = {"observer": observer, "historical_bytes": {"protocol.py": self.protocol_body}}
            seam = {"resource_package_root": str(protocol)}
            original = types.SimpleNamespace(assert_unchanged=Mock(), check_entries=Mock(),
                verify_unit2_result=Mock(), CURRENT_SOURCE_SHA="actual-bound-source-sha")
            names = list(adapter.OWNED_RECORD_VECTOR_CONTROL_NAMES)
            def process(argv, *, cwd, stdout, stderr, timeout):
                self.assertIn(Path(argv[0]), binaries.values())
                self.assertEqual(cwd, source)
                self.assertEqual(timeout, 60)
                if "--list" in argv:
                    text = "\n".join(name + ": test" for name in names) + "\n\n3 tests, 0 benchmarks\n"
                else:
                    text = "running 3 tests\n" + "\n".join("test " + name + " ... ok" for name in names) + "\ntest result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; 1 filtered out; finished in 0.01s\n"
                stdout.write(text.encode())
                return types.SimpleNamespace(returncode=0)
            yield root, output, run, captured, seam, original, binaries, process

    def test_controls_use_both_existing_binaries_and_keep_the_inherited_census(self):
        with self.control_fixture() as (_, output, _, captured, seam, original, binaries, process):
            with patch.object(gate.library.subprocess, "run", side_effect=process) as invoked:
                result = gate.run_owned_vector_observer_controls(ROOT, output, captured, seam, original, adapter, 200)
            self.assertEqual(invoked.call_count, 4)
            self.assertEqual([Path(call.args[0][0]) for call in invoked.call_args_list],
                             [binaries["debug"], binaries["debug"], binaries["release"], binaries["release"]])
            self.assertEqual(result["inherited_test_function_executions"], 200)
            self.assertEqual(result["test_function_executions"], 206)
            self.assertEqual(result["owned_record_vector_observer_control_tests_per_profile"], 3)
            self.assertEqual(result["owned_record_vector_observer_denials_per_profile"], 9)
            self.assertEqual(len(result["owned_record_vector_observer_control_receipts"]), 2)
            self.assertEqual(original.check_entries.call_count, 4)
            original.verify_unit2_result.assert_called_once_with(output, captured, seam, False)

    def test_wrong_binary_scope_or_hash_refuses_before_any_process(self):
        for variant in ("scope", "hash"):
            with self.subTest(variant=variant), self.control_fixture() as (root, output, run, captured, seam, original, binaries, _):
                receipt_path = run / "debug-receipt.json"
                receipt = json.loads(receipt_path.read_bytes())
                if variant == "scope":
                    outside = root / "outside-binary"
                    outside.write_bytes(binaries["debug"].read_bytes())
                    receipt["binary"] = str(outside)
                else:
                    receipt["binary_sha256"] = "0" * 64
                receipt_path.write_text(json.dumps(receipt))
                with patch.object(gate.library.subprocess, "run") as invoked, self.assertRaises(ValueError):
                    gate.run_owned_vector_observer_controls(ROOT, output, captured, seam, original, adapter, 200)
                invoked.assert_not_called()

    def test_coherent_source_replacement_still_fails_the_fixed_observer_pin(self):
        with self.control_fixture() as (_, output, run, captured, seam, original, _, _):
            changed = captured["observer"].replace(b"RecordId(4095)", b"RecordId(4094)", 1)
            captured["observer"] = changed
            (run / "source/src/frontend/oir/unit2_observer.rs").write_bytes(changed)
            with patch.object(gate.library.subprocess, "run") as invoked, self.assertRaisesRegex(ValueError, "pinned successor"):
                gate.run_owned_vector_observer_controls(ROOT, output, captured, seam, original, adapter, 200)
            invoked.assert_not_called()

    def test_coherent_binary_and_receipt_replacement_after_execution_is_refused(self):
        with self.control_fixture() as (_, output, run, captured, seam, original, binaries, process):
            def mutate(argv, **kwargs):
                result = process(argv, **kwargs)
                if "--list" not in argv:
                    binary = binaries["debug"]
                    binary.write_bytes(b"replacement-binary")
                    (run / "debug-receipt.json").write_text(json.dumps({
                        "binary": str(binary), "binary_sha256": gate.sha(binary.read_bytes())}))
                return result
            with patch.object(gate.library.subprocess, "run", side_effect=mutate), self.assertRaisesRegex(ValueError, "binary changed"):
                gate.run_owned_vector_observer_controls(ROOT, output, captured, seam, original, adapter, 200)
            self.assertFalse((output / "owned-record-vector-observer-controls/debug-receipt.json").exists())

    def test_control_runner_has_no_build_path_and_keeps_all_before_after_guards(self):
        body = inspect.getsource(gate.run_owned_vector_observer_controls)
        self.assertIn('for profile in ("debug", "release")', body)
        self.assertEqual(body.count('original.check_entries(run / "source"'), 2)
        self.assertEqual(body.count('verify_current_observer(output'), 2)
        self.assertEqual(body.count('original.assert_unchanged(repo, captured)'), 2)
        self.assertEqual(body.count('sha(binary.read_bytes()) == receipt["binary_sha256"]'), 2)
        self.assertIn('"test_function_executions": previous_executions + 2 * count', body)
        self.assertNotIn("cargo", body.lower())


if __name__ == "__main__":
    unittest.main()
