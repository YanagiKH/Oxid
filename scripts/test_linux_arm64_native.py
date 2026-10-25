import tempfile
from pathlib import Path
from types import SimpleNamespace
import unittest

import verify_linux_arm64_native as gate


class Arm64NativeControllerTests(unittest.TestCase):
    def test_all_public_modules_run_once_with_ignored_tests_and_bounded_timeouts(self):
        with tempfile.TemporaryDirectory() as directory:
            root, calls = Path(directory), []

            def command(name, argv, timeout):
                calls.append((name, argv, timeout))
                output = root / name
                output.mkdir()
                (output / "stdout").write_text(
                    f"test result: ok. {len(calls)} passed; 0 failed; 0 ignored; 0 measured; 0 filtered out\n")
                return output

            for profile, flags, timeout in [("debug", [], 3600), ("release", ["--release"], 30)]:
                calls.clear()
                total, modules = gate.run_public_tests(SimpleNamespace(command=command), profile, flags, timeout)
                self.assertEqual(list(modules), list(gate.PUBLIC_TESTS))
                self.assertEqual(total, sum(range(1, len(gate.PUBLIC_TESTS) + 1)))
                for index, (name, argv, actual_timeout) in enumerate(calls, 1):
                    module = gate.PUBLIC_TESTS[index - 1]
                    self.assertEqual(name, profile + "-public-native-" + module)
                    self.assertEqual(argv, ["cargo", "test", "--locked", *flags, "--test", module,
                                            "--", "--include-ignored", "--test-threads=1"])
                    self.assertEqual(actual_timeout, min(timeout, 600))
                    self.assertEqual(modules[module], {"passed": index, "command": name})

    def test_individual_empty_failed_extra_and_ignored_module_results_are_refused(self):
        summary = "test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out\n"
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            for body in ("", summary * 2, summary.replace("1 passed", "0 passed"),
                         summary.replace("0 failed", "1 failed"),
                         summary.replace("0 ignored", "1 ignored")):
                (root / "stdout").write_text(body)
                runner = SimpleNamespace(command=lambda *args, **kwargs: root)
                with self.subTest(body=body), self.assertRaisesRegex(ValueError, "missing, empty or failed"):
                    gate.run_public_tests(runner, "debug", [], 3600)

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
