"""Mutation controls for the finite-enum source compatibility boundary."""
import hashlib
import json
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch

import verify_enum_payload_compatibility as gate


class EnumSourceAdmission(unittest.TestCase):
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
        self.parent.update({name: b"// parent addition\n" for name in gate.PROJECT_ADDED})
        self.parent["src/cli.rs"] = b"// published project command\n"
        for name, body in self.parent.items():
            self.put(name, body)
        for name in gate.OWNED_ENUM_ADDED:
            self.put(name, b"// enum addition\n")
        for name in gate.OWNED_ENUM_CHANGED:
            self.put(name, b"// finite payload implementation\n")
        reviewed = {name: (len(body), gate.sha(body))
                    for name in gate.OWNED_ENUM_CHANGED | gate.OWNED_ENUM_ADDED
                    for body in [(self.root / name).read_bytes()]}
        self.addCleanup(patch.stopall)
        patch.object(gate, "OWNED_ENUM_INPUTS", reviewed).start()

    def put(self, name, body):
        path = self.root / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(body)

    def capture(self):
        return gate.capture_sources(self.root, self.baseline, self.parent)

    def test_enum_delta_is_recorded_without_claiming_new_semantic_qualification(self):
        inputs = self.capture()
        manifest = gate.current_manifest(inputs, self.baseline, {"source_kind": "worktree"}, self.parent)
        self.assertEqual({row["path"] for row in manifest["owned_enum_changes"]}, gate.OWNED_ENUM_CHANGED)
        self.assertEqual(set(manifest["owned_enum_additions"]), gate.OWNED_ENUM_ADDED)
        self.assertEqual({row["path"] for row in manifest["files"]}, set(self.parent) | gate.OWNED_ENUM_ADDED)
        self.assertEqual(manifest["source_policy_parent"], gate.PREDECESSOR)
        for key in ("owned_enum_semantics_qualified_by_this_gate",
                    "native_optimization_qualified_by_this_gate",
                    "native_reproducibility_qualified_by_this_gate",
                    "package_semantics_qualified_by_this_gate",
                    "workspace_semantics_qualified_by_this_gate"):
            self.assertIs(manifest[key], False)
        self.assertEqual(inputs["src/cli.rs"], self.parent["src/cli.rs"])

    def test_pin_roster_length_and_hash_are_independently_mandatory(self):
        name = "src/frontend/ast.rs"
        original = gate.OWNED_ENUM_INPUTS[name]
        for replacement in ((original[0] + 1, original[1]), (original[0], "0" * 64)):
            with self.subTest(replacement=replacement):
                gate.OWNED_ENUM_INPUTS[name] = replacement
                with self.assertRaisesRegex(ValueError, "reviewed input"):
                    self.capture()
        gate.OWNED_ENUM_INPUTS[name] = original
        del gate.OWNED_ENUM_INPUTS[name]
        with self.assertRaisesRegex(ValueError, "reviewed owned enum input membership"):
            self.capture()
        gate.OWNED_ENUM_INPUTS[name] = original
        gate.OWNED_ENUM_INPUTS["src/unlisted.rs"] = original
        with self.assertRaisesRegex(ValueError, "reviewed owned enum input membership"):
            self.capture()

    def test_parent_orchestration_and_added_modules_cannot_reuse_old_allowances(self):
        for name in (gate.PROJECT_CHANGED | gate.PROJECT_ADDED) - gate.OWNED_ENUM_CHANGED:
            with self.subTest(name=name):
                self.put(name, self.parent[name] + b"// extra edit\n")
                with self.assertRaisesRegex(ValueError, "outside owned enum boundary"):
                    self.capture()
                self.put(name, self.parent[name])

    def test_runtime_scalar_manifest_and_fixture_bytes_remain_protected(self):
        for name in ("native/runtime.c", "src/frontend/typeck.rs", "Cargo.toml", "fixture.ox"):
            with self.subTest(name=name):
                self.put(name, self.parent[name] + b"changed\n")
                with self.assertRaisesRegex(ValueError, "unchanged semantic/resource"):
                    self.capture()
                self.put(name, self.parent[name])

    def test_membership_is_exact_including_empty_directories(self):
        self.put("src/unreviewed.rs", b"// extra\n")
        with self.assertRaisesRegex(ValueError, "membership"):
            self.capture()
        (self.root / "src/unreviewed.rs").unlink()
        missing = next(iter(gate.OWNED_ENUM_ADDED))
        (self.root / missing).unlink()
        with self.assertRaisesRegex(ValueError, "membership"):
            self.capture()
        self.put(missing, b"// restored\n")
        (self.root / "src/empty").mkdir()
        with self.assertRaisesRegex(ValueError, "directory"):
            self.capture()

    def test_parent_membership_cannot_drop_a_protected_file(self):
        del self.parent["src/cli.rs"]
        with self.assertRaisesRegex(ValueError, "predecessor compiler input membership"):
            self.capture()

    def test_new_direct_and_indirect_compile_time_inputs_are_rejected(self):
        attempts = (
            b'include_str!("../../outside.txt");',
            b'include /* gap */ ! { "../../outside.rs" };',
            b'#[cfg_attr(unix, path="../../outside.rs")] mod extra;',
            b'use std::include_str as payload; const X: &str = payload!("../../outside.txt");',
            b'use std::{include_bytes as payload}; const X: &[u8] = payload!("../../outside.txt");',
            b'macro_rules! input { ($reader:ident) => { $reader!("../../outside.txt") }; } const X: &str = input!(include_str);',
            b'use std::r#include_str as payload;',
            b'macro_rules! load_module { ($key:ident) => { #[$key = "../../outside.rs"] mod extra; }; } load_module!(path);',
            '''const _: (char, char) = ('é','"');
use std::include_str as payload;
const X: &str = payload!("../../outside.txt");
const _: char = '"';'''.encode("utf-8"),
        )
        for name in ("src/frontend/ast.rs", "src/frontend/oir/owned/owned_enum_tests.rs"):
            for body in attempts:
                with self.subTest(name=name, body=body):
                    self.put(name, body)
                    with self.assertRaisesRegex(ValueError, "reviewed input"):
                        self.capture()
            self.put(name, b"// restored\n")

    def test_directive_comparison_still_rejects_a_changed_input_after_a_pin_update(self):
        name = "src/frontend/ast.rs"
        original = b'const X: &str = include_str!("fixture.ox");\n'
        self.baseline[name] = self.parent[name] = original
        body = original + b'// implementation changed\n'
        self.put(name, body)
        gate.OWNED_ENUM_INPUTS[name] = (len(body), gate.sha(body))
        self.capture()
        body = original.replace(b"fixture.ox", b"other.ox")
        self.put(name, body)
        gate.OWNED_ENUM_INPUTS[name] = (len(body), gate.sha(body))
        with self.assertRaisesRegex(ValueError, "directive changed"):
            self.capture()

    @unittest.skipIf(__import__("os").name == "nt", "Unix file identity controls")
    def test_executable_symlink_and_hardlink_members_are_rejected(self):
        path = self.root / "src/frontend/ast.rs"
        path.chmod(0o755)
        with self.assertRaisesRegex(ValueError, "executable"):
            self.capture()
        path.chmod(0o644)
        path.unlink()
        path.symlink_to(self.root / "src/cli.rs")
        with self.assertRaisesRegex(ValueError, "symlink"):
            self.capture()
        path.unlink()
        __import__("os").link(self.root / "src/cli.rs", path)
        with self.assertRaisesRegex(ValueError, "linked"):
            self.capture()


