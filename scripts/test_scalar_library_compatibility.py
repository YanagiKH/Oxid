"""Closed source admission and complete immutable File seek replay; no compiler runs."""
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

import verify_scalar_library_compatibility as gate

ROOT = Path(__file__).resolve().parents[1]
NATIVE_LIBRARY_STEP = (
    b"      - name: Verify returning native libraries from C Rust and Python\n"
    b"        env:\n"
    b"          TMPDIR: ${{ runner.temp }}/current-native-libraries\n"
    b"          OXID_TEST_KEEP_ARTIFACTS: '1'\n"
    b"        run: |\n"
    b'          mkdir -p "$TMPDIR"\n'
    b"          cargo test --locked --test native_library actual_same_library -- --ignored --nocapture\n"
    b"          cargo test --release --locked --test native_library actual_same_library -- --ignored --nocapture\n"
)
NATIVE_LIBRARY_EVIDENCE = b"            ${{ runner.temp }}/current-native-libraries\n"
NATIVE_LIBRARY_TEST = b"actual_same_library_is_called_by_c_rust_python_with_returning_errors_and_concurrent_calls"


def git_body(commit, name):
    return subprocess.check_output(["git", "show", commit + ":" + name], cwd=ROOT)


def baseline_names():
    body = (ROOT / gate.BINDING / "current-source.json").read_bytes()
    if gate.sha(body) != gate.BASELINE_SOURCE_SHA:
        raise AssertionError("historical source manifest changed")
    return {row["path"] for row in json.loads(body)["files"]}


