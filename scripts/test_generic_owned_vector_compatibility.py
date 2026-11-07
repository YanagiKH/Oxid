"""Closed generic-vector inputs and unchanged retained source admission."""
import argparse
import ast
from contextlib import redirect_stderr
import inspect
import io
import json
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch

import replay_owned_enum_vector_source_policy as replay
import run_current_script_tests as runner
import verify_generic_owned_vector_compatibility as gate


ROOT = Path(__file__).resolve().parents[1]


def baseline_names():
    raw = gate.regular(ROOT, gate.BINDING + "/current-source.json")
    if gate.sha(raw) != gate.BASELINE_SOURCE_SHA:
        raise AssertionError("historical source manifest changed")
    return {row["path"] for row in json.loads(raw)["files"]}


class GenericSourceAdmission(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory(prefix="oxid-generic-vector-source-policy-")
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        self.baseline = {name: b"// historical input\n" for name in baseline_names()}
        self.parent = {**self.baseline,
                       **{name: b"// enum parent input\n" for name in gate.PREDECESSOR_ADDED}}
        self.current = {**self.parent, **{name: gate.regular(ROOT, name)
                        for name in gate.REVIEWED_INPUTS.keys() | gate.SOURCE_ASSETS.keys()}}
        for name, body in self.current.items():
            self.put(name, body)

    def put(self, name, body):
        path = self.root / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(body)

    def capture(self):
        return gate.capture_sources(self.root, self.baseline, self.parent)

    def test_all_587_inputs_and_three_rust_two_source_assets_and_public_manifest_closure_are_retained(self):
        inputs = self.capture()
        self.assertEqual((len(self.baseline), len(self.parent), len(inputs)), (376, 587, 594))
        self.assertEqual(set(inputs), set(self.parent) | gate.GENERIC_ADDED
                         | set(gate.SOURCE_ASSETS) | {gate.PUBLIC_TEST})
        self.assertEqual(len(gate.GENERIC_ADDED), 3)
        self.assertEqual(set(gate.SOURCE_ASSETS), {
            "packages/owned_vec/src/lib.ox", "examples/owned-vector-results/src/main.ox",
            "packages/owned_vec/oxid.toml"})
        self.assertTrue(all(inputs[name] == body for name, body in self.parent.items()
                            if name not in gate.GENERIC_CHANGED))

    def test_unreviewed_compiler_native_and_old_assets_cannot_drift(self):
        for name in ("src/main.rs", "src/project_vendor.rs", "src/project_package.rs",
                     "native/experimental_owned_record.c", "packages/utf8_text/src/lib.ox"):
            with self.subTest(name=name):
                self.put(name, self.current[name] + b"// drift\n")
                with self.assertRaisesRegex(ValueError, "unreviewed change"):
                    self.capture()
                self.put(name, self.current[name])

    def test_all_reviewed_sources_and_assets_require_exact_bytes_and_presence(self):
        members = gate.REVIEWED_INPUTS.keys() | gate.SOURCE_ASSETS.keys()
        for name in sorted(members):
            with self.subTest(mutation=name):
                body = self.current[name]
                self.put(name, b"!" + body[1:])
                with self.assertRaisesRegex(ValueError, "reviewed input|compiled asset"):
                    self.capture()
                self.put(name, body)
            with self.subTest(missing=name):
                (self.root / name).unlink()
                with self.assertRaises(ValueError):
                    self.capture()
                self.put(name, body)

    def test_pin_directive_and_asset_membership_cannot_shrink_or_expand(self):
        for name in ("REVIEWED_INPUTS", "REVIEWED_DIRECTIVES", "SOURCE_ASSETS", "SOURCE_ASSET_INCLUDE_BINDINGS"):
            original = getattr(gate, name)
            for changed in ({}, {**original, "src/unknown.rs": None}):
                with self.subTest(bank=name), patch.object(gate, name, changed):
                    with self.assertRaisesRegex(ValueError, "membership"):
                        self.capture()
        with patch.object(gate, "GENERIC_CHANGED", gate.GENERIC_CHANGED - {"src/frontend/declaration_index/generics.rs"}):
            with self.assertRaisesRegex(ValueError, "membership"):
                self.capture()
        with patch.object(gate, "GENERIC_ADDED", gate.GENERIC_ADDED | {"src/main.rs"}):
            with self.assertRaisesRegex(ValueError, "membership"):
                self.capture()

    def test_repinning_source_cannot_admit_includes_selectors_environment_or_paths(self):
        additions = (
            b'include_str!("unreviewed.ox");\n', b'include_bytes!(concat!("x", ".bin"));\n',
            b'#[path="unknown.rs"] mod unknown;\n', b'#[cfg(feature="hidden")] fn hidden() {}\n',
            b'#[cfg_attr(test, path="unknown.rs")] mod unknown;\n',
            b'const H:&str=env!("UNREVIEWED");\n', b'const H:bool=cfg!(feature="hidden");\n')
        for name in sorted(gate.REVIEWED_INPUTS):
            for addition in additions:
                with self.subTest(name=name, addition=addition):
                    body = self.current[name] + addition
                    self.put(name, body)
                    pins = {**gate.REVIEWED_INPUTS, name: (len(body), gate.sha(body))}
                    with patch.object(gate, "REVIEWED_INPUTS", pins), self.assertRaisesRegex(ValueError, "directive"):
                        self.capture()
                    self.put(name, self.current[name])

    def test_include_resolution_count_and_owner_remain_bound_even_with_matching_hashes(self):
        for asset, (owner, relative, count) in gate.SOURCE_ASSET_INCLUDE_BINDINGS.items():
            for binding in ((owner, "../unreviewed.ox", count), (owner, relative, 2),
                            ("src/main.rs", relative, count)):
                with self.subTest(asset=asset, binding=binding), \
                        patch.object(gate, "SOURCE_ASSET_INCLUDE_BINDINGS", {
                            **gate.SOURCE_ASSET_INCLUDE_BINDINGS, asset: binding}), \
                        self.assertRaisesRegex(ValueError, "include"):
                    self.capture()

    def test_extra_sources_empty_directories_and_symlink_members_are_denied(self):
        extra = self.root / "src/unreviewed.rs"
        extra.write_bytes(b"// hidden\n")
        with self.assertRaisesRegex(ValueError, "membership"):
            self.capture()
        extra.unlink()
        empty = self.root / "src/unreviewed"
        empty.mkdir()
        with self.assertRaisesRegex(ValueError, "directory"):
            self.capture()
        empty.rmdir()
        try:
            extra.symlink_to(self.root / "packages/owned_vec/src/lib.ox")
        except (OSError, NotImplementedError) as error:
            self.skipTest(f"symlinks unavailable: {error}")
        with self.assertRaisesRegex(ValueError, "symlink"):
            self.capture()

    def test_manifest_preserves_parent_and_all_false_new_feature_claims(self):
        with patch.object(gate.predecessor, "current_manifest", return_value={
                "inherited_semantics_qualified_by_this_gate": False}) as previous:
            result = gate.current_manifest(self.capture(), self.baseline, {"head": "test"}, self.parent)
        self.assertEqual((len(previous.call_args.kwargs["inputs"]),
                          len(previous.call_args.kwargs["parent"])), (587, 576))
        self.assertEqual(len(result["files"]), 594)
        self.assertEqual(result["source_policy_parent"], gate.PREDECESSOR)
        self.assertEqual(len(result["generic_owned_vector_compiled_assets"]), 3)
        self.assertEqual(result["generic_owned_vector_public_test"]["path"], gate.PUBLIC_TEST)
        self.assertTrue(all(value is False for name, value in result.items()
                            if name.endswith("qualified_by_this_gate")))
        with patch.object(gate.predecessor, "current_manifest", return_value={
                "inherited_semantics_qualified_by_this_gate": True}):
            with self.assertRaisesRegex(ValueError, "qualification claim"):
                gate.current_manifest(self.capture(), self.baseline, {}, self.parent)


class RetainedGenericDelegation(unittest.TestCase):
    def test_every_real_source_asset_directive_and_parent_policy_matches_the_pins(self):
        self.assertEqual(gate.PREDECESSOR, "698043182916dda7276c363e20a98dfbb5b22ed3")
        self.assertFalse(gate.PINS_PROVISIONAL)
        self.assertFalse(gate.OBSERVER_PINS_PROVISIONAL)
        diff = subprocess.check_output([
            "git", "-C", str(ROOT), "diff", "--name-status", gate.PREDECESSOR,
            gate.REVIEWED_SOURCE_HEAD, "--", "src", "native"]).decode().splitlines()
        self.assertEqual(set(diff), {"M\t" + name for name in gate.GENERIC_CHANGED}
                         | {"A\t" + name for name in gate.GENERIC_ADDED})
        bodies = {name: gate.regular(ROOT, name)
                  for name in gate.REVIEWED_INPUTS.keys() | gate.SOURCE_ASSETS.keys()}
        for name, expected in {**gate.REVIEWED_INPUTS, **gate.SOURCE_ASSETS}.items():
            with self.subTest(name=name):
                self.assertEqual((len(bodies[name]), gate.sha(bodies[name])), expected)
        for name, expected in gate.REVIEWED_DIRECTIVES.items():
            with self.subTest(directives=name):
                self.assertEqual(gate.source_directives(name, bodies[name]), expected)
        gate.validate_source_assets(bodies)
        body = gate.regular(ROOT, gate.PREDECESSOR_POLICY)
        self.assertEqual((len(body), gate.sha(body)), gate.PREDECESSOR_POLICY_PIN)
        self.assertEqual(gate.sha(gate.regular(ROOT, gate.OBSERVER_ADAPTER)), gate.OBSERVER_ADAPTER_PIN[1])

    def test_retained_runner_predicates_and_both_observer_control_functions_are_unchanged(self):
        self.assertIs(gate.RETAINED_RUN, gate.library.run)
        self.assertIs(gate.RETAINED_RUN, gate.predecessor.RETAINED_RUN)
        self.assertIs(gate.owned_policy, gate.predecessor.owned_policy)
        source = inspect.getsource(gate.run)
        self.assertIn("status = RETAINED_RUN(delegated)", source)
        self.assertIn("owned_policy.run_owned_vector_observer_controls(", source)
        self.assertIn("predecessor.run_enum_observer_controls(", source)
        self.assertNotIn("subprocess.run", source)
        self.assertNotIn(".replace(", source)

    def test_source_bindings_and_original_enum_observer_restore_after_refusal(self):
        names = ("PREDECESSOR", "PREDECESSOR_ADDED", "PREDECESSOR_POLICIES", "PINS_PROVISIONAL",
                 "capture_sources", "current_manifest", "prepare_current_observer", "OBSERVER_ADAPTER")
        before = {name: getattr(gate.library, name) for name in names}
        observer_names = ("OWNED_VECTOR_OBSERVER_ADAPTER", "OWNED_VECTOR_OBSERVER_ADAPTER_PIN",
                          "authenticate_owned_vector_observer_adapter")
        old = {name: getattr(gate.owned_policy, name) for name in observer_names}
        with self.assertRaisesRegex(RuntimeError, "synthetic refusal"):
            with gate.bound_predecessor() as context:
                self.assertEqual(gate.library.PREDECESSOR, gate.PREDECESSOR)
                self.assertIs(gate.library.capture_sources, gate.capture_sources)
                self.assertIs(gate.library.current_manifest, gate.current_manifest)
                self.assertIs(gate.library.run, gate.RETAINED_RUN)
                self.assertEqual(context, {})
                self.assertEqual(gate.owned_policy.OWNED_VECTOR_OBSERVER_ADAPTER, gate.OBSERVER_ADAPTER)
                raise RuntimeError("synthetic refusal")
        for name, value in before.items():
            self.assertIs(getattr(gate.library, name), value, name)
        for name, value in old.items():
            self.assertIs(getattr(gate.owned_policy, name), value, name)

    def test_replaced_retained_runner_is_refused_before_observer_preparation(self):
        with patch.object(gate.library, "run", lambda _: 0), \
                patch.object(gate.predecessor, "bound_predecessor") as previous:
            with self.assertRaisesRegex(ValueError, "execution function"):
                with gate.bound_predecessor():
                    self.fail("replaced retained runner was accepted")
            previous.assert_not_called()

    def test_provisional_source_observer_or_parent_refuses_before_any_process(self):
        for own, observer, parent in ((True, True, False), (False, True, False), (False, False, True)):
            with self.subTest(source=own, observer=observer, parent=parent), tempfile.TemporaryDirectory() as folder:
                output = Path(folder) / "output"
                args = argparse.Namespace(repo=ROOT, output=output, prepare_only=False)
                with patch.object(gate, "PINS_PROVISIONAL", own), \
                        patch.object(gate, "OBSERVER_PINS_PROVISIONAL", observer), \
                        patch.object(gate.predecessor, "PINS_PROVISIONAL", parent), \
                        patch.object(gate, "RETAINED_RUN") as retained, \
                        patch.object(gate.library.subprocess, "run") as run, \
                        patch.object(gate.library.subprocess, "Popen") as popen, redirect_stderr(io.StringIO()):
                    self.assertEqual(gate.run(args), 1)
                retained.assert_not_called()
                run.assert_not_called()
                popen.assert_not_called()
                result = json.loads((output / "failure.json").read_bytes())
                self.assertFalse(result["semantic_pass"])
                self.assertEqual(result["compiler_executions"], 0)
                self.assertFalse((output / "retained").exists())

    def test_enum_source_suite_is_replayed_exactly_without_hiding_new_modules(self):
        name = "test_owned_enum_vector_compatibility"
        self.assertEqual(runner.PREDECESSOR_ONLY[name][:2], (replay.MODULES[name][0], gate.PREDECESSOR))
        with tempfile.TemporaryDirectory() as folder:
            directory = Path(folder)
            for module in (name, name + "_future", "test_generic_owned_vector_compatibility",
                           "test_owned_enum_vector_observer"):
                (directory / (module + ".py")).write_text("import unittest\n")
            selected, delegated = runner.selected_modules(directory)
            self.assertEqual([path.stem for path in delegated], [name])
            self.assertEqual({path.stem for path in selected}, {
                name + "_future", "test_generic_owned_vector_compatibility", "test_owned_enum_vector_observer"})

    def test_frozen_replay_keeps_all_sixteen_and_rejects_partial_or_skipped_results(self):
        engine = replay.load_engine(ROOT)
        self.assertFalse(replay.PINS_PROVISIONAL)
        self.assertEqual(engine.REVISION, gate.PREDECESSOR)
        name = "test_owned_enum_vector_compatibility"
        body = gate.regular(ROOT, "scripts/" + name + ".py")
        self.assertEqual(gate.sha(body), replay.MODULES[name][0])
        names = sorted(name + "." + cls.name + "." + method.name for cls in ast.parse(body).body
                       if isinstance(cls, ast.ClassDef) for method in cls.body
                       if isinstance(method, ast.FunctionDef) and method.name.startswith("test_"))
        self.assertEqual(len(names), 16)
        stderr = ("\n".join(test.rsplit(".", 1)[1] + " (" + test + ") ... ok" for test in names)
                  + "\n" + "-" * 70 + "\nRan 16 tests in 0.1s\n\nOK\n").encode()
        self.assertEqual(engine.validate_execution(name, body, subprocess.CompletedProcess([], 0, b"", stderr)), names)
        for child in (subprocess.CompletedProcess([], 1, b"", stderr),
                      subprocess.CompletedProcess([], 0, b"unexpected", stderr),
                      subprocess.CompletedProcess([], 0, b"", stderr.replace(b"... ok", b"... skipped", 1)),
                      subprocess.CompletedProcess([], 0, b"", stderr.replace(b"Ran 16", b"Ran 15", 1))):
            with self.assertRaises(ValueError):
                engine.validate_execution(name, body, child)

    def test_provisional_frozen_replay_refuses_before_loading_or_starting_any_process(self):
        with tempfile.TemporaryDirectory() as folder, patch.object(replay, "PINS_PROVISIONAL", True), \
                patch.object(replay, "load_engine") as engine, \
                patch.object(replay.subprocess, "run") as process, \
                patch.object(replay.sys, "argv", ["replay", "--repo", str(ROOT), "--output", folder]), \
                redirect_stderr(io.StringIO()):
            self.assertEqual(replay.main(), 1)
        engine.assert_not_called()
        process.assert_not_called()


if __name__ == "__main__":
    unittest.main()
