"""Admission mutations for the R1 current-source gate; no historical pins change."""
import copy
import json
from pathlib import Path
import tempfile
import unittest

import verify_project_workflow_current as gate


class CurrentSourceAdmission(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        self.before = {name: b"// accepted orchestration\n" for name in gate.CHANGED}
        self.before.update({"src/frontend/typeck.rs": b"// semantic authority\n",
                            "src/frontend/declaration_index/resource.rs": b"// resource rules\n",
                            "native/runtime.c": b"/* runtime */\n",
                            "Cargo.toml": b'[package]\nname="example"\n',
                            "fixture.ox": b"fn main()->i32{return 0;}\n"})
        for name, body in self.before.items():
            self.put(name, body)
        self.put("src/project_cli.rs", b"// R1 orchestration\n")

    def put(self, name, body):
        path = self.root / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(body)

    def capture(self):
        return gate.capture_sources(self.root, self.before)

    def test_all_named_r1_changes_and_complete_addition_are_recorded(self):
        for name in gate.CHANGED:
            self.put(name, b"// actual new behavior\n")
        inputs = self.capture()
        manifest = gate.current_manifest(inputs, self.before, {"source_kind": "worktree"})
        self.assertEqual({r["path"] for r in manifest["changes"]}, gate.CHANGED)
        self.assertEqual({r["path"] for r in manifest["files"]}, set(self.before) | gate.ADDED)
        self.assertEqual(manifest["unchanged_baseline_inputs"], len(self.before) - len(gate.CHANGED))
        self.assertEqual(inputs["src/frontend/typeck.rs"], self.before["src/frontend/typeck.rs"])
        self.assertNotIn("reviewed_source_head", manifest)

    def test_semantic_resource_runtime_manifest_and_fixture_changes_are_rejected(self):
        for name in set(self.before) - gate.CHANGED:
            with self.subTest(name=name):
                self.put(name, self.before[name] + b"changed")
                with self.assertRaisesRegex(ValueError, "unchanged semantic/resource input changed"):
                    self.capture()
                self.put(name, self.before[name])

    def test_added_removed_and_empty_directory_members_are_rejected(self):
        self.put("src/unlisted.rs", b"// unlisted\n")
        with self.assertRaisesRegex(ValueError, "membership"):
            self.capture()
        (self.root / "src/unlisted.rs").unlink()
        (self.root / "src/project_cli.rs").unlink()
        with self.assertRaisesRegex(ValueError, "membership"):
            self.capture()
        self.put("src/project_cli.rs", b"// R1\n")
        (self.root / "src/unused").mkdir()
        with self.assertRaisesRegex(ValueError, "directory"):
            self.capture()

    def test_new_compile_time_input_does_not_escape_the_source_roster(self):
        for directive in (b'include!("../../untracked.rs");',
                          b'include_str!("../untracked.txt");',
                          b'include_bytes!\n(\n"../untracked.txt"\n);',
                          b'include! { "../../untracked.rs" }',
                          b'include! [ "../../untracked.rs" ];',
                          b'include /* whitespace */ ! ("../untracked.rs");',
                          b'include!(concat!(env!("OUT_DIR"), "/generated.rs"));',
                          b'include!("../unclosed.rs"',
                          b'#[path = "../../untracked.rs"] mod extra;',
                          b'#[cfg_attr(unix, path = "../../untracked.rs")] mod extra;',
                          b'# /* whitespace */ [path="../untracked.rs"] mod extra;'):
            self.put("src/project_cli.rs", directive)
            with self.subTest(directive=directive), self.assertRaisesRegex(ValueError, "compile-time"):
                self.capture()

    def test_existing_include_expressions_are_preserved(self):
        name = "src/cli.rs"
        self.before[name] = b'include!("main.rs");\n'
        self.put(name, b'// new entry point\ninclude!("main.rs");\n')
        self.capture()
        self.put(name, b'include!("elsewhere.rs");\n')
        with self.assertRaisesRegex(ValueError, "compile-time"):
            self.capture()

    @unittest.skipIf(__import__("os").name == "nt", "Unix mode and symlink controls")
    def test_symlink_and_executable_inputs_are_rejected(self):
        path = self.root / "src/project_cli.rs"
        path.chmod(0o755)
        with self.assertRaisesRegex(ValueError, "executable"):
            self.capture()
        path.unlink()
        path.symlink_to(self.root / "src/cli.rs")
        with self.assertRaisesRegex(ValueError, "symlink"):
            self.capture()

    def test_repeated_capture_detects_orchestration_edits(self):
        first = self.capture()
        self.put("src/main.rs", b"// subsequent edit\n")
        second = self.capture()
        self.assertNotEqual(first, second)
        self.assertNotEqual(gate.encoded(gate.current_manifest(first, self.before, {})),
                            gate.encoded(gate.current_manifest(second, self.before, {})))


class CheckoutAdmission(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        gate.git(self.root, "init", "-q")
        (self.root / "source").write_text("original\n")
        gate.git(self.root, "add", "source")
        gate.git(self.root, "-c", "user.name=Gate test", "-c", "user.email=gate@example.invalid",
                 "commit", "-qm", "test fixture")
        self.head = gate.git(self.root, "rev-parse", "HEAD").decode().strip()

    def test_committed_head_is_exact_and_dirty_worktree_is_explicit(self):
        self.assertEqual(gate.checkout_identity(self.root, self.head, False)["source_kind"], "committed")
        with self.assertRaisesRegex(ValueError, "expected head"):
            gate.checkout_identity(self.root, "0" * 40, False)
        (self.root / "untracked").write_text("new source\n")
        with self.assertRaisesRegex(ValueError, "clean checkout"):
            gate.checkout_identity(self.root, self.head, False)
        self.assertEqual(gate.checkout_identity(self.root, self.head, True)["source_kind"], "worktree")


class MaterializedSourceAdmission(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.output = Path(temporary.name)
        self.inputs = {n: b"original\n" for n in gate.INSTRUMENTED | gate.ADDED}
        changes = []
        for name, body in self.inputs.items():
            path = self.output / "unit2/run/source" / name
            path.parent.mkdir(parents=True, exist_ok=True)
            after = body + b"instrumentation\n" if name in gate.INSTRUMENTED else body
            path.write_bytes(after)
            if name in gate.INSTRUMENTED:
                changes.append({"path": name, "before_sha256": gate.sha(body), "after_sha256": gate.sha(after)})
        self.assembly = self.output / "unit2/run/assembly.json"
        self.assembly.write_bytes(gate.encoded({"instrumentation": changes}))

    def test_current_source_tamper_cannot_be_hidden_by_an_assembly_rehash(self):
        gate.verify_materialized(self.output, self.inputs)
        (self.output / "unit2/run/source/src/project_cli.rs").write_bytes(b"tampered\n")
        value = json.loads(self.assembly.read_bytes())
        value["files"] = [{"path": "src/project_cli.rs", "sha256": gate.sha(b"tampered\n")}]
        self.assembly.write_bytes(gate.encoded(value))
        with self.assertRaisesRegex(ValueError, "materialized current input changed"):
            gate.verify_materialized(self.output, self.inputs)

    def test_missing_or_extra_instrumentation_and_wrong_source_binding_fail(self):
        value = json.loads(self.assembly.read_bytes())
        for changed in ({"instrumentation": value["instrumentation"][:-1]},
                        {"instrumentation": value["instrumentation"] + [value["instrumentation"][0]]}):
            self.assembly.write_bytes(gate.encoded(changed))
            with self.assertRaisesRegex(ValueError, "scope"):
                gate.verify_materialized(self.output, self.inputs)
        changed = copy.deepcopy(value)
        changed["instrumentation"][0]["before_sha256"] = "0" * 64
        self.assembly.write_bytes(gate.encoded(changed))
        with self.assertRaisesRegex(ValueError, "wrong instrumentation input"):
            gate.verify_materialized(self.output, self.inputs)


if __name__ == "__main__":
    unittest.main()
