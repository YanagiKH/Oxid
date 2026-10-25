import tempfile
from pathlib import Path
from types import SimpleNamespace
import unittest

import verify_linux_arm64_native as gate


class Arm64NativeControllerTests(unittest.TestCase):
    def test_no_empty_partial_or_failed_public_test_selection_can_pass(self):
        summary = "test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out\n"
        for body in ["", summary, summary * (len(gate.PUBLIC_TESTS) - 1),
                     summary * len(gate.PUBLIC_TESTS) + summary,
                     (summary * len(gate.PUBLIC_TESTS)).replace("1 passed", "0 passed", 1),
                     (summary * len(gate.PUBLIC_TESTS)).replace("0 failed", "1 failed", 1)]:
            with self.assertRaisesRegex(ValueError, "missing, empty or failed"):
                gate.public_results(body)
        self.assertEqual(gate.public_results(summary * len(gate.PUBLIC_TESTS)), len(gate.PUBLIC_TESTS))

    def test_wrong_host_is_refused_before_output_or_any_command(self):
        from unittest.mock import patch
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / "results"
            with patch.object(gate.target, "required_host", return_value=gate.target.X86_64):
                with self.assertRaisesRegex(ValueError, "ARM64 Linux host"):
                    gate.verify(SimpleNamespace(output=output))
            self.assertFalse(output.exists())

    def test_invalid_timeout_is_refused_before_output_or_any_command(self):
        from unittest.mock import patch
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / "results"
            for timeout in (0, -1, float("nan"), float("inf"), -float("inf")):
                with self.subTest(timeout=timeout):
                    with patch.object(gate.target, "required_host", return_value=gate.target.ARM64):
                        with self.assertRaisesRegex(ValueError, "positive and finite"):
                            gate.verify(SimpleNamespace(output=output, timeout=timeout))
                    self.assertFalse(output.exists())


if __name__ == "__main__":
    unittest.main()
