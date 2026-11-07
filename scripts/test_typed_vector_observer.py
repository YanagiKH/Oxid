"""Independent closed-boundary checks; this module never invokes Rust tools."""
import hashlib
import importlib.util
import inspect
from pathlib import Path
import re
import tempfile
import unittest
from unittest.mock import patch

import typed_vector_compatibility_observer as adapter
import verify_typed_vector_compatibility as gate

ROOT = Path(__file__).resolve().parents[1]
ELEMENTS = ("I8", "I16", "I32", "I64", "I128", "U8", "U16", "U32", "U64", "U128",
            "Isize", "Usize", "F32", "F64", "Bool", "Char", "Unit")


class TypedVectorObserver(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        path = ROOT / "tests/fixtures/typed_project_source_binding_byte_storage_v1/run.py"
        assert hashlib.sha256(path.read_bytes()).hexdigest() == "584e8ee937cb2d3ef62d59d3eb69a79bfb6afce23df86de3df305f81afae7c76"
        spec = importlib.util.spec_from_file_location("_vector_observer_retained", path)
        retained = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(retained)
        original = (ROOT / retained.U2 / retained.OBSERVER).read_bytes()
        cls.original = retained.adapt_enum_unit2_observer(
            retained.adapt_borrowed_unit2_observer(retained.adapt_unit2_observer(original)))
        protocol_path = ROOT / retained.U2 / "protocol.py"
        spec = importlib.util.spec_from_file_location("_vector_control_protocol", protocol_path)
        cls.protocol = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(cls.protocol)

    def test_two_explicit_vector_refusals_reverse_to_every_predecessor_byte(self):
        inherited = adapter.predecessor.adapt(self.original)
        result = adapter.adapt(self.original)
        self.assertEqual((len(inherited), gate.sha(inherited)),
                         (25912, "05776a5b0712d0f84b1604533458e8442afc5dbbb76b19f6bee406adf2642823"))
        self.assertEqual((len(result), gate.sha(result)), (adapter.DERIVED_BYTES, adapter.DERIVED_SHA))
        self.assertTrue(result.endswith(adapter.CONTROL_TESTS))
        restored = result[:-len(adapter.CONTROL_TESTS)]
        self.assertEqual(len(adapter.SEAMS), 2)
        for old, new in reversed(adapter.SEAMS):
            self.assertEqual(restored.count(new), 1)
            self.assertIn(old, new)
            inserted = new.replace(old, b"", 1)
            self.assertIn(b"AggregateTy::Vector(_)", inserted)
            self.assertIn(b'panic!("current Unit2 observer excludes vector projection")', inserted)
            self.assertNotIn(b"=> record", inserted)
            restored = restored.replace(new, old, 1)
        self.assertEqual(restored, inherited)
        for prefix in ("CONTROL", "FILE_CONTROL", "INTEGER_CONTROL", "WIDTH_CONTROL",
                       "QUAD_CONTROL", "FLOAT_CONTROL", "CHARACTER_CONTROL"):
            for suffix in ("_FILTER", "_NAMES"):
                self.assertEqual(getattr(adapter, prefix + suffix), getattr(adapter.predecessor, prefix + suffix))
        self.assertIs(adapter.descriptor()["vector_semantics_qualified"], False)

    def test_four_controls_cover_all_seventeen_elements_without_early_panic_exit(self):
        self.assertEqual(adapter.SCALAR_NAMES, ELEMENTS)
        text = adapter.CONTROL_TESTS.decode()
        array = re.search(r"fn current_unit2_vector_elements\(\) -> \[hir::Ty; 17\] \{\s*\[(.*?)\]", text, re.S).group(1)
        self.assertEqual(tuple(re.findall(r"hir::Ty::([A-Za-z0-9]+)", array)), ELEMENTS)
        functions = re.findall(r"#\[test\]\s*fn (current_unit2_vector_adapter_[A-Za-z_0-9]+)\(\) \{(.*?)\n\}", text, re.S)
        self.assertEqual(tuple("frontend::oir::unit2_observer::" + name for name, _ in functions), adapter.VECTOR_CONTROL_NAMES)
        self.assertEqual(len(functions), 4)
        self.assertNotIn("should_panic", text)
        for _, body in functions:
            self.assertIn("for element in current_unit2_vector_elements()", body)
            self.assertIn("current_unit2_vector_expect_rejection", body)
            self.assertIn("AggregateTy::Vector(element)", body)
        for route in ("value_type(ValueTy::Owned", "ParameterTy::Value(ValueTy::Owned", "BorrowKind::Shared", "BorrowKind::Exclusive"):
            self.assertIn(route, text)
        self.assertIn("catch_unwind(action).expect_err", text)
        self.assertIn('assert_eq!(message, Some("current Unit2 observer excludes vector projection"))', text)

    def test_input_seam_control_and_output_pin_mutations_are_rejected(self):
        for body in (self.original[:-1], self.original + b"\n", adapter.adapt(self.original)):
            with self.subTest(input_sha=gate.sha(body)), self.assertRaises(ValueError):
                adapter.adapt(body)
        with patch.object(adapter, "SEAMS", adapter.SEAMS[:-1]), self.assertRaisesRegex(ValueError, "inventory"):
            adapter.adapt(self.original)
        weakened = tuple((old, new.replace(b'panic!("current Unit2 observer excludes vector projection")', b'0')) for old, new in adapter.SEAMS)
        with patch.object(adapter, "SEAMS", weakened), self.assertRaisesRegex(ValueError, "derived"):
            adapter.adapt(self.original)
        changed = adapter.CONTROL_TESTS.replace(b"hir::Ty::Char", b"hir::Ty::I32")
        with patch.object(adapter, "CONTROL_TESTS", changed), self.assertRaisesRegex(ValueError, "derived"):
            adapter.adapt(self.original)
        with patch.object(adapter, "VECTOR_CONTROL_NAMES", adapter.VECTOR_CONTROL_NAMES[:-1]), self.assertRaisesRegex(ValueError, "inventory"):
            adapter.adapt(self.original)
        with patch.object(adapter, "DERIVED_SHA", "0" * 64), self.assertRaisesRegex(ValueError, "derived"):
            adapter.adapt(self.original)

    def test_adapter_parent_authenticates_before_python_execution(self):
        controller = Path(adapter.__file__).read_bytes()
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            for body in ((ROOT / "scripts" / adapter.PARENT).read_bytes() + b"\n", b'raise RuntimeError("untrusted executed")\n'):
                (root / adapter.PARENT).write_bytes(body)
                with self.assertRaisesRegex(ValueError, "changed pinned"):
                    exec(compile(controller, str(root / "adapter.py"), "exec"), {"__file__": str(root / "adapter.py")})

    def test_controller_authenticates_new_adapter_before_execution(self):
        body = (ROOT / gate.OBSERVER_ADAPTER).read_bytes()
        self.assertEqual((len(body), gate.sha(body)), (gate.OBSERVER_ADAPTER_BYTES, gate.OBSERVER_ADAPTER_SHA))
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            path = root / gate.OBSERVER_ADAPTER
            path.parent.mkdir()
            for invalid in (body + b"\n", b'raise RuntimeError("untrusted executed")\n'):
                path.write_bytes(invalid)
                with self.assertRaisesRegex(ValueError, "changed pinned typed-vector"):
                    gate.authenticate_observer_adapter(root)

    def parser_resource(self):
        path = ROOT / "tests/fixtures/typed_project_unit2_independent/archive/resource/parser-resource-review-tests.rs"
        body = path.read_bytes()
        self.assertEqual((len(body), gate.sha(body)), (1951, "5a26942d2864979a430b28aa5714ceea43edf8e80cf9a6f7c789d66c12a11b73"))
        old = b"mode:SourceMode::ProjectCandidate,tokens,"
        new = b"mode:SourceMode::ProjectCandidate,project_recovery:false,arrays:ArraySyntaxPolicy::Closed,enums:EnumSyntaxPolicy::Closed,std_imports:StdImportPolicy::Closed,storage:enums::SyntaxStorage::default(),tokens,"
        self.assertEqual(body.count(old), 1)
        return body.replace(old, new, 1)

    def test_only_unparsed_named_slice_fact_is_added_and_all_old_assertions_remain(self):
        inherited = gate.library.adapt_parser_resource(self.parser_resource())
        result = gate.adapt_parser_resource(inherited)
        self.assertEqual((len(inherited), gate.sha(inherited)), gate.PARSER_RESOURCE_INPUT)
        self.assertEqual((len(result), gate.sha(result)), gate.PARSER_RESOURCE_OUTPUT)
        self.assertEqual(result.replace(gate.PARSER_RESOURCE_AFTER, gate.PARSER_RESOURCE_BEFORE, 1), inherited)
        self.assertEqual(result.replace(b"parsed_has_named_slice:false,", b"", 1), inherited)
        self.assertEqual(result.count(b"parsed_has_named_slice:false,"), 1)
        self.assertNotIn(b"parsed_has_named_slice:true", result)
        self.assertIn(b'"crate".into()', result)
        self.assertIn(b"cursor:0,expressions:Vec::new()", result)
        self.assertEqual(result.count(b"assert"), inherited.count(b"assert"))
        for body in (inherited + b"\n", inherited.replace(b"assert_eq!", b"assert_ne!", 1), result):
            with self.assertRaisesRegex(ValueError, "parser resource"):
                gate.adapt_parser_resource(body)
        with patch.object(gate, "PARSER_RESOURCE_AFTER", gate.PARSER_RESOURCE_AFTER.replace(b"slice:false", b"slice:true")), self.assertRaisesRegex(ValueError, "successor"):
            gate.adapt_parser_resource(inherited)

    def test_parser_wrapper_uses_exact_parent_and_successor_without_mutating_capture(self):
        # The new index witness is separately pinned; this test still proves
        # the exact preceding parser transform and capture immutability.
        from unittest.mock import Mock
        index_adapter = Mock()
        index_adapter.RESOURCE = "archive/resource/resource-review-tests.rs"
        index_adapter.adapt.return_value = b"derived index fixture"
        index_adapter.descriptor.return_value = {"scope": "independent index model"}
        captured = {"resource": self.parser_resource(), "sentinel": "untouched",
                    "u8_index_resource": b"preceding index fixture"}
        before = dict(captured)
        expected = gate.adapt_parser_resource(gate.library.adapt_parser_resource(captured["resource"]))
        current = dict(captured, u8_index_resource=b"derived index fixture")
        selected = object()
        with tempfile.TemporaryDirectory() as directory:
            package = Path(directory)
            path = package / index_adapter.RESOURCE
            path.parent.mkdir(parents=True)
            path.write_bytes(b"derived index fixture")
            with patch.object(gate.library, "prepare_with_parser_resource", return_value=(selected, {"resource_package_root": directory})) as prepare:
                result, seam = gate.prepare_current_observer("original", "output", captured, "adapter", index_adapter)
        self.assertIs(result, selected)
        self.assertEqual(captured, before)
        index_adapter.adapt.assert_called_once_with(captured["u8_index_resource"])
        prepare.assert_called_once_with("original", "output", current, "adapter", expected)
        self.assertEqual(seam["typed_vector_parser_resource_adapter"]["after"], gate.identity(gate.PARSER_RESOURCE, expected))
        self.assertEqual(seam["typed_vector_index_resource_adapter"], index_adapter.descriptor.return_value)

    def test_vector_result_protocol_rejects_missing_extra_ignored_and_wrong_panic_controls(self):
        names = list(adapter.VECTOR_CONTROL_NAMES)
        listing = "\n".join(name + ": test" for name in names) + "\n\n4 tests, 0 benchmarks\n"
        execution = "running 4 tests\n" + "\n".join("test " + name + " ... ok" for name in names) + "\ntest result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 3 filtered out; finished in 0.01s\n"
        self.assertEqual(gate.verify_vector_control_output(self.protocol, listing, execution, adapter)["passed"], 4)
        for modified in (execution.replace(names[-1], "unexpected"), execution.replace(" ... ok", " ... ignored", 1), execution.replace("4 passed; 0 failed; 0 ignored", "3 passed; 0 failed; 1 ignored"), execution.replace(names[0] + " ...", names[0] + " - should panic ..."), execution.replace("running 4 tests", "running 0 tests")):
            with self.subTest(execution=modified), self.assertRaises(self.protocol.ProtocolError):
                gate.verify_vector_control_output(self.protocol, listing, modified, adapter)
        with self.assertRaises(self.protocol.ProtocolError):
            gate.verify_vector_control_output(self.protocol, listing.replace(names[0], "unexpected"), execution, adapter)

    def test_runner_binds_both_profile_binaries_source_and_prior_census(self):
        body = inspect.getsource(gate.run_vector_observer_controls)
        self.assertIn('for profile in ("debug", "release")', body)
        self.assertEqual(body.count('original.check_entries(run / "source"'), 2)
        self.assertEqual(body.count('verify_current_observer(output'), 2)
        self.assertEqual(body.count('original.assert_unchanged(repo, captured)'), 2)
        self.assertEqual(body.count('sha(binary.read_bytes()) == receipt["binary_sha256"]'), 2)
        self.assertIn('"test_function_executions": previous_executions + 2 * count', body)
        self.assertIn('verify_vector_control_output(protocol, streams["list"], streams["run"], adapter)', body)
        self.assertNotIn('cargo', body.lower())


if __name__ == "__main__":
    unittest.main()
