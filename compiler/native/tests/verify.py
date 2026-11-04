#!/usr/bin/env python3
"""Run actual scalar-compiler, Clang, differential and deployment acceptance.

The reference compiler is an offline oracle only; it is not passed to
the Oxid compiler or build driver. No self-rebuild qualification is inferred.
"""
from __future__ import annotations

import argparse
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import shutil
import subprocess

ROOT = Path(__file__).resolve().parents[1]
SPEC = importlib.util.spec_from_file_location("scalar_build", ROOT / "build.py")
driver = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(driver)
INPUT_SPEC = importlib.util.spec_from_file_location(
    "scalar_compiler_fixture_data", ROOT.parents[1] / "scripts/verify_native_compiler_fixture_data.py")
fixture_data = importlib.util.module_from_spec(INPUT_SPEC)
INPUT_SPEC.loader.exec_module(fixture_data)


def require(condition: bool, message: str) -> None:
    if not condition:
        raise RuntimeError(message)


def record(directory: Path, name: str, command: list[str], result) -> dict:
    (directory / (name + ".stdout")).write_bytes(result.stdout)
    (directory / (name + ".stderr")).write_bytes(result.stderr)
    row = {"command": command, "status": result.returncode,
           "stdout_sha256": hashlib.sha256(result.stdout).hexdigest(),
           "stderr_sha256": hashlib.sha256(result.stderr).hexdigest()}
    (directory / (name + ".json")).write_text(json.dumps(row, indent=2) + "\n")
    return row


def run(command: list[str], cwd: Path, timeout: int, clear_environment: bool = True):
    return subprocess.run(command, cwd=cwd, stdin=subprocess.DEVNULL,
                          stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                          timeout=timeout, check=False,
                          env={} if clear_environment else None)


# Emitted native bytes come from emit.ox; reference tuples come from the
# existing frontend/oir/mod.rs runtime diagnostic contract, not source paths.
RUNTIME_EXPECTATIONS = {
    "overflow": (b"integer overflow\n", "E0604", "checked i32 arithmetic overflow"),
    "division by zero": (b"division by zero\n", "E0607", "checked i32 division by zero"),
}


def runtime_expectation(case: dict):
    family = case.get("error")
    require(type(family) is str and family in RUNTIME_EXPECTATIONS,
            case["name"] + ": unknown checked runtime failure family")
    return RUNTIME_EXPECTATIONS[family]


def check_native_behavior(case: dict, result) -> None:
    if case["kind"] == "value":
        require(result.returncode == 0 and result.stderr == b""
                and result.stdout == case["stdout"].encode(), case["name"] + ": wrong value")
    elif case["kind"] == "runtime":
        expected, _, _ = runtime_expectation(case)
        require(result.returncode == 1 and result.stdout == b""
                and result.stderr == expected,
                case["name"] + ": wrong checked runtime failure")
    else:
        require(False, case["name"] + ": no native executable is expected for this case")


def unique_reference_object(pairs):
    require(len({key for key, _ in pairs}) == len(pairs), "duplicate reference diagnostic field")
    return dict(pairs)


def reference_runtime_records(result):
    require(result.returncode == 1 and result.stderr == b"",
            "reference runtime failure requires status1 and a clean JSON channel")
    lines = result.stdout.split(b"\n")
    require(len(lines) == 3 and lines[-1] == b""
            and all(line.startswith(b"{") and line.endswith(b"}") for line in lines[:2])
            and all(byte >= 32 for line in lines[:2] for byte in line),
            "reference runtime failure requires exactly two complete JSON records")
    try:
        return [json.loads(line.decode("utf-8"), object_pairs_hook=unique_reference_object)
                for line in lines[:2]]
    except (ValueError, UnicodeError) as error:
        raise RuntimeError("invalid reference runtime diagnostic JSON") from error


def reference_primary(value) -> bool:
    if type(value) is not dict or set(value) != {
        "file_id", "path", "start", "end", "line", "column", "end_line", "end_column",
    }:
        return False
    integers = ("file_id", "start", "end", "line", "column", "end_line", "end_column")
    return (all(type(value[key]) is int for key in integers)
            and value["file_id"] >= 0 and 0 <= value["start"] <= value["end"]
            and 1 <= value["line"] <= value["end_line"]
            and value["column"] >= 1 and value["end_column"] >= 1
            and (value["line"] != value["end_line"] or value["column"] <= value["end_column"])
            and type(value["path"]) is str and bool(value["path"])
            and not any(ord(char) < 32 or 127 <= ord(char) < 160 for char in value["path"]))


