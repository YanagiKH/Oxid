"""Require complete unchanged registry-policy evidence and retain raw streams."""
import argparse
import ast
import contextlib
import io
import json
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch

import replay_registry_source_policy as replay


ROOT = Path(__file__).resolve().parents[1]
MODULE = "test_registry_compatibility"


class RegistryPredecessorEvidence(unittest.TestCase):
    def setUp(self):
        self.engine = replay.load_engine(ROOT)
        self.body = (ROOT / "scripts" / (MODULE + ".py")).read_bytes()
        self.names = sorted(MODULE + "." + cls.name + "." + method.name
                            for cls in ast.parse(self.body).body if isinstance(cls, ast.ClassDef)
                            for method in cls.body if isinstance(method, ast.FunctionDef)
                            and method.name.startswith("test_"))
        self.lines = [name.rsplit(".", 1)[1] + " (" + name + ") ... ok\n"
                      for name in self.names]
        self.stderr = ("".join(self.lines) + "\n" + "-" * 70 +
                       "\nRan 37 tests in 0.001s\n\nOK\n").encode()
        self.stderr = self.stderr.replace(replay.DENIAL_SUCCESS, replay.EXPECTED_DENIAL, 1)

    def test_exact_revision_controller_required_policies_and_all_thirty_seven_tests_are_pinned(self):
        self.assertEqual(replay.REVISION, "e77402de69911a9d45a55fff5a8a0dd8de61f3e5")
        self.assertEqual(replay.MODULES, {MODULE: (
            "f499348d6de5226fb56802b6cb80f7b7d8b58df8bc25e8800792e8c70bb6e02b", 37)})
        self.assertEqual(replay.POLICIES, {
            "verify_registry_compatibility.py":
                "d6bd23496f38bde3d89e7e97a2d724b73a0f233cfef99a6dc9fad752bdec9766",
            "verify_scalar_library_compatibility.py":
                "29c1d76604adfeddf2006213922ceb59947f7a193454ffb542df2ba0f7597119",
            "replay_scalar_library_source_policy.py":
                "3c7715c35918ebb34b5ace5adcd90b30caf8d915d4e1fd4f1a3d3aa0d3f40ecb",
            "run_record_vector_predecessor_tests.py":
                "6dc9ee36351bdafab40399038133c8547dfb89ac0d0f33076b76b41ef374b75c",
        })
        self.assertEqual(self.engine.sha(self.body), replay.MODULES[MODULE][0])
        self.assertEqual(len(self.names), 37)
        child = subprocess.CompletedProcess([], 0, b"", self.stderr)
        self.assertEqual(self.engine.validate_execution(MODULE, self.body, child), self.names)
        self.assertEqual(child.stderr, self.stderr, "raw provisional refusal must remain unchanged")

    def test_missing_duplicate_skipped_or_failed_tests_and_unbound_output_are_refused(self):
        for stream in (
            b"", self.stderr.replace(self.lines[0].encode(), b"", 1),
            self.stderr + self.lines[0].encode(),
            self.stderr.replace(b"... ok", b"... skipped 'platform'", 1),
            self.stderr.replace(b"Ran 37 tests", b"Ran 36 tests"),
            self.stderr.replace(b"\nOK\n", b"\nOK (skipped=1)\n"),
            self.stderr.replace(b"\nOK\n", b"\nFAILED (failures=1)\n"),
            self.stderr.replace(b"provisional private-registry", b"unexpected private-registry"),
            self.stderr.replace(replay.EXPECTED_DENIAL, replay.DENIAL_SUCCESS),
            self.stderr + replay.EXPECTED_DENIAL,
            self.stderr + b"ValueError: provisional private-registry pins permit "
            b"prepare-only admission, not qualification\n",
            self.stderr + b"ValueError: unrelated failure\n",
            self.stderr + b"unbound output\n",
        ):
            with self.subTest(stream=stream), self.assertRaises(ValueError):
                self.engine.validate_execution(
                    MODULE, self.body, subprocess.CompletedProcess([], 0, b"", stream))
        changed = self.body.replace(b"def test_", b"def no_longer_a_test_", 1)
        with self.assertRaisesRegex(ValueError, "census changed"):
            self.engine.validate_execution(
                MODULE, changed, subprocess.CompletedProcess([], 0, b"", self.stderr))

    def test_failure_and_unexpected_stdout_cannot_be_replay_success(self):
        for code, stdout in ((1, b""), (0, b"unbound output")):
            with self.subTest(code=code), self.assertRaisesRegex(ValueError, "controls failed"):
                self.engine.validate_execution(
                    MODULE, self.body, subprocess.CompletedProcess([], code, stdout, self.stderr))

    def test_wrong_revision_and_dirty_checkout_fail_before_policy_loading(self):
        with patch.object(self.engine, "git", return_value=b"0" * 40), \
                patch.object(Path, "read_bytes") as reads:
            with self.assertRaisesRegex(ValueError, "wrong predecessor"):
                self.engine.identity(ROOT)
            reads.assert_not_called()
        with patch.object(self.engine, "git", side_effect=[replay.REVISION.encode(), b" M source.rs"]), \
                patch.object(Path, "read_bytes") as reads:
            with self.assertRaisesRegex(ValueError, "must be clean"):
                self.engine.identity(ROOT)
            reads.assert_not_called()

    def test_engine_and_every_pinned_input_refuse_mutation_or_unsafe_engine_before_execution(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "scripts").mkdir()
            engine_path = root / "scripts" / replay.ENGINE
            engine_body = (ROOT / "scripts" / replay.ENGINE).read_bytes()
            engine_path.write_bytes(engine_body + b"\n")
            with self.assertRaisesRegex(ValueError, "changed predecessor"):
                replay.load_engine(root)
            engine_path.unlink()
            engine_path.symlink_to(ROOT / "scripts" / replay.ENGINE)
            with self.assertRaisesRegex(ValueError, "unsafe predecessor"):
                replay.load_engine(root)
            engine_path.unlink()
            pins = {**{name + ".py": pin for name, (pin, _) in replay.MODULES.items()},
                    **replay.POLICIES}
            originals = {name: (ROOT / "scripts" / name).read_bytes() for name in pins}
            for name, body in originals.items():
                (root / "scripts" / name).write_bytes(body)
            with patch.object(self.engine, "git", side_effect=[replay.REVISION.encode(), b""]):
                self.assertEqual(self.engine.identity(root), originals)
            for name, body in originals.items():
                path = root / "scripts" / name
                path.write_bytes(body + b"\n")
                with self.subTest(input=name), patch.object(
                        self.engine, "git", side_effect=[replay.REVISION.encode(), b""]):
                    with self.assertRaisesRegex(ValueError, "changed pinned predecessor input"):
                        self.engine.identity(root)
                path.write_bytes(body)

    def test_engine_retains_raw_provisional_denial_and_failure_transcripts(self):
        for accepted in (True, False):
            stream = self.stderr if accepted else self.stderr + b"unbound output\n"
            child = subprocess.CompletedProcess([], 0, b"", stream)
            with self.subTest(accepted=accepted), tempfile.TemporaryDirectory() as directory:
                output = Path(directory) / "replay"
                args = argparse.Namespace(repo=ROOT, output=output)
                bodies = {MODULE + ".py": self.body}
                with patch.object(self.engine, "identity", return_value=bodies), \
                        patch.object(self.engine.subprocess, "run", return_value=child), \
                        contextlib.redirect_stdout(io.StringIO()):
                    if accepted:
                        self.assertEqual(self.engine.run(args), 0)
                    else:
                        with self.assertRaisesRegex(ValueError, "unrecognized predecessor test output"):
                            self.engine.run(args)
                self.assertEqual((output / (MODULE + ".stdout")).read_bytes(), child.stdout)
                self.assertEqual((output / (MODULE + ".stderr")).read_bytes(), stream)
                self.assertEqual(child.stderr, stream)
                report = json.loads((output / "result.json").read_bytes())
                self.assertEqual(report["status"], "passed" if accepted else "failed")
                self.assertIs(report["historical_results_apply_to_current"], False)
                row = report["modules"][MODULE]
                self.assertEqual(row["exit_status"], 0)
                self.assertEqual(row["stderr_sha256"], self.engine.sha(stream))
                if accepted:
                    self.assertEqual(report["test_executions"], 37)
                    self.assertEqual(row["tests"], self.names)
                else:
                    self.assertNotIn("test_executions", report)


if __name__ == "__main__":
    unittest.main()
