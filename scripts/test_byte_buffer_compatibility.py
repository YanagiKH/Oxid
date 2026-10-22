"""Mutation controls for the growable byte-buffer source authority boundary."""
import ast
import hashlib
import inspect
import os
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

import verify_byte_buffer_compatibility as gate
import verify_byte_field_compatibility as predecessor

ROOT = Path(__file__).resolve().parents[1]


class ByteBufferSourceAdmission(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        self.baseline = {name: b"// baseline\n" for name in gate.CHANGED - gate.ADDED}
        self.baseline.update({"native/runtime.c": b"/* runtime */\n",
                              "src/frontend/typeck.rs": b"// scalar authority\n",
                              "Cargo.toml": b'[package]\nname="test"\n',
                              "fixture.ox": b"fn main()->i32{return 7;}\n"})
        self.parent = {name: (b"// admitted predecessor\n" if name in gate.CHANGED else body)
                       for name, body in self.baseline.items()}
        self.parent.update({name: b"// predecessor addition\n" for name in gate.PARENT_ADDED})
        inputs = dict(self.parent)
        inputs.update({name: b"// reviewed buffer input\n"
                       for name in gate.BYTE_BUFFER_CHANGED | gate.BYTE_BUFFER_ADDED})
        for name, body in inputs.items():
            self.put(name, body)
        reviewed = {name: (len(inputs[name]), gate.sha(inputs[name]))
                    for name in gate.BYTE_BUFFER_CHANGED | gate.BYTE_BUFFER_ADDED}
        for name, value in (("BYTE_BUFFER_INPUTS", reviewed), ("REVIEWED_DIRECTIVES", {})):
            mocked = patch.object(gate, name, value)
            mocked.start()
            self.addCleanup(mocked.stop)

    def put(self, name, body):
        path = self.root / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(body)

    def capture(self):
        return gate.capture_sources(self.root, self.baseline, self.parent)

    def test_reviewed_delta_preserves_parent_closure_and_reports_separate_scope(self):
        inputs = self.capture()
        manifest = gate.current_manifest(inputs, self.baseline, {"source_kind": "worktree"}, self.parent)
        self.assertEqual({row["path"] for row in manifest["byte_buffer_changes"]}, gate.BYTE_BUFFER_CHANGED)
        self.assertEqual(set(manifest["byte_buffer_added_inputs"]), gate.BYTE_BUFFER_ADDED)
        self.assertEqual({row["path"] for row in manifest["files"]}, set(self.parent) | gate.BYTE_BUFFER_ADDED)
        self.assertEqual(manifest["source_policy_parent"], gate.PREDECESSOR)
        for key in ("byte_buffer_semantics_qualified_by_this_gate", "binary_io_semantics_qualified_by_this_gate",
                    "native_heap_safety_qualified_by_this_gate", "byte_field_semantics_qualified_by_this_gate",
                    "owned_enum_semantics_qualified_by_this_gate", "native_optimization_qualified_by_this_gate",
                    "native_reproducibility_qualified_by_this_gate", "package_semantics_qualified_by_this_gate",
                    "workspace_semantics_qualified_by_this_gate"):
            self.assertIs(manifest[key], False)
        for name in set(self.parent) - gate.BYTE_BUFFER_CHANGED:
            self.assertEqual(inputs[name], self.parent[name])

    def test_old_project_enum_field_and_added_allowances_cannot_authorize_new_edits(self):
        old = gate.PROJECT_CHANGED | gate.OWNED_ENUM_CHANGED | gate.BYTE_FIELD_CHANGED | gate.PARENT_ADDED
        for name in old - gate.BYTE_BUFFER_CHANGED:
            with self.subTest(name=name):
                self.put(name, self.parent[name] + b"// unreviewed edit\n")
                with self.assertRaisesRegex(ValueError, "outside byte buffer boundary"):
                    self.capture()
                self.put(name, self.parent[name])

    def test_reviewed_roster_length_and_hash_are_all_required(self):
        name = "src/frontend/oir/owned/execute.rs"
        original = gate.BYTE_BUFFER_INPUTS[name]
        for value in ((original[0] + 1, original[1]), (original[0], "0" * 64)):
            gate.BYTE_BUFFER_INPUTS[name] = value
            with self.assertRaisesRegex(ValueError, "reviewed input"):
                self.capture()
        gate.BYTE_BUFFER_INPUTS[name] = original
        del gate.BYTE_BUFFER_INPUTS[name]
        with self.assertRaisesRegex(ValueError, "reviewed byte buffer input membership"):
            self.capture()
        gate.BYTE_BUFFER_INPUTS[name] = original
        gate.BYTE_BUFFER_INPUTS["src/unlisted.rs"] = original
        with self.assertRaisesRegex(ValueError, "reviewed byte buffer input membership"):
            self.capture()

    def test_extra_missing_and_empty_directory_members_are_rejected(self):
        self.put("native/unlisted.h", b"/* extra */\n")
        with self.assertRaisesRegex(ValueError, "membership"):
            self.capture()
        (self.root / "native/unlisted.h").unlink()
        name = "native/experimental_bytebuf.h"
        original = (self.root / name).read_bytes()
        (self.root / name).unlink()
        with self.assertRaisesRegex(ValueError, "membership"):
            self.capture()
        self.put(name, original)
        (self.root / "src/unlisted-empty").mkdir()
        with self.assertRaisesRegex(ValueError, "directory"):
            self.capture()

    def test_predecessor_membership_cannot_lose_old_inputs_or_gain_buffer_additions(self):
        name = "src/frontend/oir/owned/owned_enum_tests.rs"
        original = self.parent.pop(name)
        with self.assertRaisesRegex(ValueError, "predecessor compiler input membership"):
            self.capture()
        self.parent[name] = original
        self.parent["native/typed_bytebuf.c"] = b"/* not a parent member */\n"
        with self.assertRaisesRegex(ValueError, "predecessor compiler input membership"):
            self.capture()

    def test_macros_aliases_unicode_and_generated_paths_cannot_escape_exact_pins(self):
        name = "src/frontend/oir/owned/source/typeck.rs"
        for body in (b'include_str!("../../outside");',
                     b'use std::include_bytes as input; const X: &[u8] = input!("../../outside");',
                     b'macro_rules! load { ($p:ident) => { #[$p="../../outside.rs"] mod extra; }; } load!(path);',
                     b'#[cfg_attr(all(), path="../../outside.rs")] mod extra;',
                     'const _: char = \'é\'; use std::r#include_str as input;'.encode()):
            with self.subTest(body=body):
                self.put(name, body)
                with self.assertRaisesRegex(ValueError, "reviewed input"):
                    self.capture()

    def test_all_embedded_c_inputs_remain_exact_reviewed_bytes(self):
        for name in ("native/experimental_bytebuf.c", "native/experimental_bytebuf.h", "native/typed_bytebuf.c"):
            original = (self.root / name).read_bytes()
            self.put(name, original + b'\n#include "/outside.h"\n')
            with self.subTest(name=name), self.assertRaisesRegex(ValueError, "reviewed input"):
                self.capture()
            self.put(name, original)

    def test_reviewed_directives_are_literal_and_still_require_complete_file_identity(self):
        name = "src/frontend/native.rs"
        body = b'const X:&str=include_str!("../../native/typed_bytebuf.c");\n'
        self.put(name, body)
        gate.BYTE_BUFFER_INPUTS[name] = (len(body), gate.sha(body))
        gate.REVIEWED_DIRECTIVES[name] = ((b'include_str!("../../native/typed_bytebuf.c")',), ())
        self.capture()
        gate.REVIEWED_DIRECTIVES[name] = ((), ())
        with self.assertRaisesRegex(ValueError, "compile-time input directive"):
            self.capture()
        gate.REVIEWED_DIRECTIVES[name] = ((b'include_str!("../../native/typed_bytebuf.c")',), ())
        self.put(name, body + b'const OTHER:&str=include_str!("../../outside");')
        with self.assertRaisesRegex(ValueError, "reviewed input"):
            self.capture()

    @unittest.skipIf(os.name == "nt", "Unix source mode/link admission")
    def test_symlinks_hardlinks_and_executable_sources_cannot_enter(self):
        name = "native/experimental_bytebuf.h"
        path = self.root / name
        body = path.read_bytes()
        outside = self.root.parent / (self.root.name + "-outside.h")
        self.addCleanup(lambda: outside.unlink(missing_ok=True))
        outside.write_bytes(body)
        path.unlink()
        path.symlink_to(outside)
        with self.assertRaisesRegex(ValueError, "symlink"):
            self.capture()
        path.unlink()
        os.link(outside, path)
        with self.assertRaisesRegex(ValueError, "nonregular"):
            self.capture()
        path.unlink()
        self.put(name, body)
        path.chmod(0o755)
        with self.assertRaisesRegex(ValueError, "nonregular"):
            self.capture()


class ByteBufferPublishedAuthority(unittest.TestCase):
    def test_actual_reviewed_source_roster_and_bytes_match_literal_pins(self):
        self.assertEqual(len(gate.BYTE_BUFFER_CHANGED), 49)
        self.assertEqual(len(gate.BYTE_BUFFER_ADDED), 21)
        self.assertFalse(gate.BYTE_BUFFER_CHANGED & gate.BYTE_BUFFER_ADDED)
        self.assertEqual(set(gate.BYTE_BUFFER_INPUTS), gate.BYTE_BUFFER_CHANGED | gate.BYTE_BUFFER_ADDED)
        for name, (size, digest) in gate.BYTE_BUFFER_INPUTS.items():
            with self.subTest(name=name):
                body = (ROOT / name).read_bytes()
                self.assertEqual((len(body), gate.sha(body)), (size, digest))
        self.assertEqual(len(gate.REVIEWED_DIRECTIVES), 9)
        for name, expected in gate.REVIEWED_DIRECTIVES.items():
            self.assertEqual(tuple(tuple(part) for part in gate.directives((ROOT / name).read_bytes())), expected)

    def test_archive_and_materialization_authentication_remain_unchanged(self):
        for name in ("extract_baseline", "git", "regular", "verify_materialized"):
            before = ast.dump(ast.parse(inspect.getsource(getattr(predecessor, name))))
            after = ast.dump(ast.parse(inspect.getsource(getattr(gate, name))))
            self.assertEqual(after, before, name)
        self.assertEqual(hashlib.sha256(Path(predecessor.__file__).read_bytes()).hexdigest(), gate.PREDECESSOR_POLICY_SHA)

    def test_all_preceding_source_policies_are_authenticated_before_and_after(self):
        self.assertEqual(set(gate.PREDECESSOR_POLICIES), {
            "scripts/verify_project_workflow_current.py", "scripts/verify_enum_payload_compatibility.py",
            "scripts/verify_byte_field_compatibility.py"})
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
                (repo / name).write_bytes(body + b"# unreviewed\n")
                with self.assertRaisesRegex(ValueError, "source policy changed"):
                    gate.authenticate_policies(repo, parent)
                (repo / name).write_bytes(body)
                (parent / name).write_bytes(body + b"# changed archive\n")
                with self.assertRaisesRegex(ValueError, "wrong pinned predecessor"):
                    gate.authenticate_policies(repo, parent)
                (parent / name).write_bytes(body)


if __name__ == "__main__":
    unittest.main()
