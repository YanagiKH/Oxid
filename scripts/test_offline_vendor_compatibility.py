"""Closed offline-export input admission and unchanged replay controls."""
import argparse
import ast
from contextlib import redirect_stderr
import inspect
import io
import json
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch

import replay_owned_text_source_policy as replay
import verify_offline_vendor_compatibility as gate


ROOT = Path(__file__).resolve().parents[1]


def baseline_names():
    body = gate.regular(ROOT, gate.BINDING + "/current-source.json")
    if gate.sha(body) != gate.BASELINE_SOURCE_SHA:
        raise AssertionError("historical source manifest changed")
    return {row["path"] for row in json.loads(body)["files"]}


class VendorSourceAdmission(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory(prefix="oxid-vendor-source-policy-")
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        self.baseline = {name: b"// historical input\n" for name in baseline_names()}
        self.parent = {**self.baseline, **{name: b"// owned-text input\n" for name in gate.PREDECESSOR_ADDED}}
        self.parent[gate.CLI] = (gate.MODULE_BEFORE + b"fn main() {\n" + gate.DISPATCH_BEFORE
                                 + b"        keep_original();\n    }\n}\n")
        self.parent[gate.MAIN] = b"fn help() {\n" + gate.HELP_BEFORE + b"}\n"
        self.parent[gate.PACKAGE] = b"// original package\n" + gate.PACKAGE_FOLLOWING + b"    Create,\n}\n"
        for name in (gate.TEXT_RESOURCE_TEST, gate.VECTOR_ROUTE_TEST):
            self.parent[name] = gate.git(ROOT, "show", gate.PREDECESSOR + ":" + name)
        package = gate.regular(ROOT, gate.PACKAGE)
        start = package.index(gate.PACKAGE_INSERT_START)
        end = package.index(gate.PACKAGE_INSERT_END, start) + len(gate.PACKAGE_INSERT_END)
        self.insertion = package[start:end]
        self.current = {**self.parent,
            gate.CLI: gate.approved_cli(self.parent[gate.CLI]),
            gate.MAIN: gate.approved_main(self.parent[gate.MAIN]),
            gate.PACKAGE: self.parent[gate.PACKAGE].replace(gate.PACKAGE_FOLLOWING,
                                                         self.insertion + gate.PACKAGE_FOLLOWING),
            gate.TEXT_RESOURCE_TEST: gate.approved_text_resource_test(self.parent[gate.TEXT_RESOURCE_TEST]),
            gate.VECTOR_ROUTE_TEST: gate.approved_vector_route_test(self.parent[gate.VECTOR_ROUTE_TEST]),
            gate.COMMAND: gate.regular(ROOT, gate.COMMAND), gate.TEST: gate.regular(ROOT, gate.TEST)}
        self.pins = {name: (len(self.current[name]), gate.sha(self.current[name]))
                     for name in gate.VENDOR_CHANGED | gate.VENDOR_ADDED}
        for name, body in self.current.items():
            self.put(name, body)
        selected = patch.object(gate, "REVIEWED_INPUTS", self.pins)
        selected.start()
        self.addCleanup(selected.stop)

    def put(self, name, body):
        path = self.root / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(body)

    def repin(self, name, body):
        self.put(name, body)
        self.pins[name] = (len(body), gate.sha(body))

    def capture(self):
        return gate.capture_sources(self.root, self.baseline, self.parent)

    def test_all_574_parent_inputs_and_exactly_two_modules_are_retained(self):
        inputs = self.capture()
        self.assertEqual((len(self.baseline), len(self.parent), len(inputs)), (376, 574, 576))
        self.assertEqual(set(inputs), set(self.parent) | {gate.COMMAND, gate.TEST})
        self.assertTrue(all(inputs[name] == before for name, before in self.parent.items()
                            if name not in {gate.CLI, gate.MAIN, gate.PACKAGE,
                                            gate.TEXT_RESOURCE_TEST, gate.VECTOR_ROUTE_TEST}))
        self.assertEqual(len(gate.REVIEWED_INPUTS), 7)
        ledger = b"    let expected = byte_buffer::Ledger {\n"
        self.assertEqual(inputs[gate.TEXT_RESOURCE_TEST].split(ledger, 1)[1],
                         self.parent[gate.TEXT_RESOURCE_TEST].split(ledger, 1)[1] + gate.TEXT_PACKAGE_REFUSAL)
        restored_route = inputs[gate.VECTOR_ROUTE_TEST].replace(
            gate.PACKAGE_HOSTS + b"\n" + gate.VECTOR_PACKAGE_POSITIVE, gate.VECTOR_PACKAGE_POSITIVE, 1).replace(
                gate.VECTOR_PACKAGE_REFUSAL, b"", 1)
        self.assertEqual(restored_route, self.parent[gate.VECTOR_ROUTE_TEST])

    def test_protected_existing_compiler_runtime_and_text_sources_cannot_drift(self):
        for name in ("src/frontend/oir/owned/execute.rs", "native/experimental_bytebuf.c",
                     gate.predecessor.PROGRAM, gate.predecessor.ASSET,
                     "src/frontend/oir/owned/source/owned_vector_source_resource_tests.rs"):
            with self.subTest(name=name):
                self.put(name, self.current[name] + b"// drift\n")
                with self.assertRaisesRegex(ValueError, "unreviewed change"):
                    self.capture()
                self.put(name, self.current[name])

        for name in (gate.TEXT_RESOURCE_TEST, gate.VECTOR_ROUTE_TEST):
            with self.subTest(platform_test=name):
                changed = self.current[name] + b"fn unreviewed_test_body() {}\n"
                self.put(name, changed)
                with self.assertRaisesRegex(ValueError, "platform regression"):
                    self.capture()
                self.repin(name, changed)
                with self.assertRaisesRegex(ValueError, "platform regression"):
                    self.capture()
                self.repin(name, self.current[name])
        for name, old, new in (
            (gate.TEXT_RESOURCE_TEST, b"peak_bytes: peak,", b"peak_bytes: 0,"),
            (gate.TEXT_RESOURCE_TEST, b'assert!(observed.fault_applied, "{policy:?}");',
             b'assert!(true, "{policy:?}");'),
            (gate.TEXT_RESOURCE_TEST, b"exercise(APPEND_TEXT, 2, 3, 128);", b"exercise(APPEND_TEXT, 2, 0, 0);"),
            (gate.TEXT_RESOURCE_TEST,
             b"fn owned_text_source_resource_reserve_failure_preserves_owner_and_retry_succeeds()",
             b"fn renamed_old_fault_entry()"),
            (gate.TEXT_RESOURCE_TEST, b"module.canonical_path.as_ref() == Some(&text_path)",
             b'module.relative_path == "text.ox"'),
            (gate.TEXT_RESOURCE_TEST, b"assert!(matching_modules.next().is_none());",
             b"let _ = matching_modules.next();"),
            (gate.TEXT_RESOURCE_TEST, b"let library = project.sources().get(module.file);",
             b"let library = &project.sources().files()[0];"),
            (gate.VECTOR_ROUTE_TEST, b"assert_eq!(program.run_test_i32().unwrap(), 58);",
             b"assert_eq!(program.run_test_i32().unwrap(), 0);"),
        ):
            with self.subTest(platform_assertion=old):
                changed = self.current[name].replace(old, new, 1)
                self.assertNotEqual(changed, self.current[name])
                self.repin(name, changed)
                with self.assertRaisesRegex(ValueError, "platform regression"):
                    self.capture()
                self.repin(name, self.current[name])

    def test_updated_cli_and_main_pins_cannot_authorize_any_other_behavior(self):
        for name, extra in ((gate.CLI, b"fn bypass() {}\n"), (gate.MAIN, b"fn unrelated() {}\n")):
            with self.subTest(name=name):
                self.repin(name, self.current[name] + extra)
                with self.assertRaisesRegex(ValueError, "exceeds exact"):
                    self.capture()
                self.repin(name, self.current[name])
        for before, after in ((gate.DISPATCH_INSERT, b""), (gate.MODULE_AFTER, gate.MODULE_BEFORE),
                              (gate.DISPATCH_BEFORE, b"    if let Some(code) = doctor_cli::dispatch(&args) {\n")):
            self.repin(gate.CLI, self.current[gate.CLI].replace(before, after))
            with self.assertRaises(ValueError):
                self.capture()
            self.repin(gate.CLI, self.current[gate.CLI])

    def test_updated_package_pin_cannot_modify_inherited_collection_or_publication(self):
        for changed in (self.current[gate.PACKAGE] + b"fn old_package_replacement() {}\n",
                        self.current[gate.PACKAGE].replace(b"    Create,", b"    Bypassed,"),
                        self.current[gate.PACKAGE].replace(gate.PACKAGE_INSERT_END,
                                                          gate.PACKAGE_INSERT_END + b"fn hidden() {}\n")):
            self.repin(gate.PACKAGE, changed)
            with self.assertRaisesRegex(ValueError, "package"):
                self.capture()

    def test_new_configuration_include_or_environment_inputs_cannot_be_repin_admitted(self):
        for name, extra in ((gate.COMMAND, b"#[cfg(unreviewed)]\nfn hidden() {}\n"),
                            (gate.TEST, b'const HIDDEN:&str=include_str!("outside.ox");\n'),
                            (gate.PACKAGE, b'const ENV:&str=env!("UNREVIEWED");\n'),
                            (gate.TEXT_RESOURCE_TEST, b"#[cfg(unreviewed)]\nfn hidden() {}\n"),
                            (gate.TEXT_RESOURCE_TEST, b'const HIDDEN:&str=include_str!("outside.ox");\n'),
                            (gate.VECTOR_ROUTE_TEST, b"#[cfg(unreviewed)]\nfn hidden() {}\n"),
                            (gate.VECTOR_ROUTE_TEST, b'const HIDDEN:&str=include_str!("outside.ox");\n')):
            with self.subTest(name=name):
                body = (self.current[name].replace(gate.PACKAGE_INSERT_END, extra + gate.PACKAGE_INSERT_END)
                        if name == gate.PACKAGE else self.current[name] + extra)
                self.repin(name, body)
                with self.assertRaisesRegex(ValueError, "directive"):
                    self.capture()
                self.repin(name, self.current[name])

        changed = self.current[gate.PACKAGE].replace(b'target_os = "linux"', b'target_os = "unreviewed"')
        self.assertNotEqual(changed, self.current[gate.PACKAGE])
        self.repin(gate.PACKAGE, changed)
        with self.assertRaisesRegex(ValueError, "directive"):
            self.capture()
        self.repin(gate.PACKAGE, self.current[gate.PACKAGE])

    def test_missing_input_extra_source_empty_directory_and_symlink_are_denied(self):
        for name in ("src/frontend/oir/owned/execute.rs", gate.COMMAND, gate.TEST, gate.predecessor.ASSET,
                     gate.TEXT_RESOURCE_TEST, gate.VECTOR_ROUTE_TEST):
            with self.subTest(missing=name):
                (self.root / name).unlink()
                with self.assertRaises(ValueError):
                    self.capture()
                self.put(name, self.current[name])
        extra = self.root / "src/unreviewed.rs"
        extra.write_bytes(b"fn unreviewed() {}\n")
        with self.assertRaisesRegex(ValueError, "membership"):
            self.capture()
        extra.unlink()
        empty = self.root / "src/unreviewed"
        empty.mkdir()
        with self.assertRaisesRegex(ValueError, "directory"):
            self.capture()
        empty.rmdir()
        try:
            extra.symlink_to(self.root / gate.predecessor.ASSET)
        except (OSError, NotImplementedError) as error:
            self.skipTest(f"symlinks unavailable: {error}")
        with self.assertRaisesRegex(ValueError, "symlink"):
            self.capture()

    def test_pin_membership_byte_identity_and_changed_added_partition_are_exact(self):
        for pins in ({gate.CLI: self.pins[gate.CLI]}, {**self.pins, "src/unreviewed.rs": (0, gate.sha(b""))}):
            with patch.object(gate, "REVIEWED_INPUTS", pins):
                with self.assertRaisesRegex(ValueError, "membership"):
                    self.capture()
        with patch.object(gate, "VENDOR_ADDED", gate.VENDOR_ADDED | {gate.predecessor.ASSET}):
            with self.assertRaisesRegex(ValueError, "membership"):
                self.capture()
        for changed in (gate.VENDOR_CHANGED - {gate.TEXT_RESOURCE_TEST},
                        gate.VENDOR_CHANGED | {gate.predecessor.PROGRAM}):
            with patch.object(gate, "VENDOR_CHANGED", changed):
                with self.assertRaisesRegex(ValueError, "membership"):
                    self.capture()
        for directives in ({gate.TEXT_RESOURCE_TEST: gate.PLATFORM_TEST_DIRECTIVES[gate.TEXT_RESOURCE_TEST]},
                           {**gate.PLATFORM_TEST_DIRECTIVES, gate.CLI: gate.EMPTY_DIRECTIVES}):
            with patch.object(gate, "PLATFORM_TEST_DIRECTIVES", directives):
                with self.assertRaisesRegex(ValueError, "membership"):
                    self.capture()
        self.put(gate.COMMAND, self.current[gate.COMMAND] + b"fn extra() {}\n")
        with self.assertRaisesRegex(ValueError, "reviewed input"):
            self.capture()

    def test_manifest_keeps_all_inputs_and_false_new_feature_claims(self):
        inputs = self.capture()
        inherited = {"inherited_semantics_qualified_by_this_gate": False}
        with patch.object(gate.predecessor, "current_manifest", return_value=inherited) as previous:
            manifest = gate.current_manifest(inputs, self.baseline, {"head": "test"}, self.parent)
        self.assertEqual(len(previous.call_args.kwargs["inputs"]), 574)
        self.assertEqual(len(previous.call_args.kwargs["parent"]), 572)
        self.assertEqual(len(manifest["files"]), 576)
        self.assertEqual(manifest["source_policy_parent"], gate.PREDECESSOR)
        self.assertEqual(manifest["offline_vendor_pins_provisional"], gate.PINS_PROVISIONAL)
        self.assertTrue(all(value is False for name, value in manifest.items() if name.endswith("qualified_by_this_gate")))


class RetainedVendorDelegation(unittest.TestCase):
    def test_exact_reviewed_input_pins_and_original_parent_policy(self):
        self.assertEqual(gate.REVIEWED_SOURCE_HEAD, "f3c02495c5d8aa4f155808263c0734f63e2f790c")
        for name, expected in gate.REVIEWED_INPUTS.items():
            body = gate.regular(ROOT, name)
            self.assertEqual((len(body), gate.sha(body)), expected)
        body = gate.regular(ROOT, gate.PREDECESSOR_POLICY)
        self.assertEqual(body, gate.git(ROOT, "show", gate.PREDECESSOR + ":" + gate.PREDECESSOR_POLICY))
        self.assertEqual((len(body), gate.sha(body)), gate.PREDECESSOR_POLICY_PIN)
        for name, approve in ((gate.CLI, gate.approved_cli), (gate.MAIN, gate.approved_main)):
            before = gate.git(ROOT, "show", gate.PREDECESSOR + ":" + name)
            self.assertEqual(gate.regular(ROOT, name), approve(before))
        before = gate.git(ROOT, "show", gate.PREDECESSOR + ":" + gate.PACKAGE)
        insertion = gate.unchanged_package(before, gate.regular(ROOT, gate.PACKAGE))
        self.assertEqual(gate.source_directives(gate.PACKAGE, insertion), gate.PACKAGE_INSERT_DIRECTIVES)
        for name, approve in ((gate.TEXT_RESOURCE_TEST, gate.approved_text_resource_test),
                              (gate.VECTOR_ROUTE_TEST, gate.approved_vector_route_test)):
            before = gate.git(ROOT, "show", gate.PREDECESSOR + ":" + name)
            body = gate.regular(ROOT, name)
            self.assertEqual(body, approve(before))
            self.assertEqual(gate.source_directives(name, body), gate.PLATFORM_TEST_DIRECTIVES[name])

    def test_run_and_all_inherited_predicates_are_same_function_objects(self):
        self.assertIs(gate.RETAINED_RUN, gate.library.run)
        self.assertIs(gate.RETAINED_RUN, gate.predecessor.RETAINED_RUN)
        self.assertIs(gate.RETAINED_RUN, gate.predecessor.predecessor.RETAINED_RUN)
        for name in ("verify_current_observer", "verify_materialized", "run_byte_buffer_observer_controls",
                     "run_file_observer_controls", "run_i64_observer_controls", "run_integer_width_observer_controls",
                     "run_integer128_observer_controls", "run_float_observer_controls", "run_character_observer_controls",
                     "run_vector_observer_controls", "run_record_vector_observer_controls"):
            self.assertIs(getattr(gate.predecessor.predecessor, name), getattr(gate.library, name))
        source = inspect.getsource(gate.run)
        self.assertIn("status = RETAINED_RUN(delegated)", source)
        self.assertIn("predecessor.predecessor.run_owned_vector_observer_controls(", source)
        self.assertNotIn("subprocess.run", source)
        self.assertNotIn(".replace(", source)

    def test_only_six_source_seams_change_and_all_bindings_restore_after_refusal(self):
        names = ("PREDECESSOR", "PREDECESSOR_ADDED", "PREDECESSOR_POLICIES", "PINS_PROVISIONAL",
                 "capture_sources", "current_manifest", "prepare_current_observer", "OBSERVER_ADAPTER")
        before = {name: getattr(gate.library, name) for name in names}
        with self.assertRaisesRegex(RuntimeError, "synthetic refusal"):
            with gate.bound_predecessor():
                self.assertEqual(gate.library.PREDECESSOR, gate.PREDECESSOR)
                self.assertEqual(gate.library.PREDECESSOR_ADDED, gate.predecessor.PARENT_ADDED)
                self.assertIs(gate.library.capture_sources, gate.capture_sources)
                self.assertIs(gate.library.current_manifest, gate.current_manifest)
                self.assertIs(gate.library.run, gate.RETAINED_RUN)
                raise RuntimeError("synthetic refusal")
        for name, value in before.items():
            self.assertIs(getattr(gate.library, name), value, name)

    def test_provisional_pins_refuse_qualification_before_any_process(self):
        with tempfile.TemporaryDirectory(prefix="oxid-vendor-provisional-") as folder:
            output = Path(folder) / "output"
            args = argparse.Namespace(repo=ROOT, output=output, prepare_only=False)
            with patch.object(gate, "PINS_PROVISIONAL", True), patch.object(gate, "RETAINED_RUN") as retained, \
                    patch.object(gate.library.subprocess, "run") as run, patch.object(gate.library.subprocess, "Popen") as popen, \
                    redirect_stderr(io.StringIO()):
                self.assertEqual(gate.run(args), 1)
            retained.assert_not_called()
            run.assert_not_called()
            popen.assert_not_called()
            failure = json.loads((output / "failure.json").read_bytes())
            self.assertFalse(failure["semantic_pass"])
            self.assertEqual(failure["compiler_executions"], 0)
            self.assertIn("prepare-only", failure["error"])
            self.assertFalse((output / "retained").exists())
            self.assertFalse((output / "result.json").exists())

    def test_frozen_replay_uses_unmodified_engine_and_all_fourteen_tests(self):
        engine = replay.load_engine(ROOT)
        self.assertEqual(engine.REVISION, gate.PREDECESSOR)
        self.assertEqual(engine.MODULES, replay.MODULES)
        self.assertEqual(engine.POLICIES, replay.POLICIES)
        name = "test_owned_text_compatibility"
        body = gate.regular(ROOT, "scripts/" + name + ".py")
        self.assertEqual(gate.sha(body), replay.MODULES[name][0])
        names = sorted(name + "." + cls.name + "." + method.name for cls in ast.parse(body).body
                       if isinstance(cls, ast.ClassDef) for method in cls.body
                       if isinstance(method, ast.FunctionDef) and method.name.startswith("test_"))
        self.assertEqual(len(names), 14)
        lines = [test.rsplit(".", 1)[1] + " (" + test + ") ... ok" for test in names]
        stderr = ("\n".join(lines) + "\n" + "-" * 70 + "\nRan 14 tests in 0.1s\n\nOK\n").encode()
        self.assertEqual(engine.validate_execution(name, body, subprocess.CompletedProcess([], 0, b"", stderr)), names)
        for result in (subprocess.CompletedProcess([], 1, b"", stderr),
                       subprocess.CompletedProcess([], 0, b"unexpected", stderr),
                       subprocess.CompletedProcess([], 0, b"", stderr.replace(b"... ok", b"... skipped", 1)),
                       subprocess.CompletedProcess([], 0, b"", stderr.replace(b"Ran 14", b"Ran 13", 1))):
            with self.subTest(result=result), self.assertRaises(ValueError):
                engine.validate_execution(name, body, result)

    def test_ci_keeps_frozen_jobs_and_runs_both_real_vendor_native_cases_on_both_linux_hosts(self):
        body = (ROOT / ".github/workflows/ci.yml").read_text()
        self.assertIn("python3 -B scripts/verify_offline_vendor_compatibility.py", body)
        self.assertIn("owned-text-source-policy-predecessor:", body)
        self.assertIn("ref: " + gate.PREDECESSOR, body)
        self.assertEqual(body.count('--test typed_vendor -- --ignored --test-threads=1'), 2)
        for start, end in (("  native-projects:", "  native-numeric-and-packages:"),
                           ("  linux-arm64-native:", "  workflow-lint:")):
            selected = body.split(start, 1)[1].split(end, 1)[0]
            self.assertIn('for profile in debug release; do', selected)
            self.assertIn('--test typed_vendor -- --ignored --test-threads=1', selected)
            self.assertIn('profile_flags=(--release)', selected)
        source = gate.regular(ROOT, "tests/typed_vendor.rs").decode()
        self.assertEqual(source.count('#[ignore = "requires actual LLVM, Clang and LLD 19.1.7"]'), 2)
        for job in ("quality-and-repository", "owned-record-vector-source-policy-predecessor",
                    "registry-source-policy-predecessor", "macos-native", "current-windows-native",
                    "cross-platform", "native-numeric-and-packages", "linux-arm64-native"):
            self.assertIn("  " + job + ":", body)


if __name__ == "__main__":
    unittest.main()
