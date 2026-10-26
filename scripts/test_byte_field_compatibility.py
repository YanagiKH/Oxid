"""Mutation controls for the byte-record-field compatibility boundary."""
import ast
import hashlib
import inspect
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch

import verify_byte_field_compatibility as gate
import verify_enum_payload_compatibility as predecessor


class ByteFieldSourceAdmission(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        self.baseline = {name: b"// baseline\n" for name in gate.CHANGED}
        self.baseline.update({"native/runtime.c": b"/* runtime */\n",
                              "src/frontend/typeck.rs": b"// scalar authority\n",
                              "Cargo.toml": b'[package]\nname="test"\n',
                              "fixture.ox": b"fn main() -> i32 { return 0; }\n"})
        self.parent = dict(self.baseline)
        self.parent.update({name: b"// published parent addition\n" for name in gate.ADDED})
        self.parent["src/cli.rs"] = b"// published project command\n"
        self.parent["src/frontend/oir/owned/execute.rs"] = b"// published enum executor\n"
        for name, body in self.parent.items():
            self.put(name, body)
        for name in gate.BYTE_FIELD_CHANGED:
            self.put(name, b"// reviewed byte-field implementation\n")
        reviewed = {name: (len(body), gate.sha(body)) for name in gate.BYTE_FIELD_CHANGED
                    for body in [(self.root / name).read_bytes()]}
        mocked = patch.object(gate, "BYTE_FIELD_INPUTS", reviewed)
        mocked.start()
        self.addCleanup(mocked.stop)

    def put(self, name, body):
        path = self.root / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(body)

    def capture(self):
        return gate.capture_sources(self.root, self.baseline, self.parent)

    def test_reviewed_fields_preserve_parent_input_closure_and_scope(self):
        inputs = self.capture()
        manifest = gate.current_manifest(inputs, self.baseline, {"source_kind": "worktree"}, self.parent)
        self.assertEqual({row["path"] for row in manifest["byte_field_changes"]}, gate.BYTE_FIELD_CHANGED)
        self.assertEqual({row["path"] for row in manifest["files"]}, set(self.parent))
        self.assertEqual(manifest["source_policy_parent"], gate.PREDECESSOR)
        for key in ("byte_field_semantics_qualified_by_this_gate", "owned_enum_semantics_qualified_by_this_gate",
                    "native_optimization_qualified_by_this_gate", "native_reproducibility_qualified_by_this_gate",
                    "package_semantics_qualified_by_this_gate", "workspace_semantics_qualified_by_this_gate"):
            self.assertIs(manifest[key], False)
        for name in set(self.parent) - gate.BYTE_FIELD_CHANGED:
            self.assertEqual(inputs[name], self.parent[name])

    def test_old_project_and_enum_allowances_cannot_authorize_new_changes(self):
        for name in (gate.PROJECT_CHANGED | gate.ADDED | gate.OWNED_ENUM_CHANGED) - gate.BYTE_FIELD_CHANGED:
            with self.subTest(name=name):
                self.put(name, self.parent[name] + b"// extra implementation\n")
                with self.assertRaisesRegex(ValueError, "outside byte field boundary"):
                    self.capture()
                self.put(name, self.parent[name])

    def test_reviewed_members_length_and_hash_are_all_mandatory(self):
        name = "src/frontend/oir/owned/source/resolve.rs"
        original = gate.BYTE_FIELD_INPUTS[name]
        for replacement in ((original[0] + 1, original[1]), (original[0], "0" * 64)):
            gate.BYTE_FIELD_INPUTS[name] = replacement
            with self.assertRaisesRegex(ValueError, "reviewed input"):
                self.capture()
        gate.BYTE_FIELD_INPUTS[name] = original
        del gate.BYTE_FIELD_INPUTS[name]
        with self.assertRaisesRegex(ValueError, "reviewed byte field input membership"):
            self.capture()
        gate.BYTE_FIELD_INPUTS[name] = original
        gate.BYTE_FIELD_INPUTS["src/unlisted.rs"] = original
        with self.assertRaisesRegex(ValueError, "reviewed byte field input membership"):
            self.capture()

    def test_extra_missing_and_empty_directory_members_cannot_enter(self):
        self.put("src/unreviewed.rs", b"// extra\n")
        with self.assertRaisesRegex(ValueError, "membership"):
            self.capture()
        (self.root / "src/unreviewed.rs").unlink()
        name = "src/frontend/oir/owned/source/resolve.rs"
        body = (self.root / name).read_bytes()
        (self.root / name).unlink()
        with self.assertRaisesRegex(ValueError, "membership"):
            self.capture()
        self.put(name, body)
        (self.root / "src/empty").mkdir()
        with self.assertRaisesRegex(ValueError, "directory"):
            self.capture()

    def test_parent_membership_cannot_drop_a_protected_enum_input(self):
        del self.parent["src/frontend/oir/owned/owned_enum_tests.rs"]
        with self.assertRaisesRegex(ValueError, "predecessor compiler input membership"):
            self.capture()

    def test_macro_aliases_or_generated_paths_do_not_escape_reviewed_bytes(self):
        name = "src/frontend/oir/owned/source/resolve.rs"
        for body in (b'include_str!("../../outside");',
                     b'use std::include_str as input; const X: &str = input!("../../outside");',
                     b'macro_rules! load { ($p:ident) => { #[$p="../../outside.rs"] mod extra; }; } load!(path);',
                     'const _: char = \'é\'; use std::r#include_bytes as input;'.encode()):
            with self.subTest(body=body):
                self.put(name, body)
                with self.assertRaisesRegex(ValueError, "reviewed input"):
                    self.capture()

    def test_predecessor_archive_and_materialization_authentication_are_retained(self):
        for name in ("extract_baseline", "git", "regular", "verify_materialized"):
            before = ast.dump(ast.parse(inspect.getsource(getattr(predecessor, name))))
            after = ast.dump(ast.parse(inspect.getsource(getattr(gate, name))))
            self.assertEqual(after, before, name)
        self.assertEqual(hashlib.sha256(Path(predecessor.__file__).read_bytes()).hexdigest(),
                         gate.PREDECESSOR_POLICY_SHA)


class ByteFieldPinnedArchiveAdmission(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        self.repo = self.root / "repo"
        self.repo.mkdir()
        gate.git(self.repo, "init", "-q")
        (self.repo / "source.rs").write_bytes(b"// original $Format:%H$\n")
        (self.repo / "policy.py").write_bytes(b"# unchanged policy\n")
        gate.git(self.repo, "add", ".")
        self.commit("original")
        self.original = gate.git(self.repo, "rev-parse", "HEAD").decode().strip()

    def commit(self, message):
        gate.git(self.repo, "-c", "user.name=Archive test", "-c", "user.email=archive@example.invalid",
                 "commit", "-qam", message)

    def test_local_replacement_cannot_redefine_the_published_parent(self):
        (self.repo / "source.rs").write_bytes(b"// replacement\n")
        self.commit("replacement")
        replacement = gate.git(self.repo, "rev-parse", "HEAD").decode().strip()
        gate.git(self.repo, "replace", self.original, replacement)
        self.assertEqual(subprocess.check_output(["git", "-C", str(self.repo), "show",
                                                 self.original + ":source.rs"]), b"// replacement\n")
        destination = self.root / "verified"
        gate.extract_baseline(self.repo, destination, revision=self.original)
        self.assertEqual((destination / "source.rs").read_bytes(), b"// original $Format:%H$\n")

    def test_local_archive_attributes_cannot_transform_or_omit_parent_blobs(self):
        attributes = self.repo / ".git/info/attributes"
        for attribute in ("source.rs export-subst\n", "policy.py export-ignore\n"):
            with self.subTest(attribute=attribute):
                attributes.write_text(attribute)
                with self.assertRaisesRegex(ValueError, "pinned archive (blob(?: length)?|tree membership) changed"):
                    gate.extract_baseline(self.repo, self.root / attribute.split()[1], revision=self.original)

    def test_predecessor_controller_remains_the_exact_published_file(self):
        policy = Path(__file__).with_name("verify_enum_payload_compatibility.py")
        self.assertEqual(hashlib.sha256(policy.read_bytes()).hexdigest(), gate.PREDECESSOR_POLICY_SHA)


if __name__ == "__main__":
    unittest.main()
