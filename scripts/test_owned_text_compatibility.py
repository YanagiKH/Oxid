"""Closed owned-text source admission and unchanged full replay controls."""
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

import replay_owned_record_vector_source_policy as replay
import verify_owned_text_compatibility as gate


ROOT = Path(__file__).resolve().parents[1]


def baseline_names():
    body = gate.regular(ROOT, gate.BINDING + "/current-source.json")
    if gate.sha(body) != gate.BASELINE_SOURCE_SHA:
        raise AssertionError("historical source manifest changed")
    return {row["path"] for row in json.loads(body)["files"]}


class TextSourceAdmission(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory(prefix="oxid-owned-text-policy-")
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        self.baseline = {name: b"// historical input\n" for name in baseline_names()}
        self.parent = {**self.baseline, **{name: b"// owned-vector parent input\n" for name in gate.PREDECESSOR_ADDED}}
        self.parent[gate.PROGRAM] = b"// fixed source program\n" + gate.OLD_HOOK + b"fn unchanged() {}\n"
        self.current = {**self.parent, gate.PROGRAM: gate.approved_program(self.parent[gate.PROGRAM]),
                        gate.TEST: b'const TEXT:&str=' + gate.ASSET_INCLUDE + b';\n#[test]\nfn first(){}\n',
                        gate.ASSET: b"pub struct Text { bytes: ByteBuf }\n"}
        self.pins = {name: (len(self.current[name]), gate.sha(self.current[name]))
                     for name in gate.TEXT_CHANGED | gate.TEXT_ADDED}
        self.directives = {name: gate.source_directives(name, self.current[name]) for name in self.pins}
        assets = {gate.ASSET: (len(self.current[gate.ASSET]), gate.sha(self.current[gate.ASSET]))}
        for name, body in self.current.items():
            self.put(name, body)
        for name, value in (("REVIEWED_INPUTS", self.pins), ("REVIEWED_DIRECTIVES", self.directives),
                            ("TEXT_SOURCE_ASSETS", assets)):
            selected = patch.object(gate, name, value)
            selected.start()
            self.addCleanup(selected.stop)

    def put(self, name, body):
        path = self.root / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(body)

    def capture(self):
        return gate.capture_sources(self.root, self.baseline, self.parent)

    def test_exact_parent_and_two_new_inputs_are_preserved(self):
        inputs = self.capture()
        self.assertEqual((len(self.baseline), len(self.parent), len(inputs)), (376, 572, 574))
        self.assertEqual(set(inputs), set(self.parent) | {gate.TEST, gate.ASSET})
        self.assertEqual(gate.TEXT_CHANGED, {gate.PROGRAM})
        self.assertEqual(gate.TEXT_ADDED, {gate.TEST})
        self.assertTrue(all(inputs[name] == before for name, before in self.parent.items() if name != gate.PROGRAM))

    def test_protected_existing_sources_cannot_drift(self):
        for name in ("src/main.rs", "native/experimental_bytebuf.c", "src/frontend/oir/owned/execute.rs"):
            with self.subTest(name=name):
                self.put(name, self.current[name] + b"// drift\n")
                with self.assertRaisesRegex(ValueError, "unreviewed change"):
                    self.capture()
                self.put(name, self.current[name])

    def test_program_pin_cannot_authorize_any_change_beyond_the_exact_hook(self):
        body = self.current[gate.PROGRAM] + b"fn production_change() {}\n"
        self.put(gate.PROGRAM, body)
        self.pins[gate.PROGRAM] = (len(body), gate.sha(body))
        self.directives[gate.PROGRAM] = gate.source_directives(gate.PROGRAM, body)
        with self.assertRaisesRegex(ValueError, "exceeds exact test hook"):
            self.capture()

    def test_missing_input_extra_source_empty_directory_and_symlink_are_denied(self):
        for name in ("src/main.rs", gate.TEST, gate.ASSET):
            with self.subTest(missing=name):
                path = self.root / name
                path.unlink()
                with self.assertRaises(ValueError):
                    self.capture()
                self.put(name, self.current[name])
        extra = self.root / "src/unreviewed.rs"
        extra.write_bytes(b"fn hidden() {}\n")
        with self.assertRaisesRegex(ValueError, "membership"):
            self.capture()
        extra.unlink()
        empty = self.root / "src/unreviewed"
        empty.mkdir()
        with self.assertRaisesRegex(ValueError, "directory"):
            self.capture()
        empty.rmdir()
        target = self.root / gate.ASSET
        alias = self.root / "src/unreviewed.rs"
        try:
            alias.symlink_to(target)
        except (OSError, NotImplementedError) as error:
            self.skipTest(f"symlinks unavailable: {error}")
        with self.assertRaisesRegex(ValueError, "symlink"):
            self.capture()

    def test_asset_identity_and_exact_include_are_bound_even_when_source_pins_are_updated(self):
        self.put(gate.ASSET, self.current[gate.ASSET] + b"// drift\n")
        with self.assertRaisesRegex(ValueError, "asset"):
            self.capture()
        self.put(gate.ASSET, self.current[gate.ASSET])
        redirected = self.current[gate.TEST].replace(gate.ASSET_INCLUDE, b'include_str!("outside.ox")')
        self.put(gate.TEST, redirected)
        self.pins[gate.TEST] = (len(redirected), gate.sha(redirected))
        self.directives[gate.TEST] = gate.source_directives(gate.TEST, redirected)
        with self.assertRaisesRegex(ValueError, "asset"):
            self.capture()

    def test_extra_directives_are_not_allowed_by_an_updated_byte_pin(self):
        changed = self.current[gate.TEST] + b"#[cfg(unreviewed)]\nfn hidden(){}\n"
        self.put(gate.TEST, changed)
        self.pins[gate.TEST] = (len(changed), gate.sha(changed))
        with self.assertRaisesRegex(ValueError, "directive"):
            self.capture()

    def test_pin_directive_and_asset_membership_must_be_exact(self):
        for attribute in ("REVIEWED_INPUTS", "REVIEWED_DIRECTIVES"):
            original = dict(getattr(gate, attribute))
            for table in ({gate.PROGRAM: original[gate.PROGRAM]}, {**original, "unreviewed.rs": original[gate.PROGRAM]}):
                with self.subTest(attribute=attribute), patch.object(gate, attribute, table):
                    with self.assertRaisesRegex(ValueError, "membership"):
                        self.capture()
        with patch.object(gate, "TEXT_SOURCE_ASSETS", {"packages/other/lib.ox": (0, gate.sha(b""))}):
            with self.assertRaises(ValueError):
                self.capture()

    def test_manifest_retains_false_claims_and_real_full_source_membership(self):
        inputs = self.capture()
        inherited = {"old_semantics_qualified_by_this_gate": False, "files": [], "additions": []}
        with patch.object(gate.predecessor, "current_manifest", return_value=inherited) as previous:
            manifest = gate.current_manifest(inputs, self.baseline, {"head": "test"}, self.parent)
        self.assertEqual(len(previous.call_args.kwargs["inputs"]), 572)
        self.assertEqual(len(previous.call_args.kwargs["parent"]), 527)
        self.assertEqual(len(manifest["files"]), 574)
        self.assertEqual(manifest["source_policy_parent"], gate.PREDECESSOR)
        self.assertIs(manifest["owned_text_pins_provisional"], gate.PINS_PROVISIONAL)
        self.assertTrue(all(value is False for name, value in manifest.items() if name.endswith("qualified_by_this_gate")))


class RetainedTextDelegation(unittest.TestCase):
    def test_source_pins_directives_asset_and_frozen_controller_are_exact(self):
        self.assertEqual(gate.REVIEWED_SOURCE_HEAD, "d5fed6b59d419e2f515b4dc724f318c106f18d73")
        for name, expected in gate.REVIEWED_INPUTS.items():
            body = gate.regular(ROOT, name)
            self.assertEqual((len(body), gate.sha(body)), expected)
            self.assertEqual(gate.source_directives(name, body), gate.REVIEWED_DIRECTIVES[name])
        body = gate.regular(ROOT, gate.PREDECESSOR_POLICY)
        self.assertEqual(body, gate.git(ROOT, "show", gate.PREDECESSOR + ":" + gate.PREDECESSOR_POLICY))
        self.assertEqual((len(body), gate.sha(body)), gate.PREDECESSOR_POLICY_PIN)
        gate.validate_source_assets({gate.TEST: gate.regular(ROOT, gate.TEST), gate.ASSET: gate.regular(ROOT, gate.ASSET)})
        before = gate.git(ROOT, "show", gate.PREDECESSOR + ":" + gate.PROGRAM)
        self.assertEqual(gate.regular(ROOT, gate.PROGRAM), gate.approved_program(before))

    def test_retained_runner_and_all_observer_control_functions_are_identical(self):
        self.assertIs(gate.RETAINED_RUN, gate.library.run)
        self.assertIs(gate.RETAINED_RUN, gate.predecessor.RETAINED_RUN)
        for name in ("verify_current_observer", "verify_materialized", "run_byte_buffer_observer_controls",
                     "run_file_observer_controls", "run_i64_observer_controls", "run_integer_width_observer_controls",
                     "run_integer128_observer_controls", "run_float_observer_controls", "run_character_observer_controls",
                     "run_vector_observer_controls", "run_record_vector_observer_controls"):
            self.assertIs(getattr(gate.predecessor, name), getattr(gate.library, name))
        source = inspect.getsource(gate.run)
        self.assertIn("status = RETAINED_RUN(delegated)", source)
        self.assertIn("predecessor.run_owned_vector_observer_controls(", source)
        self.assertNotIn("subprocess.run", source)
        self.assertNotIn(".replace(", source)

    def test_binding_only_existing_source_seams_restores_after_failure(self):
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

    def test_provisional_pins_cannot_start_qualification_or_any_external_process(self):
        with tempfile.TemporaryDirectory(prefix="oxid-text-provisional-") as folder:
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

    def test_frozen_replay_uses_the_unmodified_engine_and_all_twenty_one_tests(self):
        engine = replay.load_engine(ROOT)
        self.assertEqual(engine.REVISION, gate.PREDECESSOR)
        self.assertEqual(engine.MODULES, replay.MODULES)
        self.assertEqual(engine.POLICIES, replay.POLICIES)
        name = "test_owned_record_vector_compatibility"
        body = gate.regular(ROOT, "scripts/" + name + ".py")
        self.assertEqual(gate.sha(body), replay.MODULES[name][0])
        names = sorted(name + "." + cls.name + "." + method.name for cls in ast.parse(body).body
                       if isinstance(cls, ast.ClassDef) for method in cls.body
                       if isinstance(method, ast.FunctionDef) and method.name.startswith("test_"))
        self.assertEqual(len(names), 21)
        lines = [test.rsplit(".", 1)[1] + " (" + test + ") ... ok" for test in names]
        stderr = ("\n".join(lines) + "\n" + "-" * 70 + "\nRan 21 tests in 0.1s\n\nOK\n").encode()
        self.assertEqual(engine.validate_execution(name, body, subprocess.CompletedProcess([], 0, b"", stderr)), names)
        for result in (subprocess.CompletedProcess([], 1, b"", stderr),
                       subprocess.CompletedProcess([], 0, b"unexpected", stderr),
                       subprocess.CompletedProcess([], 0, b"", stderr.replace(b"... ok", b"... skipped", 1)),
                       subprocess.CompletedProcess([], 0, b"", stderr.replace(b"Ran 21", b"Ran 20", 1))):
            with self.subTest(result=result), self.assertRaises(ValueError):
                engine.validate_execution(name, body, result)

    def test_ci_keeps_mandatory_jobs_and_limits_text_native_driver_to_linux(self):
        body = (ROOT / ".github/workflows/ci.yml").read_text()
        self.assertIn("python3 -B scripts/verify_owned_text_compatibility.py", body)
        self.assertIn("owned-record-vector-source-policy-predecessor:", body)
        self.assertIn("ref: " + gate.PREDECESSOR, body)
        self.assertEqual(body.count("python3 -B scripts/verify_owned_text.py"), 2)
        self.assertIn('current-owned-text-native"', body)
        self.assertIn('linux-arm64-native/owned-text"', body)
        for start, end in (("  macos-native:", "  current-windows-native:"),
                           ("  current-windows-native:", "  cross-platform:")):
            self.assertNotIn("scripts/verify_owned_text.py", body.split(start, 1)[1].split(end, 1)[0])
        for job in ("quality-and-repository", "registry-source-policy-predecessor", "macos-native",
                    "current-windows-native", "cross-platform", "native-numeric-and-packages", "linux-arm64-native"):
            self.assertIn("  " + job + ":", body)


if __name__ == "__main__":
    unittest.main()
