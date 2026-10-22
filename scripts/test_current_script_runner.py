"""Exercise discovery and failure propagation using isolated test directories."""
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

import run_current_script_tests as runner


RUNNER_SOURCE = Path(runner.__file__).read_bytes()
FROZEN_SOURCES = {
    name + ".py": (Path(runner.__file__).parent / (name + ".py")).read_bytes()
    for name in runner.HISTORICAL_ONLY
}
PASSING = """import unittest
class CurrentTest(unittest.TestCase):
    def test_passes(self):
        self.assertEqual(6 * 7, 42)
"""


class CurrentScriptRunnerTests(unittest.TestCase):
    def execute(self, files):
        with tempfile.TemporaryDirectory(prefix="oxid-script-discovery-") as temporary:
            root = Path(temporary)
            entry = root / "run_current_script_tests.py"
            entry.write_bytes(RUNNER_SOURCE)
            for name, source in {**FROZEN_SOURCES, **files}.items():
                if source is None:
                    continue
                destination = root / name
                destination.parent.mkdir(parents=True, exist_ok=True)
                destination.write_bytes(source if isinstance(source, bytes) else source.encode())
            return subprocess.run(
                [sys.executable, "-B", str(entry)],
                cwd=root, capture_output=True, text=True, timeout=30,
            )

    def test_new_matching_modules_are_discovered_without_an_allowlist(self):
        result = self.execute({
            "test_existing.py": PASSING,
            "test_future_addition.py": PASSING,
            "test_lexer_current_ci_future.py": PASSING,
            "helper.py": "raise RuntimeError('not a test module')\n",
        })
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("Ran 3 tests", result.stderr)
        self.assertIn("test_future_addition.CurrentTest.test_passes", result.stderr)
        self.assertIn("test_lexer_current_ci_future.CurrentTest.test_passes", result.stderr)

    def test_load_tests_receives_normal_discovery_pattern_for_nested_suites(self):
        hook = """from pathlib import Path
import unittest
def load_tests(loader, tests, pattern):
    if pattern != "test_*.py":
        raise AssertionError(f"discovery pattern changed: {pattern!r}")
    nested = Path(__file__).parent / "components"
    return loader.discover(str(nested), pattern=pattern, top_level_dir=str(nested))
"""
        result = self.execute({
            "test_hook.py": hook,
            "test_current.py": PASSING,
            "components/test_nested.py": PASSING,
            "components/test_lexer_current_ci.py": PASSING,
        })
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("Ran 3 tests", result.stderr)
        self.assertIn("test_nested.CurrentTest.test_passes", result.stderr)
        self.assertIn("test_lexer_current_ci.CurrentTest.test_passes", result.stderr)

    def test_only_the_exact_source_pinned_modules_are_delegated_without_importing(self):
        approved = {
            "test_lexer_current_ci", "test_lexer_reservation_adapters_current",
            "test_lexer_reservation_source_current",
            "test_owned_array_native", "test_package_json_source",
            "test_projected_observer_adapters", "test_published_formatter_source",
            "test_record_composition_current_contracts", "test_record_composition_observer_adapters",
            "test_record_composition_runtime_view", "test_typed_project_source_binding",
        }
        self.assertEqual(set(runner.HISTORICAL_ONLY), approved)
        result = self.execute({"test_current.py": PASSING})
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("Ran 1 test", result.stderr)
        for name in approved:
            self.assertIn(f"Mandatory historical replay: {name}.py", result.stderr)
            self.assertTrue(runner.HISTORICAL_ONLY[name][1])
        self.assertIn("9ff8ef90a26f003f4616838dfc186fa3bfb6845c", result.stderr)

    def test_changed_or_missing_historical_module_cannot_hide_current_tests(self):
        name = "test_lexer_reservation_source_current.py"
        changed = FROZEN_SOURCES[name] + b"\n" + PASSING.encode()
        for source in (changed, None):
            with self.subTest(missing=source is None):
                result = self.execute({name: source, "test_current.py": PASSING})
                self.assertNotEqual(result.returncode, 0, result.stderr)
                self.assertIn(name, result.stderr)
                self.assertIn("cannot delegate", result.stderr)
                self.assertIn("current test module", result.stderr)

    def test_current_assertion_and_import_failures_are_never_suppressed(self):
        for failing in [
            PASSING.replace("6 * 7, 42", "6 * 7, 41, 'current assertion failure'"),
            "raise ImportError('current import failure')\n",
            "raise RuntimeError('current import execution failure')\n",
        ]:
            with self.subTest(source=failing):
                result = self.execute({"test_current_failure.py": failing})
                self.assertNotEqual(result.returncode, 0)
                self.assertIn("FAILED", result.stderr)
                self.assertIn("current", result.stderr)

    def test_empty_or_only_delegated_test_sets_cannot_report_success(self):
        for files in [
            {},
            {"test_lexer_current_ci.py": FROZEN_SOURCES["test_lexer_current_ci.py"]},
            {"test_empty.py": "# No test cases.\n"},
        ]:
            with self.subTest(files=list(files)):
                result = self.execute(files)
                self.assertNotEqual(result.returncode, 0)
                self.assertIn("error:", result.stderr)


if __name__ == "__main__":
    unittest.main()
