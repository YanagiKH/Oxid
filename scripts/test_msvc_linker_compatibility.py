"""Closed one-file admission and unmodified two-policy replay controls."""
import argparse
from contextlib import redirect_stderr
import inspect
import io
import json
from pathlib import Path
import subprocess
import tempfile
import types
import unittest
from unittest.mock import patch

import verify_msvc_linker_compatibility as gate


ROOT = Path(__file__).resolve().parents[1]


def git_body(commit, name):
    return subprocess.check_output(["git", "--no-replace-objects", "show", commit + ":" + name], cwd=ROOT)


def frozen_context(name):
    """Load exact fixed policy definitions; never run a producer or fabricate a result."""
    spec = gate.SOURCE_POLICIES[name]
    body = git_body(spec["parent"], spec["controller"])
    if (len(body), gate.sha(body)) != spec["controller_pin"]:
        raise AssertionError("frozen retained controller changed")
    module = types.ModuleType("_test_frozen_msvc_" + name.replace("-", "_"))
    module.__file__ = str(ROOT / spec["controller"])
    exec(compile(body, module.__file__, "exec"), module.__dict__)
    return gate.RetainedPolicy(name, module)


class OneFileLinkerAdmission(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.contexts = {name: frozen_context(name) for name in gate.SOURCE_POLICIES}
        cls.before = git_body(gate.SOURCE_POLICIES["scalar-library"]["parent"], gate.NATIVE_INPUT)
        first = cls.contexts["scalar-library"].library
        body = first.regular(ROOT, first.BINDING + "/current-source.json")
        if gate.sha(body) != first.BASELINE_SOURCE_SHA:
            raise AssertionError("historical source manifest changed")
        cls.baseline_names = {row["path"] for row in json.loads(body)["files"]}

    def setUp(self):
        temporary = tempfile.TemporaryDirectory(prefix="oxid-msvc-policy-")
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        self.baseline = {name: b"// fixed historical source\n" for name in self.baseline_names}
        self.after = self.before + b"\n// synthetic one-file admission control\n"
        directives = self.contexts["scalar-library"].library.source_directives(gate.NATIVE_INPUT, self.after)
        for name, value in (("REVIEWED_NATIVE_PIN", (len(self.after), gate.sha(self.after))),
                            ("REVIEWED_NATIVE_DIRECTIVES", directives),
                            ("REVIEWED_NATIVE_SOURCE_COMMIT", "1" * 40)):
            selected = patch.object(gate, name, value)
            selected.start()
            self.addCleanup(selected.stop)
        self.parents = {}
        for name, context in self.contexts.items():
            parent = {**self.baseline, **{path: b"// fixed parent member\n" for path in context.parent_added}}
            parent[gate.NATIVE_INPUT] = self.before
            self.parents[name] = parent
            for path, body in parent.items():
                self.put(name, path, self.after if path == gate.NATIVE_INPUT else body)

    def put(self, policy, name, body):
        path = self.root / policy / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(body)

    def capture(self, name):
        return self.contexts[name].capture_sources(self.root / name, self.baseline, self.parents[name])

    def test_both_complete_parent_rosters_are_preserved_without_additions(self):
        self.assertEqual(len(self.baseline), 376)
        for name, expected in (("scalar-library", 522), ("registry", 527)):
            with self.subTest(policy=name):
                inputs = self.capture(name)
                self.assertEqual(len(inputs), expected)
                self.assertEqual(set(inputs), set(self.parents[name]))
                self.assertEqual({path for path, body in inputs.items() if body != self.parents[name][path]},
                                 {gate.NATIVE_INPUT})

    def test_every_other_parent_byte_and_protected_input_stays_exact(self):
        for name, context in self.contexts.items():
            for path in ("src/main.rs", next(iter(context.library.UNCHANGED_INPUTS))):
                with self.subTest(policy=name, path=path):
                    self.put(name, path, self.parents[name][path] + b"// drift\n")
                    with self.assertRaisesRegex(ValueError, "one-file linker delta"):
                        self.capture(name)
                    self.put(name, path, self.parents[name][path])

    def test_wrong_frozen_native_before_input_is_not_reauthenticated(self):
        for name, parent in self.parents.items():
            with self.subTest(policy=name):
                parent[gate.NATIVE_INPUT] += b"// wrong parent\n"
                with self.assertRaisesRegex(ValueError, "native before-input"):
                    self.capture(name)

    def test_missing_extra_source_empty_directory_and_symlink_are_refused(self):
        for name in self.contexts:
            with self.subTest(policy=name):
                path = self.root / name / "src/main.rs"
                original = path.read_bytes()
                path.unlink()
                with self.assertRaisesRegex(ValueError, "membership"):
                    self.capture(name)
                path.write_bytes(original)
                extra = path.parent / "unreviewed.rs"
                extra.write_bytes(b"fn hidden() {}\n")
                with self.assertRaisesRegex(ValueError, "membership"):
                    self.capture(name)
                extra.unlink()
                directory = path.parent / "unreviewed"
                directory.mkdir()
                with self.assertRaisesRegex(ValueError, "directory"):
                    self.capture(name)
                directory.rmdir()
                extra.symlink_to("main.rs")
                with self.assertRaisesRegex(ValueError, "symlink"):
                    self.capture(name)

    def test_native_after_pin_is_exact(self):
        for name in self.contexts:
            with self.subTest(policy=name):
                self.put(name, gate.NATIVE_INPUT, self.after + b"// unreviewed edit\n")
                with self.assertRaisesRegex(ValueError, "after-pin"):
                    self.capture(name)

    def test_directive_drift_is_refused_even_with_an_updated_byte_pin(self):
        changed = self.after + b"\n#[cfg(any())]\nfn hidden() {}\n"
        with patch.object(gate, "REVIEWED_NATIVE_PIN", (len(changed), gate.sha(changed))):
            for name in self.contexts:
                with self.subTest(policy=name):
                    self.put(name, gate.NATIVE_INPUT, changed)
                    with self.assertRaisesRegex(ValueError, "directive changed"):
                        self.capture(name)

    def test_one_file_delta_cannot_add_an_uncaptured_include_asset(self):
        changed = self.after + b'\nconst EXTRA: &str = include_str!("unreviewed.ox");\n'
        for name, context in self.contexts.items():
            with self.subTest(policy=name):
                self.put(name, gate.NATIVE_INPUT, changed)
                directives = context.library.source_directives(gate.NATIVE_INPUT, changed)
                with patch.object(gate, "REVIEWED_NATIVE_PIN", (len(changed), gate.sha(changed))), \
                        patch.object(gate, "REVIEWED_NATIVE_DIRECTIVES", directives), \
                        self.assertRaisesRegex(ValueError, "source asset"):
                    self.capture(name)

    def test_manifest_keeps_parent_policy_bytes_and_every_qualification_false(self):
        for name, context in self.contexts.items():
            with self.subTest(policy=name):
                inputs = self.capture(name)
                result = context.current_manifest(inputs, self.baseline, {"source_kind": "worktree"}, self.parents[name])
                self.assertEqual(result["source_policy_parent"], context.spec["parent"])
                self.assertEqual(len(result["files"]), context.spec["input_members"])
                self.assertEqual(result["source_policy_inputs"][context.spec["controller"]], context.spec["controller_pin"][1])
                self.assertEqual(result["source_policy_input_members"], 59 if name == "scalar-library" else 60)
                self.assertEqual(result["msvc_linker_reviewed_change"]["before"], gate.identity(gate.NATIVE_INPUT, self.before))
                self.assertEqual(result["msvc_linker_reviewed_change"]["after"], gate.identity(gate.NATIVE_INPUT, self.after))
                inherited = context.original_manifest(self.parents[name], self.baseline, {}, self.parents[name])
                claims = {key: value for key, value in result.items() if key.endswith("qualified_by_this_gate")}
                self.assertLessEqual({key for key in inherited if key.endswith("qualified_by_this_gate")}, set(claims))
                self.assertTrue(all(value is False for value in claims.values()))


class FrozenPolicyDelegation(unittest.TestCase):
    def test_fixed_controller_and_native_before_pins_are_the_exact_parent_objects(self):
        self.assertEqual(set(gate.SOURCE_POLICIES), {"scalar-library", "registry"})
        for name, spec in gate.SOURCE_POLICIES.items():
            with self.subTest(policy=name):
                body = git_body(spec["parent"], spec["controller"])
                self.assertEqual((len(body), gate.sha(body)), spec["controller_pin"])
                native = git_body(spec["parent"], gate.NATIVE_INPUT)
                self.assertEqual((len(native), gate.sha(native)), gate.BEFORE_NATIVE_PIN)
                self.assertEqual(frozen_context(name).library.source_directives(gate.NATIVE_INPUT, native),
                                 gate.BEFORE_NATIVE_DIRECTIVES)

    def test_selected_current_policy_is_required_and_loading_is_lazy(self):
        for name, spec in gate.SOURCE_POLICIES.items():
            with self.subTest(policy=name):
                path = ROOT / spec["controller"]
                if path.exists():
                    with patch.object(gate, "read_policy", wraps=gate.read_policy) as read:
                        selected = gate.load_policy(ROOT, name)
                    read.assert_called_once_with(ROOT, spec)
                    self.assertEqual(selected.name, name)
                else:
                    with self.assertRaises(OSError):
                        gate.load_policy(ROOT, name)
        with self.assertRaisesRegex(ValueError, "unknown"):
            gate.load_policy(ROOT, "foreign")

    def test_changed_or_symlinked_frozen_controller_is_not_executed(self):
        for name, spec in gate.SOURCE_POLICIES.items():
            with self.subTest(policy=name), tempfile.TemporaryDirectory(prefix="oxid-msvc-controller-") as folder:
                root = Path(folder)
                path = root / spec["controller"]
                path.parent.mkdir(parents=True)
                body = git_body(spec["parent"], spec["controller"])
                path.write_bytes(body)
                self.assertEqual(gate.read_policy(root, spec), body)
                path.write_bytes(body[:-1] + (b"x" if body[-1:] != b"x" else b"y"))
                with self.assertRaisesRegex(ValueError, "changed"):
                    gate.read_policy(root, spec)
                path.unlink()
                path.symlink_to(ROOT / spec["controller"])
                with self.assertRaisesRegex(ValueError, "symlink"):
                    gate.read_policy(root, spec)

    def test_six_seams_restore_on_refusal_and_all_predicate_objects_stay_original(self):
        names = ("PREDECESSOR", "PREDECESSOR_ADDED", "PREDECESSOR_POLICIES", "PINS_PROVISIONAL",
                 "capture_sources", "current_manifest")
        helpers = ("prepare_current_observer", "verify_current_observer", "verify_materialized",
                   "authenticate_observer_adapter", "authenticate_index_resource_adapter", "authenticate_parser_resource_adapter",
                   "run_byte_buffer_observer_controls", "run_file_observer_controls", "run_i64_observer_controls",
                   "run_integer_width_observer_controls", "run_integer128_observer_controls", "run_float_observer_controls",
                   "run_character_observer_controls", "run_vector_observer_controls", "run_record_vector_observer_controls")
        for name in gate.SOURCE_POLICIES:
            with self.subTest(policy=name):
                context = frozen_context(name)
                before = {key: getattr(context.library, key) for key in names}
                functions = {key: getattr(context.library, key) for key in helpers}
                with self.assertRaisesRegex(RuntimeError, "synthetic refusal"):
                    with context.bound():
                        self.assertIs(context.library.run, context.original_run)
                        self.assertEqual(context.library.PREDECESSOR, context.spec["parent"])
                        for key, function in functions.items():
                            self.assertIs(getattr(context.library, key), function)
                        raise RuntimeError("synthetic refusal")
                for key, value in before.items():
                    self.assertIs(getattr(context.library, key), value)
                self.assertIs(context.library.run, context.original_run)
        self.assertIn("status = selected.original_run(delegated)", inspect.getsource(gate.run))
        self.assertNotIn(".replace(", inspect.getsource(gate.run))

    def test_missing_pin_refuses_preparation_and_qualification_before_any_process(self):
        for name in gate.SOURCE_POLICIES:
            for prepare in (False, True):
                with self.subTest(policy=name, prepare_only=prepare), tempfile.TemporaryDirectory(prefix="oxid-msvc-missing-") as folder:
                    output = Path(folder) / "output"
                    args = argparse.Namespace(repo=ROOT, output=output, source_policy=name, prepare_only=prepare)
                    with patch.object(gate, "REVIEWED_NATIVE_PIN", None), patch.object(gate, "REVIEWED_NATIVE_DIRECTIVES", None), \
                            patch.object(subprocess, "run") as run, patch.object(subprocess, "Popen") as popen, \
                            patch.object(gate, "load_policy") as load, redirect_stderr(io.StringIO()):
                        self.assertEqual(gate.run(args), 1)
                    run.assert_not_called()
                    popen.assert_not_called()
                    load.assert_not_called()
                    result = json.loads((output / "failure.json").read_text())
                    self.assertIs(result["reviewed_native_pin_missing"], True)
                    self.assertEqual(result["compiler_executions"], 0)
                    self.assertIs(result["semantic_pass"], False)
                    self.assertIn("missing independently reviewed", result["error"])
                    self.assertFalse((output / "retained").exists())
                    self.assertFalse((output / "result.json").exists())
                    self.assertFalse((output / "prepared.json").exists())

    def test_provisional_full_request_refuses_before_controller_or_process(self):
        census = {"legacy": ([], []), "attributes": (), "macros": (), "c_preprocessor": (), "build_script": None}
        for name in gate.SOURCE_POLICIES:
            with self.subTest(policy=name), tempfile.TemporaryDirectory(prefix="oxid-msvc-provisional-") as folder:
                output = Path(folder) / "output"
                args = argparse.Namespace(repo=ROOT, output=output, source_policy=name, prepare_only=False)
                with patch.object(gate, "PINS_PROVISIONAL", True), patch.object(gate, "REVIEWED_NATIVE_PIN", (1, "0" * 64)), \
                        patch.object(gate, "REVIEWED_NATIVE_DIRECTIVES", census), \
                        patch.object(gate, "REVIEWED_NATIVE_SOURCE_COMMIT", "1" * 40), \
                        patch.object(subprocess, "run") as run, patch.object(subprocess, "Popen") as popen, \
                        patch.object(gate, "load_policy") as load, redirect_stderr(io.StringIO()):
                    self.assertEqual(gate.run(args), 1)
                run.assert_not_called()
                popen.assert_not_called()
                load.assert_not_called()
                result = json.loads((output / "failure.json").read_text())
                self.assertIn("prepare-only", result["error"])
                self.assertEqual(result["compiler_executions"], 0)
                self.assertFalse((output / "retained").exists())

    def test_final_after_pin_is_either_explicitly_missing_or_bound_to_its_reviewed_commit(self):
        if gate.REVIEWED_NATIVE_PIN is None or gate.REVIEWED_NATIVE_DIRECTIVES is None or gate.REVIEWED_NATIVE_SOURCE_COMMIT is None:
            with self.assertRaisesRegex(ValueError, "missing independently reviewed"):
                gate.reviewed_pins()
        else:
            pin, directives = gate.reviewed_pins()
            body = git_body(gate.REVIEWED_NATIVE_SOURCE_COMMIT, gate.NATIVE_INPUT)
            self.assertEqual((len(body), gate.sha(body)), pin)
            for name in gate.SOURCE_POLICIES:
                self.assertEqual(frozen_context(name).library.source_directives(gate.NATIVE_INPUT, body), directives)

    def test_reviewed_patch_keeps_exact_unix_cache_link_and_publication_sections(self):
        self.assertEqual(gate.REVIEWED_NATIVE_SOURCE_COMMIT, "563860bfef73254d460faebc3c88d8b34ed77ab2")
        before = git_body(gate.SOURCE_POLICIES["scalar-library"]["parent"], gate.NATIVE_INPUT)
        after = git_body(gate.REVIEWED_NATIVE_SOURCE_COMMIT, gate.NATIVE_INPUT)
        # Compare immutable source sections literally; no identifier/output
        # normalization or independent Rust selector is involved.
        cache = b"    let key = || {"
        link = b"    let mut link = if NativeBuildSettings::windows_msvc_host() {"
        self.assertEqual(before.split(cache, 1)[1].split(link, 1)[0],
                         after.split(cache, 1)[1].split(link, 1)[0])
        unix = b'    } else if cfg!(target_os = "macos") {'
        before_end = b'    let mut linker = OsString::from("--ld-path=");'
        after_end = b"    let (link_tool, link) =\n        link_tool_and_args("
        self.assertEqual(after.count(after_end), 1)
        self.assertEqual(before.split(link, 1)[1].split(unix, 1)[1].split(before_end, 1)[0],
                         after.split(link, 1)[1].split(unix, 1)[1].split(after_end, 1)[0])
        publication = b"    // Atomic no-replace publication on the same filesystem."
        self.assertEqual(before.split(publication, 1)[1].split(b"\n#[cfg(test)]", 1)[0],
                         after.split(publication, 1)[1].split(b"\n#[cfg(test)]", 1)[0])


if __name__ == "__main__":
    unittest.main()
