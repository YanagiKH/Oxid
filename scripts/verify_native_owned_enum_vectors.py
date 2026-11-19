#!/usr/bin/env python3
"""Produce real debug/release enum schemas and run both C runtime modes.

Runs actual Rust test producers, then strict O0/O2 C harnesses with and without
ASan/UBSan. Every source, schema, command, raw stream and binary remains outside
the source tree. This is Linux x86_64/ARM64 acceptance, not a whole-v1 claim.
"""
import argparse
import json
import os
from pathlib import Path
import platform
import re
import sys

import test_native_owned_enum_vectors as contracts


CLI_NATIVE_TESTS = (
    "native_builtin_enum_vectors_keep_final_ids_after_source_free_relocation",
    "native_enum_vector_record_fields_and_memory_limit_recover_after_source_free_relocation",
    "native_owned_enum_token_queue_runs_after_source_free_relocation",
)


def run_cli_cases(repo, output, profile, cargo, environment):
    directory = output / ("native-cli-" + profile)
    directory.mkdir()
    command = [cargo, "test", "--locked"]
    if profile == "release":
        command.append("--release")
    command += ["--test", "typed_owned_enum_vectors"]
    catalog = contracts.retained_command(directory, "catalog", command + ["--", "--list", "--color", "never"],
                                         environment, 1800, repo)
    contracts.require(catalog.returncode == 0, "cannot list the complete native CLI integration test catalog")
    command += ["--", "--ignored"]
    listing = contracts.retained_command(directory, "list", command + ["--list", "--color", "never"], environment, 1800, repo)
    expected = ("\n".join(name + ": test" for name in CLI_NATIVE_TESTS) + "\n\n3 tests, 0 benchmarks\n").encode()
    contracts.require(listing.returncode == 0 and listing.stdout == expected,
                      "native CLI acceptance must list exactly its three real ignored cases")
    result = contracts.retained_command(directory, "run",
        command + ["--test-threads=1", "--nocapture", "--color", "never"], environment, 1800, repo)
    contracts.require(result.returncode == 0, "native CLI acceptance failed")
    selection = contracts.validate_selected_rust_tests(catalog.stdout, listing.stdout, result.stdout, CLI_NATIVE_TESTS)
    return {"profile": profile, "test_names": CLI_NATIVE_TESTS,
            "catalog": contracts.catalog_receipt(directory, "catalog", selection),
            "run_command": contracts.identity(directory / "run.json")}


def run_c_case(repo, output, label, generated, environment, sanitize):
    selected = output / label
    selected.mkdir()
    command_environment = dict(environment)
    command_environment["OXID_TEST_OWNED_ENUM_NATIVE_EVIDENCE"] = str(selected / "evidence")
    command_environment["OXID_BYTEBUF_SANITIZE"] = "1" if sanitize else "0"
    command_environment[contracts.REQUIRE_GENERATED] = "1"
    if generated:
        command_environment[contracts.GENERATED_MANIFEST] = str(generated)
        name = "test_generated_schemas_from_the_real_rust_compiler"
        mode = "generated"
    else:
        command_environment.pop(contracts.GENERATED_MANIFEST, None)
        name = "test_adversarial_schemas_and_exact_ownership_boundaries"
        mode = "adversarial"
    command = [sys.executable, "-B", repo / "scripts/test_native_owned_enum_vectors.py",
               "NativeOwnedEnumVectors." + name, "-v"]
    result = contracts.retained_command(selected, "python-contract", command, command_environment, 900, repo)
    contracts.require(result.returncode == 0 and result.stderr.endswith(b"\nOK\n")
                      and re.search(rb"\nRan 1 test in [0-9.]+s\n", result.stderr) is not None,
                      "mandatory C contract did not execute its real test; inspect " + str(selected))
    path = selected / "evidence" / mode / "result.json"
    report = json.loads(contracts.regular(path))
    contracts.require(report.get("status") == "passed" and report.get("mode") == mode
                      and report.get("sanitizer") is sanitize
                      and [row.get("optimization") for row in report.get("configurations", [])] == ["O0", "O2"],
                      "mandatory C contract lacks both actual O0/O2 results")
    return {"label": label, "mode": mode, "sanitizer": sanitize, "result": contracts.identity(path)}


