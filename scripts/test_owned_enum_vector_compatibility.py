"""Finite enum-vector input admission and unchanged full replay controls."""
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

import replay_offline_vendor_source_policy as replay
import run_current_script_tests as runner
import verify_owned_enum_vector_compatibility as gate


ROOT = Path(__file__).resolve().parents[1]


def baseline_names():
    raw = gate.regular(ROOT, gate.BINDING + "/current-source.json")
    if gate.sha(raw) != gate.BASELINE_SOURCE_SHA:
        raise AssertionError("historical source manifest changed")
    return {row["path"] for row in json.loads(raw)["files"]}


class EnumSourceAdmission(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory(prefix="oxid-enum-source-policy-")
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        self.baseline = {name: b"// historical input\n" for name in baseline_names()}
        self.parent = {**self.baseline, **{name: b"// vendor parent input\n" for name in gate.PREDECESSOR_ADDED}}
        self.current = {**self.parent, **{name: b"// reviewed enum input: " + name.encode() + b"\n"
                                        for name in gate.ENUM_CHANGED | gate.ENUM_ADDED}}
        self.pins = {name: (len(self.current[name]), gate.sha(self.current[name]))
                     for name in gate.ENUM_CHANGED | gate.ENUM_ADDED}
        self.directives = {name: gate.source_directives(name, self.current[name]) for name in self.pins}
        for name, body in self.current.items():
            self.put(name, body)
        for name, value in (("REVIEWED_INPUTS", self.pins), ("REVIEWED_DIRECTIVES", self.directives)):
            selected = patch.object(gate, name, value)
            selected.start()
            self.addCleanup(selected.stop)

    def put(self, name, body):
        path = self.root / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(body)

    def capture(self):
        return gate.capture_sources(self.root, self.baseline, self.parent)

    def test_all_576_inputs_and_exact_eleven_additions_are_retained(self):
        inputs = self.capture()
        self.assertEqual((len(self.baseline), len(self.parent), len(inputs)), (376, 576, 587))
        self.assertEqual(set(inputs), set(self.parent) | gate.ENUM_ADDED)
        self.assertEqual((len(gate.ENUM_CHANGED), len(gate.ENUM_ADDED)), (45, 11))
        self.assertTrue(all(inputs[name] == body for name, body in self.parent.items() if name not in gate.ENUM_CHANGED))

    def test_unreviewed_compiler_vendor_runtime_and_compiled_assets_cannot_drift(self):
        for name in ("src/main.rs", "src/project_vendor.rs", "src/project_package.rs",
                     "native/experimental_bytebuf.c", "packages/utf8_text/src/lib.ox"):
            with self.subTest(name=name):
                self.put(name, self.current[name] + b"// drift\n")
                with self.assertRaisesRegex(ValueError, "unreviewed change"):
                    self.capture()
                self.put(name, self.current[name])

    def test_exact_byte_pin_and_all_reviewed_members_are_required(self):
        name = "src/frontend/oir/owned_types.rs"
        self.put(name, self.current[name] + b"// extra behavior\n")
        with self.assertRaisesRegex(ValueError, "reviewed input"):
            self.capture()
        self.put(name, self.current[name])
        for member in (*sorted(self.pins), "packages/utf8_text/src/lib.ox"):
            with self.subTest(missing=member):
                (self.root / member).unlink()
                with self.assertRaises(ValueError):
                    self.capture()
                self.put(member, self.current[member])

    def test_pin_or_directive_membership_cannot_shrink_or_expand(self):
        for name in ("REVIEWED_INPUTS", "REVIEWED_DIRECTIVES"):
            original = getattr(gate, name)
            for changed in ({}, {**original, "src/unknown.rs": None}):
                with self.subTest(bank=name), patch.object(gate, name, changed):
                    with self.assertRaisesRegex(ValueError, "membership"):
                        self.capture()
        for name, changed in (("ENUM_CHANGED", gate.ENUM_CHANGED - {"src/frontend/oir/owned_types.rs"}),
                              ("ENUM_ADDED", gate.ENUM_ADDED | {"src/main.rs"})):
            with patch.object(gate, name, changed):
                with self.assertRaisesRegex(ValueError, "membership"):
                    self.capture()

    def test_updating_source_pin_cannot_admit_new_includes_cfg_environment_or_c_defines(self):
        rust_additions = (
            b'include_str!("unreviewed.ox");\n', b'include_bytes!(concat!("x", ".bin"));\n',
            b'#[path="unknown.rs"] mod unknown;\n', b'#[cfg(feature="hidden")] fn hidden() {}\n',
            b'#[cfg_attr(test, path="unknown.rs")] mod unknown;\n',
            b'const H:&str=env!("UNREVIEWED");\n', b'const H:bool=cfg!(feature="hidden");\n')
        for name, additions in (
            *((name, rust_additions) for name in (
                "src/frontend/oir/owned_types.rs", "src/frontend/oir/owned/mod.rs",
                "src/frontend/oir/owned/source/budget.rs", "src/frontend/oir/owned/source/diagnostic.rs",
                "src/frontend/oir/owned/source/record_enum_layouts.rs", "src/frontend/oir/owned/source/resolve.rs")),
            ("native/experimental_owned_record.c", (b'#include "unreviewed.c"\n', b'#define HIDDEN 1\n')),
        ):
            for addition in additions:
                with self.subTest(name=name, addition=addition):
                    current = self.current[name] + addition
                    self.put(name, current)
                    self.pins[name] = (len(current), gate.sha(current))
                    with self.assertRaisesRegex(ValueError, "directive"):
                        self.capture()
                    self.put(name, self.current[name])
                    self.pins[name] = (len(self.current[name]), gate.sha(self.current[name]))

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
            extra.symlink_to(self.root / "packages/utf8_text/src/lib.ox")
        except (OSError, NotImplementedError) as error:
            self.skipTest(f"symlinks unavailable: {error}")
        with self.assertRaisesRegex(ValueError, "symlink"):
            self.capture()

    def test_same_length_source_mutation_is_not_hidden_by_membership(self):
        for name in sorted(self.pins):
            with self.subTest(name=name):
                body = self.current[name]
                self.put(name, b"!" + body[1:])
                with self.assertRaisesRegex(ValueError, "reviewed input"):
                    self.capture()
                self.put(name, body)

    def test_manifest_retains_all_inputs_parent_and_false_new_feature_claims(self):
        inherited = {"inherited_semantics_qualified_by_this_gate": False}
        with patch.object(gate.predecessor, "current_manifest", return_value=inherited) as previous:
            manifest = gate.current_manifest(self.capture(), self.baseline, {"head": "test"}, self.parent)
        self.assertEqual((len(previous.call_args.kwargs["inputs"]), len(previous.call_args.kwargs["parent"])), (576, 574))
        self.assertEqual(len(manifest["files"]), 587)
        self.assertEqual(len(manifest["owned_enum_vector_reviewed_inputs"]), 56)
        self.assertEqual(len(manifest["owned_enum_vector_directive_inventory"]), 56)
        self.assertEqual(manifest["source_policy_parent"], gate.PREDECESSOR)
        self.assertTrue(all(value is False for name, value in manifest.items() if name.endswith("qualified_by_this_gate")))
        with patch.object(gate.predecessor, "current_manifest", return_value={"inherited_qualified_by_this_gate": True}):
            with self.assertRaisesRegex(ValueError, "qualification claim"):
                gate.current_manifest(self.capture(), self.baseline, {}, self.parent)


class RetainedEnumDelegation(unittest.TestCase):
    def test_every_real_source_matches_exact_pins_and_literal_directive_inventory(self):
        self.assertEqual(gate.PREDECESSOR, "5917f581e69c0a0c69e4b5b059bb97dced55331d")
        self.assertEqual(gate.REVIEWED_SOURCE_HEAD, "a54fc93c3d2ee47f098de59cf1d220f35a1fd86b")
        self.assertTrue(gate.PINS_PROVISIONAL)
        self.assertTrue(gate.OBSERVER_PINS_PROVISIONAL)
        self.assertEqual((len(gate.ENUM_CHANGED), len(gate.ENUM_ADDED)), (45, 11))
        self.assertEqual(len(gate.REVIEWED_INPUTS), 56)
        new_paths = {
            "src/frontend/oir/owned/mod.rs", "src/frontend/oir/owned/source/budget.rs",
            "src/frontend/oir/owned/source/diagnostic.rs", "src/frontend/oir/owned/source/record_enum_layouts.rs",
            "src/frontend/oir/owned/source/resolve.rs",
        }
        self.assertTrue(new_paths <= gate.ENUM_CHANGED)
        diff = subprocess.check_output([
            "git", "-C", str(ROOT), "diff", "--name-status", gate.PREDECESSOR,
            gate.REVIEWED_SOURCE_HEAD, "--", "src", "native",
        ]).decode().splitlines()
        self.assertEqual(set(diff), {"M\t" + name for name in gate.ENUM_CHANGED}
                         | {"A\t" + name for name in gate.ENUM_ADDED})
        for name, expected in gate.REVIEWED_INPUTS.items():
            with self.subTest(name=name):
                body = gate.regular(ROOT, name)
                self.assertEqual((len(body), gate.sha(body)), expected)
                self.assertEqual(gate.source_directives(name, body), gate.REVIEWED_DIRECTIVES[name])
        for name in sorted(new_paths):
            with self.subTest(new_path_directives=name):
                parent_body = subprocess.check_output(["git", "-C", str(ROOT), "show", gate.PREDECESSOR + ":" + name])
                self.assertEqual(gate.source_directives(name, parent_body), gate.REVIEWED_DIRECTIVES[name])
        body = gate.regular(ROOT, gate.PREDECESSOR_POLICY)
        self.assertEqual((len(body), gate.sha(body)), gate.PREDECESSOR_POLICY_PIN)
        self.assertEqual(len(gate.predecessor.REVIEWED_INPUTS), 7)
        self.assertTrue((gate.ENUM_CHANGED | gate.ENUM_ADDED).isdisjoint(gate.predecessor.REVIEWED_INPUTS))
        for name, expected in gate.predecessor.REVIEWED_INPUTS.items():
            with self.subTest(inherited_vendor=name):
                body = gate.regular(ROOT, name)
                self.assertEqual((len(body), gate.sha(body)), expected)

    def test_retained_runner_and_all_old_predicates_keep_the_same_function_objects(self):
        self.assertIs(gate.RETAINED_RUN, gate.library.run)
        self.assertIs(gate.RETAINED_RUN, gate.predecessor.RETAINED_RUN)
        self.assertIs(gate.RETAINED_RUN, gate.owned_policy.RETAINED_RUN)
        for name in ("verify_current_observer", "verify_materialized", "run_byte_buffer_observer_controls",
                     "run_file_observer_controls", "run_i64_observer_controls", "run_integer_width_observer_controls",
                     "run_integer128_observer_controls", "run_float_observer_controls", "run_character_observer_controls",
                     "run_vector_observer_controls", "run_record_vector_observer_controls"):
            self.assertIs(getattr(gate.owned_policy, name), getattr(gate.library, name))
        source = inspect.getsource(gate.run)
        self.assertIn("status = RETAINED_RUN(delegated)", source)
        self.assertIn("owned_policy.run_owned_vector_observer_controls(", source)
        self.assertIn("report.update(run_enum_observer_controls(", source)
        self.assertNotIn("subprocess.run", source)
        self.assertNotIn(".replace(", source)

    def test_six_source_and_three_observer_bindings_restore_after_refusal(self):
        source_names = ("PREDECESSOR", "PREDECESSOR_ADDED", "PREDECESSOR_POLICIES", "PINS_PROVISIONAL",
                        "capture_sources", "current_manifest", "prepare_current_observer", "OBSERVER_ADAPTER")
        observer_names = ("OWNED_VECTOR_OBSERVER_ADAPTER", "OWNED_VECTOR_OBSERVER_ADAPTER_PIN",
                          "authenticate_owned_vector_observer_adapter")
        before = {name: getattr(gate.library, name) for name in source_names}
        old = {name: getattr(gate.owned_policy, name) for name in observer_names}
        with self.assertRaisesRegex(RuntimeError, "synthetic refusal"):
            with gate.bound_predecessor():
                self.assertEqual(gate.library.PREDECESSOR, gate.PREDECESSOR)
                self.assertIs(gate.library.capture_sources, gate.capture_sources)
                self.assertIs(gate.library.current_manifest, gate.current_manifest)
                self.assertIs(gate.library.run, gate.RETAINED_RUN)
                self.assertEqual(gate.library.OBSERVER_ADAPTER, gate.OBSERVER_ADAPTER)
                self.assertEqual(gate.owned_policy.OWNED_VECTOR_OBSERVER_ADAPTER, gate.OBSERVER_ADAPTER)
                raise RuntimeError("synthetic refusal")
        for name, value in before.items():
            self.assertIs(getattr(gate.library, name), value, name)
        for name, value in old.items():
            self.assertIs(getattr(gate.owned_policy, name), value, name)

    def test_replaced_retained_runner_is_refused_before_observer_preparation(self):
        with patch.object(gate.library, "run", lambda _: 0), patch.object(gate, "bound_observer") as observer:
            with self.assertRaisesRegex(ValueError, "execution function"):
                with gate.bound_predecessor():
                    self.fail("replaced retained runner was accepted")
            observer.assert_not_called()

    def test_provisional_source_observer_or_parent_refuse_before_any_process(self):
        for own, observer, parent in ((True, True, True), (False, True, False), (False, False, True)):
            with self.subTest(source=own, observer=observer, parent=parent), tempfile.TemporaryDirectory() as folder:
                output = Path(folder) / "output"
                args = argparse.Namespace(repo=ROOT, output=output, prepare_only=False)
                with patch.object(gate, "PINS_PROVISIONAL", own), patch.object(gate, "OBSERVER_PINS_PROVISIONAL", observer), \
                        patch.object(gate.predecessor, "PINS_PROVISIONAL", parent), \
                        patch.object(gate, "RETAINED_RUN") as retained, patch.object(gate.library.subprocess, "run") as run, \
                        patch.object(gate.library.subprocess, "Popen") as popen, redirect_stderr(io.StringIO()):
                    self.assertEqual(gate.run(args), 1)
                retained.assert_not_called()
                run.assert_not_called()
                popen.assert_not_called()
                failure = json.loads((output / "failure.json").read_bytes())
                self.assertFalse(failure["semantic_pass"])
                self.assertEqual(failure["compiler_executions"], 0)
                self.assertFalse((output / "retained").exists())
                self.assertFalse((output / "result.json").exists())

    def test_frozen_vendor_replay_keeps_all_fourteen_and_the_exact_unmodified_engine(self):
        engine = replay.load_engine(ROOT)
        self.assertFalse(replay.PINS_PROVISIONAL)
        self.assertEqual(engine.REVISION, gate.PREDECESSOR)
        self.assertEqual(engine.MODULES, replay.MODULES)
        self.assertEqual(engine.POLICIES, replay.POLICIES)
        name = "test_offline_vendor_compatibility"
        body = gate.regular(ROOT, "scripts/" + name + ".py")
        self.assertEqual(gate.sha(body), replay.MODULES[name][0])
        names = sorted(name + "." + cls.name + "." + method.name for cls in ast.parse(body).body
                       if isinstance(cls, ast.ClassDef) for method in cls.body
                       if isinstance(method, ast.FunctionDef) and method.name.startswith("test_"))
        self.assertEqual(len(names), 14)
        lines = [test.rsplit(".", 1)[1] + " (" + test + ") ... ok" for test in names]
        stderr = ("\n".join(lines) + "\n" + "-" * 70 + "\nRan 14 tests in 0.1s\n\nOK\n").encode()
        self.assertEqual(engine.validate_execution(name, body, subprocess.CompletedProcess([], 0, b"", stderr)), names)
        for child in (subprocess.CompletedProcess([], 1, b"", stderr),
                      subprocess.CompletedProcess([], 0, b"unexpected", stderr),
                      subprocess.CompletedProcess([], 0, b"", stderr.replace(b"... ok", b"... skipped", 1)),
                      subprocess.CompletedProcess([], 0, b"", stderr.replace(b"Ran 14", b"Ran 13", 1))):
            with self.assertRaises(ValueError):
                engine.validate_execution(name, body, child)

    def test_provisional_frozen_replay_refuses_before_loading_or_starting_any_process(self):
        with tempfile.TemporaryDirectory() as folder, patch.object(replay, "PINS_PROVISIONAL", True), \
                patch.object(replay, "load_engine") as engine, patch.object(replay.subprocess, "run") as process, \
                patch.object(replay.sys, "argv", ["replay", "--repo", str(ROOT), "--output", folder]), \
                redirect_stderr(io.StringIO()):
            self.assertEqual(replay.main(), 1)
        engine.assert_not_called()
        process.assert_not_called()

    def test_current_runner_delegates_only_the_authenticated_vendor_module(self):
        row = runner.PREDECESSOR_ONLY["test_offline_vendor_compatibility"]
        self.assertEqual(row[:2], (replay.MODULES["test_offline_vendor_compatibility"][0], gate.PREDECESSOR))
        with tempfile.TemporaryDirectory() as folder:
            directory = Path(folder)
            for name in ("test_offline_vendor_compatibility.py", "test_offline_vendor_compatibility_future.py",
                         "test_owned_enum_vector_compatibility.py", "test_owned_enum_vector_observer.py"):
                (directory / name).write_text("import unittest\n")
            selected, delegated = runner.selected_modules(directory)
            self.assertEqual([path.stem for path in delegated], ["test_offline_vendor_compatibility"])
            self.assertEqual({path.stem for path in selected}, {
                "test_offline_vendor_compatibility_future", "test_owned_enum_vector_compatibility", "test_owned_enum_vector_observer"})
            loader = runner.CurrentTestLoader(directory)
            for path in selected:
                self.assertTrue(loader._match_path(path.name, str(path), "test_*.py"))
            self.assertFalse(loader._match_path(delegated[0].name, str(delegated[0]), "test_*.py"))


if __name__ == "__main__":
    unittest.main()
