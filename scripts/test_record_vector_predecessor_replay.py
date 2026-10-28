"""Refuse incomplete or unsuccessful evidence from the mandatory old suites."""
from pathlib import Path
import subprocess
import unittest
from unittest.mock import patch

import run_record_vector_predecessor_tests as replay

ROOT = Path(__file__).resolve().parents[1]


class PredecessorReplayEvidence(unittest.TestCase):
    def test_both_complete_original_suites_are_required(self):
        self.assertEqual({name: row[1] for name, row in replay.MODULES.items()}, {
            "test_checked_copy_compatibility": 20,
            "test_typed_vector_compatibility": 18,
        })
        for name, (expected, _) in replay.MODULES.items():
            body = (ROOT / "scripts" / (name + ".py")).read_bytes()
            self.assertEqual(replay.sha(body), expected)
            # No terminal success or individual test observations is evidence.
            for stderr in (b"", b"Ran 0 tests in 0.000s\n\nOK\n",
                           b"Ran 20 tests in 0.000s\n\nOK (skipped=20)\n"):
                with self.subTest(module=name, stderr=stderr):
                    child = subprocess.CompletedProcess([], 0, b"", stderr)
                    with self.assertRaisesRegex(ValueError, "partial predecessor"):
                        replay.validate_execution(name, body, child)

    def test_nonzero_exit_and_unexpected_stdout_cannot_be_success(self):
        name = "test_checked_copy_compatibility"
        body = (ROOT / "scripts" / (name + ".py")).read_bytes()
        for code, stdout in ((1, b""), (0, b"unbound output")):
            with self.subTest(code=code, stdout=stdout):
                with self.assertRaisesRegex(ValueError, "controls failed"):
                    replay.validate_execution(
                        name, body, subprocess.CompletedProcess([], code, stdout, b""))

    def test_wrong_revision_and_dirty_checkout_are_refused_before_loading(self):
        with patch.object(replay, "git", return_value=b"0" * 40):
            with self.assertRaisesRegex(ValueError, "wrong predecessor"):
                replay.identity(ROOT)
        with patch.object(replay, "git", side_effect=[replay.REVISION.encode(), b" M changed.py"]):
            with self.assertRaisesRegex(ValueError, "must be clean"):
                replay.identity(ROOT)


if __name__ == "__main__":
    unittest.main()