def run(args):
    repo = args.repo.resolve(strict=True)
    output = args.output.resolve()
    contracts.require(contracts.SUPPORTED_HOST and platform.system() == "Linux",
                      "mandatory enum-schema acceptance requires Linux x86_64/ARM64")
    contracts.require(repo == contracts.ROOT and args.expected_head
                      and re.fullmatch("[a-f0-9]{40}", args.expected_head), "run against an exact current source head")
    contracts.require(output != repo and repo not in output.parents and not output.exists(),
                      "output must be new outside the source checkout")
    output.mkdir(parents=True)
    environment = dict(os.environ)
    environment["OXID_TEST_OWNED_ENUM_LLVM_BIN"] = str(args.llvm_bin.resolve(strict=True))
    report = {"schema": "oxid-owned-enum-vector-native-acceptance-v1", "status": "running",
              "expected_head": args.expected_head, "producers": [], "c_contracts": [], "native_cli": [],
              "whole_v1_qualified": False, "platform": platform.platform(),
              "driver_sources": [contracts.identity(repo / "scripts" / name) for name in
                                 ("verify_native_owned_enum_vectors.py", "test_native_owned_enum_vectors.py")]}
    try:
        snapshot = contracts.repository_snapshot(repo, output, "repository-before", environment)
        contracts.require(snapshot["head"] == args.expected_head, "current source head differs from expected head")
        clean = contracts.retained_command(output, "tracked-source-clean-before",
                                           ["git", "diff", "HEAD", "--quiet"], environment, 30, repo)
        contracts.require(clean.returncode == 0 and not clean.stdout and not clean.stderr,
                          "schema acceptance requires committed tracked sources")
        report["repository"] = snapshot
        for sanitize in (False, True):
            label = "adversarial-" + ("sanitized" if sanitize else "ordinary")
            report["c_contracts"].append(run_c_case(repo, output, label, None, environment, sanitize))
        for profile in ("debug", "release"):
            produced = output / ("producer-" + profile)
            produced.mkdir()
            schema = produced / "compiler-schemas.ll"
            contracts.require(not schema.exists(), "real Rust schema output must be new")
            producer_environment = dict(environment)
            producer_environment["OXID_TEST_OWNED_ENUM_SCHEMA_OUTPUT"] = str(schema)
            arguments = [args.cargo, "test", "--locked"]
            if profile == "release":
                arguments.append("--release")
            arguments += ["--bin", "oxid"]
            catalog = contracts.retained_command(produced, "producer-catalog",
                arguments + ["--", "--list", "--color", "never"], producer_environment, 1800, repo)
            contracts.require(catalog.returncode == 0 and not schema.exists(),
                              "real complete Rust unit catalog failed or unexpectedly executed a producer")
            arguments += [contracts.PRODUCER_TEST, "--", "--exact"]
            listing = contracts.retained_command(produced, "producer-list", arguments + ["--list", "--color", "never"],
                                                  producer_environment, 1800, repo)
            contracts.require(listing.returncode == 0 and not schema.exists(),
                              "real Rust schema listing failed or unexpectedly executed the producer")
            execution = contracts.retained_command(produced, "producer-run",
                arguments + ["--nocapture", "--test-threads=1", "--color", "never"], producer_environment, 1800, repo)
            contracts.require(execution.returncode == 0, "real Rust schema producer failed")
            selection = contracts.validate_producer_streams(catalog.stdout, listing.stdout, execution.stdout)
            producer = {"schema": "oxid-owned-enum-vector-schema-producer-v1", "status": "passed",
                        "profile": profile, "producer_test": contracts.PRODUCER_TEST,
                        "repository": snapshot, "output": contracts.identity(schema),
                        "catalog": contracts.catalog_receipt(produced, "producer-catalog", selection),
                        "source_files": {name: contracts.identity(repo / name)
                                         for name in (contracts.PRODUCER, contracts.EMITTER)}}
            manifest = produced / "result.json"
            manifest.write_text(json.dumps(producer, indent=2) + "\n")
            contracts.load_generated_manifest(manifest, repo, snapshot)
            report["producers"].append({"profile": profile, "manifest": contracts.identity(manifest)})
            for sanitize in (False, True):
                label = "generated-" + profile + ("-sanitized" if sanitize else "-ordinary")
                report["c_contracts"].append(run_c_case(repo, output, label, manifest, environment, sanitize))
            report["native_cli"].append(run_cli_cases(repo, output, profile, args.cargo, environment))
        contracts.require(contracts.repository_snapshot(repo, output, "repository-after", environment) == snapshot,
                          "source commit/tree changed during schema acceptance")
        clean = contracts.retained_command(output, "tracked-source-clean-after",
                                           ["git", "diff", "HEAD", "--quiet"], environment, 30, repo)
        contracts.require(clean.returncode == 0 and not clean.stdout and not clean.stderr,
                          "tracked source bytes changed during schema acceptance")
        contracts.require(report["driver_sources"] == [contracts.identity(repo / "scripts" / name) for name in
                              ("verify_native_owned_enum_vectors.py", "test_native_owned_enum_vectors.py")],
                          "native acceptance driver changed during execution")
        report["status"] = "passed"
        return 0
    except (Exception, KeyboardInterrupt) as error:
        report.update(status="failed", error=f"{type(error).__name__}: {error}")
        print(report["error"], file=sys.stderr)
        return 1
    finally:
        (output / "result.json").write_text(json.dumps(report, indent=2) + "\n")
        print("retained enum-vector native acceptance: " + str(output), flush=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo", type=Path, required=True)
    parser.add_argument("--expected-head", required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--llvm-bin", type=Path, required=True)
    parser.add_argument("--cargo", default="cargo")
    return run(parser.parse_args())


if __name__ == "__main__":
    raise SystemExit(main())
