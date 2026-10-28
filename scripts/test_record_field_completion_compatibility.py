"""Closed source admission and complete immutable Windows-source replay; no compiler runs."""
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

import verify_record_field_completion_compatibility as gate

ROOT = Path(__file__).resolve().parents[1]


def git_body(commit, name):
    return subprocess.check_output(["git", "show", commit + ":" + name], cwd=ROOT)


def baseline_names():
    body = (ROOT / gate.BINDING / "current-source.json").read_bytes()
    if gate.sha(body) != gate.BASELINE_SOURCE_SHA:
        raise AssertionError("historical source manifest changed")
    return {row["path"] for row in json.loads(body)["files"]}


class RecordFieldCompletionAdmission(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory(prefix="oxid-record-field-completion-policy-")
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        self.baseline = {name: b"// fixed historical input\n" for name in baseline_names()}
        self.parent = {**self.baseline,
                       **{name: b"// fixed R2 addition\n" for name in gate.PREDECESSOR_ADDED}}
        for name in gate.UNCHANGED_INPUTS:
            self.parent[name] = git_body(gate.PREDECESSOR, name)
        self.current = dict(self.parent)
        self.current.update({name: b"// reviewed record-field-completion input\n"
                             for name in gate.RECORD_FIELD_COMPLETION_CHANGED | gate.RECORD_FIELD_COMPLETION_ADDED})
        self.changed = "src/frontend/driver.rs"
        self.current[self.changed] = b"#[test]\nfn first(){}\n#[test]\nfn second(){}\n"
        for name, body in self.current.items():
            self.put(name, body)
        for name, value in (
            ("REVIEWED_INPUTS", {name: (len(self.current[name]), gate.sha(self.current[name]))
                                 for name in gate.RECORD_FIELD_COMPLETION_CHANGED | gate.RECORD_FIELD_COMPLETION_ADDED}),
            ("REVIEWED_DIRECTIVES", {name: gate.source_directives(name, self.current[name])
                                     for name in gate.RECORD_FIELD_COMPLETION_CHANGED | gate.RECORD_FIELD_COMPLETION_ADDED}),
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

    def test_exact_Windows_roster_and_only_five_member_modules_are_admitted(self):
        inputs = self.capture()
        self.assertEqual(len(self.baseline), 376)
        self.assertEqual(len(self.parent), 512)
        self.assertEqual(len(inputs), 517)
        self.assertEqual(gate.RECORD_FIELD_COMPLETION_ADDED, {
            "src/frontend/declaration_index/editor_completion/members.rs",
            "src/frontend/editor/members.rs", "src/frontend/editor/members/resources.rs",
            "src/frontend/editor/members/tests.rs", "src/frontend/parser/editor_query/members.rs",
        })
        self.assertEqual(gate.RECORD_FIELD_COMPLETION_CHANGED, {
            "src/frontend/declaration_index/editor_completion.rs", "src/frontend/driver.rs",
            "src/frontend/editor.rs", "src/frontend/parser/editor_query.rs", "src/lsp_cli.rs",
        })
        self.assertEqual(set(inputs), set(self.parent) | gate.RECORD_FIELD_COMPLETION_ADDED)
        for name in set(self.parent) - gate.RECORD_FIELD_COMPLETION_CHANGED:
            self.assertEqual(inputs[name], self.parent[name])

    def test_manifest_binds_all_56_policies_and_keeps_every_claim_false(self):
        inputs = self.capture()
        manifest = gate.current_manifest(inputs, self.baseline, {"source_kind": "worktree"}, self.parent)
        self.assertEqual(manifest["source_policy_input_members"], 56)
        self.assertEqual(manifest["predecessor_compiler_input_members"], 512)
        self.assertEqual(manifest["source_policy_parent"], "8180b1bbcbcf9ccd4d648ff5ba0ec9ea3d5b971f")
        self.assertEqual({row["path"] for row in manifest["files"]}, set(self.current))
        self.assertEqual(len(manifest["files"]), 517)
        self.assertEqual(manifest["record_field_completion_pins_provisional"], gate.PINS_PROVISIONAL)
        claims = {key: value for key, value in manifest.items() if key.endswith("qualified_by_this_gate")}
        inherited = gate.library.current_manifest(self.parent, self.baseline, {}, self.parent)
        self.assertLessEqual({key for key in inherited if key.endswith("qualified_by_this_gate")}, set(claims))
        for name in ("copy_record_vectors_semantics", "windows_module_sources_semantics",
                     "windows_activation", "qualified_module_completion_semantics",
                     "copy_record_debug_semantics", "record_field_completion_semantics"):
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
        self.assertIs(manifest["record_field_completion_pins_provisional"], False)
        for key in (
            "windows_native_semantics_qualified_by_this_gate",
            "win32_native_doctor_semantics_qualified_by_this_gate",
            "windows_native_platform_semantics_qualified_by_this_gate",
            "windows_native_debug_semantics_qualified_by_this_gate",
            "msvc_pe_execution_qualified_by_this_gate",
            "copy_record_debug_semantics_qualified_by_this_gate",
            "qualified_module_completion_semantics_qualified_by_this_gate",
            "record_field_completion_semantics_qualified_by_this_gate",
        ):
            self.assertIs(manifest[key], False, key)
        self.assertIs(manifest["debug_information_enabled_for_retained_claim"], False)

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
        with self.assertRaisesRegex(ValueError, "reviewed record-field-completion input membership"):
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
        for name in ("src/frontend/ast.rs", "src/frontend/source.rs",
                     "src/frontend/oir/source/sealed.rs", "native/typed_preview.c"):
            self.put(name, self.parent[name] + b"// changed\n")
            with self.subTest(name=name), self.assertRaisesRegex(
                    ValueError, "outside record-field-completion boundary"):
                self.capture()
            self.put(name, self.parent[name])

    def test_each_member_sidecar_requires_its_exact_source_pin_and_compile_time_inputs(self):
        self.assertEqual(self.capture(), self.current)
        for name in sorted(gate.RECORD_FIELD_COMPLETION_ADDED):
            original = self.current[name]
            changed = original + b"#[cfg(any())]\nfn altered_member(){}\n"
            self.put(name, changed)
            with self.subTest(name=name, seam="hash"), self.assertRaisesRegex(
                    ValueError, "does not match reviewed input"):
                self.capture()
            with patch.dict(gate.REVIEWED_INPUTS, {name: (len(changed), gate.sha(changed))}):
                with self.subTest(name=name, seam="directive"), self.assertRaisesRegex(
                        ValueError, "directive changed"):
                    self.capture()
            self.put(name, original)
        self.assertEqual(self.capture(), self.current)

    def test_already_accepted_windows_dispatch_bytes_cannot_change(self):
        name, = gate.UNCHANGED_INPUTS
        self.put(name, self.parent[name] + b"// changed\n")
        with self.assertRaisesRegex(ValueError, "outside record-field-completion boundary"):
            self.capture()

    def test_protected_member_cannot_enter_generic_reviewed_delta(self):
        name, = gate.UNCHANGED_INPUTS
        with patch.object(gate, "RECORD_FIELD_COMPLETION_CHANGED", gate.RECORD_FIELD_COMPLETION_CHANGED | {name}):
            with self.assertRaisesRegex(ValueError, "protected input cannot enter"):
                self.capture()

    def test_extra_and_missing_compiler_members_are_refused(self):
        extra = self.root / "src/unreviewed.rs"
        self.put("src/unreviewed.rs", b"// extra\n")
        with self.assertRaisesRegex(ValueError, "membership"):
            self.capture()
        extra.unlink()
        (self.root / sorted(gate.RECORD_FIELD_COMPLETION_ADDED)[0]).unlink()
        with self.assertRaisesRegex(ValueError, "membership"):
            self.capture()

    def test_empty_unlisted_directory_is_refused(self):
        (self.root / "src/unreviewed").mkdir()
        with self.assertRaisesRegex(ValueError, "directory"):
            self.capture()

    def test_parent_member_loss_cannot_lower_the_roster(self):
        name = sorted(gate.PREDECESSOR_ADDED)[0]
        del self.parent[name]
        with self.assertRaisesRegex(ValueError, "Windows native compiler input membership"):
            self.capture()
        with patch.object(gate, "PREDECESSOR_ADDED", gate.PREDECESSOR_ADDED - {name}):
            with self.assertRaisesRegex(ValueError, "Windows native compiler input membership"):
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


class FrozenWindowsReplay(unittest.TestCase):
    def test_ten_source_pins_use_a_reachable_integrated_revision(self):
        frozen = "146c51cf193b78208c18b7dd2dc604843b61d1d7"
        subprocess.check_call(["git", "--no-replace-objects", "merge-base", "--is-ancestor",
                               frozen, "HEAD"], cwd=ROOT)
        self.assertEqual(len(gate.RECORD_FIELD_COMPLETION_CHANGED), 5)
        self.assertEqual(len(gate.RECORD_FIELD_COMPLETION_ADDED), 5)
        self.assertEqual(len(gate.REVIEWED_INPUTS), 10)
        self.assertEqual(set(gate.REVIEWED_INPUTS),
                         gate.RECORD_FIELD_COMPLETION_CHANGED | gate.RECORD_FIELD_COMPLETION_ADDED)
        self.assertEqual(set(gate.REVIEWED_DIRECTIVES), set(gate.REVIEWED_INPUTS))
        for name, pin in gate.REVIEWED_INPUTS.items():
            body = git_body(frozen, name)
            self.assertEqual(gate.regular(ROOT, name), body, name)
            self.assertEqual((len(body), gate.sha(body)), pin, name)
            self.assertEqual(gate.source_directives(name, body), gate.REVIEWED_DIRECTIVES[name], name)

    def test_editor_file_namespace_path_query_and_old_layout_rules_are_byte_exact(self):
        additions = {
            "src/frontend/parser/editor_query.rs": b"pub(in crate::frontend) mod members;\n\n",
            "src/frontend/declaration_index/editor_completion.rs": b"pub(in crate::frontend) mod members;\n\n",
            "src/frontend/editor.rs": b"pub(crate) mod members;\n",
        }
        for name, added in additions.items():
            body = gate.regular(ROOT, name)
            self.assertEqual(body.count(added), 1, name)
            self.assertEqual(body.replace(added, b"", 1), git_body(gate.PREDECESSOR, name), name)
        for name in ("src/frontend/source.rs", "src/frontend/ast.rs", "src/frontend/parser.rs",
                     "src/frontend/declaration_index.rs", "src/frontend/project/editor_sources.rs",
                     "src/frontend/project/module_syntax.rs", "src/frontend/oir/source/sealed.rs"):
            self.assertEqual(gate.regular(ROOT, name), git_body(gate.PREDECESSOR, name), name)

    def test_member_only_storage_is_charged_before_query_and_inside_existing_namespace_cap(self):
        member = gate.regular(ROOT, "src/frontend/editor/members.rs").decode()
        lookup = gate.regular(ROOT, "src/frontend/declaration_index/editor_completion/members.rs").decode()
        self.assertLess(member.index("budget.charge(resources::fixed_bytes())?"),
                        member.index(".member_query(source, offset, work)?"))
        self.assertIn("limit: limit.min(IndexLimits::default().scratch) as usize", member)
        self.assertIn("Namespace::build_for_members(session, work, allocator, budget.used, limit)?", member)
        self.assertIn(".min(IndexLimits::default().scratch)", lookup)
        self.assertIn(".checked_sub(extra as u64)", lookup)
        self.assertIn("Self::build_limited(sources, work, allocator, remaining)", lookup)
        for marker, allocation in (("budget.charge(\n        count", "allocator\n        .vector_exact(&mut fields"),
                                   ("budget.charge(payload)?", "allocator\n        .vector_exact(&mut entries")):
            self.assertLess(member.index(marker), member.index(allocation))

    def test_narrow_observer_closure_adds_only_the_two_actual_member_children(self):
        added = ("parser/editor_query/members.rs", "declaration_index/editor_completion/members.rs")
        for name in ("scripts/build_typed_parser_observer.py", "scripts/build_typed_static_observer.py"):
            self.assertNotIn(name, gate.PREDECESSOR_POLICIES)
            current, previous = gate.regular(ROOT, name), git_body(gate.PREDECESSOR, name)
            def roster(body):
                tree = ast.parse(body)
                node = next(node for node in tree.body if isinstance(node, ast.Assign)
                            and any(isinstance(target, ast.Name) and target.id == "SOURCE_FILES"
                                    for target in node.targets))
                return ast.literal_eval(node.value)
            before, after = roster(previous), roster(current)
            self.assertEqual(tuple(member for member in after if member not in added), before)
            for member in added:
                self.assertEqual(after.count(member), 1)
                self.assertIn("src/frontend/" + member, gate.RECORD_FIELD_COMPLETION_ADDED)
                self.assertTrue((ROOT / "src/frontend" / member).is_file())
                current = current.replace(b' "' + member.encode() + b'",', b"", 1)
            self.assertEqual(current, previous, name)
        name = "scripts/test_legacy_scalar_observer_byte_ranges.py"
        self.assertEqual(gate.regular(ROOT, name), git_body(gate.PREDECESSOR, name))

    def test_all_56_predecessor_policies_and_adapters_are_exact(self):
        self.assertEqual(len(gate.PREDECESSOR_POLICIES), 56)
        self.assertEqual(gate.PREDECESSOR_POLICIES, {
            **gate.library.PREDECESSOR_POLICIES,
            "scripts/verify_windows_native_compatibility.py": gate.PREDECESSOR_POLICY_PIN[1],
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
                with self.assertRaisesRegex(ValueError, "changed pinned Windows native policy"):
                    gate.load_predecessor(root)
                execute.assert_not_called()
            path.write_bytes(body)
            path.chmod(0o755)
            with patch.object(gate, "exec", create=True) as execute:
                with self.assertRaisesRegex(ValueError, "unsafe pinned Windows native policy"):
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

    def test_both_complete_Windows_replay_blocks_and_every_selector_are_unchanged(self):
        def replay(body):
            tree = ast.parse(body)
            run = next(node for node in tree.body if isinstance(node, ast.FunctionDef) and node.name == "run")
            return [node for node in ast.walk(run) if isinstance(node, ast.If)
                    and ast.unparse(node.test) == "not args.prepare_only"]
        current = replay((ROOT / "scripts/verify_record_field_completion_compatibility.py").read_bytes())
        predecessor = replay(git_body(gate.PREDECESSOR, gate.PREDECESSOR_POLICY))
        self.assertEqual(len(current), 2)
        self.assertEqual(len(predecessor), 2)
        self.assertEqual([ast.dump(node, include_attributes=False) for node in current],
                         [ast.dump(node, include_attributes=False) for node in predecessor])
        self.assertGreater(len(current[0].body), 10)
        self.assertIn("run_record_vector_observer_controls", ast.unparse(current[1]))

    def test_every_Windows_control_function_and_resource_preparation_are_unmodified(self):
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
            self.assertNotIn(name, gate.RECORD_FIELD_COMPLETION_CHANGED)
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
