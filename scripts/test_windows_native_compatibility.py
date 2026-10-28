"""Closed source admission and complete immutable Copy debug replay; no compiler runs."""
import argparse
import ast
import hashlib
import inspect
import json
import os
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch

import verify_windows_native_compatibility as gate

ROOT = Path(__file__).resolve().parents[1]


def git_body(commit, name):
    return subprocess.check_output(["git", "show", commit + ":" + name], cwd=ROOT)


def baseline_names():
    body = (ROOT / gate.BINDING / "current-source.json").read_bytes()
    if gate.sha(body) != gate.BASELINE_SOURCE_SHA:
        raise AssertionError("historical source manifest changed")
    return {row["path"] for row in json.loads(body)["files"]}


class WindowsNativeAdmission(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory(prefix="oxid-Windows-native-policy-")
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        self.baseline = {name: b"// fixed historical input\n" for name in baseline_names()}
        self.parent = {**self.baseline,
                       **{name: b"// fixed R2 addition\n" for name in gate.PREDECESSOR_ADDED}}
        for name in gate.UNCHANGED_INPUTS:
            self.parent[name] = git_body(gate.PREDECESSOR, name)
        self.current = dict(self.parent)
        self.current.update({name: b"// reviewed Windows-native input\n"
                             for name in gate.WINDOWS_NATIVE_CHANGED | gate.WINDOWS_NATIVE_ADDED})
        self.changed = "src/frontend/native.rs"
        self.current[self.changed] = b"#[test]\nfn first(){}\n#[test]\nfn second(){}\n"
        self.current["native/typed_preview.c"] = b"#ifdef _WIN32\nint win;\n#else\nint posix;\n#endif\n"
        for name, body in self.current.items():
            self.put(name, body)
        for name, value in (
            ("REVIEWED_INPUTS", {name: (len(self.current[name]), gate.sha(self.current[name]))
                                 for name in gate.WINDOWS_NATIVE_CHANGED | gate.WINDOWS_NATIVE_ADDED}),
            ("REVIEWED_DIRECTIVES", {name: gate.source_directives(name, self.current[name])
                                     for name in gate.WINDOWS_NATIVE_CHANGED | gate.WINDOWS_NATIVE_ADDED}),
        ):
            mocked = patch.object(gate, name, value)
            mocked.start()
            self.addCleanup(mocked.stop)

    def put(self, name, body):
        path = self.root / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(body)

    def capture(self):
        return gate.capture_sources(self.root, self.baseline, self.parent)

    def test_exact_Copy_debug_roster_and_only_two_windows_modules_are_admitted(self):
        inputs = self.capture()
        self.assertEqual(len(self.baseline), 376)
        self.assertEqual(len(self.parent), 510)
        self.assertEqual(len(inputs), 512)
        self.assertEqual(gate.WINDOWS_NATIVE_ADDED, {
            "src/frontend/native/doctor_windows.rs", "src/frontend/windows_native.rs",
        })
        self.assertEqual(gate.WINDOWS_NATIVE_CHANGED, {
            "native/typed_preview.c", "src/frontend/build_config.rs", "src/frontend/driver.rs",
            "src/frontend/mod.rs", "src/frontend/native.rs", "src/frontend/native/doctor.rs",
            "src/frontend/oir/native.rs", "src/frontend/oir/owned/native.rs",
            "src/frontend/oir/source/sealed.rs",
            "src/frontend/project/filesystem/windows.rs",
            "src/project_cli.rs", "src/project_cli/workspace.rs",
        })
        self.assertEqual(set(inputs), set(self.parent) | gate.WINDOWS_NATIVE_ADDED)
        for name in set(self.parent) - gate.WINDOWS_NATIVE_CHANGED:
            self.assertEqual(inputs[name], self.parent[name])

    def test_manifest_binds_all_55_policies_and_keeps_every_claim_false(self):
        inputs = self.capture()
        manifest = gate.current_manifest(inputs, self.baseline, {"source_kind": "worktree"}, self.parent)
        self.assertEqual(manifest["source_policy_input_members"], 55)
        self.assertEqual(manifest["predecessor_compiler_input_members"], 510)
        self.assertEqual(manifest["source_policy_parent"], "9a5ecaaeb5fa50aff2296bf990d63167143cf312")
        self.assertEqual({row["path"] for row in manifest["files"]}, set(self.current))
        self.assertEqual(len(manifest["files"]), 512)
        self.assertEqual(manifest["windows_native_pins_provisional"], gate.PINS_PROVISIONAL)
        claims = {key: value for key, value in manifest.items() if key.endswith("qualified_by_this_gate")}
        inherited = gate.library.current_manifest(self.parent, self.baseline, {}, self.parent)
        self.assertLessEqual({key for key in inherited if key.endswith("qualified_by_this_gate")}, set(claims))
        for name in ("copy_record_vectors_semantics", "windows_module_sources_semantics",
                     "windows_activation", "qualified_module_completion_semantics",
                     "copy_record_debug_semantics"):
            self.assertIs(claims[name + "_qualified_by_this_gate"], False)
        self.assertTrue(claims)
        for key, value in claims.items():
            self.assertIs(value, False, key)
        for name in gate.UNCHANGED_INPUTS:
            self.assertEqual(manifest["retained_protected_inputs"][name], gate.identity(name, self.parent[name]))

    def test_windows_platform_doctor_PE_and_debug_claims_stay_false_after_pin_approval(self):
        inputs = self.capture()
        with patch.object(gate, "PINS_PROVISIONAL", False):
            manifest = gate.current_manifest(inputs, self.baseline, {}, self.parent)
        self.assertIs(manifest["windows_native_pins_provisional"], False)
        for key in (
            "windows_native_semantics_qualified_by_this_gate",
            "win32_native_doctor_semantics_qualified_by_this_gate",
            "windows_native_platform_semantics_qualified_by_this_gate",
            "windows_native_debug_semantics_qualified_by_this_gate",
            "msvc_pe_execution_qualified_by_this_gate",
            "copy_record_debug_semantics_qualified_by_this_gate",
            "qualified_module_completion_semantics_qualified_by_this_gate",
        ):
            self.assertIs(manifest[key], False, key)
        self.assertIs(manifest["debug_information_enabled_for_retained_claim"], False)

    def test_C_preprocessor_order_and_multiplicity_cannot_be_re_pinned_away(self):
        name = "native/typed_preview.c"
        original = gate.REVIEWED_DIRECTIVES[name]
        directives = original["c_preprocessor"]
        self.assertEqual(len(directives), 3)
        for changed in (tuple(reversed(directives)), directives + directives[:1], directives[1:]):
            gate.REVIEWED_DIRECTIVES[name] = {**original, "c_preprocessor": changed}
            with self.subTest(changed=changed), self.assertRaisesRegex(ValueError, "directive changed"):
                self.capture()
        gate.REVIEWED_DIRECTIVES[name] = original

    def test_source_hash_and_length_are_both_required(self):
        self.put(self.changed, self.current[self.changed] + b"// changed\n")
        with self.assertRaisesRegex(ValueError, "reviewed input"):
            self.capture()
        self.put(self.changed, self.current[self.changed])
        length, digest = gate.REVIEWED_INPUTS[self.changed]
        gate.REVIEWED_INPUTS[self.changed] = (length + 1, digest)
        with self.assertRaisesRegex(ValueError, "reviewed input"):
            self.capture()

    def test_ordered_repeated_compile_time_directives_stay_complete(self):
        original = gate.REVIEWED_DIRECTIVES[self.changed]
        self.assertEqual(original["attributes"], ((b"#[test]", 2),))
        for attributes in (((b"#[test]", 1),), ((b"#[test]", 3),),
                           ((b"#[ignore]", 1), (b"#[test]", 1)),
                           ((b"#[test]", 1), (b"#[ignore]", 1))):
            gate.REVIEWED_DIRECTIVES[self.changed] = {**original, "attributes": attributes}
            with self.subTest(attributes=attributes), self.assertRaisesRegex(ValueError, "directive changed"):
                self.capture()
        for field in ("macros", "c_preprocessor", "legacy", "build_script"):
            gate.REVIEWED_DIRECTIVES[self.changed] = {**original, field: ((b"unreviewed", 1),)}
            with self.subTest(field=field), self.assertRaisesRegex(ValueError, "directive changed"):
                self.capture()
        gate.REVIEWED_DIRECTIVES[self.changed] = original

    def test_reviewed_pin_and_directive_membership_must_be_exact(self):
        pin = gate.REVIEWED_INPUTS.pop(self.changed)
        with self.assertRaisesRegex(ValueError, "reviewed Windows-native input membership"):
            self.capture()
        gate.REVIEWED_INPUTS[self.changed] = pin
        directives = gate.REVIEWED_DIRECTIVES.pop(self.changed)
        with self.assertRaisesRegex(ValueError, "directive membership"):
            self.capture()
        gate.REVIEWED_DIRECTIVES[self.changed] = directives
        gate.REVIEWED_DIRECTIVES["src/unreviewed.rs"] = directives
        with self.assertRaisesRegex(ValueError, "directive membership"):
            self.capture()

    def test_unchanged_source_stays_closed(self):
        name = "src/frontend/ast.rs"
        self.put(name, self.parent[name] + b"// changed\n")
        with self.assertRaisesRegex(ValueError, "outside Windows-native boundary"):
            self.capture()

    def test_windows_file_sharing_member_requires_its_exact_reviewed_pin(self):
        name = "src/frontend/project/filesystem/windows.rs"
        self.assertIn(name, gate.WINDOWS_NATIVE_CHANGED)
        self.put(name, self.current[name] + b"// unreviewed sharing behavior\n")
        with self.assertRaisesRegex(ValueError, "does not match reviewed input"):
            self.capture()
        self.put(name, self.current[name])
        with patch.object(gate, "WINDOWS_NATIVE_CHANGED", gate.WINDOWS_NATIVE_CHANGED - {name}):
            with self.assertRaisesRegex(ValueError, "reviewed Windows-native input membership"):
                self.capture()

    def test_pure_test_adapter_and_sealed_entry_changes_require_hash_and_directives(self):
        self.assertEqual(self.capture(), self.current)
        for name in ("src/frontend/driver.rs", "src/frontend/windows_native.rs",
                     "src/frontend/oir/source/sealed.rs", "src/project_cli.rs"):
            original = self.current[name]
            changed = original + b"#[cfg(any())]\nfn altered_adapter(){}\n"
            self.put(name, changed)
            with self.subTest(name=name, boundary="hash"), self.assertRaisesRegex(
                    ValueError, "does not match reviewed input"):
                self.capture()
            # A replacement hash alone cannot hide a changed executable guard.
            with patch.dict(gate.REVIEWED_INPUTS, {name: (len(changed), gate.sha(changed))}):
                with self.subTest(name=name, boundary="directives"), self.assertRaisesRegex(
                        ValueError, "directive changed"):
                    self.capture()
            self.put(name, original)
        self.assertEqual(self.capture(), self.current)

    def test_already_accepted_windows_dispatch_bytes_cannot_change(self):
        name, = gate.UNCHANGED_INPUTS
        self.put(name, self.parent[name] + b"// changed\n")
        with self.assertRaisesRegex(ValueError, "outside Windows-native boundary"):
            self.capture()

    def test_protected_member_cannot_enter_generic_reviewed_delta(self):
        name, = gate.UNCHANGED_INPUTS
        with patch.object(gate, "WINDOWS_NATIVE_CHANGED", gate.WINDOWS_NATIVE_CHANGED | {name}):
            with self.assertRaisesRegex(ValueError, "protected input cannot enter"):
                self.capture()

    def test_extra_and_missing_compiler_members_are_refused(self):
        extra = self.root / "src/unreviewed.rs"
        self.put("src/unreviewed.rs", b"// extra\n")
        with self.assertRaisesRegex(ValueError, "membership"):
            self.capture()
        extra.unlink()
        (self.root / sorted(gate.WINDOWS_NATIVE_ADDED)[0]).unlink()
        with self.assertRaisesRegex(ValueError, "membership"):
            self.capture()

    def test_empty_unlisted_directory_is_refused(self):
        (self.root / "src/unreviewed").mkdir()
        with self.assertRaisesRegex(ValueError, "directory"):
            self.capture()

    def test_parent_member_loss_cannot_lower_the_roster(self):
        name = sorted(gate.PREDECESSOR_ADDED)[0]
        del self.parent[name]
        with self.assertRaisesRegex(ValueError, "Copy debug compiler input membership"):
            self.capture()
        with patch.object(gate, "PREDECESSOR_ADDED", gate.PREDECESSOR_ADDED - {name}):
            with self.assertRaisesRegex(ValueError, "Copy debug compiler input membership"):
                self.capture()

    def test_source_symlink_hardlink_and_executable_are_refused(self):
        path = self.root / self.changed
        alias = self.root / "aliased-source"
        path.rename(alias)
        path.symlink_to(alias)
        with self.assertRaisesRegex(ValueError, "symlink"):
            self.capture()
        path.unlink()
        path.write_bytes(self.current[self.changed])
        alias.unlink()
        os.link(path, alias)
        with self.assertRaisesRegex(ValueError, "linked"):
            self.capture()
        alias.unlink()
        path.chmod(0o755)
        with self.assertRaisesRegex(ValueError, "executable"):
            self.capture()


class FrozenCopyDebugReplay(unittest.TestCase):
    def test_all_fourteen_reviewed_inputs_match_the_frozen_source_bytes_and_directives(self):
        frozen = "3d9648f3d2a87fb585adecccd73e55ff7f950a82"
        self.assertEqual(len(gate.WINDOWS_NATIVE_CHANGED), 12)
        self.assertEqual(len(gate.WINDOWS_NATIVE_ADDED), 2)
        self.assertEqual(set(gate.REVIEWED_INPUTS), gate.WINDOWS_NATIVE_CHANGED | gate.WINDOWS_NATIVE_ADDED)
        self.assertEqual(set(gate.REVIEWED_DIRECTIVES), set(gate.REVIEWED_INPUTS))
        for name, pin in gate.REVIEWED_INPUTS.items():
            body = git_body(frozen, name)
            self.assertEqual(gate.regular(ROOT, name), body, name)
            self.assertEqual((len(body), gate.sha(body)), pin, name)
            self.assertEqual(gate.source_directives(name, body), gate.REVIEWED_DIRECTIVES[name], name)

    def test_pure_test_selection_keeps_the_qualified_internal_invocation_restrictions(self):
        current = gate.regular(ROOT, "src/project_cli.rs").decode()
        previous = git_body(gate.PREDECESSOR, "src/project_cli.rs").decode()

        def section(body, start, end):
            begin = body.index(start)
            return body[begin:body.index(end, begin)]

        # These original rules reject unknown/duplicate flags, explicit source
        # operands, missing edition/process, JSON and workspace/package routing.
        for start, end in (
            ("    fn scan(args:", "    fn json(&self)"),
            ("    if options.project_test_seen", "    if options.invalid ||"),
            ("fn project_check_run(", "fn lock_snapshot("),
        ):
            self.assertEqual(section(current, start, end), section(previous, start, end))
        selection = (
            "    let pure_test = frontend::NativeBuildSettings::windows_msvc_host()\n"
            "        && matches!(command, frontend::PackageCommand::Run)\n"
            "        && options.project_test.is_some();"
        )
        self.assertEqual(current.count(selection), 1)
        self.assertEqual(current.count("prepared.execute_test("), 1)
        self.assertIn("    if pure_test {\n"
                      "        return Ok(prepared.execute_test(execution_policy(&snapshot.manifest)?));\n"
                      "    }", current)
        driver = gate.regular(ROOT, "src/frontend/driver.rs").decode()
        adapter = section(driver, "    pub(crate) fn execute_test(",
                          "    /// Consume the retained input logically")
        self.assertLess(adapter.index("let checked = self.checked()?"),
                        adapter.index("super::windows_native::test_preflight(project, execution)"))
        self.assertLess(adapter.index("test_preflight(project, execution)"),
                        adapter.index("checked.run_test_i32()"))
        self.assertIn("report(project.sources(), errors, false, Summary::Run(None))", adapter)
        for forbidden in ("process::", "native::compile", "parse(", "check_source("):
            self.assertNotIn(forbidden, adapter)

    def test_pure_tests_preserve_the_original_typed_entry_rule_and_scalar_denials(self):
        current = gate.regular(ROOT, "src/frontend/oir/source/sealed.rs").decode()
        previous = git_body("ea6e25b1fd9b8ca1df5d302b5fa80b0d0e7ba846",
                            "src/frontend/oir/source/sealed.rs").decode()

        def rule(body, start, end):
            begin = body.index("        let signature =", body.index(start))
            return body[begin:body.index(end, begin)].rstrip()

        message = '"process entry requires original-root fn main() -> i32 with no parameters"'
        self.assertEqual(rule(previous, "    fn checked_process_entry(", "\n    fn require_process_host("),
                         rule(current, "    fn checked_i32_entry(", "\n    /// A pure declared test")
                         .replace("                message,", "                " + message + ","))
        wrapper = current[current.index("    fn checked_process_entry("):
                          current.index("    fn checked_i32_entry(")]
        self.assertIn("self.checked_i32_entry(\n            " + message, wrapper)
        pure = current[current.index("    pub(in crate::frontend) fn run_test_i32("):
                       current.index("    fn require_process_host(")]
        self.assertLess(pure.index("self.checked_i32_entry("), pure.index("self.run_result("))
        self.assertIn("self.run_result(crate::frontend::build_config::ExecutionPolicy::Bounded)?", pure)
        self.assertIn("RuntimeValue::Scalar(RuntimeScalar::Narrow(Scalar::I32(status))) => Ok(status)", pure)
        self.assertNotIn("process::", pure)
        scope = gate.regular(ROOT, "src/frontend/windows_native.rs").decode()
        previous = git_body("ea6e25b1fd9b8ca1df5d302b5fa80b0d0e7ba846",
                            "src/frontend/windows_native.rs").decode()
        self.assertEqual(scope[scope.index("fn outside_scalar_scope("):scope.index("fn reject()")],
                         previous[previous.index("fn outside_scalar_scope("):previous.index("fn reject()")])
        preflight = scope[scope.index("pub(super) fn test_preflight("):
                          scope.index("fn outside_project_scope(")]
        self.assertLess(preflight.index("if execution.hosted()"),
                        preflight.index("if outside_project_scope(project)"))
        self.assertNotIn("windows_msvc_host()", preflight)

    def test_all_55_predecessor_policies_and_adapters_are_exact(self):
        self.assertEqual(len(gate.PREDECESSOR_POLICIES), 55)
        self.assertEqual(gate.PREDECESSOR_POLICIES, {
            **gate.library.PREDECESSOR_POLICIES,
            "scripts/verify_copy_record_debug_compatibility.py": gate.PREDECESSOR_POLICY_PIN[1],
        })
        for name, expected in gate.PREDECESSOR_POLICIES.items():
            body = git_body(gate.PREDECESSOR, name)
            self.assertEqual(gate.sha(body), expected, name)
            self.assertEqual(gate.regular(ROOT, name), body, name)
        old = git_body(gate.PREDECESSOR, gate.PREDECESSOR_POLICY)
        self.assertEqual((len(old), gate.sha(old)), gate.PREDECESSOR_POLICY_PIN)

    def test_policy_authentication_refuses_current_and_shared_parent_mutation(self):
        with tempfile.TemporaryDirectory() as directory:
            parent, current = Path(directory) / "parent", Path(directory) / "current"
            for name in gate.PREDECESSOR_POLICIES:
                body = git_body(gate.PREDECESSOR, name)
                for root in (parent, current):
                    (root / name).parent.mkdir(parents=True, exist_ok=True)
                    (root / name).write_bytes(body)
            gate.authenticate_policies(current, parent)
            for name in (gate.PREDECESSOR_POLICY, gate.library.OBSERVER_ADAPTER,
                         "scripts/record_vector_dispatch_activation.py", sorted(gate.library.PREDECESSOR_POLICIES)[0]):
                original = (current / name).read_bytes()
                (current / name).write_bytes(original + b"# changed\n")
                with self.subTest(name=name), self.assertRaisesRegex(ValueError, "policy changed"):
                    gate.authenticate_policies(current, parent)
                (parent / name).write_bytes(original + b"# changed\n")
                with self.subTest(name=name), self.assertRaisesRegex(ValueError, "wrong pinned predecessor"):
                    gate.authenticate_policies(current, parent)
                (current / name).write_bytes(original)
                (parent / name).write_bytes(original)

    def test_pinned_policy_is_not_executed_after_a_hash_or_mode_mutation(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            path = root / gate.PREDECESSOR_POLICY
            path.parent.mkdir(parents=True)
            body = git_body(gate.PREDECESSOR, gate.PREDECESSOR_POLICY)
            path.write_bytes(body + b"# changed\n")
            with patch.object(gate, "exec", create=True) as execute:
                with self.assertRaisesRegex(ValueError, "changed pinned Copy debug policy"):
                    gate.load_predecessor(root)
                execute.assert_not_called()
            path.write_bytes(body)
            path.chmod(0o755)
            with patch.object(gate, "exec", create=True) as execute:
                with self.assertRaisesRegex(ValueError, "unsafe pinned Copy debug policy"):
                    gate.load_predecessor(root)
                execute.assert_not_called()

    def test_parent_blob_mode_and_identity_bind_the_fixed_qualified_predecessor(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            body = b"// parent source\n"
            (root / "source.rs").write_bytes(body)
            oid = hashlib.sha1(b"blob " + str(len(body)).encode() + b"\0" + body, usedforsecurity=False).hexdigest()
            row = f"100644 blob {oid} {len(body)}\tsource.rs\0".encode()
            with patch.object(gate, "git", return_value=row) as git:
                self.assertEqual(gate.capture_parent(ROOT, root, {"source.rs"}), {"source.rs": body})
            git.assert_called_once_with(ROOT, "ls-tree", "-rzl", "--full-tree", gate.PREDECESSOR)
            for changed in (row.replace(b"100644", b"100755"), row.replace(oid.encode(), b"0" * 40),
                            row.replace(str(len(body)).encode(), str(len(body)+1).encode(), 1)):
                with patch.object(gate, "git", return_value=changed), self.assertRaisesRegex(ValueError, "pinned Git blob"):
                    gate.capture_parent(ROOT, root, {"source.rs"})

    def test_both_complete_Copy_debug_replay_blocks_and_every_selector_are_unchanged(self):
        def replay(body):
            tree = ast.parse(body)
            run = next(node for node in tree.body if isinstance(node, ast.FunctionDef) and node.name == "run")
            return [node for node in ast.walk(run) if isinstance(node, ast.If)
                    and ast.unparse(node.test) == "not args.prepare_only"]
        current = replay((ROOT / "scripts/verify_windows_native_compatibility.py").read_bytes())
        predecessor = replay(git_body(gate.PREDECESSOR, gate.PREDECESSOR_POLICY))
        self.assertEqual(len(current), 2)
        self.assertEqual(len(predecessor), 2)
        self.assertEqual([ast.dump(node, include_attributes=False) for node in current],
                         [ast.dump(node, include_attributes=False) for node in predecessor])
        self.assertGreater(len(current[0].body), 10)
        self.assertIn("run_record_vector_observer_controls", ast.unparse(current[1]))

    def test_every_Copy_debug_control_function_and_resource_preparation_are_unmodified(self):
        for name in (
            "run_byte_buffer_observer_controls", "run_file_observer_controls", "run_i64_observer_controls",
            "run_integer_width_observer_controls", "run_integer128_observer_controls",
            "run_float_observer_controls", "run_character_observer_controls", "run_vector_observer_controls",
            "run_record_vector_observer_controls", "authenticate_index_resource_adapter",
            "authenticate_parser_resource_adapter", "verify_current_observer", "verify_materialized",
        ):
            self.assertIs(getattr(gate, name), getattr(gate.library, name), name)
        self.assertIn("return library.prepare_current_observer", inspect.getsource(gate.prepare_current_observer))
        with patch.object(gate.library, "prepare_current_observer", return_value=("selected", "seam")) as prepare:
            self.assertEqual(gate.prepare_current_observer(1, 2, 3, 4, 5, 6), ("selected", "seam"))
        prepare.assert_called_once_with(1, 2, 3, 4, 5, 6)

    def test_existing_AST_resources_and_observer_domain_remain_exact(self):
        for name in ("src/frontend/ast.rs", "src/frontend/declaration_index/resource.rs"):
            self.assertEqual((ROOT / name).read_bytes(), git_body(gate.PREDECESSOR, name), name)
        adapter, body = gate.authenticate_observer_adapter(ROOT)
        self.assertEqual(body, git_body(gate.PREDECESSOR, gate.library.OBSERVER_ADAPTER))
        self.assertEqual(adapter.DERIVED_PIN, (30768, "96b8c45c257740a56b550ee24287bede07d3ca4ecd728ff5d40903cefc8b84d0"))
        self.assertEqual(len(adapter.SCALAR_NAMES), 17)
        self.assertEqual(len(adapter.RECORD_VECTOR_CONTROL_NAMES), 6)
        self.assertIs(adapter.descriptor()["copy_record_vector_semantics_qualified"], False)

    def test_ordinary_AST_kernel_witness_layout_and_cap_sources_are_unchanged(self):
        for name in (
            "src/frontend/ast.rs", "src/frontend/oir/owned_types.rs",
            "src/frontend/declaration_index/resource.rs", "src/frontend/oir/owned/budget.rs",
            "src/frontend/oir/owned/verified.rs", "src/frontend/oir/owned/plan.rs",
            "src/frontend/oir/owned/native_storage.rs",
        ):
            self.assertNotIn(name, gate.WINDOWS_NATIVE_CHANGED)
            self.assertEqual(gate.regular(ROOT, name), git_body(gate.PREDECESSOR, name), name)

    def test_provisional_pins_refuse_qualification_before_any_process(self):
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / "result"
            args = argparse.Namespace(repo=ROOT, output=output, prepare_only=False)
            with patch.object(gate, "PINS_PROVISIONAL", True), patch.object(gate.subprocess, "run") as run:
                self.assertEqual(gate.run(args), 1)
            run.assert_not_called()
            result = json.loads((output / "failure.json").read_text())
            self.assertEqual(result["status"], "failed")
            self.assertIs(result["semantic_pass"], False)
            self.assertEqual(result["compiler_executions"], 0)
            self.assertIn("prepare-only", result["error"])
            self.assertFalse((output / "result.json").exists())
            self.assertFalse((output / "prepared.json").exists())


if __name__ == "__main__":
    unittest.main()
