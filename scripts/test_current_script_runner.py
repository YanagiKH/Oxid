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
    for name in runner.HISTORICAL_ONLY.keys() | runner.PREDECESSOR_ONLY.keys()
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
            "test_bounded_byte_storage",
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

    def test_byte_registry_delegation_is_exact_and_refuses_changed_or_missing_source(self):
        name = "test_bounded_byte_storage.py"
        for source in (FROZEN_SOURCES[name] + PASSING.encode(), None):
            with self.subTest(missing=source is None):
                result = self.execute({name: source, "test_current.py": PASSING})
                self.assertNotEqual(result.returncode, 0, result.stderr)
                self.assertIn("cannot delegate", result.stderr)
                self.assertIn(name, result.stderr)
        result = self.execute({"test_bounded_byte_storage_extra.py": PASSING, "test_current.py": PASSING})
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("Ran 2 tests", result.stderr)
        self.assertIn("test_bounded_byte_storage_extra.CurrentTest.test_passes", result.stderr)

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

    def test_predecessor_replay_is_exact_and_does_not_hide_future_modules(self):
        self.assertEqual(set(runner.PREDECESSOR_ONLY), {
            "test_byte_buffer_compatibility", "test_hosted_execution_compatibility",
            "test_buffer_transfer_compatibility", "test_byte_string_compatibility",
            "test_arm64_host_compatibility", "test_checked_copy_compatibility",
            "test_typed_vector_compatibility", "test_qualified_completion_compatibility",
            "test_copy_record_debug_compatibility", "test_windows_native_compatibility",
            "test_record_field_completion_compatibility", "test_file_seek_compatibility",
            "test_scalar_library_compatibility", "test_arm64_package_compatibility",
            "test_registry_compatibility", "test_owned_record_vector_compatibility",
            "test_owned_text_compatibility", "test_offline_vendor_compatibility",
            "test_owned_enum_vector_compatibility", "test_generic_owned_vector_compatibility"})
        for module, (_, revision, _) in runner.PREDECESSOR_ONLY.items():
            name = module + ".py"
            for source in (FROZEN_SOURCES[name] + PASSING.encode(), None):
                with self.subTest(module=module, missing=source is None):
                    result = self.execute({name: source, "test_current.py": PASSING})
                    self.assertNotEqual(result.returncode, 0, result.stderr)
                    self.assertIn("cannot delegate", result.stderr)
                    self.assertIn(name, result.stderr)
            result = self.execute({module + "_future.py": PASSING})
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertIn("Ran 1 test", result.stderr)
            self.assertIn("Mandatory predecessor replay: " + name, result.stderr)
            self.assertIn(revision, result.stderr)

    def test_registry_replay_is_bound_to_its_exact_revision_without_hiding_a_neighbor(self):
        self.assertEqual(runner.PREDECESSOR_ONLY["test_registry_compatibility"], (
            "f499348d6de5226fb56802b6cb80f7b7d8b58df8bc25e8800792e8c70bb6e02b",
            "e77402de69911a9d45a55fff5a8a0dd8de61f3e5",
            "requires the pre-owned-vector compiler and exact registry workflow source authority"))
        self.assertNotIn("test_registry_compatibility", runner.HISTORICAL_ONLY)
        result = self.execute({"test_registry_compatibility_future.py": PASSING})
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("Ran 1 test", result.stderr)
        self.assertIn("test_registry_compatibility_future.CurrentTest.test_passes", result.stderr)
        self.assertIn("Mandatory predecessor replay: test_registry_compatibility.py @ "
                      "e77402de69911a9d45a55fff5a8a0dd8de61f3e5", result.stderr)

    def test_owned_vector_replay_keeps_its_exact_source_and_does_not_hide_text_successors(self):
        self.assertEqual(runner.PREDECESSOR_ONLY["test_owned_record_vector_compatibility"], (
            "0981b5cc24a281f892ee1c8b6d26cbfc3c2a7d0f4a4dfa8c2743714c35723f3b",
            "0e8f23f7481c7ec4e24cd416ed30607f8eb75daf",
            "requires the pre-owned-text compiler and exact 572-input source authority"))
        self.assertNotIn("test_owned_record_vector_compatibility", runner.HISTORICAL_ONLY)
        result = self.execute({"test_owned_text_compatibility_future.py": PASSING})
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("test_owned_text_compatibility_future.CurrentTest.test_passes", result.stderr)
        self.assertIn("Mandatory predecessor replay: test_owned_record_vector_compatibility.py @ "
                      "0e8f23f7481c7ec4e24cd416ed30607f8eb75daf", result.stderr)

    def test_owned_text_replay_keeps_its_exact_source_and_does_not_hide_vendor_successors(self):
        self.assertEqual(runner.PREDECESSOR_ONLY["test_owned_text_compatibility"], (
            "1fe80493856cf720c76525321ca7530f54c782282b95793d29ef47a889946ebc",
            "7279208226a3d4185c0b996491332519971e916e",
            "requires the pre-vendor compiler and exact 574-input source authority"))
        self.assertNotIn("test_owned_text_compatibility", runner.HISTORICAL_ONLY)
        result = self.execute({"test_owned_text_compatibility_future.py": PASSING,
                               "test_offline_vendor_future.py": PASSING})
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn("Ran 2 tests", result.stderr)
        self.assertIn("test_offline_vendor_future.CurrentTest.test_passes", result.stderr)
        self.assertIn("Mandatory predecessor replay: test_owned_text_compatibility.py @ "
                      "7279208226a3d4185c0b996491332519971e916e", result.stderr)

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
