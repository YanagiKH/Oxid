"""Mutation controls for buffer transfer's exact compiler-input boundary."""
import ast
import builtins
import hashlib
import inspect
import os
from pathlib import Path
import symtable
import tempfile
import unittest
from unittest.mock import patch

import verify_buffer_transfer_compatibility as gate

ROOT = Path(__file__).resolve().parents[1]


class BufferTransferAdmission(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        self.baseline = {name: b"// original\n" for name in gate.TRANSFER_CHANGED - gate.PARENT_ADDED}
        self.baseline.update({"native/runtime.c": b"/* runtime */\n", "Cargo.toml": b"[package]\n",
                              "src/frontend/oir/owned_types.rs": b"// unchanged type authority\n"})
        self.parent = {**self.baseline, **{name: b"// parent addition\n" for name in gate.PARENT_ADDED}}
        self.current = dict(self.parent)
        self.current.update({name: b"// reviewed buffer transfer implementation\n" for name in gate.TRANSFER_CHANGED})
        for name, body in self.current.items():
            self.put(name, body)
        reviewed = {name: (len(self.current[name]), gate.sha(self.current[name])) for name in gate.TRANSFER_CHANGED}
        mocked = patch.object(gate, "TRANSFER_INPUTS", reviewed)
        mocked.start()
        self.addCleanup(mocked.stop)

    def put(self, name, body):
        path = self.root / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(body)

    def capture(self):
        return gate.capture_sources(self.root, self.baseline, self.parent)

    def test_reviewed_delta_preserves_every_other_parent_byte_and_reports_bounded_scope(self):
        inputs = self.capture()
        self.assertEqual(set(inputs), set(self.parent))
        for name in set(self.parent) - gate.TRANSFER_CHANGED:
            self.assertEqual(inputs[name], self.parent[name])
        manifest = gate.current_manifest(inputs, self.baseline, {"source_kind": "worktree"}, self.parent)
        self.assertEqual({row["path"] for row in manifest["buffer_transfer_changes"]}, gate.TRANSFER_CHANGED)
        self.assertEqual(manifest["source_policy_parent"], gate.PREDECESSOR)
        self.assertEqual({row["path"] for row in manifest["files"]}, set(inputs))
        for key, value in manifest.items():
            if key.endswith("qualified_by_this_gate"):
                self.assertIs(value, False, key)
        self.assertIs(manifest["owned_buffer_transfer_semantics_qualified_by_this_gate"], False)

    def test_all_old_feature_allowances_stay_closed_outside_the_new_boundary(self):
        for name in set(self.parent) - gate.TRANSFER_CHANGED:
            with self.subTest(name=name):
                self.put(name, self.parent[name] + b"// unreviewed\n")
                with self.assertRaisesRegex(ValueError, "outside buffer-transfer boundary"):
                    self.capture()
                self.put(name, self.parent[name])

    def test_reviewed_lengths_hashes_and_exact_roster_are_all_required(self):
        name = "src/frontend/oir/owned/execute_experimental.rs"
        original = gate.TRANSFER_INPUTS[name]
        for value in ((original[0] + 1, original[1]), (original[0], "0" * 64)):
            with self.subTest(value=value):
                gate.TRANSFER_INPUTS[name] = value
                with self.assertRaisesRegex(ValueError, "reviewed input"):
                    self.capture()
        gate.TRANSFER_INPUTS[name] = original
        del gate.TRANSFER_INPUTS[name]
        with self.assertRaisesRegex(ValueError, "reviewed buffer-transfer input membership"):
            self.capture()
        gate.TRANSFER_INPUTS[name] = original
        gate.TRANSFER_INPUTS["src/extra.rs"] = original
        with self.assertRaisesRegex(ValueError, "reviewed buffer-transfer input membership"):
            self.capture()

    def test_extra_missing_and_empty_directory_members_are_rejected(self):
        self.put("native/extra.h", b"/* extra */\n")
        with self.assertRaisesRegex(ValueError, "membership"):
            self.capture()
        (self.root / "native/extra.h").unlink()
        name = "native/typed_bytebuf.c"
        (self.root / name).unlink()
        with self.assertRaisesRegex(ValueError, "membership"):
            self.capture()
        self.put(name, self.current[name])
        (self.root / "src/unlisted-empty").mkdir()
        with self.assertRaisesRegex(ValueError, "directory"):
            self.capture()

    def test_missing_parent_members_cannot_redefine_the_authority(self):
        name = next(iter(gate.PARENT_ADDED))
        del self.parent[name]
        with self.assertRaisesRegex(ValueError, "predecessor compiler input membership"):
            self.capture()

    def test_even_a_coherently_changed_pin_cannot_add_compile_time_inputs(self):
        name = "src/frontend/experimental_builtins.rs"
        body = self.current[name] + b'const X:&str=include_str!("/outside.txt");\n'
        self.put(name, body)
        gate.TRANSFER_INPUTS[name] = (len(body), gate.sha(body))
        with self.assertRaisesRegex(ValueError, "compile-time input directive"):
            self.capture()

    @unittest.skipIf(os.name == "nt", "Unix source link and mode checks")
    def test_linked_and_executable_sources_cannot_enter(self):
        name = "native/typed_bytebuf.c"
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


class BufferTransferPolicyAuthority(unittest.TestCase):
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
                with self.assertRaisesRegex(ValueError, "changed pinned hosted"):
                    gate.load_predecessor(root)

    def test_all_five_preceding_policy_files_are_unchanged_and_authenticated(self):
        self.assertEqual(len(gate.PREDECESSOR_POLICIES), 5)
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
        for name in ("extract_baseline", "checkout_identity", "verify_materialized", "prepare_current_observer",
                     "verify_current_observer", "run_byte_buffer_observer_controls"):
            self.assertIs(getattr(gate, name), getattr(gate.library, name))
        self.assertEqual(len(gate.TRANSFER_CHANGED), 7)
        self.assertEqual(set(gate.TRANSFER_INPUTS), gate.TRANSFER_CHANGED)
        self.assertEqual(gate.PARENT_ADDED, gate.library.PARENT_ADDED)

    def test_every_controller_function_global_resolves_in_both_execution_modes(self):
        table = symtable.symtable(Path(gate.__file__).read_text(), gate.__file__, "exec")
        available = set(vars(gate)) | set(vars(builtins))
        for child in table.get_children():
            if child.get_type() == "function":
                self.assertFalse(set(child.get_globals()) - available, child.get_name())


if __name__ == "__main__":
    unittest.main()
