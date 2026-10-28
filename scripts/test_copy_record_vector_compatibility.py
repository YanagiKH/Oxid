"""Source, protected-activation and immutable-replay controls; no compiler runs."""
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

import record_vector_dispatch_activation as activation
import verify_copy_record_vector_compatibility as gate

ROOT = Path(__file__).resolve().parents[1]


def git_body(commit, name):
    return subprocess.check_output(["git", "show", commit + ":" + name], cwd=ROOT)


def baseline_names():
    body = (ROOT / gate.BINDING / "current-source.json").read_bytes()
    if gate.sha(body) != gate.BASELINE_SOURCE_SHA:
        raise AssertionError("historical source manifest changed")
    return {row["path"] for row in json.loads(body)["files"]}


class RecordVectorAdmission(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory(prefix="oxid-record-vector-policy-")
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        self.baseline = {name: b"// fixed historical input\n" for name in baseline_names()}
        self.parent = {**self.baseline,
                       **{name: b"// fixed R1 addition\n" for name in gate.PREDECESSOR_ADDED}}
        self.parent[activation.PATH] = git_body(gate.PREDECESSOR, activation.PATH)
        self.current = dict(self.parent)
        self.current.update({name: b"// reviewed record-vector input\n"
                             for name in gate.RECORD_VECTOR_CHANGED | gate.RECORD_VECTOR_ADDED})
        self.changed = next(name for name in sorted(gate.RECORD_VECTOR_CHANGED) if name.endswith(".rs"))
        self.current[self.changed] = b"#[test]\nfn first(){}\n#[test]\nfn second(){}\n"
        for name, body in self.current.items():
            self.put(name, body)
        self.put(gate.DISPATCH_ADAPTER, (ROOT / gate.DISPATCH_ADAPTER).read_bytes())
        for name, value in (
            ("WINDOWS_ACTIVATION_ENABLED", False),
            ("PROTECTED_CURRENT_PIN", activation.PREDECESSOR_PIN),
            ("REVIEWED_INPUTS", {name: (len(self.current[name]), gate.sha(self.current[name]))
                                 for name in gate.RECORD_VECTOR_CHANGED | gate.RECORD_VECTOR_ADDED}),
            ("REVIEWED_DIRECTIVES", {name: gate.source_directives(name, self.current[name])
                                     for name in gate.RECORD_VECTOR_CHANGED | gate.RECORD_VECTOR_ADDED}),
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

    def test_exact_R1_roster_and_only_declared_additions_are_admitted(self):
        inputs = self.capture()
        self.assertEqual(len(self.parent), 499)
        self.assertEqual(set(inputs), set(self.parent) | gate.RECORD_VECTOR_ADDED)
        self.assertEqual(len(inputs), 499 + len(gate.RECORD_VECTOR_ADDED))
        for name in set(self.parent) - gate.RECORD_VECTOR_CHANGED:
            self.assertEqual(inputs[name], self.parent[name])

    def test_actual_manifest_keeps_all_new_and_old_semantic_claims_false(self):
        inputs = self.capture()
        manifest = gate.current_manifest(inputs, self.baseline, {"source_kind": "worktree"}, self.parent, self.root)
        self.assertEqual(manifest["source_policy_input_members"], 50)
        self.assertEqual(manifest["predecessor_compiler_input_members"], 499)
        self.assertEqual({row["path"] for row in manifest["files"]}, set(self.current))
        self.assertEqual(manifest["record_vector_pins_provisional"], gate.PINS_PROVISIONAL)
        for key, value in manifest.items():
            if key.endswith("qualified_by_this_gate"):
                self.assertIs(value, False, key)
        self.assertIs(manifest["protected_dispatch_activation"]["activation_evolved"], False)

    def test_source_hash_and_length_are_both_required(self):
        self.put(self.changed, self.current[self.changed] + b"// changed\n")
        with self.assertRaisesRegex(ValueError, "reviewed input"):
            self.capture()
        self.put(self.changed, self.current[self.changed])
        length, digest = gate.REVIEWED_INPUTS[self.changed]
        gate.REVIEWED_INPUTS[self.changed] = (length + 1, digest)
        with self.assertRaisesRegex(ValueError, "reviewed input"):
            self.capture()

    def test_complete_ordered_repeated_directives_cannot_be_weakened(self):
        original = gate.REVIEWED_DIRECTIVES[self.changed]
        self.assertEqual(original["attributes"], ((b"#[test]", 2),))
        for attributes in (((b"#[test]", 1),), ((b"#[test]", 3),),
                           ((b"#[ignore]", 1), (b"#[test]", 1)),
                           ((b"#[test]", 1), (b"#[ignore]", 1))):
            gate.REVIEWED_DIRECTIVES[self.changed] = {**original, "attributes": attributes}
            with self.subTest(attributes=attributes), self.assertRaisesRegex(ValueError, "directive changed"):
                self.capture()
        gate.REVIEWED_DIRECTIVES[self.changed] = original
        for field in ("macros", "c_preprocessor"):
            gate.REVIEWED_DIRECTIVES[self.changed] = {**original, field: ((b"unreviewed", 1),)}
            with self.subTest(field=field), self.assertRaisesRegex(ValueError, "directive changed"):
                self.capture()
        gate.REVIEWED_DIRECTIVES[self.changed] = original

    def test_reviewed_directive_membership_must_be_exact(self):
        del gate.REVIEWED_DIRECTIVES[self.changed]
        with self.assertRaisesRegex(ValueError, "directive membership"):
            self.capture()

    def test_unchanged_source_stays_closed(self):
        name = next(name for name in sorted(self.parent)
                    if name not in gate.RECORD_VECTOR_CHANGED and name != activation.PATH)
        self.put(name, self.parent[name] + b"// changed\n")
        with self.assertRaisesRegex(ValueError, "outside record-vector boundary"):
            self.capture()

    def test_extra_and_missing_compiler_members_are_refused(self):
        extra = self.root / "src/unreviewed.rs"
        self.put("src/unreviewed.rs", b"// extra\n")
        with self.assertRaisesRegex(ValueError, "membership"):
            self.capture()
        extra.unlink()
        (self.root / sorted(gate.RECORD_VECTOR_ADDED)[0]).unlink()
        with self.assertRaisesRegex(ValueError, "membership"):
            self.capture()

    def test_empty_unlisted_directory_is_refused(self):
        (self.root / "src/unreviewed").mkdir()
        with self.assertRaisesRegex(ValueError, "directory"):
            self.capture()

    def test_reviewed_roster_and_parent_membership_cannot_drift(self):
        saved = gate.REVIEWED_INPUTS.pop(self.changed)
        with self.assertRaisesRegex(ValueError, "reviewed record-vector input membership"):
            self.capture()
        gate.REVIEWED_INPUTS[self.changed] = saved
        del self.parent[sorted(gate.PREDECESSOR_ADDED)[0]]
        with self.assertRaisesRegex(ValueError, "R1 compiler input membership"):
            self.capture()

    def test_protected_member_cannot_enter_generic_delta(self):
        with patch.object(gate, "RECORD_VECTOR_CHANGED", gate.RECORD_VECTOR_CHANGED | {activation.PATH}):
            with self.assertRaisesRegex(ValueError, "protected input cannot enter"):
                self.capture()

    def test_protected_body_mutation_is_rejected_even_with_a_new_current_pin(self):
        current = self.parent[activation.PATH].replace(b"fn child_discovery", b"fn changed_child_discovery", 1)
        self.put(activation.PATH, current)
        with patch.object(gate, "PROTECTED_CURRENT_PIN", activation.pin(current)):
            with self.assertRaisesRegex(ValueError, "evolution is not admitted"):
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

    def test_changed_dispatch_adapter_is_rejected_before_source_admission(self):
        self.put(gate.DISPATCH_ADAPTER, (ROOT / gate.DISPATCH_ADAPTER).read_bytes() + b"# changed\n")
        with self.assertRaisesRegex(ValueError, "pinned successor adapter"):
            self.capture()

    def test_new_activation_keeps_actual_bytes_in_manifest_and_capture(self):
        before = self.parent[activation.PATH]
        current = before.replace(activation.BEFORE, activation.AFTER, 1)
        self.put(activation.PATH, current)
        with patch.object(gate, "WINDOWS_ACTIVATION_ENABLED", True), patch.object(gate, "PROTECTED_CURRENT_PIN", activation.pin(current)):
            inputs = self.capture()
            manifest = gate.current_manifest(inputs, self.baseline, {"source_kind": "worktree"}, self.parent, self.root)
        self.assertEqual(inputs[activation.PATH], current)
        self.assertEqual((self.root / activation.PATH).read_bytes(), current)
        row = next(row for row in manifest["files"] if row["path"] == activation.PATH)
        self.assertEqual((row["bytes"], row["sha256"]), activation.pin(current))
        self.assertNotEqual(row["sha256"], activation.PREDECESSOR_PIN[1])
        proof = manifest["protected_dispatch_activation"]
        self.assertTrue(proof["activation_evolved"])
        self.assertTrue(proof["whole_file_inverse_equal"])
        self.assertEqual(proof["compile_time_directive_delta_count"], 1)
        self.assertIs(proof["windows_activation_qualified_by_this_gate"], False)


class ProtectedActivation(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.before = git_body(gate.PREDECESSOR, activation.PATH)
        cls.current = cls.before.replace(activation.BEFORE, activation.AFTER, 1)

    def authenticate(self, body):
        return activation.authenticate(self.before, body, activation.pin(body), True)

    def test_only_qualified_windows_host_changes_activation(self):
        self.assertEqual(activation.activation_difference(), [{
            "os": "windows", "arch": "x86_64", "pointer_width": "64", "endian": "little",
            "before": True, "after": False,
        }])
        proof = self.authenticate(self.current)
        self.assertEqual(proof["predicate_equivalence_classes"], 48)
        self.assertEqual(self.current.replace(activation.AFTER, activation.BEFORE, 1), self.before)

    def test_wrong_arch_width_endian_or_repeated_predicate_is_refused(self):
        for body in (self.current.replace(b'"x86_64"', b'"aarch64"'),
                     self.current.replace(b'"64"', b'"32"'),
                     self.current.replace(b'"little"', b'"big"'),
                     self.current.replace(activation.AFTER, activation.AFTER * 2, 1), self.before):
            with self.subTest(body=gate.sha(body)), self.assertRaisesRegex(ValueError, "activation seam"):
                self.authenticate(body)

    def test_test_body_name_ignore_and_surrounding_bytes_cannot_change(self):
        for body in (self.current + b"\n", self.current.replace(b"#[test]", b"#[test]\n#[ignore]", 1),
                     self.current.replace(b"fn child_discovery", b"fn renamed_child_discovery", 1),
                     self.current.replace(b"assert_eq!", b"assert_ne!", 1)):
            with self.subTest(body=gate.sha(body)), self.assertRaisesRegex(ValueError, "inverse changes other bytes"):
                self.authenticate(body)

    def test_wrong_predecessor_current_identity_and_disabled_evolution_are_refused(self):
        with self.assertRaisesRegex(ValueError, "predecessor"):
            activation.authenticate(self.before + b"\n", self.current, activation.pin(self.current), True)
        with self.assertRaisesRegex(ValueError, "current input"):
            activation.authenticate(self.before, self.current, activation.PREDECESSOR_PIN, True)
        with self.assertRaisesRegex(ValueError, "not admitted"):
            activation.authenticate(self.before, self.current, activation.pin(self.current), False)


class FrozenReplay(unittest.TestCase):
    def test_all_fifty_predecessor_policies_and_resource_adapters_are_exact(self):
        self.assertEqual(len(gate.PREDECESSOR_POLICIES), 50)
        for name, expected in gate.PREDECESSOR_POLICIES.items():
            body = git_body(gate.PREDECESSOR, name)
            self.assertEqual(gate.sha(body), expected, name)
            self.assertEqual(gate.regular(ROOT, name), body, name)
        for name in (gate.INDEX_RESOURCE_ADAPTER, gate.PARSER_RESOURCE_ADAPTER,
                     "scripts/typed_vector_compatibility_observer.py"):
            self.assertEqual(gate.regular(ROOT, name), git_body(gate.PREDECESSOR, name))

    def test_policy_authentication_rejects_current_mutation(self):
        with tempfile.TemporaryDirectory() as directory:
            parent, current = Path(directory) / "parent", Path(directory) / "current"
            for name in gate.PREDECESSOR_POLICIES:
                for root in (parent, current):
                    (root / name).parent.mkdir(parents=True, exist_ok=True)
                    (root / name).write_bytes(git_body(gate.PREDECESSOR, name))
            gate.authenticate_policies(current, parent)
            name = sorted(gate.PREDECESSOR_POLICIES)[0]
            with (current / name).open("ab") as file:
                file.write(b"# changed\n")
            with self.assertRaisesRegex(ValueError, "policy changed"):
                gate.authenticate_policies(current, parent)

    def test_parent_blob_mode_and_identity_are_authenticated(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            body = b"// parent source\n"
            (root / "source.rs").write_bytes(body)
            oid = hashlib.sha1(b"blob " + str(len(body)).encode() + b"\0" + body, usedforsecurity=False).hexdigest()
            row = f"100644 blob {oid} {len(body)}\tsource.rs\0".encode()
            with patch.object(gate, "git", return_value=row):
                self.assertEqual(gate.capture_parent(ROOT, root, {"source.rs"}), {"source.rs": body})
            for changed in (row.replace(b"100644", b"100755"), row.replace(oid.encode(), b"0" * 40)):
                with patch.object(gate, "git", return_value=changed), self.assertRaisesRegex(ValueError, "pinned Git blob"):
                    gate.capture_parent(ROOT, root, {"source.rs"})

    def test_complete_old_replay_block_and_every_selector_are_unchanged(self):
        def replay(path):
            tree = ast.parse(path.read_bytes())
            run = next(node for node in tree.body if isinstance(node, ast.FunctionDef) and node.name == "run")
            return [
                node for node in ast.walk(run) if isinstance(node, ast.If)
                and ast.unparse(node.test) == "not args.prepare_only"]
        current = replay(ROOT / "scripts/verify_copy_record_vector_compatibility.py")
        predecessor = replay(ROOT / gate.PREDECESSOR_POLICY)
        self.assertEqual(len(predecessor), 1)
        self.assertEqual(len(current), 2)
        self.assertEqual(ast.dump(current[0], include_attributes=False), ast.dump(predecessor[0], include_attributes=False))
        self.assertGreater(len(current[0].body), 10)
        self.assertIn("run_record_vector_observer_controls", ast.unparse(current[1]))

    def test_resource_preparation_delegates_to_actual_R1_without_formula_changes(self):
        self.assertIn("return library.prepare_current_observer", inspect.getsource(gate.prepare_current_observer))
        with patch.object(gate.library, "prepare_current_observer", return_value=("selected", "seam")) as prepare:
            self.assertEqual(gate.prepare_current_observer(1, 2, 3, 4, 5, 6), ("selected", "seam"))
        prepare.assert_called_once_with(1, 2, 3, 4, 5, 6)
        for name in ("src/frontend/ast.rs", "src/frontend/declaration_index/resource.rs"):
            self.assertEqual((ROOT / name).read_bytes(), git_body(gate.PREDECESSOR, name), name)

    def test_provisional_pin_guard_refuses_qualification_before_any_process(self):
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


if __name__ == "__main__":
    unittest.main()
