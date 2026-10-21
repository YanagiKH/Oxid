"""Failure controls for the current native runner; native oracle bodies are unchanged."""
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

import verify_current_native as gate


class CurrentNativeControls(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        self.repo = self.root / "repo"
        self.repo.mkdir()

    def test_output_must_be_fresh_and_outside_checkout(self):
        for path in (self.repo, self.repo / "inside"):
            with self.subTest(path=path), self.assertRaises(ValueError):
                gate.fresh_output(self.repo, path)
        output = gate.fresh_output(self.repo, self.root / "results")
        with self.assertRaises(ValueError):
            gate.fresh_output(self.repo, output)
        link = self.root / "alias"
        link.symlink_to(self.repo / "missing")
        with self.assertRaises(ValueError):
            gate.fresh_output(self.repo, link)

    def test_zero_wrong_or_duplicate_test_selection_is_rejected(self):
        for body in ("0 tests, 0 benchmarks\n", "name: benchmark\n", "name: test\nname: test\n"):
            with self.subTest(body=body), self.assertRaises(ValueError):
                gate.selected_tests(body)
        with self.assertRaises(ValueError):
            gate.selected_tests("other: test\n", exact="name")
        self.assertEqual(gate.selected_tests("name: test\n\n1 test, 0 benchmarks\n", "name"), ["name"])

    def test_success_requires_all_selected_tests_and_no_ignored_results(self):
        for body in ("", "test result: ok. 0 passed; 0 failed; 0 ignored;",
                     "test result: ok. 1 passed; 0 failed; 1 ignored;",
                     "test result: FAILED. 1 passed; 1 failed; 0 ignored;"):
            with self.subTest(body=body), self.assertRaises(ValueError):
                gate.check_test_result(body, ["one", "two"])
        gate.check_test_result("test result: ok. 2 passed; 0 failed; 0 ignored; 0 measured;\n", ["one", "two"])

    def test_build_requires_unique_matching_executable_and_success(self):
        binary = self.root / "oxid"
        binary.write_text("#!/bin/sh\nexit 0\n")
        binary.chmod(0o755)
        artifact = {"reason": "compiler-artifact", "target": {"name": "oxid", "kind": ["bin"]},
                    "profile": {"test": False}, "executable": str(binary)}
        success = {"reason": "build-finished", "success": True}
        log = self.root / "cargo.jsonl"
        for rows in ([artifact], [success], [artifact, artifact, success],
                     [artifact, {"reason": "build-finished", "success": False}]):
            log.write_text("\n".join(json.dumps(r) for r in rows))
            with self.subTest(rows=rows), self.assertRaises(ValueError):
                gate.built_executable(log, False)
        log.write_text("\n".join(json.dumps(r) for r in [artifact, success]))
        self.assertEqual(gate.built_executable(log, False), binary)
        with self.assertRaises(ValueError):
            gate.built_executable(log, True)

    @unittest.skipIf(sys.platform == "win32", "native runner requires POSIX process groups")
    def test_command_failure_and_timeout_retain_streams_and_failure_receipts(self):
        output = self.root / "evidence"
        output.mkdir()
        runner = gate.Runner(self.repo, output, Path("/unused"), 0.2)
        with self.assertRaises(ValueError):
            runner.command("failed", [sys.executable, "-c",
                "import sys; print('kept'); print('diagnostic', file=sys.stderr); sys.exit(7)"])
        receipt = json.loads((output / "commands/failed/command.json").read_text())
        self.assertEqual((receipt["status"], receipt["exit_code"]), ("failed", 7))
        self.assertEqual((output / "commands/failed/stdout").read_text(), "kept\n")
        self.assertEqual((output / "commands/failed/stderr").read_text(), "diagnostic\n")
        with self.assertRaises(subprocess.TimeoutExpired):
            runner.command("timeout", [sys.executable, "-c", "import time; time.sleep(10)"])
        receipt = json.loads((output / "commands/timeout/command.json").read_text())
        self.assertEqual(receipt["status"], "failed")
        self.assertIn("timed out", receipt["error"])

    def test_missing_native_artifacts_do_not_count_as_evidence(self):
        output = self.root / "native"
        output.mkdir()
        with self.assertRaises(ValueError):
            gate.native_artifacts(output)
        (output / "sample.ll").write_text("example")
        with self.assertRaises(ValueError):
            gate.native_artifacts(output)


if __name__ == "__main__":
    unittest.main()
