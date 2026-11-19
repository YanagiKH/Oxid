"""Reject incomplete evidence and edits to the fixed predecessor replay."""
import ast
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch

import run_copy_record_debug_predecessor_tests as replay

ROOT = Path(__file__).resolve().parents[1]
MODULE = "test_copy_record_debug_compatibility"


class CopyRecordDebugPredecessorEvidence(unittest.TestCase):
    def setUp(self):
        self.engine = replay.load_engine(ROOT)
        self.body = (ROOT / "scripts" / (MODULE + ".py")).read_bytes()
        self.names = sorted(MODULE + "." + cls.name + "." + method.name
                            for cls in ast.parse(self.body).body if isinstance(cls, ast.ClassDef)
                            for method in cls.body if isinstance(method, ast.FunctionDef)
                            and method.name.startswith("test_"))
        self.lines = [name.rsplit(".", 1)[1] + " (" + name + ") ... ok\n"
                      for name in self.names]
        self.stderr = ("".join(self.lines) + "\nRan 21 tests in 0.001s\n\nOK\n").encode()
        self.stderr = self.stderr.replace(replay.DENIAL_SUCCESS, replay.EXPECTED_DENIAL, 1)

    def test_all_twenty_one_unchanged_tests_are_required_with_terminal_success(self):
        self.assertEqual(replay.REVISION, "9a5ecaaeb5fa50aff2296bf990d63167143cf312")
        self.assertEqual(replay.MODULES, {MODULE: (
            "8fb0a586a790371179f98ffdba89090d5e5d092d0a14dd5a91aa951d0130b549", 21)})
        self.assertEqual(self.engine.sha(self.body), replay.MODULES[MODULE][0])
        self.assertEqual(self.engine.validate_execution(
            MODULE, self.body, subprocess.CompletedProcess([], 0, b"", self.stderr)), self.names)
        for stream in (
            b"", self.stderr.replace(self.lines[0].encode(), b"", 1),
            self.stderr + self.lines[0].encode(),
            self.stderr.replace(b"... ok", b"... skipped 'platform'", 1),
            self.stderr.replace(b"Ran 21 tests", b"Ran 20 tests"),
            self.stderr.replace(b"\nOK\n", b"\nOK (skipped=1)\n"),
            self.stderr.replace(b"provisional Copy-record-debug", b"unexpected Copy-record-debug"),
            self.stderr.replace(replay.EXPECTED_DENIAL, replay.DENIAL_SUCCESS),
            self.stderr + b"ValueError: unrelated failure\n",
        ):
            with self.subTest(stream=stream), self.assertRaises(ValueError):
                self.engine.validate_execution(
                    MODULE, self.body, subprocess.CompletedProcess([], 0, b"", stream))

    def test_failure_and_unexpected_stdout_cannot_be_replay_success(self):
        for code, stdout in ((1, b""), (0, b"unbound output")):
            with self.subTest(code=code), self.assertRaisesRegex(ValueError, "controls failed"):
                self.engine.validate_execution(
                    MODULE, self.body, subprocess.CompletedProcess([], code, stdout, self.stderr))

    def test_wrong_revision_and_dirty_checkout_fail_before_policy_loading(self):
        with patch.object(self.engine, "git", return_value=b"0" * 40):
            with self.assertRaisesRegex(ValueError, "wrong predecessor"):
                self.engine.identity(ROOT)
        with patch.object(self.engine, "git", side_effect=[replay.REVISION.encode(), b" M source.rs"]):
            with self.assertRaisesRegex(ValueError, "must be clean"):
                self.engine.identity(ROOT)

    def test_replay_engine_mutation_and_symlinks_fail_before_execution(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            (root / "scripts").mkdir()
            destination = root / "scripts" / replay.ENGINE
            destination.write_bytes((ROOT / "scripts" / replay.ENGINE).read_bytes() + b"\n")
            with self.assertRaisesRegex(ValueError, "changed predecessor"):
                replay.load_engine(root)
            destination.unlink()
            destination.symlink_to(ROOT / "scripts" / replay.ENGINE)
            with self.assertRaisesRegex(ValueError, "unsafe predecessor"):
                replay.load_engine(root)


if __name__ == "__main__":
    unittest.main()