class PinnedArchiveAdmission(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        self.repo = self.root / "repo"
        self.repo.mkdir()
        gate.git(self.repo, "init", "-q")
        (self.repo / "source.rs").write_bytes(b"// original $Format:%H$\n")
        (self.repo / "policy.py").write_bytes(b"# unchanged policy\n")
        (self.repo / "nested").mkdir()
        (self.repo / "nested/data").write_bytes(b"original nested bytes\n")
        gate.git(self.repo, "add", ".")
        self.commit("original")
        self.original = gate.git(self.repo, "rev-parse", "HEAD").decode().strip()

    def commit(self, message):
        gate.git(self.repo, "-c", "user.name=Archive test", "-c", "user.email=archive@example.invalid",
                 "commit", "-qam", message)

    def test_local_replacement_cannot_redefine_a_pinned_parent(self):
        (self.repo / "source.rs").write_bytes(b"// replacement source\n")
        self.commit("replacement")
        replacement = gate.git(self.repo, "rev-parse", "HEAD").decode().strip()
        gate.git(self.repo, "replace", self.original, replacement)
        # Establish that the mutation really redirects ordinary Git reads.
        result = subprocess.check_output(["git", "-C", str(self.repo), "show", self.original + ":source.rs"])
        self.assertEqual(result, b"// replacement source\n")
        destination = self.root / "verified"
        gate.extract_baseline(self.repo, destination, revision=self.original)
        self.assertEqual((destination / "source.rs").read_bytes(), b"// original $Format:%H$\n")
        self.assertEqual((destination / "nested/data").read_bytes(), b"original nested bytes\n")

    def test_local_archive_attributes_cannot_transform_or_omit_pinned_blobs(self):
        attributes = self.repo / ".git/info/attributes"
        for attribute in ("source.rs export-subst\n", "policy.py export-ignore\n"):
            with self.subTest(attribute=attribute):
                attributes.write_text(attribute)
                with self.assertRaisesRegex(ValueError, "pinned archive (blob(?: length)?|tree membership) changed"):
                    gate.extract_baseline(self.repo, self.root / attribute.split()[1], revision=self.original)

    def test_predecessor_controller_remains_the_exact_published_file(self):
        policy = Path(__file__).with_name("verify_project_workflow_current.py")
        self.assertEqual(hashlib.sha256(policy.read_bytes()).hexdigest(), gate.PREDECESSOR_POLICY_SHA)


class MaterializedSourceAdmission(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.output = Path(temporary.name)
        self.inputs = {name: b"// captured input\n" for name in
                       gate.INSTRUMENTED | gate.OWNED_ENUM_ADDED | {"src/frontend/ast.rs"}}
        self.source = self.output / "unit2/run/source"
        rows = []
        for name, body in self.inputs.items():
            path = self.source / name
            path.parent.mkdir(parents=True, exist_ok=True)
            after = body + b"// observer instrumentation\n" if name in gate.INSTRUMENTED else body
            path.write_bytes(after)
            if name in gate.INSTRUMENTED:
                rows.append({"path": name, "before_sha256": gate.sha(body), "after_sha256": gate.sha(after)})
        self.assembly = self.output / "unit2/run/assembly.json"
        self.assembly.write_bytes(gate.encoded({"instrumentation": rows}))

    def test_rehashed_assembly_cannot_hide_changed_enum_materialization(self):
        gate.verify_materialized(self.output, self.inputs)
        name = "src/frontend/ast.rs"
        (self.source / name).write_bytes(b"// changed semantics\n")
        value = json.loads(self.assembly.read_bytes())
        value["files"] = [{"path": name, "sha256": gate.sha(b"// changed semantics\n")}]
        self.assembly.write_bytes(gate.encoded(value))
        with self.assertRaisesRegex(ValueError, "materialized current input changed"):
            gate.verify_materialized(self.output, self.inputs)

    def test_instrumentation_must_bind_both_captured_and_emitted_bytes(self):
        original = self.assembly.read_bytes()
        value = json.loads(original)
        value["instrumentation"][0]["before_sha256"] = "0" * 64
        self.assembly.write_bytes(gate.encoded(value))
        with self.assertRaisesRegex(ValueError, "wrong instrumentation input"):
            gate.verify_materialized(self.output, self.inputs)
        self.assembly.write_bytes(original)
        name = json.loads(original)["instrumentation"][0]["path"]
        (self.source / name).write_bytes(b"// changed observer\n")
        with self.assertRaisesRegex(ValueError, "changed instrumentation output"):
            gate.verify_materialized(self.output, self.inputs)


if __name__ == "__main__":
    unittest.main()