def check_reference_behavior(case: dict, result) -> None:
    if case["kind"] == "value":
        check_native_behavior(case, result)
    elif case["kind"] == "runtime":
        _, code, message = runtime_expectation(case)
        diagnostic, summary = reference_runtime_records(result)
        require(type(diagnostic) is dict and set(diagnostic) == {
            "schema_version", "edition", "kind", "severity", "code", "stage", "message",
            "primary", "secondary", "notes",
        } and type(diagnostic["schema_version"]) is int and diagnostic["schema_version"] == 1
                and diagnostic["edition"] == "typed-preview" and diagnostic["kind"] == "diagnostic"
                and diagnostic["severity"] == "error" and diagnostic["code"] == code
                and diagnostic["stage"] == "oir-run" and diagnostic["message"] == message
                and reference_primary(diagnostic["primary"])
                and diagnostic["secondary"] == [] and diagnostic["notes"] == [],
                case["name"] + ": wrong reference runtime diagnostic code, stage or family")
        require(type(summary) is dict and set(summary) == {
            "schema_version", "edition", "kind", "success", "errors", "result",
        } and type(summary["schema_version"]) is int and summary["schema_version"] == 1
                and summary["edition"] == "typed-preview" and summary["kind"] == "run-summary"
                and summary["success"] is False and type(summary["errors"]) is int
                and summary["errors"] == 1 and summary["result"] is None,
                case["name"] + ": wrong reference failed-run summary")
    else:
        require(case["kind"] == "reject" and result.returncode != 0,
                case["name"] + ": reference accepted invalid source")


def source_for(case: dict) -> bytes:
    if "file" in case:
        return (ROOT / "tests/fixtures" / case["file"]).read_bytes()
    return case["source"].encode()


def actual_protocol(compiler: Path, clang: Path, runtime: Path,
                    output: Path, timeout: int) -> list[dict]:
    evidence = output / "actual-protocol"
    evidence.mkdir()
    source = evidence / "valid.ox"
    source.write_text("fn main()->i32{return 42;}\n")
    sentinel = evidence / "existing"
    sentinel.write_bytes(b"original\x00bytes\n")
    original = sentinel.stat()
    try:
        driver.build(compiler, source, clang, runtime, sentinel, "dev", timeout)
    except driver.BuildError as error:
        require("already exists" in str(error), "existing output failed for an unrelated reason")
    else:
        raise RuntimeError("build replaced an existing output")
    require(sentinel.read_bytes() == b"original\x00bytes\n"
            and sentinel.stat().st_ino == original.st_ino,
            "existing output bytes/inode changed")

    missing = evidence / "absent.ox"
    missing_command = [str(compiler), str(missing)]
    absent = run(missing_command, evidence, timeout)
    require(absent.returncode == 66 and absent.stdout == b"", "missing input did not fail without LLVM")
    rows = [record(evidence, "missing-input", missing_command, absent)]

    # This uses the real compiler and real Clang, but an invalid C runtime input
    # deliberately forces linking/compilation failure after accepted LLVM.
    broken_runtime = evidence / "invalid-runtime.c"
    broken_runtime.write_text("this is not a C translation unit\n")
    failed_destination = evidence / "clang-failure-output"
    try:
        driver.build(compiler, source, clang, broken_runtime, failed_destination,
                     "release", timeout, evidence / "clang-failure")
    except driver.BuildError as error:
        require("Clang rejected" in str(error), "Clang failure control did not reach Clang")
    else:
        raise RuntimeError("invalid runtime unexpectedly linked")
    require(not os.path.lexists(failed_destination), "Clang failure published output")

    # Process entry mode installs SIGPIPE handling before source delivery. A
    # real failed stdout write must return74 rather than print a main result,
    # report success, or terminate with SIGPIPE.
    reader, writer = os.pipe()
    os.close(reader)
    command = [str(compiler), str(source)]
    try:
        child = subprocess.Popen(command, cwd=evidence, stdin=subprocess.DEVNULL,
                                 stdout=writer, stderr=subprocess.PIPE, env={})
    finally:
        os.close(writer)
    _, stderr = child.communicate(timeout=timeout)
    require(child.returncode == 74 and stderr == b"", "closed stdout did not return process status74")
    pipe_result = subprocess.CompletedProcess(command, child.returncode, b"", stderr)
    rows.append(record(evidence, "closed-stdout", command, pipe_result))

    too_large = evidence / "too-large.ox"
    too_large.write_bytes(b" " * 65537)
    large_command = [str(compiler), str(too_large)]
    large_result = run(large_command, evidence, timeout)
    require(large_result.returncode == 68 and large_result.stdout == b"", "input byte limit was not enforced")
    rows.append(record(evidence, "input-limit", large_command, large_result))
    return rows


