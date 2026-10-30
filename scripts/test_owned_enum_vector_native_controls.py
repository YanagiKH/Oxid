"""Mutation controls for native evidence admission; no compiler qualification here."""
import json
import os
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

import test_native_owned_enum_vectors as contracts
import verify_native_owned_enum_vectors as verify


ROOT = Path(__file__).resolve().parents[1]


class EnumVectorNativeEvidenceControls(unittest.TestCase):
    def test_both_optimizations_keep_assertions_strict_diagnostics_and_both_sanitizers(self):
        for optimization in ("O0", "O2"):
            expected = ["-std=c11", "-Wall", "-Wextra", "-Werror", "-" + optimization, "-UNDEBUG"]
            self.assertEqual(contracts.strict_flags(optimization, False), expected)
            self.assertEqual(contracts.strict_flags(optimization, True), expected + [
                "-fsanitize=address,undefined", "-fno-omit-frame-pointer", "-fno-sanitize-recover=all"])
        with self.assertRaises(ValueError):
            contracts.strict_flags("O3", True)
        with patch.dict(os.environ, {"ASAN_OPTIONS": "exitcode=0:halt_on_error=0",
                                     "UBSAN_OPTIONS": "halt_on_error=0", "LSAN_OPTIONS": "exitcode=0"}):
            environment = contracts.strict_environment(True)
        self.assertEqual(environment["ASAN_OPTIONS"], "halt_on_error=1:abort_on_error=1:exitcode=1")
        self.assertEqual(environment["UBSAN_OPTIONS"], "halt_on_error=1:print_stacktrace=1")
        self.assertNotIn("LSAN_OPTIONS", environment)

    def test_producer_must_list_and_execute_the_one_exact_real_rust_test(self):
        names = ("another::ordinary_case", contracts.PRODUCER_TEST, "yet_another::ordinary_case")
        catalog = ("\n".join(name + ": test" for name in names) + "\n\n3 tests, 0 benchmarks\n").encode()
        listing = (contracts.PRODUCER_TEST + ": test\n\n1 test, 0 benchmarks\n").encode()
        execution = ("\nrunning 1 test\ntest " + contracts.PRODUCER_TEST
                     + " ... ok\n\ntest result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; "
                       "2 filtered out; finished in 0.00s\n\n").encode()
        selection = contracts.validate_producer_streams(catalog, listing, execution)
        self.assertEqual(selection["catalog_test_count"], 3)
        self.assertEqual(selection["selected_tests"], [contracts.PRODUCER_TEST])
        self.assertEqual(selection["expected_filtered_out"], 2)
        self.assertEqual(selection["execution_scope"], "selected tests; complete binary catalog listed only")
        for changed_listing, changed_execution in (
            (listing.replace(b"1 test", b"0 tests"), execution),
            (listing + b"extra_test: test\n", execution),
            (listing, execution.replace(b"... ok", b"... ignored")),
            (listing, execution.replace(b"1 passed", b"0 passed")),
            (listing, execution.replace(b"2 filtered out", b"0 filtered out")),
            (listing, execution.replace(b"2 filtered out", b"1 filtered out")),
            (listing, execution.replace(b"2 filtered out", b"3 filtered out")),
            (listing, b""),
        ):
            with self.subTest(listing=changed_listing, execution=changed_execution), self.assertRaises(ValueError):
                contracts.validate_producer_streams(catalog, changed_listing, changed_execution)
        missing = catalog.replace(contracts.PRODUCER_TEST.encode(), b"other::producer_name")
        with self.assertRaisesRegex(ValueError, "occur exactly once"):
            contracts.validate_producer_streams(missing, listing, execution)

    def test_complete_catalog_requires_actual_unique_names_and_its_exact_summary(self):
        catalog = b"first::test: test\nsecond::test: test\n\n2 tests, 0 benchmarks\n"
        self.assertEqual(contracts.rust_test_catalog(catalog), ("first::test", "second::test"))
        for changed in (catalog.replace(b"2 tests", b"1 test"),
                        catalog.replace(b"second::test", b"first::test"),
                        catalog.replace(b"0 benchmarks", b"1 benchmarks"),
                        catalog.replace(b"2 tests", b"2 test"),
                        catalog + b"unlisted::extra: test\n", b""):
            with self.subTest(catalog=changed), self.assertRaises(ValueError):
                contracts.rust_test_catalog(changed)

    def test_three_native_cases_derive_filtered_ordinary_tests_from_the_full_catalog(self):
        names = ("ordinary_a", *verify.CLI_NATIVE_TESTS, "ordinary_b")
        catalog = ("\n".join(name + ": test" for name in names) + "\n\n5 tests, 0 benchmarks\n").encode()
        selected = ("\n".join(name + ": test" for name in verify.CLI_NATIVE_TESTS)
                    + "\n\n3 tests, 0 benchmarks\n").encode()
        execution = ("\nrunning 3 tests\n"
                     + "".join("test " + name + " ... ok\n" for name in verify.CLI_NATIVE_TESTS)
                     + "\ntest result: ok. 3 passed; 0 failed; 0 ignored; 0 measured; "
                       "2 filtered out; finished in 0.00s\n\n").encode()
        selection = contracts.validate_selected_rust_tests(catalog, selected, execution, verify.CLI_NATIVE_TESTS)
        self.assertEqual(selection["catalog_test_count"], 5)
        self.assertEqual(selection["expected_filtered_out"], 2)
        for wrong in (0, 1, 3):
            with self.subTest(filtered=wrong), self.assertRaises(ValueError):
                contracts.validate_selected_rust_tests(catalog, selected,
                    execution.replace(b"2 filtered out", f"{wrong} filtered out".encode()), verify.CLI_NATIVE_TESTS)

    def test_catalog_receipt_binds_full_raw_listing_command_and_count(self):
        with tempfile.TemporaryDirectory(prefix="oxid-enum-native-catalog-control-") as temporary:
            directory = Path(temporary)
            for suffix, body in (("json", b'{"exit_status": 0}\n'),
                                 ("stdout", b"fixture::test: test\n\n1 test, 0 benchmarks\n"),
                                 ("stderr", b"")):
                (directory / ("catalog." + suffix)).write_bytes(body)
            selection = {"catalog_test_count": 1, "selected_tests": ["fixture::test"],
                         "expected_filtered_out": 0, "execution_scope": "control fixture only"}
            receipt = contracts.catalog_receipt(directory, "catalog", selection)
            self.assertEqual(receipt["catalog_test_count"], 1)
            self.assertEqual(receipt["stdout"], contracts.identity(directory / "catalog.stdout"))
            self.assertEqual(receipt["command"], contracts.identity(directory / "catalog.json"))
            (directory / "catalog.stdout").write_bytes(b"tampered listing\n")
            self.assertNotEqual(contracts.catalog_receipt(directory, "catalog", selection), receipt)

    def test_optional_generated_case_skips_explicitly_but_mandatory_ci_case_fails(self):
        case = contracts.NativeOwnedEnumVectors("test_generated_schemas_from_the_real_rust_compiler")
        with patch.dict(os.environ, {}, clear=True):
            with self.assertRaisesRegex(unittest.SkipTest, "real Rust schema producer manifest absent"):
                case.test_generated_schemas_from_the_real_rust_compiler()
        with patch.dict(os.environ, {contracts.REQUIRE_GENERATED: "1"}, clear=True):
            with self.assertRaisesRegex(AssertionError, "require the real Rust producer manifest"):
                case.test_generated_schemas_from_the_real_rust_compiler()

    def test_source_closure_copies_real_relative_includes_and_refuses_escaping_headers(self):
        with tempfile.TemporaryDirectory(prefix="oxid-enum-native-source-control-") as temporary:
            parent = Path(temporary)
            repo = parent / "repo"
            (repo / "tests").mkdir(parents=True)
            (repo / "native").mkdir()
            harness = repo / contracts.HARNESS
            harness.write_bytes(b'#include "../native/runtime.c"\nint main(void) { return 0; }\n')
            runtime = repo / "native/runtime.c"
            runtime.write_bytes(b"int actual_runtime;\n")
            self.assertEqual(contracts.capture_sources(repo), {
                contracts.HARNESS: harness.read_bytes(), "native/runtime.c": runtime.read_bytes()})
            (parent / "foreign.c").write_bytes(b"int foreign;\n")
            harness.write_bytes(b'#include "../../foreign.c"\n')
            with self.assertRaisesRegex(ValueError, "escapes checkout"):
                contracts.capture_sources(repo)

    def test_generated_manifest_rejects_stale_source_instead_of_accepting_a_handwritten_schema_path(self):
        with tempfile.TemporaryDirectory(prefix="oxid-enum-native-manifest-control-") as temporary:
            directory = Path(temporary)
            snapshot = {"head": "a" * 40, "tree": "b" * 40}
            manifest = directory / "result.json"
            for invalid in (
                {"status": "passed", "output": {"path": str(directory / "handwritten.ll")}},
                {"schema": "oxid-owned-enum-vector-schema-producer-v1", "status": "passed",
                 "profile": "release", "producer_test": contracts.PRODUCER_TEST,
                 "repository": {"head": "c" * 40, "tree": "b" * 40}},
                {"schema": "oxid-owned-enum-vector-schema-producer-v1", "status": "skipped",
                 "profile": "debug", "producer_test": contracts.PRODUCER_TEST, "repository": snapshot},
            ):
                manifest.write_text(json.dumps(invalid))
                with self.subTest(invalid=invalid), self.assertRaisesRegex(ValueError, "successful current-source Rust producer receipt"):
                    contracts.load_generated_manifest(manifest, directory, snapshot)

    def test_ci_binds_current_head_and_requires_both_linux_hosts_and_retained_evidence(self):
        body = (ROOT / ".github/workflows/ci.yml").read_text()
        self.assertEqual(body.count("python3 -B scripts/verify_native_owned_enum_vectors.py"), 2)
        for start, end, output in (
            ("  native-projects:", "  native-numeric-and-packages:", "current-owned-enum-vector-native"),
            ("  linux-arm64-native:", "  workflow-lint:", "linux-arm64-native/owned-enum-vectors"),
        ):
            selected = body.split(start, 1)[1].split(end, 1)[0]
            self.assertIn('EXPECTED_HEAD: ${{ github.event.pull_request.head.sha || github.sha }}', selected)
            self.assertIn('--repo . --expected-head "$EXPECTED_HEAD"', selected)
            self.assertIn('--output "$RUNNER_TEMP/' + output + '"', selected)
            self.assertIn('--llvm-bin "$OXID_LLVM_BIN"', selected)
            self.assertNotIn('continue-on-error:', selected)
        self.assertIn('name: current-owned-enum-vector-native\n', body)
        self.assertEqual(verify.CLI_NATIVE_TESTS, (
            "native_builtin_enum_vectors_keep_final_ids_after_source_free_relocation",
            "native_enum_vector_record_fields_and_memory_limit_recover_after_source_free_relocation",
            "native_owned_enum_token_queue_runs_after_source_free_relocation"))


if __name__ == "__main__":
    unittest.main()
