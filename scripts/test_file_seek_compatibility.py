"""Closed source admission and complete immutable record-field-source replay; no compiler runs."""
import argparse
import ast
import hashlib
import inspect
import json
import os
from pathlib import Path
import re
import subprocess
import tempfile
import unittest
from unittest.mock import patch

import verify_file_seek_compatibility as gate

ROOT = Path(__file__).resolve().parents[1]


def git_body(commit, name):
    return subprocess.check_output(["git", "show", commit + ":" + name], cwd=ROOT)


def baseline_names():
    body = (ROOT / gate.BINDING / "current-source.json").read_bytes()
    if gate.sha(body) != gate.BASELINE_SOURCE_SHA:
        raise AssertionError("historical source manifest changed")
    return {row["path"] for row in json.loads(body)["files"]}


class FileSeekAdmission(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory(prefix="oxid-file-seek-policy-")
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        self.baseline = {name: b"// fixed historical input\n" for name in baseline_names()}
        self.parent = {**self.baseline,
                       **{name: b"// fixed R2 addition\n" for name in gate.PREDECESSOR_ADDED}}
        for name in gate.UNCHANGED_INPUTS:
            self.parent[name] = git_body(gate.PREDECESSOR, name)
        self.current = dict(self.parent)
        self.current.update({name: b"// reviewed file-seek input\n"
                             for name in gate.FILE_SEEK_CHANGED | gate.FILE_SEEK_ADDED})
        self.changed = "src/frontend/experimental_builtins.rs"
        self.current[self.changed] = b"#[test]\nfn first(){}\n#[test]\nfn second(){}\n"
        self.current["native/experimental_files.c"] = b"#ifdef SEEK_TEST\nint seek;\n#else\nint inherited;\n#endif\n"
        for name, body in self.current.items():
            self.put(name, body)
        for name, value in (
            ("REVIEWED_INPUTS", {name: (len(self.current[name]), gate.sha(self.current[name]))
                                 for name in gate.FILE_SEEK_CHANGED | gate.FILE_SEEK_ADDED}),
            ("REVIEWED_DIRECTIVES", {name: gate.source_directives(name, self.current[name])
                                     for name in gate.FILE_SEEK_CHANGED | gate.FILE_SEEK_ADDED}),
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

    def test_exact_record_field_roster_and_only_two_seek_tests_are_added(self):
        inputs = self.capture()
        self.assertEqual(len(self.baseline), 376)
        self.assertEqual(len(self.parent), 517)
        self.assertEqual(len(inputs), 519)
        self.assertEqual(gate.FILE_SEEK_ADDED, {
            "src/frontend/oir/owned/execute_file_seek_tests.rs",
            "src/frontend/oir/owned/source/file_seek_tests.rs",
        })
        self.assertEqual(gate.FILE_SEEK_CHANGED, {
            "native/experimental_files.c", "src/frontend/builtin_catalog.rs",
            "src/frontend/declaration_index/experimental_tests.rs", "src/frontend/experimental_builtins.rs",
            "src/frontend/oir/owned/byte_buffer.rs", "src/frontend/oir/owned/execute.rs",
            "src/frontend/oir/owned/execute_experimental.rs", "src/frontend/oir/owned/execute_file_delivery.rs",
            "src/frontend/oir/owned/file_resources.rs", "src/frontend/oir/owned/native.rs",
            "src/frontend/oir/owned/native_experimental.rs", "src/frontend/oir/owned/plan.rs",
            "src/frontend/oir/owned/source/experimental_source_tests.rs",
        })
        self.assertEqual(set(inputs), set(self.parent) | gate.FILE_SEEK_ADDED)
        for name in set(self.parent) - gate.FILE_SEEK_CHANGED:
            self.assertEqual(inputs[name], self.parent[name])

    def test_manifest_binds_all_57_policies_and_keeps_every_claim_false(self):
        inputs = self.capture()
        manifest = gate.current_manifest(inputs, self.baseline, {"source_kind": "worktree"}, self.parent)
        self.assertEqual(manifest["source_policy_input_members"], 57)
        self.assertEqual(manifest["predecessor_compiler_input_members"], 517)
        self.assertEqual(manifest["source_policy_parent"], "b273b71237cc6a1d2db76dfe7c971e073d1d5cb3")
        self.assertEqual({row["path"] for row in manifest["files"]}, set(self.current))
        self.assertEqual(len(manifest["files"]), 519)
        self.assertEqual(manifest["file_seek_pins_provisional"], gate.PINS_PROVISIONAL)
        claims = {key: value for key, value in manifest.items() if key.endswith("qualified_by_this_gate")}
        inherited = gate.library.current_manifest(self.parent, self.baseline, {}, self.parent)
        self.assertLessEqual({key for key in inherited if key.endswith("qualified_by_this_gate")}, set(claims))
        for name in ("copy_record_vectors_semantics", "windows_module_sources_semantics",
                     "windows_activation", "qualified_module_completion_semantics",
                     "copy_record_debug_semantics", "record_field_completion_semantics",
                     "file_seek_semantics", "seekable_streaming_datasets_semantics"):
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
        self.assertIs(manifest["file_seek_pins_provisional"], False)
        for key in (
            "windows_native_semantics_qualified_by_this_gate",
            "win32_native_doctor_semantics_qualified_by_this_gate",
            "windows_native_platform_semantics_qualified_by_this_gate",
            "windows_native_debug_semantics_qualified_by_this_gate",
            "msvc_pe_execution_qualified_by_this_gate",
            "copy_record_debug_semantics_qualified_by_this_gate",
            "qualified_module_completion_semantics_qualified_by_this_gate",
            "record_field_completion_semantics_qualified_by_this_gate",
            "file_seek_semantics_qualified_by_this_gate",
            "seekable_streaming_datasets_semantics_qualified_by_this_gate",
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

    def test_C_preprocessor_order_and_multiplicity_cannot_be_re_pinned_away(self):
        name = "native/experimental_files.c"
        original = gate.REVIEWED_DIRECTIVES[name]
        directives = original["c_preprocessor"]
        self.assertEqual(len(directives), 3)
        for changed in (tuple(reversed(directives)), directives + directives[:1], directives[1:]):
            gate.REVIEWED_DIRECTIVES[name] = {**original, "c_preprocessor": changed}
            with self.subTest(changed=changed), self.assertRaisesRegex(ValueError, "directive changed"):
                self.capture()
        gate.REVIEWED_DIRECTIVES[name] = original

    def test_new_seek_inputs_require_their_source_hash_and_compile_time_selectors(self):
        self.assertEqual(self.capture(), self.current)
        for name in ("native/experimental_files.c", *sorted(gate.FILE_SEEK_ADDED)):
            original = self.current[name]
            changed = original + (b"#if SEEK_ALTERED\n#endif\n" if name.endswith(".c")
                                  else b"#[cfg(any())]\nfn altered_seek(){}\n")
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
        with self.assertRaisesRegex(ValueError, "reviewed file-seek input membership"):
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
                    ValueError, "outside file-seek boundary"):
                self.capture()
            self.put(name, self.parent[name])

    def test_already_accepted_windows_dispatch_bytes_cannot_change(self):
        name, = gate.UNCHANGED_INPUTS
        self.put(name, self.parent[name] + b"// changed\n")
        with self.assertRaisesRegex(ValueError, "outside file-seek boundary"):
            self.capture()

    def test_protected_member_cannot_enter_generic_reviewed_delta(self):
        name, = gate.UNCHANGED_INPUTS
        with patch.object(gate, "FILE_SEEK_CHANGED", gate.FILE_SEEK_CHANGED | {name}):
            with self.assertRaisesRegex(ValueError, "protected input cannot enter"):
                self.capture()

    def test_extra_and_missing_compiler_members_are_refused(self):
        extra = self.root / "src/unreviewed.rs"
        self.put("src/unreviewed.rs", b"// extra\n")
        with self.assertRaisesRegex(ValueError, "membership"):
            self.capture()
        extra.unlink()
        (self.root / sorted(gate.FILE_SEEK_ADDED)[0]).unlink()
        with self.assertRaisesRegex(ValueError, "membership"):
            self.capture()

    def test_empty_unlisted_directory_is_refused(self):
        (self.root / "src/unreviewed").mkdir()
        with self.assertRaisesRegex(ValueError, "directory"):
            self.capture()

    def test_parent_member_loss_cannot_lower_the_roster(self):
        name = sorted(gate.PREDECESSOR_ADDED)[0]
        del self.parent[name]
        with self.assertRaisesRegex(ValueError, "record-field-completion compiler input membership"):
            self.capture()
        with patch.object(gate, "PREDECESSOR_ADDED", gate.PREDECESSOR_ADDED - {name}):
            with self.assertRaisesRegex(ValueError, "record-field-completion compiler input membership"):
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


class FrozenRecordFieldReplay(unittest.TestCase):
    def test_fifteen_source_pins_use_only_the_integrated_reachable_seek_revision(self):
        frozen = "6c93d00079a491e003bd53d624612e56f333b84f"
        subprocess.check_call(["git", "--no-replace-objects", "merge-base", "--is-ancestor",
                               frozen, "HEAD"], cwd=ROOT)
        self.assertEqual(len(gate.FILE_SEEK_CHANGED), 13)
        self.assertEqual(len(gate.FILE_SEEK_ADDED), 2)
        self.assertEqual(len(gate.REVIEWED_INPUTS), 15)
        self.assertEqual(set(gate.REVIEWED_INPUTS), gate.FILE_SEEK_CHANGED | gate.FILE_SEEK_ADDED)
        self.assertEqual(set(gate.REVIEWED_DIRECTIVES), set(gate.REVIEWED_INPUTS))
        for name, pin in gate.REVIEWED_INPUTS.items():
            body = git_body(frozen, name)
            self.assertEqual(gate.regular(ROOT, name), body, name)
            self.assertEqual((len(body), gate.sha(body)), pin, name)
            self.assertEqual(gate.source_directives(name, body), gate.REVIEWED_DIRECTIVES[name], name)

    def test_catalog_old_ordinals_bits_and_real_iterator_carriers_are_independently_preserved(self):
        name = "src/frontend/experimental_builtins.rs"
        current = gate.regular(ROOT, name).decode()
        previous = git_body(gate.PREDECESSOR, name).decode()
        def variants(source, enum):
            match = re.search(r"enum " + enum + r" \{([^}]+)\}", source)
            self.assertIsNotNone(match)
            return [row.strip() for row in match.group(1).split(",") if row.strip()]
        for enum, count in (("Item", 28), ("Function", 20)):
            before, after = variants(previous, enum), variants(current, enum)
            self.assertEqual(after[:-1], before)
            self.assertEqual(after[-1], "SeekAbsolute")
            self.assertEqual(len(after), count)
        self.assertIn("const VECTOR_ITEM_START: usize = 27;", current)
        self.assertIn("const SEEK_ITEM_BIT: usize = 37;", current)
        self.assertIn("Self::SeekAbsolute => SEEK_ITEM_BIT,", current)
        self.assertIn("_ => self as usize,", current)
        self.assertIn("struct Inventory([u8; 8]);", current)
        def carriers(source):
            start = source.index("struct CompilerCarriers {")
            return source[start:source.index("struct PackedInventoryCarriers", start)]
        actual = carriers(current)
        self.assertIn("selected_item_bit: usize,", actual)
        self.assertIn("IntoIter<Item, 28>; 4]", actual)
        self.assertIn("IntoIter<Function, 20>; 4]", actual)
        restored = actual.replace("    selected_item_bit: usize,\n", "", 1)
        restored = restored.replace("IntoIter<Item, 28>", "IntoIter<Item, 27>")
        restored = restored.replace("IntoIter<Function, 20>", "IntoIter<Function, 19>")
        self.assertEqual(restored, carriers(previous))
        self.assertIn("std::mem::size_of::<CompilerCarriers>()", current)
        # The unchanged packed inventory width does not replace the actual
        # catalog transport bank charged by collection and HIR admission.
        for path in ("src/frontend/declaration_index/sealed.rs",
                     "src/frontend/oir/owned/source/hir_budget.rs", "src/frontend/oir/owned/budget.rs"):
            source = gate.regular(ROOT, path)
            self.assertEqual(source, git_body(gate.PREDECESSOR, path), path)
            self.assertIn(b"experimental::compiler_carrier_bytes()", source)

    def test_complete_file_return_preflight_and_conditional_seek_banks_keep_old_caps(self):
        name = "src/frontend/oir/owned/execute_file_delivery.rs"
        current = gate.regular(ROOT, name).decode()
        previous = git_body(gate.PREDECESSOR, name).decode()
        def preparation(source):
            return source[source.index("    fn prepare_file_delivery("):source.index("    fn file_parameter(")]
        self.assertEqual(preparation(current), preparation(previous))
        self.assertLess(current.index("self.prepare_file_delivery(frame, operation, destination, span)?"),
                        current.index(".file_seek_absolute(&file, offset)"))
        self.assertLess(current.index("fuel\n                .checked_sub(delivery.cost)"),
                        current.index(".file_seek_absolute(&file, offset)"))
        self.assertIn("size_of::<SeekCarriers<V>>() + byte_buffer::file_seek_helper_control_bytes()", current)
        vm = gate.regular(ROOT, "src/frontend/oir/owned/execute.rs").decode()
        native = gate.regular(ROOT, "src/frontend/oir/owned/native.rs").decode()
        self.assertIn("if plan.has_file_seek() {\n                    experimental::file_seek_control_bytes::<V>()", vm)
        self.assertIn("if plan.has_file_seek() {\n                experimental::FILE_SEEK_BYTES", native)
        self.assertIn("if plan.has_file_seek() {\n                experimental::seek_emitter_control_bytes()", native)
        plan = gate.regular(ROOT, "src/frontend/oir/owned/plan.rs").decode()
        added = '''    pub(super) fn has_file_seek(&self) -> bool {
        self.witness
            .experimental_function(
                crate::frontend::builtin_catalog::experimental::Function::SeekAbsolute,
            )
            .is_some()
    }
'''
        self.assertEqual(plan.count(added), 1)
        self.assertEqual(plan.replace(added, "", 1), git_body(gate.PREDECESSOR, "src/frontend/oir/owned/plan.rs").decode())

    def test_dedicated_C_seek_helper_keeps_old_runtime_bytes_and_preflights_before_lseek(self):
        name = "native/experimental_files.c"
        current = gate.regular(ROOT, name)
        previous = git_body(gate.PREDECESSOR, name)
        self.assertTrue(current.startswith(previous))
        added = current[len(previous):].decode()
        self.assertIn("__oxid_file_seek_absolute(const oxid_file *file, int64_t offset", added)
        self.assertIn("_Static_assert(sizeof(off_t) == sizeof(int64_t)", added)
        self.assertIn("_Static_assert((off_t)-1 < (off_t)0", added)
        self.assertLess(added.index("locate_file(&execution_heap, file, &node)"), added.index("lseek("))
        self.assertLess(added.index("if (*fuel < return_cost)"), added.index("lseek("))
        self.assertLess(added.index("if (offset < 0)"), added.index("lseek("))
        self.assertIn("result->tag = error ? 1 : 0;", added)
        self.assertIn("__oxid_file_seek_absolute_hosted", added)

    def test_all_57_predecessor_policies_and_adapters_are_exact(self):
        self.assertEqual(len(gate.PREDECESSOR_POLICIES), 57)
        self.assertEqual(gate.PREDECESSOR_POLICIES, {
            **gate.library.PREDECESSOR_POLICIES,
            "scripts/verify_record_field_completion_compatibility.py": gate.PREDECESSOR_POLICY_PIN[1],
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
                with self.assertRaisesRegex(ValueError, "changed pinned record-field-completion policy"):
                    gate.load_predecessor(root)
                execute.assert_not_called()
            path.write_bytes(body)
            path.chmod(0o755)
            with patch.object(gate, "exec", create=True) as execute:
                with self.assertRaisesRegex(ValueError, "unsafe pinned record-field-completion policy"):
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
        current = replay((ROOT / "scripts/verify_file_seek_compatibility.py").read_bytes())
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
            "src/frontend/oir/owned/verified.rs", "src/frontend/source.rs",
            "src/frontend/oir/owned/native_storage.rs",
        ):
            self.assertNotIn(name, gate.FILE_SEEK_CHANGED)
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
