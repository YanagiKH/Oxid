"""Exact current Unit2D carrier controls; no Rust or native execution."""
import copy
from pathlib import Path
import unittest
from unittest.mock import patch

import replay_fixed_array_unit2d as frozen
import replay_fixed_array_unit2d_current as adapter


class CurrentCarrierTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        fixture = Path(frozen.__file__).resolve().parents[1] / frozen.FIXTURE_REL
        reviewer = (fixture / frozen.PUBLIC_ARRAY_ACTIVATION["frozen_module_path"]).read_bytes()
        cls.inputs = {"reviewer": frozen.projected_loan_reviewer(
            frozen.borrowed_slot_reviewer(frozen.public_array_reviewer(reviewer)))}
        for name in frozen.CONTROL_FILES:
            raw = (fixture / "sources" / name).read_bytes()
            cls.inputs[name] = frozen.old_ir_resource_exporter(raw) if name == "old-ir-export-v1.rs" else raw

    def test_stdin_successors_preserve_all_exact_enum_predecessors(self):
        self.assertEqual(set(adapter.STDIN_IDENTITIES), set(self.inputs))
        counts = {"reviewer": 6, "checkpoint1-heldout-v1.rs": 1,
                  "checkpoint1-partial-peak-v1.rs": 1, "checkpoint1-early-denial-v1.rs": 1,
                  "old-ir-export-v1.rs": 0}
        for name, raw in self.inputs.items():
            with self.subTest(name=name):
                predecessor = adapter.enum_carrier_bytes(name, raw)
                current = adapter.stdin_carrier_bytes(name, predecessor)
                self.assertEqual(adapter.STDIN_IDENTITIES[name][0], adapter.IDENTITIES[name][1])
                self.assertEqual(current.count(b"BuiltinOrigins::None"), counts[name])
                # Independent reversal removes only the new explicit carrier.
                restored = current.replace(b"            builtins: BuiltinOrigins::None,\n", b"")
                restored = restored.replace(b"        builtins: BuiltinOrigins::None,\n", b"")
                restored = restored.replace(b"builtins:BuiltinOrigins::None,", b"")
                restored = restored.replace(b"builtins: BuiltinOrigins::None, ", b"")
                self.assertEqual(restored, predecessor)
                self.assertEqual(adapter.stdin_carrier_bytes(name, current, reverse=True), predecessor)
                self.assertEqual(adapter.enum_carrier_bytes(name, predecessor, reverse=True), raw)
                if counts[name]:
                    self.assertNotEqual(current, predecessor)
                else:
                    self.assertEqual(current, predecessor)

    def test_stdin_carrier_rejects_drift_double_application_and_unapproved_builtin(self):
        for name, raw in self.inputs.items():
            predecessor = adapter.enum_carrier_bytes(name, raw)
            current = adapter.stdin_carrier_bytes(name, predecessor)
            with self.subTest(name=name):
                for changed in (predecessor + b"\n", predecessor.replace(b"#[test]", b"#[ignore]", 1)):
                    with self.assertRaisesRegex(ValueError, "stdin adapter input identity differs"):
                        adapter.stdin_carrier_bytes(name, changed)
                if adapter.STDIN_REPLACEMENTS[name]:
                    with self.assertRaisesRegex(ValueError, "stdin adapter input identity differs"):
                        adapter.stdin_carrier_bytes(name, current)
                    with self.assertRaisesRegex(ValueError, "stdin adapter input identity differs"):
                        adapter.stdin_carrier_bytes(name, current.replace(b"BuiltinOrigins::None", b"BuiltinOrigins::ReadStdin"), reverse=True)

    def test_stdin_carrier_rejects_changed_substitution_count_and_output(self):
        name = "reviewer"
        predecessor = adapter.enum_carrier_bytes(name, self.inputs[name])
        replacements = adapter.STDIN_REPLACEMENTS[name]
        old, new, count = replacements[0]
        for changed, message in (((old, new, count + 1), "replacement count"),
                                 ((old, new.replace(b"::None", b"::ReadStatus"), count), "output identity")):
            with patch.dict(adapter.STDIN_REPLACEMENTS, {name: (changed,) + replacements[1:]}):
                with self.assertRaisesRegex(ValueError, "stdin adapter " + message + " differs"):
                    adapter.stdin_carrier_bytes(name, predecessor)

    def test_runner_successor_is_exact_and_reversible_at_each_stage(self):
        original = Path(frozen.__file__).read_bytes()
        enum = adapter.current_runner_bytes(original)
        current = adapter.stdin_runner_bytes(enum)
        self.assertEqual(adapter.digest(original), adapter.FROZEN_RUNNER_SHA)
        self.assertEqual(adapter.digest(enum), adapter.CURRENT_RUNNER_SHA)
        self.assertEqual(adapter.digest(current), adapter.STDIN_RUNNER_SHA)
        self.assertEqual(adapter.stdin_runner_bytes(current, reverse=True), enum)
        self.assertEqual(adapter.current_runner_bytes(enum, reverse=True), original)
        for changed in (original, enum + b"\n", current):
            with self.assertRaisesRegex(ValueError, "stdin adapter input identity differs"):
                adapter.stdin_runner_bytes(changed)

    def test_loaded_runner_binds_both_carriers_and_keeps_historical_rosters(self):
        runner = adapter.load_runner()
        for name in ("ORDINARY", "NATIVE", "CONTROL_FILES", "PROVENANCE", "OLD_IR_RESOURCE_SUCCESSOR"):
            self.assertEqual(getattr(runner, name), getattr(frozen, name))
        receipt = runner.STDIN_CARRIER_COMPATIBILITY
        self.assertEqual(receipt["predecessor_adapter"], adapter.VERSION)
        self.assertEqual(receipt["predecessor_runner_sha256"], adapter.CURRENT_RUNNER_SHA)
        self.assertEqual(receipt["runner_sha256"], adapter.STDIN_RUNNER_SHA)
        self.assertEqual({name: sum(s["count"] for s in rows) for name, rows in receipt["substitutions"].items()},
                         {"reviewer": 6, "checkpoint1-heldout-v1.rs": 1, "checkpoint1-partial-peak-v1.rs": 1,
                          "checkpoint1-early-denial-v1.rs": 1, "old-ir-export-v1.rs": 0})
        binding = {
            "public_array_activation": runner.PUBLIC_ARRAY_ACTIVATION,
            "borrowed_slot_compatibility": runner.BORROWED_SLOT_COMPATIBILITY,
            "projected_loan_compatibility": runner.PROJECTED_LOAN_COMPATIBILITY,
            "old_ir_resource_successor": runner.OLD_IR_RESOURCE_SUCCESSOR,
            "enum_carrier_compatibility": runner.ENUM_CARRIER_COMPATIBILITY,
            "enum_adapter_sha256": runner.ENUM_ADAPTER_SHA,
            "stdin_carrier_compatibility": receipt,
            "stdin_adapter_sha256": runner.STDIN_ADAPTER_SHA,
            "module_sha256": adapter.STDIN_IDENTITIES["reviewer"][1],
        }
        runner.assert_current_module_binding(binding)
        for field in ("stdin_carrier_compatibility", "stdin_adapter_sha256", "enum_carrier_compatibility"):
            altered = copy.deepcopy(binding)
            del altered[field]
            with self.assertRaisesRegex(RuntimeError, "current public-array module binding differs"):
                runner.assert_current_module_binding(altered)
        for module_sha in adapter.IDENTITIES["reviewer"]:
            with self.assertRaisesRegex(RuntimeError, "current public-array module binding differs"):
                runner.assert_current_module_binding({**binding, "module_sha256": module_sha})


if __name__ == "__main__":
    unittest.main()
