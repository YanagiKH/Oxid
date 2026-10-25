#!/usr/bin/env python3
"""Verify current native programs on a real Linux ARM64 build host.

Historical certificates keep their original target and scope. This controller
records current compiler builds, public native tests and private LLVM controls.
"""
import argparse
import math
from pathlib import Path
import re

import linux_native_target as target
import verify_current_native as common


PUBLIC_TESTS = (
    "project_workflow", "typed_path_dependencies", "typed_package_lock",
    "typed_workspace", "typed_package", "native_incremental", "native_doctor",
    "typed_fixed_arrays", "typed_division", "typed_unary_negation", "typed_slices",
    "typed_projected_slices", "typed_record_composition", "typed_hir_import",
    "reproducible_native", "native_profiles", "owned_enum_payloads",
    "owned_enum_payload_acceptance", "byte_record_fields", "bytebuf_binary",
    "hosted_execution", "byte_buffer_transfer", "byte_strings", "native_files",
    "process_arguments", "typed_i64", "typed_integer_widths", "typed_byte_integers",
    "typed_integer128", "typed_integer_bits", "typed_floats", "typed_characters",
    "record_field_borrows", "typed_git_dependencies", "hosted_recursion",
    "typed_generics", "typed_nominal_generics", "typed_record_patterns",
    "typed_record_enum_fields", "typed_constructor_inference", "typed_range_for", "native_debug_info",
)


def public_results(body):
    summaries = re.findall(r"^test result: ok\. (\d+) passed; 0 failed; 0 ignored;", body, re.M)
    common.require(len(summaries) == len(PUBLIC_TESTS) and all(int(n) > 0 for n in summaries),
                   "a required public native test module is missing, empty or failed")
    return sum(int(n) for n in summaries)


def verify(args):
    common.require(__debug__, "qualification refuses optimized Python")
    host = target.required_host()
    common.require(host == target.ARM64, "this qualification requires an ARM64 Linux host")
    common.require(math.isfinite(args.timeout) and args.timeout > 0,
                   "timeout must be positive and finite")
    repo = args.repo.resolve(strict=True)
    common.require(Path(__file__).resolve() == repo / "scripts/verify_linux_arm64_native.py",
                   "run the controller from the selected checkout")
    output = common.fresh_output(repo, args.output)
    runner = common.Runner(repo, output, args.llvm_bin.resolve(strict=True), args.timeout)
    runner.environment["OXID_TEST_LLVM_BIN"] = str(args.llvm_bin.resolve(strict=True))
    report = {"schema": "oxid-linux-arm64-native-v1", "status": "running",
              "controller_sha256": common.digest(Path(__file__)),
              "target": host.triple, "elf_machine": host.elf_machine,
              "scope": "current sources on Linux ARM64; no reissue of historical certificates",
              "binaries": {}, "public_test_functions": {}}
    common.save(output / "result.json", report)
    try:
        head = runner.checkout("before")
        common.require(head == args.expected_head, "unexpected compiler source revision")
        report["head"] = head
        for profile in ("debug", "release"):
            flags = ["--release"] if profile == "release" else []
            build = runner.command(profile + "-build", ["cargo", "build", "--locked", "--bin", "oxid",
                                                         "--message-format=json", *flags])
            compiler = common.built_executable(build / "stdout", False)
            build = runner.command(profile + "-test-build", ["cargo", "test", "--locked", "--bin", "oxid",
                                                              "--no-run", "--message-format=json", *flags])
            binary = common.built_executable(build / "stdout", True)
            data = compiler.read_bytes()[:64]
            common.require(len(data) == 64 and data[:7] == b"\x7fELF\x02\x01\x01"
                           and data[18:20] == b"\xb7\x00", "compiler is not an ARM64 ELF")
            report["binaries"][profile] = {"compiler": str(compiler), "compiler_sha256": common.digest(compiler),
                                           "tests": str(binary), "tests_sha256": common.digest(binary)}
            argv = ["cargo", "test", "--locked", *flags]
            for name in PUBLIC_TESTS:
                argv += ["--test", name]
            selected = runner.command(profile + "-public-native", [*argv, "--", "--include-ignored", "--test-threads=1"])
            report["public_test_functions"][profile] = public_results((selected / "stdout").read_text())
            evidence = output / profile
            evidence.mkdir()
            for label, name in common.EXACT_TESTS:
                extra = {"OXID_REVIEWER_SOURCE_NATIVE_EVIDENCE": str(evidence / label)} if label == "heldout-source" else None
                runner.rust_tests(profile + "-" + label, binary, [name, "--ignored", "--exact"], extra, exact=name)
                if extra:
                    common.native_artifacts(evidence / label)
            for label, name, mode, variable in common.GROUP_TESTS:
                runner.rust_tests(profile + "-" + label, binary, [name, mode], {variable: str(evidence / label)})
                common.native_artifacts(evidence / label)
            runner.rust_tests(profile + "-composed-abi", binary,
                              ["record_enum_native_helpers_validate_active_cells", "--ignored"])
            runner.rust_tests(profile + "-retained-project", binary,
                              ["frontend::driver::tests::prepared_reproducible_native_uses_retained_paths_and_bytes_after_source_removal",
                               "--ignored", "--exact"],
                              exact="frontend::driver::tests::prepared_reproducible_native_uses_retained_paths_and_bytes_after_source_removal")
        common.require(runner.checkout("after") == head, "compiler checkout changed during qualification")
        for row in report["binaries"].values():
            common.require(common.digest(row["compiler"]) == row["compiler_sha256"]
                           and common.digest(row["tests"]) == row["tests_sha256"], "compiler changed during qualification")
        report["status"] = "passed"
        return 0
    except BaseException as error:
        report.update(status="failed", error=f"{type(error).__name__}: {error}")
        raise
    finally:
        report["commands"] = runner.commands
        common.save(output / "result.json", report)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo", type=Path, required=True)
    parser.add_argument("--expected-head", required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--llvm-bin", type=Path, required=True)
    parser.add_argument("--timeout", type=float, default=3600)
    return verify(parser.parse_args())


if __name__ == "__main__":
    raise SystemExit(main())
