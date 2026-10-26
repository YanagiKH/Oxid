#!/usr/bin/env python3
"""Check scalar vectors, borrowing failures and source-free native execution."""
import argparse
import hashlib
import json
import os
import platform
import resource
import struct
import sys
from pathlib import Path
import subprocess

from verify_float_vector import main_for_package
from verify_owned_source import parse_records

ROOT = Path(__file__).resolve().parents[1]
EXAMPLE = Path("examples/typed-vectors")
FIXTURES = Path("tests/fixtures/typed_vectors")


def digest(path):
    with Path(path).open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def macos_native_limits():
    # Mach-O executables map the system shared cache outside the program heap.
    # Runtime heap accounting still applies; bound CPU time independently.
    resource.setrlimit(resource.RLIMIT_CPU, (15, 15))


def native_identity(path):
    with Path(path).open("rb") as stream:
        header = stream.read(32)
    machine = platform.machine().lower()
    if sys.platform == "linux":
        expected = {"x86_64": 62, "aarch64": 183}.get(machine)
        if (expected is None or len(header) != 32 or header[:6] != b"\x7fELF\x02\x01"
                or struct.unpack_from("<H", header, 16)[0] not in (2, 3)
                or struct.unpack_from("<H", header, 18)[0] != expected):
            raise ValueError("native output is not a host ELF executable")
        return {"format": "ELF64", "machine": machine}
    if sys.platform == "darwin":
        expected = {"x86_64": 0x01000007, "arm64": 0x0100000c}.get(machine)
        if (expected is None or len(header) != 32 or header[:4] != b"\xcf\xfa\xed\xfe"
                or struct.unpack_from("<I", header, 4)[0] != expected
                or struct.unpack_from("<I", header, 12)[0] != 2):
            raise ValueError("native output is not a host Mach-O executable")
        return {"format": "Mach-O64", "machine": machine}
    raise ValueError("native vector verification requires Linux or macOS")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("compilers", nargs="+", type=Path)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--llvm-bin", type=Path)
    parser.add_argument("--reference-only", action="store_true")
    args = parser.parse_args()
    manifest = json.loads((ROOT / FIXTURES / "cases.json").read_text())
    if manifest.get("schema") != 1:
        raise ValueError("unknown vector fixture schema")
    rows = manifest["cases"]
    accepted = [row for row in rows if row["status"] == "accept"]
    rejected = [row for row in rows if row["status"] == "reject"]
    types = {"bool", "unit", "char", "f32", "f64", "i8", "i16", "i32", "i64", "i128",
             "isize", "u8", "u16", "u32", "u64", "u128", "usize"}
    if (len(rows) != 27 or {row["name"] for row in accepted} != types
            or len(accepted) != 17 or len(rejected) != 10
            or len({row["name"] for row in rejected}) != 10):
        raise ValueError("incomplete scalar vector fixtures")
    scenarios = manifest.get("scenarios", [])
    expected_scenarios = {"projected-vector": 85, "dynamic-range": 32, "array-ranges": 205}
    if (len(scenarios) != 3 or {row["name"]: row["expected_result"] for row in scenarios}
            != expected_scenarios):
        raise ValueError("incomplete typed range scenarios")
    sources = [EXAMPLE / "oxid.toml", EXAMPLE / "src/main.ox", EXAMPLE / "tests/vectors.ox",
               FIXTURES / "cases.json"]
    for row in rows + scenarios:
        path = Path(row["source"])
        if path.is_absolute() or ".." in path.parts or not path.is_relative_to(FIXTURES):
            raise ValueError("fixture escaped its source directory")
        if digest(ROOT / path) != row["sha256"]:
            raise ValueError("vector fixture changed: " + row["name"])
        sources.append(path)
    cases = [("example", (ROOT / EXAMPLE / "src/main.ox").read_text(), b"42\n"),
             ("contracts", (ROOT / EXAMPLE / "tests/vectors.ox").read_text(), b"0\n")]
    for row in accepted:
        if type(row.get("expected_result")) is not int or row["expected_result"] != 42:
            raise ValueError("changed literal scalar oracle")
        cases.append((row["name"], (ROOT / row["source"]).read_text(), b"42\n"))
    for row in scenarios:
        cases.append((row["name"], (ROOT / row["source"]).read_text(),
                      (str(row["expected_result"]) + "\n").encode()))
    main_for_package(ROOT, EXAMPLE, tuple(sources), "typed-vectors-example", "vectors", cases,
                     {"scalar_types": 17, "public_operations": 9, "range_scenarios": 3},
                     __file__, description=__doc__, finalize=False,
                     native_guard=macos_native_limits if sys.platform == "darwin" else None,
                     warm_cache_status="unavailable" if sys.platform == "darwin" else "hit",
                     policy_equivalent_cases=("array-ranges",))
    output = args.output.resolve()
    receipt_path = output / "result.json"
    receipt = json.loads(receipt_path.read_text())
    if receipt.get("status") != "positive-phase-passed":
        raise ValueError("positive vector verification is incomplete")
    receipt["native_executables"] = receipt.pop("native_elfs")
    receipt["native_process_limits"] = (None if args.reference_only else (
        {"cpu_seconds": 15, "command_timeout_seconds": 90}
        if sys.platform == "darwin" else
        {"address_space_bytes": 32 * 1024 * 1024, "command_timeout_seconds": 90}))
    receipt["native_cache_expected_repeated_status"] = (
        None if args.reference_only else "unavailable" if sys.platform == "darwin" else "hit")
    receipt["positive_commands"] = receipt["commands"]
    receipt.update(status="running", negative_cases=0)
    receipt_path.write_text(json.dumps(receipt, indent=2) + "\n")
    transcript = output / "negative-commands.jsonl"
    environment = dict(os.environ, OXID_CACHE_DIR=str(output / "negative-cache"))
    environment.pop("OXID_PATH", None)
    try:
        for executable in receipt["native_executables"]:
            executable.update(native_identity(executable["path"]))
        with transcript.open("x") as log:
            for compiler_index, compiler in enumerate(args.compilers):
                compiler = compiler.resolve(strict=True)
                for row in rejected:
                    project = output / "negative-projects" / str(compiler_index) / row["name"]
                    project.mkdir(parents=True)
                    (project / "main.ox").write_bytes((ROOT / row["source"]).read_bytes())
                    (project / "oxid.toml").write_text(
                        '[project]\nname = "vector-rejection"\nedition = "typed-preview"\n'
                        'entry = "main.ox"\n[execution]\npolicy = "hosted"\n')
                    argv = [str(compiler), "check", "--message-format=json"]
                    completed = subprocess.run(argv, cwd=project, env=environment,
                                               stdin=subprocess.DEVNULL, stdout=subprocess.PIPE,
                                               stderr=subprocess.PIPE, timeout=90)
                    log.write(json.dumps({"name": row["name"], "argv": argv,
                                          "status": completed.returncode,
                                          "stdout_hex": completed.stdout.hex(),
                                          "stderr_hex": completed.stderr.hex()}) + "\n")
                    log.flush()
                    receipt["commands"] += 1
                    records = parse_records(completed, "check")
                    if completed.returncode != 1 or records[-1]["success"] is not False:
                        raise ValueError("unsafe vector program was not rejected: " + row["name"])
                    expected = {(item["stage"], item["code"])
                                for item in row["expected_diagnostics"]}
                    if not expected or any((record["stage"], record["code"]) not in expected
                                           for record in records[:-1]):
                        raise ValueError("unexpected vector refusal: " + row["name"])
                    receipt["negative_cases"] += 1
        for pin in receipt["compilers"]:
            if digest(pin["path"]) != pin["sha256"]:
                raise ValueError("compiler changed during negative verification")
        for name, expected in receipt["sources"].items():
            if digest(ROOT / name) != expected:
                raise ValueError("source changed during negative verification")
        receipt["status"] = "passed"
    except BaseException as error:
        receipt.update(status="failed", failure=f"{type(error).__name__}: {error}")
        raise
    finally:
        if transcript.exists():
            receipt["negative_commands_sha256"] = digest(transcript)
        receipt_path.write_text(json.dumps(receipt, indent=2) + "\n")
    print(json.dumps({key: value for key, value in receipt.items()
                      if key not in {"native_executables", "sources"}}, sort_keys=True))


if __name__ == "__main__":
    main()
