"""Mutation controls for the ARM64 host test corrections compiler-input boundary."""
import ast
import builtins
import hashlib
import inspect
import io
import json
from contextlib import redirect_stderr
from types import SimpleNamespace
import os
from pathlib import Path
import symtable
import subprocess
import tempfile
import unittest
from unittest.mock import patch

import verify_arm64_host_compatibility as gate

ROOT = Path(__file__).resolve().parents[1]


class Arm64HostTestsAdmission(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        self.baseline = {name: b"// original\n" for name in gate.ARM64_HOST_TESTS_CHANGED - gate.PARENT_ADDED}
        self.baseline.update({"native/runtime.c": b"/* runtime */\n", "Cargo.toml": b"[package]\n",
                              "src/frontend/oir/owned_types.rs": b"// unchanged type authority\n"})
        self.parent = {**self.baseline, **{name: b"// parent addition\n" for name in gate.PREDECESSOR_ADDED}}
        self.current = dict(self.parent)
        self.current.update({name: b"// reviewed ARM64 host test corrections\n" for name in gate.ARM64_HOST_TESTS_CHANGED | gate.ARM64_HOST_TESTS_ADDED})
        for name, body in self.current.items():
            self.put(name, body)
        reviewed = {name: (len(self.current[name]), gate.sha(self.current[name])) for name in gate.ARM64_HOST_TESTS_CHANGED | gate.ARM64_HOST_TESTS_ADDED}
        mocked = patch.object(gate, "ARM64_HOST_TESTS_INPUTS", reviewed)
        mocked.start()
        self.addCleanup(mocked.stop)
        directives = patch.object(gate, "REVIEWED_DIRECTIVES", {})
        directives.start()
        self.addCleanup(directives.stop)
        scope = patch.object(gate, "authenticate_test_only_scope")
        scope.start()
        self.addCleanup(scope.stop)

    def put(self, name, body):
        path = self.root / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(body)

    def capture(self):
        return gate.capture_sources(self.root, self.baseline, self.parent)

    def test_reviewed_delta_preserves_every_other_parent_byte_and_reports_bounded_scope(self):
        inputs = self.capture()
        self.assertEqual(set(inputs), set(self.parent) | gate.ARM64_HOST_TESTS_ADDED)
        for name in set(self.parent) - gate.ARM64_HOST_TESTS_CHANGED:
            self.assertEqual(inputs[name], self.parent[name])
        manifest = gate.current_manifest(inputs, self.baseline, {"source_kind": "worktree"}, self.parent)
        self.assertEqual({row["path"] for row in manifest["arm64_host_test_changes"]}, gate.ARM64_HOST_TESTS_CHANGED)
        self.assertEqual(manifest["source_policy_parent"], gate.PREDECESSOR)
        self.assertEqual({row["path"] for row in manifest["files"]}, set(inputs))
        for key, value in manifest.items():
            if key.endswith("qualified_by_this_gate"):
                self.assertIs(value, False, key)
        self.assertIs(manifest["program_arguments_semantics_qualified_by_this_gate"], False)

    def test_all_old_feature_allowances_stay_closed_outside_the_new_boundary(self):
        for name in set(self.parent) - gate.ARM64_HOST_TESTS_CHANGED:
            with self.subTest(name=name):
                self.put(name, self.parent[name] + b"// unreviewed\n")
                with self.assertRaisesRegex(ValueError, "outside arm64-host-tests boundary"):
                    self.capture()
                self.put(name, self.parent[name])

    def test_reviewed_lengths_hashes_and_exact_roster_are_all_required(self):
        name = "src/frontend/driver.rs"
        original = gate.ARM64_HOST_TESTS_INPUTS[name]
        for value in ((original[0] + 1, original[1]), (original[0], "0" * 64)):
            with self.subTest(value=value):
                gate.ARM64_HOST_TESTS_INPUTS[name] = value
                with self.assertRaisesRegex(ValueError, "reviewed input"):
                    self.capture()
        gate.ARM64_HOST_TESTS_INPUTS[name] = original
        del gate.ARM64_HOST_TESTS_INPUTS[name]
        with self.assertRaisesRegex(ValueError, "reviewed arm64-host-tests input membership"):
            self.capture()
        gate.ARM64_HOST_TESTS_INPUTS[name] = original
        gate.ARM64_HOST_TESTS_INPUTS["src/extra.rs"] = original
        with self.assertRaisesRegex(ValueError, "reviewed arm64-host-tests input membership"):
            self.capture()

    def test_extra_missing_and_empty_directory_members_are_rejected(self):
        self.put("native/extra.h", b"/* extra */\n")
        with self.assertRaisesRegex(ValueError, "membership"):
            self.capture()
        (self.root / "native/extra.h").unlink()
        name = "src/frontend/stdin_public_tests.rs"
        (self.root / name).unlink()
        with self.assertRaisesRegex(ValueError, "membership"):
            self.capture()
        self.put(name, self.current[name])
        (self.root / "src/unlisted-empty").mkdir()
        with self.assertRaisesRegex(ValueError, "directory"):
            self.capture()

    def test_reviewed_arm64_host_test_changes_are_required(self):
        self.assertEqual(gate.ARM64_HOST_TESTS_ADDED, set())
        name = "src/frontend/driver.rs"
        self.put(name, self.parent[name])
        with self.assertRaisesRegex(ValueError, "reviewed input"):
            self.capture()

    def test_missing_parent_members_cannot_redefine_the_authority(self):
        name = next(iter(gate.PREDECESSOR_ADDED))
        del self.parent[name]
        with self.assertRaisesRegex(ValueError, "predecessor compiler input membership"):
            self.capture()

    def test_even_a_coherently_changed_pin_cannot_add_compile_time_inputs(self):
        name = "src/frontend/driver.rs"
        body = self.current[name] + b'const X:&str=include_str!("/outside.txt");\n'
        self.put(name, body)
        gate.ARM64_HOST_TESTS_INPUTS[name] = (len(body), gate.sha(body))
        with self.assertRaisesRegex(ValueError, "compile-time input directive"):
            self.capture()

    @unittest.skipIf(os.name == "nt", "Unix source link and mode checks")
    def test_linked_and_executable_sources_cannot_enter(self):
        name = "src/frontend/stdin_public_tests.rs"
        path = self.root / name
        outside = self.root / "outside"
        outside.write_bytes(self.current[name])
        path.unlink()
        path.symlink_to(outside)
        with self.assertRaisesRegex(ValueError, "symlink"):
            self.capture()
        path.unlink()
        os.link(outside, path)
        with self.assertRaisesRegex(ValueError, "nonregular"):
            self.capture()
        path.unlink()
        self.put(name, self.current[name])
        path.chmod(0o755)
        with self.assertRaisesRegex(ValueError, "nonregular"):
            self.capture()


class Arm64HostTestsPolicyAuthority(unittest.TestCase):
    def test_parent_library_is_authenticated_before_executing_python(self):
        expected = (ROOT / gate.PREDECESSOR_POLICY).read_bytes()
        self.assertEqual(gate.sha(expected), gate.PREDECESSOR_POLICY_SHA)
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            path = root / gate.PREDECESSOR_POLICY
            path.parent.mkdir(parents=True)
            for body in (expected + b"\n", expected.replace(b"import argparse", b"import subprocess", 1),
                         b'raise RuntimeError("untrusted code executed")\n'):
                path.write_bytes(body)
                with self.assertRaisesRegex(ValueError, "changed pinned native-debug"):
                    gate.load_predecessor(root)

    def test_all_thirty_preceding_policy_files_are_unchanged_and_authenticated(self):
        self.assertEqual(len(gate.PREDECESSOR_POLICIES), 30)
        with tempfile.TemporaryDirectory() as directory:
            repo, parent = Path(directory) / "repo", Path(directory) / "parent"
            for name, digest in gate.PREDECESSOR_POLICIES.items():
                body = (ROOT / name).read_bytes()
                self.assertEqual(gate.sha(body), digest)
                for root in (repo, parent):
                    path = root / name
                    path.parent.mkdir(parents=True, exist_ok=True)
                    path.write_bytes(body)
            gate.authenticate_policies(repo, parent)
            for name in gate.PREDECESSOR_POLICIES:
                body = (ROOT / name).read_bytes()
                (repo / name).write_bytes(body + b"# changed\n")
                with self.assertRaisesRegex(ValueError, "source policy changed"):
                    gate.authenticate_policies(repo, parent)
                (repo / name).write_bytes(body)
                (parent / name).write_bytes(body + b"# changed\n")
                with self.assertRaisesRegex(ValueError, "wrong pinned predecessor"):
                    gate.authenticate_policies(repo, parent)
                (parent / name).write_bytes(body)

    def test_parent_capture_requires_the_actual_pinned_git_blob(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            path = root / "src/a.rs"
            path.parent.mkdir()
            body = b"// frozen source\n"
            path.write_bytes(body)
            blob = b"blob " + str(len(body)).encode() + b"\0" + body
            digest = hashlib.sha1(blob, usedforsecurity=False).hexdigest().encode()
            row = b"100644 blob " + digest + b" " + str(len(body)).encode() + b"\tsrc/a.rs\0"
            with patch.object(gate, "git", return_value=row):
                self.assertEqual(gate.capture_parent(root, root, {"src/a.rs"}), {"src/a.rs": body})
                path.write_bytes(body.replace(b"source", b"SOURCE"))
                with self.assertRaisesRegex(ValueError, "pinned Git blob"):
                    gate.capture_parent(root, root, {"src/a.rs"})
                path.write_bytes(body)
            for changed in (row.replace(b"100644", b"100755"), row.replace(digest, b"0" * 40),
                            row.replace(str(len(body)).encode() + b"\t", b"0\t")):
                with patch.object(gate, "git", return_value=changed):
                    with self.assertRaisesRegex(ValueError, "pinned Git blob"):
                        gate.capture_parent(root, root, {"src/a.rs"})

    def test_helpers_and_observer_controls_are_the_unchanged_authenticated_parent_functions(self):
        self.assertEqual(ast.dump(ast.parse(inspect.getsource(gate.regular))),
                         ast.dump(ast.parse(inspect.getsource(gate.library.regular))))
        for name in ("extract_baseline", "checkout_identity", "verify_materialized",
                     "verify_current_observer", "run_byte_buffer_observer_controls"):
            self.assertIs(getattr(gate, name), getattr(gate.library, name))
        self.assertEqual(gate.ARM64_HOST_TESTS_CHANGED, {'src/frontend/stdin_public_tests.rs', 'src/frontend/driver.rs', 'src/frontend/oir/source/hir_import/emit_failure_tests.rs', 'src/frontend/oir/source/hir_import/emit_resource_tests.rs', 'src/frontend/oir/source/hir_import/emit_tests.rs'})
        self.assertEqual(gate.ARM64_HOST_TESTS_ADDED, set())
        self.assertIs(gate.run_character_observer_controls, gate.library.run_character_observer_controls)
        self.assertEqual(set(gate.ARM64_HOST_TESTS_INPUTS), gate.ARM64_HOST_TESTS_CHANGED | gate.ARM64_HOST_TESTS_ADDED)
        self.assertEqual(gate.PREDECESSOR_ADDED, gate.library.PARENT_ADDED)
        self.assertEqual(gate.PARENT_ADDED, gate.PREDECESSOR_ADDED | gate.ARM64_HOST_TESTS_ADDED)

    def parser_resource(self):
        body = (ROOT / "tests/fixtures/typed_project_unit2_independent/archive/resource/parser-resource-review-tests.rs").read_bytes()
        self.assertEqual((len(body), gate.sha(body)), (1951, "5a26942d2864979a430b28aa5714ceea43edf8e80cf9a6f7c789d66c12a11b73"))
        before = b"mode:SourceMode::ProjectCandidate,tokens,"
        after = b"mode:SourceMode::ProjectCandidate,project_recovery:false,arrays:ArraySyntaxPolicy::Closed,enums:EnumSyntaxPolicy::Closed,std_imports:StdImportPolicy::Closed,storage:enums::SyntaxStorage::default(),tokens,"
        self.assertEqual(body.count(before), 1)
        return body.replace(before, after, 1)

    def test_parser_resource_initializes_only_empty_metadata_and_preserves_all_old_assertions(self):
        before = self.parser_resource()
        after = gate.adapt_parser_resource(before)
        self.assertEqual(after.replace(gate.PARSER_RESOURCE_AFTER, gate.PARSER_RESOURCE_BEFORE, 1), before)
        self.assertEqual(after.count(b"generic_headers:Vec::new(),"), 1)
        self.assertEqual(after.count(b"generic_calls:Vec::new(),"), 1)
        self.assertEqual(after.count(b"pending_equal:None,"), 1)
        self.assertEqual(after.count(b"assert"), before.count(b"assert"))
        for changed in (before + b"\n", before.replace(b"assert_eq!", b"assert_ne!", 1), after):
            with self.subTest(changed=gate.sha(changed)):
                with self.assertRaisesRegex(ValueError, "parser resource"):
                    gate.adapt_parser_resource(changed)

    def test_parser_resource_wrapper_passes_only_the_authenticated_successor(self):
        captured = {"resource": self.parser_resource(), "unchanged": object()}
        sentinel = object()
        with patch.object(gate, "prepare_with_parser_resource", return_value=sentinel) as prepare:
            self.assertIs(gate.prepare_current_observer("original", "output", captured, "adapter"), sentinel)
        prepare.assert_called_once_with("original", "output", captured, "adapter", gate.adapt_parser_resource(captured["resource"]))

    def test_selected_controller_enters_qualification_before_checkout_validation(self):
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / "qualification"
            args = SimpleNamespace(repo=ROOT, output=output, expected_head="not-evaluated", allow_dirty=False)
            with patch.object(gate, "checkout_identity", side_effect=ValueError("checkout marker")) as checkout:
                with redirect_stderr(io.StringIO()):
                    self.assertEqual(gate.run(args), 1)
            checkout.assert_called_once_with(ROOT, "not-evaluated", False)
            report = json.loads((output / "failure.json").read_text())
            self.assertEqual(report["error"], "ValueError: checkout marker")
            self.assertEqual(report["controller"]["path"], "scripts/verify_arm64_host_compatibility.py")

    def test_other_checkout_cannot_enter_this_controllers_qualification(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            output = root / "qualification"
            args = SimpleNamespace(repo=root, output=output)
            with self.assertRaisesRegex(ValueError, "selected checkout"):
                gate.run(args)
            self.assertFalse(output.exists())

    def test_every_controller_function_global_resolves_in_both_execution_modes(self):
        table = symtable.symtable(Path(gate.__file__).read_text(), gate.__file__, "exec")
        available = set(vars(gate)) | set(vars(builtins))
        for child in table.get_children():
            if child.get_type() == "function":
                self.assertFalse(set(child.get_globals()) - available, child.get_name())





class Arm64HostTestScope(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        names = gate.ARM64_HOST_TESTS_CHANGED | {"src/frontend/mod.rs", "src/frontend/oir/source/hir_import.rs"}
        cls.parent = {name: subprocess.check_output(["git", "show", gate.PREDECESSOR + ":" + name], cwd=ROOT) for name in names}
        cls.current = {name: (ROOT / name).read_bytes() for name in names}

    def test_actual_corrections_are_confined_to_test_compilation(self):
        gate.authenticate_test_only_scope(self.parent, self.current)

    def test_module_edges_cannot_enable_tests_in_production(self):
        name = "src/frontend/mod.rs"
        changed = dict(self.current)
        changed[name] = changed[name].replace(b"#[cfg(test)]\nmod stdin_public_tests;", b"mod stdin_public_tests;", 1)
        with self.assertRaisesRegex(ValueError, "test-only module owner"):
            gate.authenticate_test_only_scope(self.parent, changed)
        parent = dict(self.parent)
        parent[name] = changed[name]
        with self.assertRaisesRegex(ValueError, "test-only module edge"):
            gate.authenticate_test_only_scope(parent, changed)

    def test_driver_production_prefix_and_other_tests_remain_closed(self):
        name = "src/frontend/driver.rs"
        for body in (b"pub const UNREVIEWED:u8=0;\n" + self.current[name], self.current[name] + b"\n// unreviewed\n"):
            changed = dict(self.current)
            changed[name] = body
            with self.assertRaisesRegex(ValueError, "test-only conditional"):
                gate.authenticate_test_only_scope(self.parent, changed)


if __name__ == "__main__":
    unittest.main()
