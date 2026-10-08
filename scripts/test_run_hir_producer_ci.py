"""Admission tests for hosted exact-build producer qualification."""
import json
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch

from run_hir_producer_ci import Runner, select_executable


class BuildReceiptControls(unittest.TestCase):
    def fixture(self, root, profile="debug"):
        executable = root / profile / "oxid"
        executable.parent.mkdir()
        executable.write_bytes(b"selected compiler")
        artifact = dict(reason="compiler-artifact", target=dict(name="oxid", kind=["bin"]),
                        profile=dict(test=False, opt_level="0" if profile == "debug" else "3"),
                        executable=str(executable))
        return executable, artifact

    def data(self, records, success=True):
        return b"\n".join(json.dumps(r).encode() for r in records + [dict(reason="build-finished", success=success)])

    def test_selects_actual_ordinary_non_test_executable(self):
        for profile in ("debug", "release"):
            with self.subTest(profile=profile), tempfile.TemporaryDirectory() as tmp:
                root = Path(tmp)
                executable, artifact = self.fixture(root, profile)
                selected, record = select_executable(self.data([artifact]), root, profile)
                self.assertEqual(selected, executable)
                self.assertEqual(record, artifact)

    def test_missing_duplicate_wrong_profile_or_test_artifact_is_rejected(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            _, artifact = self.fixture(root)
            cases = [[], [artifact, artifact],
                     [dict(artifact, profile=dict(test=True, opt_level="0"))],
                     [dict(artifact, profile=dict(test=False, opt_level="3"))]]
            for case in cases:
                with self.subTest(case=case), self.assertRaises(RuntimeError):
                    select_executable(self.data(case), root, "debug")
            with self.assertRaises(RuntimeError):
                select_executable(self.data([artifact], success=False), root, "debug")

    def test_outside_path_and_symlink_are_rejected(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            executable, artifact = self.fixture(root)
            outside = root / "outside"
            outside.write_bytes(b"other")
            with self.assertRaises(RuntimeError):
                select_executable(self.data([dict(artifact, executable=str(outside))]), root, "debug")
            executable.unlink()
            executable.symlink_to(outside)
            with self.assertRaises(RuntimeError):
                select_executable(self.data([artifact]), root, "debug")

    def test_failed_and_timed_out_commands_retain_incomplete_evidence(self):
        for result in (subprocess.CompletedProcess(["fake"], 7, b"out", b"err"),
                       subprocess.TimeoutExpired(["fake"], 1, output=b"out", stderr=b"err")):
            with self.subTest(result=result), tempfile.TemporaryDirectory() as tmp:
                runner = Runner.__new__(Runner)
                runner.output = runner.repo = Path(tmp)
                runner.env = {}
                runner.receipt = dict(complete=False, commands=[])
                kwargs = {"side_effect": result} if isinstance(result, Exception) else {"return_value": result}
                with patch("run_hir_producer_ci.subprocess.run", **kwargs):
                    with self.assertRaises(RuntimeError):
                        runner.call("failure", ["fake"], timeout=1)
                receipt = json.loads((runner.output / "receipt.json").read_text())
                self.assertFalse(receipt["complete"])
                self.assertNotEqual(receipt["commands"][0]["status"], 0)
                self.assertEqual((runner.output / "failure.stdout").read_bytes(), b"out")
                self.assertEqual((runner.output / "failure.stderr").read_bytes(), b"err")

    def test_unlaunchable_command_retains_explicit_spawn_failure(self):
        with tempfile.TemporaryDirectory() as tmp:
            runner = Runner.__new__(Runner)
            runner.output = runner.repo = Path(tmp)
            runner.env = {}
            runner.receipt = dict(complete=False, commands=[])
            with patch("run_hir_producer_ci.subprocess.run", side_effect=FileNotFoundError("missing-tool")):
                with self.assertRaises(RuntimeError):
                    runner.call("missing", ["missing-tool"])
            receipt = json.loads((runner.output / "receipt.json").read_text())
            self.assertEqual(receipt["commands"][0]["status"], "spawn-failed")
            self.assertEqual((runner.output / "missing.stdout").read_bytes(), b"")
            self.assertIn(b"missing-tool", (runner.output / "missing.stderr").read_bytes())

    def test_profile_override_is_rejected_before_source_or_build_access(self):
        runner = Runner.__new__(Runner)
        runner.forbidden = ["RUSTFLAGS"]
        with patch.object(runner, "identity") as identity:
            with self.assertRaisesRegex(RuntimeError, "forbids overrides"):
                runner.run()
            identity.assert_not_called()

    def test_workflow_keeps_exact_head_profiles_and_failure_upload(self):
        workflow = (Path(__file__).resolve().parents[1] / ".github/workflows/ci.yml").read_text()
        job = workflow.split("  local-hir-producers:\n", 1)[1]
        self.assertIn("github.event.pull_request.head.sha || github.sha", job)
        self.assertIn("scripts/run_hir_producer_ci.py", job)
        self.assertIn("if: always()", job)
        self.assertIn("cargo fetch --locked", job)
        self.assertNotIn("continue-on-error", job)
        self.assertNotIn("permissions:", job)


if __name__ == "__main__":
    unittest.main()