class ScalarLibraryAdmission(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory(prefix="oxid-scalar-library-policy-")
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        self.baseline = {name: b"// fixed historical input\n" for name in baseline_names()}
        self.parent = {**self.baseline,
                       **{name: b"// fixed R2 addition\n" for name in gate.PREDECESSOR_ADDED}}
        for name in gate.UNCHANGED_INPUTS:
            self.parent[name] = git_body(gate.PREDECESSOR, name)
        self.current = dict(self.parent)
        self.current.update({name: b"// reviewed scalar-library input\n"
                             for name in gate.SCALAR_LIBRARY_CHANGED | gate.SCALAR_LIBRARY_ADDED})
        self.changed = "src/frontend/oir/source/sealed.rs"
        self.current[self.changed] = b"#[test]\nfn first(){}\n#[test]\nfn second(){}\n"
        for name, body in self.current.items():
            self.put(name, body)
        for name, value in (
            ("REVIEWED_INPUTS", {name: (len(self.current[name]), gate.sha(self.current[name]))
                                 for name in gate.SCALAR_LIBRARY_CHANGED | gate.SCALAR_LIBRARY_ADDED}),
            ("REVIEWED_DIRECTIVES", {name: gate.source_directives(name, self.current[name])
                                     for name in gate.SCALAR_LIBRARY_CHANGED | gate.SCALAR_LIBRARY_ADDED}),
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

    def test_exact_File_seek_roster_and_only_three_library_modules_are_added(self):
        inputs = self.capture()
        self.assertEqual(len(self.baseline), 376)
        self.assertEqual(len(self.parent), 519)
        self.assertEqual(len(inputs), 522)
        self.assertEqual(gate.SCALAR_LIBRARY_ADDED, {
            "src/frontend/native/library.rs", "src/frontend/native_library.rs",
            "src/frontend/oir/native_library.rs",
        })
        self.assertEqual(gate.SCALAR_LIBRARY_CHANGED, {
            "src/frontend/driver.rs", "src/frontend/mod.rs", "src/frontend/native.rs",
            "src/frontend/oir/mod.rs", "src/frontend/oir/native.rs", "src/frontend/oir/source.rs",
            "src/frontend/oir/source/sealed.rs", "src/frontend/options.rs",
            "src/frontend/oir/owned/byte_storage_native_tests.rs",
            "src/frontend/oir/source/hir_import/emit_tests.rs",
        })
        self.assertEqual(set(inputs), set(self.parent) | gate.SCALAR_LIBRARY_ADDED)
        for name in set(self.parent) - gate.SCALAR_LIBRARY_CHANGED:
            self.assertEqual(inputs[name], self.parent[name])

    def test_manifest_binds_all_58_policies_and_keeps_every_claim_false(self):
        inputs = self.capture()
        manifest = gate.current_manifest(inputs, self.baseline, {"source_kind": "worktree"}, self.parent)
        self.assertEqual(manifest["source_policy_input_members"], 58)
        self.assertEqual(manifest["predecessor_compiler_input_members"], 519)
        self.assertEqual(manifest["source_policy_parent"], "104c900222ead9d9177025102a90520b08e761a4")
        self.assertEqual({row["path"] for row in manifest["files"]}, set(self.current))
        self.assertEqual(len(manifest["files"]), 522)
        self.assertEqual(manifest["scalar_library_pins_provisional"], gate.PINS_PROVISIONAL)
        claims = {key: value for key, value in manifest.items() if key.endswith("qualified_by_this_gate")}
        inherited = gate.library.current_manifest(self.parent, self.baseline, {}, self.parent)
        self.assertLessEqual({key for key in inherited if key.endswith("qualified_by_this_gate")}, set(claims))
        for name in ("copy_record_vectors_semantics", "windows_module_sources_semantics",
                     "windows_activation", "qualified_module_completion_semantics",
                     "copy_record_debug_semantics", "record_field_completion_semantics",
                     "file_seek_semantics", "seekable_streaming_datasets_semantics",
                     "scalar_native_library_semantics", "general_ffi_semantics"):
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
        self.assertIs(manifest["scalar_library_pins_provisional"], False)
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
            "scalar_native_library_semantics_qualified_by_this_gate",
            "general_ffi_semantics_qualified_by_this_gate",
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

    def test_each_new_library_module_requires_its_hash_and_ordered_compile_time_inputs(self):
        self.assertEqual(self.capture(), self.current)
        for name in sorted(gate.SCALAR_LIBRARY_ADDED):
            original = self.current[name]
            changed = original + b"#[cfg(any())]\nfn altered_library(){}\n"
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
        with self.assertRaisesRegex(ValueError, "reviewed scalar-library input membership"):
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
                     "src/frontend/oir/source/association.rs", "native/typed_preview.c"):
            self.put(name, self.parent[name] + b"// changed\n")
            with self.subTest(name=name), self.assertRaisesRegex(
                    ValueError, "outside scalar-library boundary"):
                self.capture()
            self.put(name, self.parent[name])

    def test_already_accepted_windows_dispatch_bytes_cannot_change(self):
        name, = gate.UNCHANGED_INPUTS
        self.put(name, self.parent[name] + b"// changed\n")
        with self.assertRaisesRegex(ValueError, "outside scalar-library boundary"):
            self.capture()

    def test_protected_member_cannot_enter_generic_reviewed_delta(self):
        name, = gate.UNCHANGED_INPUTS
        with patch.object(gate, "SCALAR_LIBRARY_CHANGED", gate.SCALAR_LIBRARY_CHANGED | {name}):
            with self.assertRaisesRegex(ValueError, "protected input cannot enter"):
                self.capture()

    def test_extra_and_missing_compiler_members_are_refused(self):
        extra = self.root / "src/unreviewed.rs"
        self.put("src/unreviewed.rs", b"// extra\n")
        with self.assertRaisesRegex(ValueError, "membership"):
            self.capture()
        extra.unlink()
        (self.root / sorted(gate.SCALAR_LIBRARY_ADDED)[0]).unlink()
        with self.assertRaisesRegex(ValueError, "membership"):
            self.capture()

    def test_empty_unlisted_directory_is_refused(self):
        (self.root / "src/unreviewed").mkdir()
        with self.assertRaisesRegex(ValueError, "directory"):
            self.capture()

    def test_parent_member_loss_cannot_lower_the_roster(self):
        name = sorted(gate.PREDECESSOR_ADDED)[0]
        del self.parent[name]
        with self.assertRaisesRegex(ValueError, "File seek compiler input membership"):
            self.capture()
        with patch.object(gate, "PREDECESSOR_ADDED", gate.PREDECESSOR_ADDED - {name}):
            with self.assertRaisesRegex(ValueError, "File seek compiler input membership"):
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


class FrozenFileSeekReplay(unittest.TestCase):
    def test_thirteen_input_pins_use_only_the_integrated_reachable_revision(self):
        frozen = "9be31bc374bffee230768ecd1fe3e52b5a4ea965"
        subprocess.check_call(["git", "--no-replace-objects", "merge-base", "--is-ancestor",
                               frozen, "HEAD"], cwd=ROOT)
        self.assertEqual(len(gate.SCALAR_LIBRARY_CHANGED), 10)
        self.assertEqual(len(gate.SCALAR_LIBRARY_ADDED), 3)
        self.assertEqual(len(gate.REVIEWED_INPUTS), 13)
        self.assertEqual(set(gate.REVIEWED_INPUTS), gate.SCALAR_LIBRARY_CHANGED | gate.SCALAR_LIBRARY_ADDED)
        self.assertEqual(set(gate.REVIEWED_DIRECTIVES), set(gate.REVIEWED_INPUTS))
        for name, pin in gate.REVIEWED_INPUTS.items():
            body = git_body(frozen, name)
            self.assertEqual(gate.regular(ROOT, name), body, name)
            self.assertEqual((len(body), gate.sha(body)), pin, name)
            self.assertEqual(gate.source_directives(name, body), gate.REVIEWED_DIRECTIVES[name], name)

    def test_Windows_test_headers_preserve_every_old_literal_and_resource_formula(self):
        branch = re.compile(
            rb'if cfg!\(all\(\s*target_os = "windows",\s*target_env = "msvc",\s*'
            rb'target_arch = "x86_64",\s*target_pointer_width = "64",\s*'
            rb'target_endian = "little"\s*\)\) \{\s*"x86_64-pc-windows-msvc"\s*\} else '
        )
        for name in ("src/frontend/oir/owned/byte_storage_native_tests.rs",
                     "src/frontend/oir/source/hir_import/emit_tests.rs"):
            current = gate.regular(ROOT, name)
            restored, count = branch.subn(b"", current)
            self.assertEqual(count, 1, name)
            if name.endswith("/emit_tests.rs"):
                old_comment = b"also handles shorter Darwin/MSVC headers"
                self.assertEqual(restored.count(old_comment), 1)
                restored = restored.replace(old_comment, b"also handles the shorter Darwin headers", 1)
            self.assertEqual(restored, git_body(gate.PREDECESSOR, name), name)
        source = gate.regular(ROOT, "src/frontend/oir/source/hir_import/emit_tests.rs")
        self.assertIn(b'linux_x86_64_bytes - "x86_64-unknown-linux-gnu".len() + TARGET_TRIPLE.len()', source)
        self.assertEqual(len("x86_64-unknown-linux-gnu") - len("x86_64-pc-windows-msvc"), 2)

    def test_library_authority_is_separate_from_all_existing_source_owners_and_paths(self):
        name = "src/frontend/oir/source/sealed.rs"
        current = gate.regular(ROOT, name)
        start = current.index(b"/// Separate opt-in authority:")
        end = current.index(b"pub(in crate::frontend) fn check_source", start)
        self.assertEqual(current[:start] + current[end:], git_body(gate.PREDECESSOR, name))
        added = current[start:end]
        self.assertIn(b"struct CheckedScalarLibrary<'s>", added)
        self.assertIn(b"std::mem::size_of::<LibrarySourceBank>() as u64", added)
        self.assertEqual(added.count(b".checked_sub(banks)"), 2)
        self.assertLess(added.index(b".checked_sub(banks)"), added.index(b"index::collect_originals("))
        self.assertIn(b"work: limits.work.min(IndexLimits::default().work)", added)
        self.assertIn(b"association::authenticate_scalar(raw, project.sources()", added)

    def test_existing_OIR_and_executable_emitters_remain_byte_exact(self):
        additions = {
            "src/frontend/mod.rs": b"mod native_library;\n",
            "src/frontend/native.rs": b"pub(super) mod library;\n",
            "src/frontend/oir/native.rs": (
                b'#[path = "native_library.rs"]\npub(in crate::frontend::oir) mod library;\n\n'),
            "src/frontend/oir/source.rs": (
                b"pub(in crate::frontend) use sealed::{check_scalar_library, CheckedScalarLibrary};\n"),
        }
        for name, added in additions.items():
            current = gate.regular(ROOT, name)
            self.assertEqual(current.count(added), 1, name)
            self.assertEqual(current.replace(added, b"", 1), git_body(gate.PREDECESSOR, name), name)
        name = "src/frontend/oir/mod.rs"
        current, previous = gate.regular(ROOT, name), git_body(gate.PREDECESSOR, name)
        selector = rb"pub\(super\) use source::\{.*?\};"
        old_selector = re.search(selector, previous, re.S).group()
        new_selector = re.search(selector, current, re.S).group()
        restored = current.replace(new_selector, old_selector, 1)
        accessors = (
            b"pub(super) const fn native_library_max_fuel() -> usize {\n"
            b"    execute::MAX_FUEL\n}\n\n"
            b"pub(super) const fn native_library_max_params() -> usize {\n"
            b"    native::library::MAX_LIBRARY_PARAMS\n}\n\n"
        )
        self.assertEqual(restored.count(accessors), 1)
        self.assertEqual(restored.replace(accessors, b"", 1), previous)

    def test_library_route_and_driver_banks_do_not_change_existing_command_bodies(self):
        name = "src/frontend/options.rs"
        current = gate.regular(ROOT, name)
        variant = b"    TypedLibrary {\n        request: Box<[super::native_library::Options; 1]>,\n    },\n"
        self.assertEqual(current.count(variant), 1)
        restored = current.replace(variant, b"", 1)
        start = restored.index(b'    if forwarded.first().map(String::as_str) == Some("library") {')
        end = restored.index(b'    if forwarded.first().map(String::as_str) == Some("fmt") {', start)
        self.assertEqual(restored[:start] + restored[end:], git_body(gate.PREDECESSOR, name))
        name = "src/frontend/driver.rs"
        current = gate.regular(ROOT, name)
        arm = b"        Route::TypedLibrary { request } => Some(process_library(&request[0])),\n"
        self.assertEqual(current.count(arm), 1)
        restored = current.replace(arm, b"", 1)
        start = restored.index(b"fn process_library(")
        self.assertEqual(restored[start - 2:start], b"\n\n")
        start -= 1  # The new function includes its one separating blank line.
        end = restored.index(b"/// Actual bridge-only I/O/result carriers", start)
        added = restored[start:end]
        self.assertIn(b"let limits = super::native_library::limits(options)", added)
        self.assertIn(b"oir::check_scalar_library(&project, &options.export, limits, &work", added)
        self.assertEqual(restored[:start] + restored[end:], git_body(gate.PREDECESSOR, name))

    def test_both_narrow_observer_closures_and_all_frozen_wrappers_stay_unchanged(self):
        for kind, count in (("parser", 40), ("static", 41)):
            name = "scripts/build_typed_" + kind + "_observer.py"
            body = gate.regular(ROOT, name)
            self.assertEqual(body, git_body(gate.PREDECESSOR, name), name)
            definitions = {node.targets[0].id: ast.literal_eval(node.value)
                           for node in ast.parse(body).body if isinstance(node, ast.Assign)
                           and len(node.targets) == 1 and isinstance(node.targets[0], ast.Name)
                           and node.targets[0].id in ("SOURCE_FILES", "WRAPPER_FILES")}
            sources = {"src/frontend/" + path for path in definitions["SOURCE_FILES"]}
            self.assertEqual(len(sources), count)
            self.assertFalse(sources & (gate.SCALAR_LIBRARY_CHANGED | gate.SCALAR_LIBRARY_ADDED))
            for source in sources:
                self.assertEqual(gate.regular(ROOT, source), git_body(gate.PREDECESSOR, source), source)
            for wrapper, _ in definitions["WRAPPER_FILES"]:
                name = "tests/fixtures/bounded_typed_" + kind + "/observer/" + wrapper
                self.assertEqual(gate.regular(ROOT, name), git_body(gate.PREDECESSOR, name), name)
        for path in sorted((ROOT / "scripts").glob("legacy_scalar_observer_*.py")):
            name = path.relative_to(ROOT).as_posix()
            self.assertEqual(gate.regular(ROOT, name), git_body(gate.PREDECESSOR, name), name)
        name = "scripts/test_legacy_scalar_observer_byte_ranges.py"
        self.assertEqual(gate.regular(ROOT, name), git_body(gate.PREDECESSOR, name), name)

    def test_all_58_predecessor_policies_and_adapters_are_exact(self):
        self.assertEqual(len(gate.PREDECESSOR_POLICIES), 58)
        self.assertEqual(gate.PREDECESSOR_POLICIES, {
            **gate.library.PREDECESSOR_POLICIES,
            "scripts/verify_file_seek_compatibility.py": gate.PREDECESSOR_POLICY_PIN[1],
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
                with self.assertRaisesRegex(ValueError, "changed pinned File seek policy"):
                    gate.load_predecessor(root)
                execute.assert_not_called()
            path.write_bytes(body)
            path.chmod(0o755)
            with patch.object(gate, "exec", create=True) as execute:
                with self.assertRaisesRegex(ValueError, "unsafe pinned File seek policy"):
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
        current_path = Path(gate.__file__).resolve()
        self.assertEqual(current_path, ROOT / "scripts/verify_scalar_library_compatibility.py")
        current = replay(current_path.read_bytes())
        predecessor = replay(git_body(gate.PREDECESSOR, gate.PREDECESSOR_POLICY))
        self.assertEqual(len(current), 2)
        self.assertEqual(len(predecessor), 2)
        self.assertEqual([ast.dump(node, include_attributes=False) for node in current],
                         [ast.dump(node, include_attributes=False) for node in predecessor])
        self.assertGreater(len(current[0].body), 10)
        self.assertIn("run_record_vector_observer_controls", ast.unparse(current[1]))

    def test_replay_comparison_rejects_each_mutated_successor_block(self):
        current_path = Path(gate.__file__).resolve()
        original = current_path.read_bytes()
        read_bytes = Path.read_bytes
        for index in (0, 1):
            parsed = ast.parse(original)
            run = next(node for node in parsed.body if isinstance(node, ast.FunctionDef) and node.name == "run")
            blocks = [node for node in ast.walk(run) if isinstance(node, ast.If)
                      and ast.unparse(node.test) == "not args.prepare_only"]
            self.assertEqual(len(blocks), 2)
            blocks[index].body.pop()
            changed = ast.unparse(parsed).encode()
            def selected(path):
                return changed if path.resolve() == current_path else read_bytes(path)
            with self.subTest(block=index), patch.object(Path, "read_bytes", new=selected):
                with self.assertRaises(AssertionError):
                    self.test_both_complete_Windows_replay_blocks_and_every_selector_are_unchanged()

    def test_CI_keeps_only_the_new_replay_job_and_exact_reviewed_SDK_patch(self):
        frozen = "9be31bc374bffee230768ecd1fe3e52b5a4ea965"
        name = ".github/workflows/ci.yml"
        current = gate.regular(ROOT, name)
        job = re.search(rb"^  file-seek-source-policy-predecessor:.*?(?=^  [A-Za-z0-9_-]+:|\Z)",
                        current, flags=re.M | re.S)
        self.assertIsNotNone(job)
        self.assertEqual((len(job.group()), gate.sha(job.group())),
                         (1036, "295be31f0fb6a6e3c2beca9819f0e50a17a7e479b80aa321b788de9ed0b81129"))
        restored, count = re.subn(
            rb"^  file-seek-source-policy-predecessor:.*?(?=^  [A-Za-z0-9_-]+:|\Z)", b"", current,
            flags=re.M | re.S,
        )
        self.assertEqual(count, 1)
        self.assertEqual(restored.count(NATIVE_LIBRARY_STEP), 1)
        self.assertEqual(restored.count(NATIVE_LIBRARY_EVIDENCE), 1)
        restored = restored.replace(NATIVE_LIBRARY_STEP, b"", 1).replace(NATIVE_LIBRARY_EVIDENCE, b"", 1)
        sdk = re.search(rb"^          \$sdkStart = .*?(?=^          \$names = )",
                        restored, flags=re.M | re.S)
        self.assertIsNotNone(sdk)
        self.assertEqual((len(sdk.group()), gate.sha(sdk.group())),
                         (3445, "e841b69ad012783a61cafd1e85a2b6d2e8c480e0a9934f1c587d89932db1c866"))
        previous_sdk = (
            b"          $PSNativeCommandArgumentPassing = 'Legacy'\n"
            + (r'          $settings = & $env:ComSpec /d /s /c "`"$devcmd`" -no_logo -arch=x64 -host_arch=x64 >nul && set"' + "\n").encode()
            + b"          if ($LASTEXITCODE -ne 0) { throw 'Visual Studio environment initialization failed' }\n"
        )
        restored = restored[:sdk.start()] + previous_sdk + restored[sdk.end():]
        gate_name = b"scripts/verify_scalar_library_compatibility.py"
        self.assertEqual(restored.count(gate_name), 1)
        canonical = git_body(frozen, name)
        self.assertEqual(restored.replace(gate_name, b"scripts/verify_file_seek_compatibility.py", 1), canonical)
        check = (b"          python -B scripts/test_windows_native_ci.py\n"
                 b"          if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }\n")
        legacy = b"          $PSNativeCommandArgumentPassing = 'Legacy'\n"
        before = (r'          $settings = & $env:ComSpec /d /s /c "`"`"$devcmd`" -no_logo -arch=x64 -host_arch=x64 >nul && set`""' + "\n").encode()
        after = (r'          $settings = & $env:ComSpec /d /s /c "`"$devcmd`" -no_logo -arch=x64 -host_arch=x64 >nul && set"' + "\n").encode()
        for added in (check, legacy, after):
            self.assertEqual(canonical.count(added), 1)
        self.assertEqual(canonical.replace(check, b"", 1).replace(legacy, b"", 1).replace(after, before, 1),
                         git_body(gate.PREDECESSOR, name))

    def test_CI_comparison_refuses_changed_SDK_transport_and_bounds(self):
        name = ".github/workflows/ci.yml"
        current = gate.regular(ROOT, name)
        regular = gate.regular
        for before, after in (
            (b"$sdkStart.Arguments = ", b"$sdkStart.ArgumentList = "),
            (b"/d /u /s /c", b"/d /s /c"),
            (b"-ge 120", b"-ge 3600"),
            (b"-gt 1048576", b"-gt 2147483647"),
            (b"-gt 65536", b"-gt 2147483647"),
            (b"$sdk.ExitCode -ne 0", b"$sdk.ExitCode -ne 37"),
            (b"$sdk.Dispose()", b"# no dispose"),
        ):
            self.assertEqual(current.count(before), 1)
            changed = current.replace(before, after, 1)
            def selected(root, path):
                return changed if path == name else regular(root, path)
            with self.subTest(change=before), patch.object(gate, "regular", side_effect=selected):
                with self.assertRaises(AssertionError):
                    self.test_CI_keeps_only_the_new_replay_job_and_exact_reviewed_SDK_patch()

    def test_CI_comparison_refuses_an_unrelated_job_after_the_new_replay_job(self):
        name = ".github/workflows/ci.yml"
        current = gate.regular(ROOT, name)
        anchor = b"  macos-native:\n"
        self.assertEqual(current.count(anchor), 1)
        regular = gate.regular
        for heading in (b"unrelated-job", b"'unrelated-job'", b'"unrelated-job"'):
            extra = (b"  " + heading + b":\n    runs-on: ubuntu-latest\n"
                     b"    steps:\n      - run: true\n\n")
            changed = current.replace(anchor, extra + anchor, 1)
            def selected(root, path):
                return changed if path == name else regular(root, path)
            with self.subTest(heading=heading), patch.object(gate, "regular", side_effect=selected):
                with self.assertRaises(AssertionError):
                    self.test_CI_keeps_only_the_new_replay_job_and_exact_reviewed_SDK_patch()

    def test_native_clients_are_required_in_both_profiles_with_retained_evidence(self):
        current = gate.regular(ROOT, ".github/workflows/ci.yml")
        job = re.search(rb"^  native-projects:.*?(?=^  [A-Za-z0-9_-]+:|\Z)", current,
                        flags=re.M | re.S)
        self.assertIsNotNone(job)
        job = job.group()
        self.assertNotRegex(job, rb"(?m)^    if:|continue-on-error:")
        self.assertEqual(current.count(NATIVE_LIBRARY_STEP), 1)
        self.assertEqual(job.count(NATIVE_LIBRARY_STEP), 1)
        self.assertLess(job.index(NATIVE_LIBRARY_STEP),
                        job.index(b"      - name: Verify source-free file copies larger than the process memory limit\n"))
        upload = re.search(rb"^      - name: Retain current native evidence\n.*?(?=^      -|\Z)", job,
                           flags=re.M | re.S)
        self.assertIsNotNone(upload)
        self.assertEqual(current.count(NATIVE_LIBRARY_EVIDENCE), 1)
        self.assertEqual(upload.group().count(NATIVE_LIBRARY_EVIDENCE), 1)
        self.assertIn(b"        if: always()\n", upload.group())
        self.assertIn(b"        uses: actions/upload-artifact@v4\n", upload.group())
        self.assertIn(b"          retention-days: 14\n", upload.group())
        public = gate.regular(ROOT, "tests/native_library.rs")
        tests = re.findall(rb"#\[test\]\s*#\[ignore[^\]]*\]\s*fn (actual_same_library\w*)\s*\(", public)
        self.assertEqual(tests, [NATIVE_LIBRARY_TEST])
        self.assertEqual(re.findall(rb"\bfn\s+(actual_same_library\w*)\s*\(", public), tests)
        self.assertEqual(public.count(b"fn " + NATIVE_LIBRARY_TEST + b"("), 1)
        guard = re.search(
            rb'#\[cfg\(all\(\s*target_os = "linux",\s*target_pointer_width = "64",\s*'
            rb'target_endian = "little",\s*any\(target_arch = "x86_64", target_arch = "aarch64"\)\s*'
            rb'\)\)\]\s*mod linux \{', public,
        )
        self.assertIsNotNone(guard)
        # Follow the repository's rustfmt module indentation, independently of
        # the native test body. The next unindented closing brace ends linux.
        end = re.search(rb"(?m)^}\s*$", public[guard.end():])
        self.assertIsNotNone(end)
        selected = re.search(rb"(?m)^    fn " + NATIVE_LIBRARY_TEST + rb"\(", public)
        self.assertIsNotNone(selected)
        self.assertLess(guard.end(), selected.start())
        self.assertLess(selected.start(), guard.end() + end.start())

    def test_native_client_CI_refuses_lost_profiles_filters_flags_and_artifacts(self):
        name = ".github/workflows/ci.yml"
        current = gate.regular(ROOT, name)
        debug = b"          cargo test --locked --test native_library actual_same_library -- --ignored --nocapture"
        release = b"          cargo test --release --locked --test native_library actual_same_library -- --ignored --nocapture"
        keep = b"          OXID_TEST_KEEP_ARTIFACTS: '1'\n"
        temporary = b"          TMPDIR: ${{ runner.temp }}/current-native-libraries\n"
        mkdir = b'          mkdir -p "$TMPDIR"\n'
        marker = b"      - name: Verify returning native libraries from C Rust and Python\n"
        cases = [
            ("missing-debug", debug + b"\n", b""),
            ("missing-release", release + b"\n", b""),
            ("lost-release-profile", release, release.replace(b" --release", b"")),
            ("not-kept", keep, keep.replace(b"'1'", b"'0'")),
            ("missing-keep", keep, b""),
            ("wrong-tempdir", temporary, temporary.replace(b"current-native-libraries", b"unretained")),
            ("missing-tempdir", temporary, b""),
            ("missing-mkdir", mkdir, b""),
            ("conditional-step", marker, marker + b"        if: false\n"),
            ("soft-failed-step", marker, marker + b"        continue-on-error: true\n"),
            ("conditional-job", b"  native-projects:\n", b"  native-projects:\n    if: false\n"),
            ("missing-evidence", NATIVE_LIBRARY_EVIDENCE, b""),
            ("wrong-evidence", NATIVE_LIBRARY_EVIDENCE,
             NATIVE_LIBRARY_EVIDENCE.replace(b"current-native-libraries", b"unretained")),
            ("duplicate-step", NATIVE_LIBRARY_STEP, NATIVE_LIBRARY_STEP * 2),
            ("duplicate-evidence", NATIVE_LIBRARY_EVIDENCE, NATIVE_LIBRARY_EVIDENCE * 2),
        ]
        for profile, command in (("debug", debug), ("release", release)):
            cases.extend([
                (profile + "-not-ignored", command, command.replace(b" --ignored", b"")),
                (profile + "-wrong-filter", command, command.replace(b"actual_same_library", b"another_library")),
                (profile + "-missing-capture", command, command.replace(b" --nocapture", b"")),
            ])
        regular = gate.regular
        for label, before, after in cases:
            self.assertEqual(current.count(before), 1, label)
            changed = current.replace(before, after, 1)
            def selected(root, path):
                return changed if path == name else regular(root, path)
            with self.subTest(case=label), patch.object(gate, "regular", side_effect=selected):
                with self.assertRaises(AssertionError):
                    self.test_native_clients_are_required_in_both_profiles_with_retained_evidence()
                with self.assertRaises(AssertionError):
                    self.test_CI_keeps_only_the_new_replay_job_and_exact_reviewed_SDK_patch()
        public_path = "tests/native_library.rs"
        public = regular(ROOT, public_path)
        ignore = re.search(rb'(?m)^    #\[ignore[^\]]*\]\n(?=    fn ' + NATIVE_LIBRARY_TEST + rb'\()', public)
        self.assertIsNotNone(ignore)
        module_guard = re.search(rb'#\[cfg\(all\([^\]]+\)\)\]\s*mod linux \{', public)
        self.assertIsNotNone(module_guard)
        public_mutations = (
            public.replace(b"fn " + NATIVE_LIBRARY_TEST, b"fn renamed_native_client", 1),
            public.replace(ignore.group(), b"", 1),
            public.replace(b"    #[test]\n" + ignore.group(), ignore.group(), 1),
            public.replace(module_guard.group(), b"mod linux {", 1),
            public.replace(module_guard.group(), module_guard.group().replace(b'"linux"', b'"windows"', 1), 1),
            public + b"\nfn " + NATIVE_LIBRARY_TEST + b"() {}\n",
        )
        for index, changed in enumerate(public_mutations):
            self.assertNotEqual(public, changed)
            def selected(root, path):
                return changed if path == public_path else regular(root, path)
            with self.subTest(public_selector=index), patch.object(gate, "regular", side_effect=selected):
                with self.assertRaises(AssertionError):
                    self.test_native_clients_are_required_in_both_profiles_with_retained_evidence()

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
            self.assertNotIn(name, gate.SCALAR_LIBRARY_CHANGED)
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