def verify(args) -> dict:
    cases, authenticated_sources = fixture_data.load_cases(ROOT.parents[1])
    output = args.output.resolve()
    require(not output.exists(), "evidence directory must be new")
    require(output != ROOT and ROOT not in output.parents, "evidence must be outside compiler source")
    output.mkdir(parents=True)
    deploy = output / "deployed"
    deploy.mkdir()
    compiler = deploy / "compiler"
    shutil.copy2(args.compiler.resolve(strict=True), compiler)
    require(os.access(compiler, os.X_OK), "copied compiler is not executable")
    clang = args.clang.resolve(strict=True)
    runtime = args.runtime.resolve(strict=True)
    report = {"status": "running", "self_hosting": False, "cases": [],
              "compiler_sha256": hashlib.sha256(compiler.read_bytes()).hexdigest(),
              "runtime_sha256": hashlib.sha256(runtime.read_bytes()).hexdigest(),
              "driver_sha256": hashlib.sha256((ROOT / "build.py").read_bytes()).hexdigest(),
              "cases_sha256": hashlib.sha256((ROOT / "tests/cases.json").read_bytes()).hexdigest()}
    (output / "retained-inputs").mkdir()
    try:
        require(report["cases_sha256"] == fixture_data.CASES_JSON[2],
                "scalar compiler case manifest changed after admission")
        require(len({case["name"] for case in cases}) == len(cases), "duplicate case name")
        for case in cases:
            work = output / ("work-" + case["name"])
            work.mkdir()
            evidence = output / ("evidence-" + case["name"])
            evidence.mkdir()
            source = work / "main.ox"
            source_bytes = source_for(case)
            require(source_bytes == authenticated_sources[case["name"]],
                    "scalar compiler subject input changed after admission")
            source.write_bytes(source_bytes)
            (evidence / "source.ox").write_bytes(source_bytes)
            row = {"name": case["name"], "kind": case["kind"],
                   "source_sha256": hashlib.sha256(source_bytes).hexdigest(), "profiles": []}

            command = [str(compiler), str(source)]
            compiled = run(command, work, args.timeout)
            row["compiler"] = record(evidence, "compiler", command, compiled)
            if case["kind"] == "reject":
                require(compiled.returncode == 65 and compiled.stdout == b"" and compiled.stderr == b"",
                        case["name"] + ": rejected source leaked output or wrong status")
                rejected_destination = deploy / (case["name"] + "-rejected")
                try:
                    driver.build(compiler, source, clang, runtime, rejected_destination, "dev", args.timeout)
                except driver.BuildError:
                    pass
                else:
                    raise RuntimeError(case["name"] + ": invalid source built")
                require(not os.path.lexists(rejected_destination), "rejected source published an artifact")
            else:
                driver.module_from_result(compiled)
                for profile in ("dev", "release"):
                    binary = deploy / (case["name"] + "-" + profile)
                    receipt = driver.build(compiler, source, clang, runtime, binary,
                                           profile, args.timeout, evidence / profile)
                    require(receipt["module_sha256"] == hashlib.sha256(compiled.stdout).hexdigest(),
                            "repeated compilation changed LLVM bytes")
                    row["profiles"].append(receipt)

            if args.reference is not None and case["reference"]:
                reference = args.reference.resolve(strict=True)
                (work / "oxid.toml").write_text(
                    '[project]\nname="scalar-reference"\nedition="typed-preview"\nentry="main.ox"\n'
                    '[build]\nentry-mode="result"\n[execution]\npolicy="hosted"\n')
                reference_command = [str(reference), "run"]
                if case["kind"] == "runtime":
                    reference_command.append("--message-format=json")
                expected = run(reference_command, work, args.timeout, clear_environment=False)
                row["reference"] = record(evidence, "reference", reference_command, expected)
                check_reference_behavior(case, expected)

            # Preserve every generated input/cache byte by moving the whole
            # owned fixture, while making its original source path unavailable.
            retained = output / "retained-inputs" / case["name"]
            work.rename(retained)
            require(not work.exists(), "original compilation source path is still available")
            for receipt in row["profiles"]:
                native_command = [receipt["output"]]
                executed = run(native_command, deploy, args.timeout)
                receipt["source_unavailable_run"] = record(
                    evidence, "run-" + receipt["profile"], native_command, executed)
                check_native_behavior(case, executed)
            report["cases"].append(row)
        report["protocol"] = actual_protocol(compiler, clang, runtime, output, args.timeout)
        report["reference_used_only_as_offline_oracle"] = args.reference is not None
        report["status"] = "passed"
        return report
    except Exception as error:
        report.update(status="failed", error=f"{type(error).__name__}: {error}")
        raise
    finally:
        (output / "result.json").write_text(json.dumps(report, indent=2) + "\n")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--compiler", type=Path, required=True)
    parser.add_argument("--clang", type=Path, required=True)
    parser.add_argument("--runtime", type=Path, default=Path(__file__).resolve().parents[3] / "native/typed_preview.c")
    parser.add_argument("--reference", type=Path, required=True,
                        help="offline differential oracle; never invoked by the compiler or build driver")
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--timeout", type=int, default=120)
    args = parser.parse_args()
    try:
        report = verify(args)
    except (OSError, RuntimeError, subprocess.SubprocessError) as error:
        parser.exit(1, f"scalar compiler verification failed: {error}\n")
    print(json.dumps({"status": report["status"], "cases": len(report["cases"]), "self_hosting": False}))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
