"""Closed source admission and unchanged inherited replay delegation controls."""
import argparse
from contextlib import redirect_stderr
import inspect
import io
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

import verify_owned_record_vector_compatibility as gate


ROOT = Path(__file__).resolve().parents[1]


def baseline_names():
    body = gate.regular(ROOT, gate.BINDING + "/current-source.json")
    if gate.sha(body) != gate.BASELINE_SOURCE_SHA:
        raise AssertionError("historical source manifest changed")
    return {row["path"] for row in json.loads(body)["files"]}


class OwnedVectorSourceAdmission(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory(prefix="oxid-owned-vector-policy-")
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        self.baseline = {name: b"// fixed historical input\n" for name in baseline_names()}
        self.parent = {**self.baseline, **{name: b"// fixed registry input\n" for name in gate.PREDECESSOR_ADDED}}
        self.current = dict(self.parent)
        for name in gate.OWNED_VECTOR_CHANGED | gate.OWNED_VECTOR_ADDED:
            self.current[name] = b"#[cfg(test)]\nfn observed() {}\n"
        self.asset = next(iter(gate.OWNED_VECTOR_SOURCE_ASSETS))
        self.asset_owner, _, included, _ = gate.SOURCE_ASSET_INCLUDE_BINDINGS[self.asset]
        self.current[self.asset] = b"fn main()->i32{return 100;}\n"
        self.current[self.asset_owner] = b"#[test]\nfn first(){" + included + b";}\nfn second(){" + included + b";}\n"
        self.changed = "src/frontend/oir/owned/execute.rs"
        self.pins = {name: (len(self.current[name]), gate.sha(self.current[name]))
                     for name in gate.OWNED_VECTOR_CHANGED | gate.OWNED_VECTOR_ADDED}
        self.directives = {name: gate.source_directives(name, self.current[name]) for name in self.pins}
        for name, body in self.current.items():
            self.put(name, body)
        assets = {self.asset: (len(self.current[self.asset]), gate.sha(self.current[self.asset]))}
        for name, value in (("REVIEWED_INPUTS", self.pins), ("REVIEWED_DIRECTIVES", self.directives),
                            ("OWNED_VECTOR_SOURCE_ASSETS", assets)):
            selected = patch.object(gate, name, value)
            selected.start()
            self.addCleanup(selected.stop)

    def put(self, name, body):
        path = self.root / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(body)

    def capture(self):
        return gate.capture_sources(self.root, self.baseline, self.parent)

    def test_complete_parent_and_exact_new_roster_are_preserved(self):
        inputs = self.capture()
        self.assertEqual(len(self.baseline), 376)
        self.assertEqual(len(self.parent), 527)
        self.assertEqual(len(inputs), 572)
        self.assertEqual(len(gate.OWNED_VECTOR_CHANGED), 62)
        self.assertEqual(len(gate.OWNED_VECTOR_ADDED), 44)
        self.assertEqual(set(inputs), set(self.parent) | gate.OWNED_VECTOR_ADDED | set(gate.OWNED_VECTOR_SOURCE_ASSETS))
        self.assertEqual(gate.PREDECESSOR_ADDED, gate.library.PARENT_ADDED)
        self.assertFalse(gate.OWNED_VECTOR_ADDED & set(self.parent))

    def test_unreviewed_parent_input_and_protected_input_cannot_drift(self):
        for name in ("src/main.rs", next(iter(gate.UNCHANGED_INPUTS))):
            with self.subTest(name=name):
                self.put(name, self.current[name] + b"// drift\n")
                with self.assertRaisesRegex(ValueError, "unreviewed change"):
                    self.capture()
                self.put(name, self.current[name])

    def test_missing_parent_and_added_source_are_both_refused(self):
        for name in ("src/main.rs", next(iter(gate.OWNED_VECTOR_ADDED))):
            with self.subTest(name=name):
                path = self.root / name
                original = path.read_bytes()
                path.unlink()
                with self.assertRaisesRegex(ValueError, "membership"):
                    self.capture()
                self.put(name, original)

    def test_extra_source_empty_directory_and_symlink_are_refused(self):
        added = self.root / "src/frontend/unreviewed.rs"
        added.write_bytes(b"fn hidden() {}\n")
        with self.assertRaisesRegex(ValueError, "membership"):
            self.capture()
        added.unlink()
        directory = self.root / "src/frontend/unreviewed"
        directory.mkdir()
        with self.assertRaisesRegex(ValueError, "directory"):
            self.capture()
        directory.rmdir()
        added.symlink_to("execute.rs")
        with self.assertRaisesRegex(ValueError, "symlink"):
            self.capture()

    def test_reviewed_bytes_do_not_receive_a_blanket_delta_permission(self):
        self.put(self.changed, self.current[self.changed] + b"fn unreviewed() {}\n")
        with self.assertRaisesRegex(ValueError, "reviewed input"):
            self.capture()

    def test_directive_change_is_refused_even_with_an_updated_full_file_pin(self):
        body = self.current[self.changed] + b"#[cfg(any())]\nfn hidden() {}\n"
        self.put(self.changed, body)
        self.pins[self.changed] = (len(body), gate.sha(body))
        with self.assertRaisesRegex(ValueError, "compile-time directive"):
            self.capture()

    def test_missing_or_extra_reviewed_pin_and_directive_rows_are_refused(self):
        for name in ("REVIEWED_INPUTS", "REVIEWED_DIRECTIVES"):
            original = getattr(gate, name)
            for value in ({key: row for key, row in original.items() if key != self.changed},
                          {**original, "src/not_reviewed.rs": next(iter(original.values()))}):
                with self.subTest(name=name), patch.object(gate, name, value), self.assertRaisesRegex(ValueError, "membership"):
                    self.capture()

    def test_compiled_source_asset_has_exact_bytes_and_no_broad_data_admission(self):
        self.put(self.asset, self.current[self.asset] + b"// unreviewed input\n")
        with self.assertRaisesRegex(ValueError, "compiled source asset"):
            self.capture()
        self.put(self.asset, self.current[self.asset])
        extra = {**gate.OWNED_VECTOR_SOURCE_ASSETS, "scripts/owned_record_vector_acceptance/other.ox": (0, gate.sha(b""))}
        self.put("scripts/owned_record_vector_acceptance/other.ox", b"")
        with patch.object(gate, "OWNED_VECTOR_SOURCE_ASSETS", extra), self.assertRaisesRegex(ValueError, "asset membership"):
            self.capture()

    def test_compiled_source_asset_include_cannot_be_redirected_with_updated_censuses(self):
        original = self.current[self.asset_owner]
        changed = original.replace(b"enum16.ox", b"other.ox")
        self.put(self.asset_owner, changed)
        self.pins[self.asset_owner] = (len(changed), gate.sha(changed))
        self.directives[self.asset_owner] = gate.source_directives(self.asset_owner, changed)
        with self.assertRaisesRegex(ValueError, "include binding"):
            self.capture()

    def test_compiled_asset_path_and_literal_must_describe_the_same_include(self):
        owner, relative, included, count = gate.SOURCE_ASSET_INCLUDE_BINDINGS[self.asset]
        changed = {self.asset: (owner, relative.replace("enum16.ox", "other.ox"), included, count)}
        with patch.object(gate, "SOURCE_ASSET_INCLUDE_BINDINGS", changed), self.assertRaisesRegex(ValueError, "include binding"):
            self.capture()

    def test_manifest_preserves_inherited_false_claims_and_lossless_directive_data(self):
        inputs = self.capture()
        manifest = gate.current_manifest(inputs, self.baseline, {"source_kind": "worktree"}, self.parent)
        self.assertEqual(manifest["predecessor_compiler_input_members"], 527)
        self.assertEqual(len(manifest["files"]), 572)
        self.assertEqual(manifest["source_policy_parent"], gate.PREDECESSOR)
        self.assertEqual(manifest["source_policy_input_members"], 60)
        self.assertEqual(manifest["owned_vector_pins_provisional"], gate.PINS_PROVISIONAL)
        self.assertEqual(len(manifest["owned_vector_directive_inventory"]), 106)
        self.assertEqual([row["path"] for row in manifest["owned_vector_compiled_source_assets"]], [self.asset])
        self.assertNotIn(self.asset, {row["path"] for row in manifest["owned_vector_directive_inventory"]})
        self.assertEqual(manifest["owned_vector_compiled_source_assets"][0]["include_occurrences"], 2)
        self.assertEqual({row["path"] for row in manifest["files"]}, set(inputs))
        claims = {name: value for name, value in manifest.items() if name.endswith("qualified_by_this_gate")}
        inherited = gate.REGISTRY_MANIFEST(self.parent, self.baseline, {}, self.parent)
        self.assertLessEqual({name for name in inherited if name.endswith("qualified_by_this_gate")}, set(claims))
        self.assertTrue(claims)
        self.assertTrue(all(value is False for value in claims.values()))
        self.assertEqual(gate.directive_manifest(b"\x00\xff#[cfg(test)]"), {"bytes_hex": "00ff235b6366672874657374295d"})


class FrozenRegistryDelegation(unittest.TestCase):
    def test_reviewed_actual_source_pins_and_every_directive_are_exact(self):
        self.assertEqual(gate.REVIEWED_SOURCE_HEAD, "d71786845668be721897ea7d42ffa516fd8ca493")
        self.assertEqual(set(gate.REVIEWED_INPUTS), gate.OWNED_VECTOR_CHANGED | gate.OWNED_VECTOR_ADDED)
        self.assertEqual(set(gate.REVIEWED_DIRECTIVES), set(gate.REVIEWED_INPUTS))
        for name, expected in gate.REVIEWED_INPUTS.items():
            with self.subTest(name=name):
                body = gate.regular(ROOT, name)
                self.assertEqual((len(body), gate.sha(body)), expected)
                self.assertEqual(gate.source_directives(name, body), gate.REVIEWED_DIRECTIVES[name])
        self.assertIn("src/frontend/declaration_index/tests.rs", gate.TEST_NAMED_INPUTS)
        self.assertIn("src/frontend/oir/source/owned_vector_route_tests.rs", gate.TEST_NAMED_INPUTS)

    def test_exact_compiled_asset_is_present_at_its_real_include_path(self):
        self.assertEqual(set(gate.OWNED_VECTOR_SOURCE_ASSETS), {"scripts/owned_record_vector_acceptance/enum16.ox"})
        inputs = {}
        for name, expected in gate.OWNED_VECTOR_SOURCE_ASSETS.items():
            inputs[name] = gate.regular(ROOT, name)
            self.assertEqual((len(inputs[name]), gate.sha(inputs[name])), expected)
            owner = gate.SOURCE_ASSET_INCLUDE_BINDINGS[name][0]
            inputs[owner] = gate.regular(ROOT, owner)
        gate.validate_source_assets(ROOT, inputs)

    def test_registry_policy_bytes_are_the_exact_af0_object(self):
        body = gate.regular(ROOT, gate.PREDECESSOR_POLICY)
        frozen = gate.git(ROOT, "show", gate.PREDECESSOR + ":" + gate.PREDECESSOR_POLICY)
        self.assertEqual(body, frozen)
        self.assertEqual((len(body), gate.sha(body)), gate.PREDECESSOR_POLICY_PIN)
        self.assertEqual(gate.PREDECESSOR_POLICIES[gate.PREDECESSOR_POLICY], gate.PREDECESSOR_POLICY_PIN[1])
        self.assertEqual(len(gate.PREDECESSOR_POLICIES), 60)
        for name, expected in gate.PREDECESSOR_POLICIES.items():
            with self.subTest(name=name):
                self.assertEqual(gate.sha(gate.regular(ROOT, name)), expected)

    def test_original_runner_assembly_comparators_and_controls_are_identical_objects(self):
        self.assertIs(gate.RETAINED_RUN, gate.library.run)
        self.assertIs(gate.REGISTRY_MANIFEST, gate.library.current_manifest)
        for name in (
            "prepare_current_observer", "verify_current_observer", "verify_materialized",
            "authenticate_observer_adapter", "authenticate_index_resource_adapter", "authenticate_parser_resource_adapter",
            "run_byte_buffer_observer_controls", "run_file_observer_controls", "run_i64_observer_controls",
            "run_integer_width_observer_controls", "run_integer128_observer_controls", "run_float_observer_controls",
            "run_character_observer_controls", "run_vector_observer_controls", "run_record_vector_observer_controls",
        ):
            self.assertIs(getattr(gate, name), getattr(gate.library, name), name)
        source = inspect.getsource(gate.run)
        self.assertIn("status = RETAINED_RUN(delegated)", source)
        self.assertNotIn("subprocess.run", source)
        self.assertNotIn(".replace(", source)

    def test_binding_changes_only_parent_manifest_seams_and_restores_on_error(self):
        names = ("PREDECESSOR", "PREDECESSOR_ADDED", "PREDECESSOR_POLICIES", "PINS_PROVISIONAL",
                 "capture_sources", "current_manifest")
        before = {name: getattr(gate.library, name) for name in names}
        runner = gate.library.run
        with self.assertRaisesRegex(RuntimeError, "retained synthetic refusal"):
            with gate.bound_registry():
                self.assertEqual(gate.library.PREDECESSOR, gate.PREDECESSOR)
                self.assertEqual(gate.library.PREDECESSOR_ADDED, gate.PREDECESSOR_ADDED)
                self.assertIs(gate.library.capture_sources, gate.capture_sources)
                self.assertIs(gate.library.current_manifest, gate.current_manifest)
                self.assertIs(gate.library.run, runner)
                raise RuntimeError("retained synthetic refusal")
        for name, value in before.items():
            self.assertIs(getattr(gate.library, name), value, name)
        self.assertIs(gate.library.run, runner)

    def test_provisional_qualification_is_refused_before_any_process(self):
        with tempfile.TemporaryDirectory(prefix="oxid-owned-vector-provisional-") as folder:
            output = Path(folder) / "output"
            args = argparse.Namespace(repo=ROOT, output=output, prepare_only=False)
            stderr = io.StringIO()
            with patch.object(gate, "PINS_PROVISIONAL", True), patch.object(gate.library.subprocess, "run") as run, \
                    patch.object(gate.library.subprocess, "Popen") as popen, redirect_stderr(stderr):
                self.assertEqual(gate.run(args), 1)
            run.assert_not_called()
            popen.assert_not_called()
            failure = json.loads((output / "failure.json").read_text())
            self.assertEqual(failure["status"], "failed")
            self.assertIs(failure["semantic_pass"], False)
            self.assertEqual(failure["compiler_executions"], 0)
            self.assertIn("prepare-only", failure["error"])
            self.assertFalse((output / "result.json").exists())
            self.assertFalse((output / "prepared.json").exists())
            self.assertFalse((output / "retained").exists())
            self.assertIn("prepare-only", stderr.getvalue())


class OwnedVectorObserverBinding(unittest.TestCase):
    def test_source_anchor_and_all_old_function_objects_stay_exact(self):
        self.assertEqual(gate.REVIEWED_SOURCE_HEAD, "d71786845668be721897ea7d42ffa516fd8ca493")
        self.assertIs(gate.PINS_PROVISIONAL, False)
        names = ("verify_current_observer", "verify_materialized", "authenticate_index_resource_adapter",
                 "authenticate_parser_resource_adapter", "run_byte_buffer_observer_controls",
                 "run_file_observer_controls", "run_i64_observer_controls", "run_integer_width_observer_controls",
                 "run_integer128_observer_controls", "run_float_observer_controls",
                 "run_character_observer_controls", "run_vector_observer_controls", "run_record_vector_observer_controls")
        original = {name: getattr(gate.library, name) for name in names}
        prepare, authenticate, runner = gate.prepare_current_observer, gate.authenticate_observer_adapter, gate.RETAINED_RUN
        with gate.bound_registry():
            self.assertIs(gate.library.run, runner)
            self.assertIs(gate.prepare_current_observer, prepare)
            self.assertIs(gate.authenticate_observer_adapter, authenticate)
            for name, function in original.items():
                self.assertIs(getattr(gate.library, name), function, name)
        self.assertIs(gate.library.prepare_current_observer, prepare)
        self.assertIs(gate.library.authenticate_observer_adapter, authenticate)

    def test_prepare_delegates_once_and_returns_the_real_context_by_identity(self):
        original, output, captured, adapter, index, parser = (object() for _ in range(6))
        selected, seam = {"observer": b"real returned observer"}, {"real": "assembly seam"}
        prepared = (selected, seam)
        with patch.object(gate, "prepare_current_observer", return_value=prepared) as prepare:
            with gate.bound_registry() as context:
                result = gate.library.prepare_current_observer(original, output, captured, adapter, index, parser)
                self.assertIs(result, prepared)
                self.assertIs(context["prepared"], prepared)
                self.assertIs(context["original"], original)
                self.assertIs(context["output"], output)
                self.assertIs(context["selected"], selected)
                self.assertIs(context["seam"], seam)
                self.assertIs(context["adapter"], adapter)
                with self.assertRaisesRegex(ValueError, "preparation repeated"):
                    gate.library.prepare_current_observer(original, output, captured, adapter, index, parser)
            prepare.assert_called_once_with(original, output, captured, adapter, index, parser)

    def test_prepare_authentication_and_path_bindings_restore_after_failure(self):
        names = ("PREDECESSOR", "PREDECESSOR_ADDED", "PREDECESSOR_POLICIES", "PINS_PROVISIONAL",
                 "capture_sources", "current_manifest", "OBSERVER_ADAPTER",
                 "authenticate_observer_adapter", "prepare_current_observer")
        before = {name: getattr(gate.library, name) for name in names}
        with patch.object(gate, "prepare_current_observer", side_effect=RuntimeError("real preparation refused")):
            with self.assertRaisesRegex(RuntimeError, "real preparation refused"):
                with gate.bound_registry():
                    self.assertEqual(gate.library.OBSERVER_ADAPTER, gate.OWNED_VECTOR_OBSERVER_ADAPTER)
                    self.assertIs(gate.library.authenticate_observer_adapter, gate.authenticate_owned_vector_observer_adapter)
                    gate.library.prepare_current_observer(*(object() for _ in range(6)))
        for name, value in before.items():
            self.assertIs(getattr(gate.library, name), value, name)

    def test_observer_provisional_refuses_before_parent_runner_or_process(self):
        with tempfile.TemporaryDirectory(prefix="oxid-owned-vector-observer-provisional-") as folder:
            output = Path(folder) / "output"
            args = argparse.Namespace(repo=ROOT, output=output, prepare_only=False)
            with patch.object(gate, "PINS_PROVISIONAL", False), \
                    patch.object(gate, "OWNED_VECTOR_OBSERVER_PROVISIONAL", True), \
                    patch.object(gate, "RETAINED_RUN") as inherited, \
                    patch.object(gate.library.subprocess, "run") as run, \
                    patch.object(gate.library.subprocess, "Popen") as popen, redirect_stderr(io.StringIO()):
                self.assertEqual(gate.run(args), 1)
            inherited.assert_not_called()
            run.assert_not_called()
            popen.assert_not_called()
            failure = json.loads((output / "failure.json").read_text())
            self.assertIs(failure["semantic_pass"], False)
            self.assertEqual(failure["compiler_executions"], 0)
            self.assertIn("observer pins permit prepare-only", failure["error"])
            self.assertFalse((output / "retained").exists())
            self.assertFalse((output / "result.json").exists())


if __name__ == "__main__":
    unittest.main()
