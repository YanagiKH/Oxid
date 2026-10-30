"""Closed Windows package inputs and unchanged retained source admission."""
import argparse
import ast
from contextlib import redirect_stderr
import inspect
import io
import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch

import replay_generic_owned_vector_source_policy as replay
import run_current_script_tests as runner
import verify_windows_local_package_compatibility as gate


ROOT = Path(__file__).resolve().parents[1]
INHERITED_ASSETS = frozenset((
    "packages/utf8_text/src/lib.ox", "scripts/owned_record_vector_acceptance/enum16.ox",
    "packages/owned_vec/src/lib.ox", "examples/owned-vector-results/src/main.ox",
    "packages/owned_vec/oxid.toml", "tests/typed_owned_vector_wrappers.rs"))


def baseline_names():
    raw = gate.regular(ROOT, gate.BINDING + "/current-source.json")
    if gate.sha(raw) != gate.BASELINE_SOURCE_SHA:
        raise AssertionError("historical source manifest changed")
    return {row["path"] for row in json.loads(raw)["files"]}


class WindowsLocalPackageSourceAdmission(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory(prefix="oxid-windows-local-package-source-policy-")
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        self.baseline = {name: b"// historical input\n" for name in baseline_names()}
        self.parent = {**self.baseline,
                       **{name: b"// generic parent input\n" for name in gate.PREDECESSOR_ADDED}}
        self.current = {**self.parent, **{name: gate.regular(ROOT, name)
                        for name in gate.REVIEWED_INPUTS}}
        for name, body in self.current.items():
            self.put(name, body)

    def put(self, name, body):
        path = self.root / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(body)

    def capture(self):
        return gate.capture_sources(self.root, self.baseline, self.parent)

    def test_all_594_inputs_and_seven_existing_public_tests_are_retained(self):
        inputs = self.capture()
        self.assertEqual((len(self.baseline), len(self.parent), len(inputs)), (376, 594, 601))
        self.assertEqual(set(inputs), set(self.parent) | gate.WINDOWS_ADDED)
        self.assertEqual(len(gate.WINDOWS_CHANGED), 13)
        self.assertEqual(len(gate.WINDOWS_ADDED), 7)
        self.assertEqual(sum(name.startswith(("src/", "native/")) for name in inputs), 500)
        self.assertTrue(all(inputs[name] == body for name, body in self.parent.items()
                            if name not in gate.WINDOWS_CHANGED))
        self.assertTrue(INHERITED_ASSETS <= self.parent.keys())
        self.assertFalse(INHERITED_ASSETS & gate.REVIEWED_INPUTS.keys())

    def test_unreviewed_compiler_native_and_all_six_inherited_assets_cannot_drift(self):
        for name in ("src/project_vendor.rs", "src/project_package.rs",
                     "native/experimental_owned_record.c", *sorted(INHERITED_ASSETS)):
            with self.subTest(name=name):
                self.put(name, self.current[name] + b"// drift\n")
                with self.assertRaisesRegex(ValueError, "unreviewed change"):
                    self.capture()
                self.put(name, self.current[name])

    def test_all_twenty_reviewed_inputs_require_exact_bytes_and_presence(self):
        self.assertEqual(len(gate.REVIEWED_INPUTS), 20)
        for name in sorted(gate.REVIEWED_INPUTS):
            with self.subTest(mutation=name):
                body = self.current[name]
                self.put(name, b"!" + body[1:])
                with self.assertRaisesRegex(ValueError, "reviewed input"):
                    self.capture()
                self.put(name, body)
            with self.subTest(missing=name):
                (self.root / name).unlink()
                with self.assertRaises(ValueError):
                    self.capture()
                self.put(name, body)

    def test_parent_pin_directive_and_finite_membership_cannot_shrink_or_expand(self):
        for name in ("REVIEWED_INPUTS", "REVIEWED_DIRECTIVES"):
            original = getattr(gate, name)
            for changed in ({}, {**original, "src/unknown.rs": None}):
                with self.subTest(bank=name), patch.object(gate, name, changed):
                    with self.assertRaisesRegex(ValueError, "membership"):
                        self.capture()
        for changed in ({}, {**self.parent, "src/unknown.rs": b"// hidden\n"}):
            with self.subTest(parent=set(changed)), self.assertRaisesRegex(ValueError, "parent membership"):
                gate.capture_sources(self.root, self.baseline, changed)
        with patch.object(gate, "WINDOWS_CHANGED", gate.WINDOWS_CHANGED - {"src/main.rs"}):
            with self.assertRaisesRegex(ValueError, "membership"):
                self.capture()
        with patch.object(gate, "WINDOWS_ADDED", gate.WINDOWS_ADDED | {"src/main.rs"}):
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

    def test_extra_compiler_sources_and_empty_directories_are_denied(self):
        extra = self.root / "src/unreviewed.rs"
        extra.write_bytes(b"// hidden\n")
        with self.assertRaisesRegex(ValueError, "membership"):
            self.capture()
        extra.unlink()
        empty = self.root / "src/unreviewed"
        empty.mkdir()
        with self.assertRaisesRegex(ValueError, "directory"):
            self.capture()

    def test_symlink_compiler_and_new_authority_members_are_denied(self):
        for name in ("src/unreviewed.rs", "tests/typed_windows_modules.rs"):
            with self.subTest(name=name):
                member = self.root / name
                old = member.read_bytes() if member.exists() else None
                if old is not None:
                    member.unlink()
                try:
                    member.symlink_to(self.root / "packages/owned_vec/src/lib.ox")
                except (OSError, NotImplementedError) as error:
                    self.skipTest(f"symlinks unavailable: {error}")
                with self.assertRaisesRegex(ValueError, "symlink"):
                    self.capture()
                member.unlink()
                if old is not None:
                    self.put(name, old)

    def test_executable_and_linked_public_test_members_are_denied(self):
        name = "tests/typed_windows_modules.rs"
        member = self.root / name
        mode = member.stat().st_mode
        member.chmod(mode | 0o100)
        try:
            with self.assertRaisesRegex(ValueError, "nonregular"):
                self.capture()
        finally:
            member.chmod(mode)
        link = self.root / "linked-input.rs"
        try:
            os.link(member, link)
        except (OSError, NotImplementedError) as error:
            self.skipTest(f"hard links unavailable: {error}")
        with self.assertRaisesRegex(ValueError, "nonregular"):
            self.capture()

    def test_manifest_retains_generic_authority_and_all_false_windows_claims(self):
        with patch.object(gate.predecessor, "current_manifest", return_value={
                "inherited_semantics_qualified_by_this_gate": False,
                "generic_owned_vector_compiled_assets": ["inherited marker"]}) as previous:
            result = gate.current_manifest(self.capture(), self.baseline, {"head": "test"}, self.parent)
        self.assertEqual((len(previous.call_args.kwargs["inputs"]),
                          len(previous.call_args.kwargs["parent"])), (594, 587))
        self.assertEqual(len(result["files"]), 601)
        self.assertEqual(result["source_policy_parent"], gate.PREDECESSOR)
        self.assertEqual(result["generic_owned_vector_compiled_assets"], ["inherited marker"])
        self.assertEqual(len(result["windows_local_package_reviewed_inputs"]), 20)
        rows = result["windows_local_package_directive_inventory"]
        self.assertEqual({row["path"] for row in rows if row["before"] is None}, gate.WINDOWS_ADDED)
        self.assertTrue(all(value is False for name, value in result.items()
                            if name.endswith("qualified_by_this_gate")))
        with patch.object(gate.predecessor, "current_manifest", return_value={
                "inherited_semantics_qualified_by_this_gate": True}):
            with self.assertRaisesRegex(ValueError, "qualification claim"):
                gate.current_manifest(self.capture(), self.baseline, {}, self.parent)


class RetainedWindowsLocalPackageDelegation(unittest.TestCase):
    def test_every_reviewed_input_directive_and_source_anchor_matches_exact_candidate(self):
        self.assertEqual(gate.PREDECESSOR, "1ed9a4b9c373b62d115f31ef9abf1b84debd2e3e")
        self.assertEqual(gate.REVIEWED_SOURCE_HEAD, "be2d3b1584467ba84eac7ea8667f1161591549ec")
        self.assertFalse(gate.PINS_PROVISIONAL)
        self.assertFalse(gate.OBSERVER_PINS_PROVISIONAL)
        diff = subprocess.check_output([
            "git", "-C", str(ROOT), "diff", "--name-status", gate.PREDECESSOR,
            gate.REVIEWED_SOURCE_HEAD, "--", "src", "native", "tests"]).decode().splitlines()
        self.assertEqual(set(diff), {"M\t" + name for name in gate.WINDOWS_CHANGED | gate.WINDOWS_ADDED})
        for name, expected in gate.REVIEWED_INPUTS.items():
            with self.subTest(name=name):
                body = gate.regular(ROOT, name)
                self.assertEqual((len(body), gate.sha(body)), expected)
                self.assertEqual(gate.source_directives(name, body), gate.REVIEWED_DIRECTIVES[name])
                self.assertEqual(body, subprocess.check_output([
                    "git", "-C", str(ROOT), "show", gate.REVIEWED_SOURCE_HEAD + ":" + name]))
        body = gate.regular(ROOT, gate.PREDECESSOR_POLICY)
        self.assertEqual((len(body), gate.sha(body)), gate.PREDECESSOR_POLICY_PIN)
        for name in INHERITED_ASSETS:
            self.assertEqual(gate.regular(ROOT, name), subprocess.check_output([
                "git", "-C", str(ROOT), "show", gate.PREDECESSOR + ":" + name]))
        self.assertEqual(gate.sha(gate.regular(ROOT, gate.OBSERVER_ADAPTER)), gate.OBSERVER_ADAPTER_PIN[1])

    def test_parent_controller_symlink_mode_link_and_hash_drift_are_denied_before_import(self):
        body = gate.regular(ROOT, gate.PREDECESSOR_POLICY)
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            path = root / gate.PREDECESSOR_POLICY
            path.parent.mkdir()
            path.write_bytes(b"!" + body[1:])
            with self.assertRaisesRegex(ValueError, "changed pinned"):
                gate.load_predecessor(root)
            path.write_bytes(body)
            path.chmod(0o744)
            with self.assertRaisesRegex(ValueError, "unsafe pinned"):
                gate.load_predecessor(root)
            path.chmod(0o644)
            other = root / "other.py"
            try:
                os.link(path, other)
            except (OSError, NotImplementedError) as error:
                self.skipTest(f"hard links unavailable: {error}")
            with self.assertRaisesRegex(ValueError, "unsafe pinned"):
                gate.load_predecessor(root)
            other.unlink()
            path.unlink()
            try:
                path.symlink_to(ROOT / gate.PREDECESSOR_POLICY)
            except (OSError, NotImplementedError) as error:
                self.skipTest(f"symlinks unavailable: {error}")
            with self.assertRaisesRegex(ValueError, "symlink"):
                gate.load_predecessor(root)

    def test_retained_runner_predicates_and_both_observer_control_functions_are_unchanged(self):
        self.assertIs(gate.RETAINED_RUN, gate.library.run)
        self.assertIs(gate.RETAINED_RUN, gate.predecessor.RETAINED_RUN)
        self.assertIs(gate.owned_policy, gate.predecessor.owned_policy)
        self.assertIs(gate.enum_policy, gate.predecessor.predecessor)
        self.assertIs(gate.enum_policy.run_enum_observer_controls,
                      gate.predecessor.predecessor.run_enum_observer_controls)
        source = inspect.getsource(gate.run)
        self.assertIn("status = RETAINED_RUN(delegated)", source)
        self.assertIn("owned_policy.run_owned_vector_observer_controls(", source)
        self.assertIn("enum_policy.run_enum_observer_controls(", source)
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


    def test_generic_source_suite_is_replayed_exactly_without_hiding_new_modules(self):
        name = "test_generic_owned_vector_compatibility"
        self.assertEqual(runner.PREDECESSOR_ONLY[name][:2], (replay.MODULES[name][0], gate.PREDECESSOR))
        with tempfile.TemporaryDirectory() as folder:
            directory = Path(folder)
            for module in (name, name + "_future", "test_windows_local_package_compatibility",
                           "test_owned_enum_vector_observer"):
                (directory / (module + ".py")).write_text("import unittest\n")
            selected, delegated = runner.selected_modules(directory)
            self.assertEqual([path.stem for path in delegated], [name])
            self.assertEqual({path.stem for path in selected}, {
                name + "_future", "test_windows_local_package_compatibility", "test_owned_enum_vector_observer"})


    def test_frozen_replay_keeps_all_sixteen_and_rejects_partial_or_skipped_results(self):
        engine = replay.load_engine(ROOT)
        self.assertFalse(replay.PINS_PROVISIONAL)
        self.assertEqual(engine.REVISION, gate.PREDECESSOR)
        name = "test_generic_owned_vector_compatibility"
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
