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
import re
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


SOURCE_ERROR = b": error: invalid or unsupported Oxid source\n"
INPUT_ERROR = b"error: could not read Oxid source\n"
RESOURCE_ERROR = b"error: compiler resource limit exceeded\n"


def check_source_rejection(name, result, source: Path | None = None):
    path = re.escape(os.fsencode(source)) if source is not None else rb"[^\r\n]+"
    pattern = path + rb":[1-9][0-9]*:[1-9][0-9]*" + re.escape(SOURCE_ERROR)
    require(result.returncode == 65 and result.stdout == b"" and re.fullmatch(pattern, result.stderr),
            name + ": rejected source needs one location diagnostic and no LLVM")


# Emitted native bytes come from emit.ox; reference tuples come from the
# existing frontend/oir/mod.rs runtime diagnostic contract, not source paths.
RUNTIME_EXPECTATIONS = {
    "overflow": (b"integer overflow\n", "E0604", "checked i32 arithmetic overflow"),
    "division by zero": (b"division by zero\n", "E0607", "checked i32 division by zero"),
    "byte overflow": (b"integer overflow\n", "E0604", "checked u8 arithmetic overflow"),
    "byte division by zero": (b"division by zero\n", "E0607", "checked u8 division by zero"),
    "byte conversion": (b"byte conversion out of range\n", "E0610",
                        "checked i32 to u8 conversion out of range"),
    "i64 overflow": (b"integer overflow\n", "E0604", "checked i64 arithmetic overflow"),
    "i64 division by zero": (b"division by zero\n", "E0607", "checked i64 division by zero"),
    "i64 int conversion": (b"i32 conversion out of range\n", "E0610",
                           "checked i64 to i32 conversion out of range"),
    "i64 byte conversion": (b"byte conversion out of range\n", "E0610",
                            "checked i64 to u8 conversion out of range"),
    "array index": (b"array index out of bounds\n", "E0606", "array index out of bounds"),
    "shift count": (b"integer shift count must be nonnegative and below the bit width\n",
                    "E0611", "integer shift count must be nonnegative and below the bit width"),
    "u32 overflow": (b"integer overflow\n", "E0604", "checked u32 arithmetic overflow"),
    "u32 division by zero": (b"division by zero\n", "E0607", "checked u32 division by zero"),
    "i32 to u32": (b"u32 conversion out of range\n", "E0610",
                   "checked i32 to u32 conversion out of range"),
    "i64 to u32": (b"u32 conversion out of range\n", "E0610",
                   "checked i64 to u32 conversion out of range"),
    "u32 to i32": (b"i32 conversion out of range\n", "E0610",
                   "checked u32 to i32 conversion out of range"),
    "u32 to u8": (b"byte conversion out of range\n", "E0610",
                  "checked u32 to u8 conversion out of range"),
    "character conversion": (b"integer is outside the Unicode scalar range\n", "E0612",
                             "integer is outside the Unicode scalar range"),
}
NATIVE_DEPTH_ERROR = b"call depth limit exceeded\n"


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
        if case.get("error") == "call depth limit":
            expected = NATIVE_DEPTH_ERROR
        else:
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
        stage = "oir-owned-run" if case["error"] == "array index" else "oir-run"
        diagnostic, summary = reference_runtime_records(result)
        require(type(diagnostic) is dict and set(diagnostic) == {
            "schema_version", "edition", "kind", "severity", "code", "stage", "message",
            "primary", "secondary", "notes",
        } and type(diagnostic["schema_version"]) is int and diagnostic["schema_version"] == 1
                and diagnostic["edition"] == "typed-preview" and diagnostic["kind"] == "diagnostic"
                and diagnostic["severity"] == "error" and diagnostic["code"] == code
                and diagnostic["stage"] == stage and diagnostic["message"] == message
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
    target = driver.host_target()
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

    rows = []
    for selected in (driver.DEFAULT_TARGET, driver.ARM64_TARGET):
        command = [str(compiler), str(source), f"--target={selected}"]
        result = run(command, evidence, timeout)
        driver.module_from_result(result, selected)
        rows.append(record(evidence, selected, command, result))
    command = [str(compiler), str(source), "--target=unknown"]
    result = run(command, evidence, timeout)
    require(result.returncode == 64 and result.stdout == b"" and result.stderr.startswith(b"usage: "),
            "unsupported targets must fail before writing LLVM")
    rows.append(record(evidence, "invalid-target", command, result))

    missing = evidence / "absent.ox"
    missing_command = driver.compiler_command(compiler, missing, target)
    absent = run(missing_command, evidence, timeout)
    require((absent.returncode, absent.stdout, absent.stderr) == (66, b"", INPUT_ERROR),
            "missing input must report its error without LLVM")
    rows.append(record(evidence, "missing-input", missing_command, absent))

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
    command = driver.compiler_command(compiler, source, target)
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
    large_command = driver.compiler_command(compiler, too_large, target)
    large_result = run(large_command, evidence, timeout)
    require((large_result.returncode, large_result.stdout, large_result.stderr) == (68, b"", RESOURCE_ERROR),
            "input byte limit was not enforced")
    rows.append(record(evidence, "input-limit", large_command, large_result))

    nested = evidence / "nested"
    nested.mkdir()
    nested_entry = nested / "main.ox"
    nested_entry.write_text("mod outer;fn main()->i32{return 42;}\n")
    (nested / "outer.ox").write_text("mod inner;\n")
    (nested / "outer").mkdir()
    inner = nested / "outer/inner.ox"
    command = driver.compiler_command(compiler, nested_entry, target)
    for name, code, position in (
        ("nested-lexical-error", "// 雪\r\n  @\n", b"2:3"),
        ("nested-type-error", "fn read()->i32{\n  return true;\n}\n", b"2:10"),
        ("nested-source-eof", "fn read()->i32{\n  return 1;\n", b"3:1"),
    ):
        inner.write_text(code, encoding="utf-8")
        outcome = run(command, nested, timeout)
        check_source_rejection(name, outcome, inner)
        expected = os.fsencode(inner) + b":" + position + SOURCE_ERROR
        require(outcome.stderr == expected, name + ": incorrect module-relative position")
        rows.append(record(evidence, name, command, outcome))

    destination = nested / "rejected"
    try:
        driver.build(compiler, nested_entry, clang, runtime, destination, "dev", timeout)
    except driver.BuildError as error:
        require(f"{inner}:3:1" in str(error), "the build driver hid the compiler location")
    else:
        raise RuntimeError("rejected nested source was linked")
    require(not destination.exists(), "rejected nested source was published")

    reader, writer = os.pipe()
    os.close(reader)
    try:
        child = subprocess.Popen(command, cwd=nested, stdin=subprocess.DEVNULL,
                                 stdout=subprocess.PIPE, stderr=writer, env={})
    finally:
        os.close(writer)
    stdout, _ = child.communicate(timeout=timeout)
    require((child.returncode, stdout) == (65, b""), "closed stderr masked the source error")
    rows.append(record(evidence, "closed-stderr", command,
                       subprocess.CompletedProcess(command, child.returncode, stdout, b"")))

    # Keep each recursive call non-tail so -O0 retains all admitted frames.
    # One i32 parameter plus 4095 scalar locals fills the scalar slot budget.
    frame_cases = []
    for ty, suffix, prefix in (("i32", "", "full-frames"), ("i64", "i64", "full-i64-frames")):
        locals_source = "".join(f"let v{index}=0{suffix};" for index in range(4095))
        descend = (f"fn descend(n:i32)->{ty}{{" + locals_source
                   + f"if n==0{{return v4094;}}return 1{suffix}+descend(n-1);}}")
        frame_cases.append((ty, prefix, descend, f"descend(126)-84{suffix}"))
    # One parameter, 31 1024-byte arrays and one 1016-byte array use 4096 cells.
    arrays = "".join(f'let v{index}=b"' + "*" + "A" * (length - 1) + '";'
                     for index, length in enumerate([1024] * 31 + [1016]))
    byte_frames = ("fn descend(n:i32)->u8{" + arrays
                   + "if n==0{return v31[0];}return 1u8+descend(n-1);}")
    frame_cases.append(("u8", "full-byte-array-frames", byte_frames, "descend(126)-126u8"))
    record_fields = ",".join(f"x{index}:i64" for index in range(64))
    record_values = ",".join(f"x{index}:42i64" for index in range(64))
    record_frame = (f"#[derive(Copy)]struct Row{{{record_fields}}}"
                    f"fn main()->i64{{let original=Row{{{record_values}}};"
                    + "".join(f"let copy{index}=original;" for index in range(63)))
    for ty, prefix, descend, admitted in frame_cases:
        for name, entry, expected in (
            (prefix, admitted, (0, b"42\n", b"")),
            (prefix + "-refused", "descend(127)", (1, b"", NATIVE_DEPTH_ERROR)),
        ):
            source = evidence / f"{name}.ox"
            source.write_text(descend + f"fn main()->{ty}{{return {entry};}}\n", encoding="ascii")
            receipts = []
            for profile in ("dev", "release"):
                binary = evidence / f"{name}-{profile}"
                receipts.append(driver.build(compiler, source, clang, runtime, binary,
                                             profile, timeout, evidence / f"{name}-{profile}-build"))
            source.rename(evidence / f"{name}.retained")
            for receipt in receipts:
                command = [receipt["output"]]
                result = run(command, evidence, timeout)
                row = record(evidence, f'{name}-{receipt["profile"]}', command, result)
                rows.append({**row, "build": receipt})
                require((result.returncode, result.stdout, result.stderr) == expected,
                        f'{name}: wrong result at {receipt["profile"]}')
    source = evidence / "full-record-frame.ox"
    source.write_text(record_frame + "return copy62.x63;}", encoding="ascii")
    receipts = []
    for profile in ("dev", "release"):
        binary = evidence / f"full-record-frame-{profile}"
        receipts.append(driver.build(compiler, source, clang, runtime, binary,
                                     profile, timeout, evidence / f"full-record-frame-{profile}-build"))
    source.rename(evidence / "full-record-frame.retained")
    for receipt in receipts:
        command = [receipt["output"]]
        result = run(command, evidence, timeout)
        row = record(evidence, f'full-record-frame-{receipt["profile"]}', command, result)
        rows.append({**row, "build": receipt})
        require((result.returncode, result.stdout, result.stderr) == (0, b"42\n", b""),
                "full record frame did not preserve the final field")
    # Each full i64 array uses 1024 eight-byte cells, including the parameter.
    # A fifth binding exceeds the function's 4096-cell budget even when no
    # element of that binding would be used.
    wide_frame = ("fn at_limit(value:[i64;1024])->i64{"
                  "let first=value;let second=first;let third=second;")
    source = evidence / "full-wide-array-frame.ox"
    values = "0i64," * 1023 + "42i64"
    entry = f"fn main()->i64{{return at_limit([{values}]);}}"
    source.write_text(wide_frame + "return third[1023];}" + entry, encoding="ascii")
    receipts = []
    for profile in ("dev", "release"):
        name = f"full-wide-array-frame-{profile}"
        receipts.append(driver.build(compiler, source, clang, runtime, evidence / name,
                                     profile, timeout, evidence / (name + "-build")))
    source.rename(evidence / "full-wide-array-frame.retained")
    for receipt in receipts:
        command = [receipt["output"]]
        result = run(command, evidence, timeout)
        name = f'full-wide-array-frame-{receipt["profile"]}'
        row = record(evidence, name, command, result)
        rows.append({**row, "build": receipt})
        require((result.returncode, result.stdout, result.stderr) == (0, b"42\n", b""),
                "full i64 array frame did not preserve its last element")
    for name, code in (
        ("wide-array-frame-limit", wide_frame + "let extra=third;return 0i64;}"
         + "fn main()->i32{return 42;}"),
        ("enum-variant-limit", "enum Choice{" + ",".join(f"V{i}" for i in range(65))
         + "}fn main()->i32{return 42;}"),
        ("enum-storage-limit", "enum Choice{" + ",".join(f"V{i}([i64;1024])" for i in range(4))
         + "}fn main()->i32{return 42;}"),
        ("record-array-storage-limit",
         "struct Wide{a:[i64;1024],b:[i64;1024],c:[i64;1024],d:[i64;1024],extra:bool}"
         "fn unsupported(a:Wide)->i32{return 0;}fn main()->i32{return 42;}"),
        ("wide-array-length-limit", "fn unsupported(a:[i64;1025])->i32{return 0;}"
         "fn main()->i32{return 42;}"),
        ("byte-array-frame-limit", byte_frames.replace("if n==0", "let extra=0;if n==0")
         + "fn main()->u8{return descend(0);}"),
        ("byte-literal-limit", 'fn main()->i32{let bytes=b"' + 'A' * 1025 + '";return 0;}'),
        ("byte-list-limit", 'fn main()->i32{let bytes=[' + '0u8,' * 1025 + '];return 0;}'),
        ("record-field-limit", f"#[derive(Copy)]struct Row{{{record_fields},extra:i32}}"
         "fn main()->i32{return 42;}"),
        ("record-frame-limit", record_frame + "let extra=original;return extra.x63;}"),
    ):
        source = evidence / (name + ".ox")
        source.write_text(code, encoding="ascii")
        command = driver.compiler_command(compiler, source, target)
        outcome = run(command, evidence, timeout)
        require((outcome.returncode, outcome.stdout, outcome.stderr) == (68, b"", RESOURCE_ERROR),
                name + ": oversized input did not fail before emitting LLVM")
        rows.append(record(evidence, name, command, outcome))
    for name, malformed in (
            ("utf8-overlong", b"\xc0\xaf"),
            ("utf8-surrogate", b"\xed\xa0\x80"),
            ("utf8-truncated", b"\xf0\x9f\xa6")):
        source = evidence / (name + ".ox")
        source.write_bytes(b"fn main()->i32{return 42;}\n// " + malformed)
        command = driver.compiler_command(compiler, source, target)
        outcome = run(command, evidence, timeout)
        check_source_rejection(name, outcome, source)
        require(outcome.stderr == os.fsencode(source) + b":2:4" + SOURCE_ERROR,
                name + ": malformed UTF-8 was reported at the wrong position")
        rows.append(record(evidence, name, command, outcome))
    return rows


def verify_modules(compiler: Path, clang: Path, runtime: Path, reference: Path,
                   output: Path, timeout: int) -> list[dict]:
    target = driver.host_target()
    evidence = output / "modules"
    evidence.mkdir()
    cases = (
        ("else-if-chain", 0, True, {
            "main.ox": "mod score;fn main()->i32{return crate::score::value(2);}",
            "score.ox": "fn skipped()->bool{let zero=0;return 1/zero==0;}"
                        "pub fn value(n:i32)->i32{if n==1{return 0;}else if n==2{return 42;}"
                        "else if skipped(){return 1;}else{return 2;}}",
        }),
        ("sibling-modules", 0, True, {
            "main.ox": "mod a;fn base()->i32{return 40;}mod b;"
                       "fn main()->i32{return crate::a::value()+crate::b::value();}",
            "a.ox": "pub fn value()->i32{return crate::base()+local();}fn local()->i32{return 1;}",
            "b.ox": "pub fn value()->i32{return crate::b::local();}fn local()->i32{return 1;}",
        }),
        ("function-imports", 0, True, {
            "main.ox": "use crate::a::value as answer;mod a;mod b;use crate::b::extra;"
                       "fn base()->i32{return 41;}fn main()->i32{return answer()+extra();}",
            "a.ox": "use crate::base as seed;pub fn value()->i32{return seed();}",
            "b.ox": "pub fn extra()->i32{return 1;}",
        }),
        ("private-import", 65, True, {
            "main.ox": "mod a;use crate::a::hidden;fn main()->i32{return 42;}",
            "a.ox": "fn hidden()->i32{return 42;}",
        }),
        ("duplicate-target-import", 65, True, {
            "main.ox": "mod a;use crate::a::value as first;use crate::a::value as second;"
                       "fn main()->i32{return first();}",
            "a.ox": "pub fn value()->i32{return 42;}",
        }),
        ("shadowed-import", 65, True, {
            "main.ox": "mod a;use crate::a::value as amount;"
                       "fn main()->i32{let amount=42;return amount;}",
            "a.ox": "pub fn value()->i32{return 42;}",
        }),
        ("unknown-import", 65, True, {
            "main.ox": "mod a;use crate::a::missing;fn main()->i32{return 42;}",
            "a.ox": "pub fn value()->i32{return 42;}",
        }),
        ("private-export", 65, True, {
            "main.ox": "mod a;fn main()->i32{return crate::a::hidden();}",
            "a.ox": "fn hidden()->i32{return 42;}",
        }),
        ("duplicate-module", 65, True, {
            "main.ox": "mod a;mod a;fn main()->i32{return 42;}",
            "a.ox": "pub fn value()->i32{return 42;}",
        }),
        ("missing-module", 66, True, {
            "main.ox": "mod a;fn main()->i32{return crate::a::value();}",
        }),
        ("nested-modules", 0, True, {
            "main.ox": "mod a;mod b;use crate::a::detail::value as left;"
                       "fn main()->i32{return left()+crate::b::detail::value();}",
            "a.ox": "pub mod detail;fn base()->i32{return 40;}",
            "a/detail.ox": "pub mod inner;pub fn value()->i32{return crate::a::detail::inner::one()+crate::a::base();}",
            "a/detail/inner.ox": "pub fn one()->i32{return 1;}",
            "b.ox": "pub mod detail;",
            "b/detail.ox": "pub fn value()->i32{return 1;}",
        }),
        ("private-module-in-own-subtree", 0, True, {
            "main.ox": "mod a;fn main()->i32{return crate::a::result();}",
            "a.ox": "mod detail;pub fn result()->i32{return crate::a::detail::value();}",
            "a/detail.ox": "pub fn value()->i32{return 42;}",
        }),
        ("private-module-call", 65, True, {
            "main.ox": "mod a;fn main()->i32{return crate::a::detail::value();}",
            "a.ox": "mod detail;",
            "a/detail.ox": "pub fn value()->i32{return 42;}",
        }),
        ("private-module-import", 65, True, {
            "main.ox": "mod a;use crate::a::detail::value;fn main()->i32{return 42;}",
            "a.ox": "mod detail;",
            "a/detail.ox": "pub fn value()->i32{return 42;}",
        }),
        ("private-nested-function", 65, True, {
            "main.ox": "mod a;fn main()->i32{return crate::a::detail::value();}",
            "a.ox": "pub mod detail;",
            "a/detail.ox": "fn value()->i32{return 42;}",
        }),
        ("duplicate-nested-module", 65, True, {
            "main.ox": "mod a;fn main()->i32{return 42;}",
            "a.ox": "mod detail;mod Detail;",
            "a/detail.ox": "",
        }),
        ("missing-nested-module", 66, True, {
            "main.ox": "mod a;fn main()->i32{return 42;}",
            "a.ox": "mod nested;",
        }),
        ("repeated-entry-module", 65, True, {
            "main.ox": "mod main;fn main()->i32{return 42;}",
        }),
        ("maximum-module-depth", 0, True, {
            "main.ox": "mod a;fn main()->i32{return crate::" + "a::" * 32 + "value();}",
            **{"a/" * depth + "a.ox": "pub mod a;" for depth in range(31)},
            "a/" * 31 + "a.ox": "pub fn value()->i32{return 42;}",
        }),
        ("module-depth-limit", 68, True, {
            "main.ox": "mod a;fn main()->i32{return 42;}",
            **{"a/" * depth + "a.ox": "mod a;" for depth in range(33)},
        }),
        ("combined-input-limit", 68, False, {
            "main.ox": "mod a;fn main()->i32{return 42;}",
            "a.ox": " " * 65536,
        }),
        ("module-local-records", 0, True, {
            "main.ox": "mod data;#[derive(Copy)]struct Row{local:i32}"
                       "fn main()->i32{let value=Row{local:40};return value.local+crate::data::amount();}",
            "data.ox": "#[derive(Copy)]struct Row{value:i32}"
                       "fn make()->Row{return Row{value:2};}"
                       "pub fn amount()->i32{let value=make();return value.value;}",
        }),
        ("record-not-visible-in-a-different-module", 65, True, {
            "main.ox": "mod data;fn main()->i32{let value=Row{value:42};return value.value;}",
            "data.ox": "#[derive(Copy)]struct Row{value:i32}",
        }),
        ("public-copy-records", 0, True, {
            "main.ox": "mod data;fn get(value:crate::data::Row)->i32{return value.number;}"
                       "fn main()->i32{let mut row:crate::data::Row=crate::data::Row{number:1};"
                       "row.number=2;let other=crate::data::open();return get(row)+other.number;}",
            "data.ox": "#[derive(Copy)]pub struct Row{pub number:i32}"
                       "pub fn open()->Row{return Row{number:40};}",
        }),
        ("public-owned-record", 0, True, {
            "main.ox": "mod data;fn identity(row:crate::data::Row)->crate::data::Row{return row;}"
                       "fn main()->i32{let row=identity(crate::data::open());return row.number;}",
            "data.ox": "pub struct Row{pub number:i32,bytes:[u8;2]}"
                       "pub fn open()->Row{return Row{number:42,bytes:[1u8,2u8]};}",
        }),
        ("aliased-record-import", 0, True, {
            "main.ox": "use crate::data::Row as Entry;mod data;"
                       "fn id(row:Entry)->Entry{return row;}"
                       "fn main()->i32{let Entry{number:answer}=id(Entry{number:42});return answer;}",
            "data.ox": "#[derive(Copy)]pub struct Row{pub number:i32}",
        }),
        ("cross-module-record-borrows", 0, True, {
            "main.ox": "mod data;use crate::data::Row as Entry;"
                       "fn update(row:&mut Entry)->(){crate::data::add(&mut *row,40);return;}"
                       "fn main()->i32{let mut row=crate::data::open();update(&mut row);"
                       "return crate::data::read(&row);}",
            "data.ox": "pub struct Row{number:i32,bytes:[u8;2]}"
                       "pub fn open()->Row{return Row{number:2,bytes:[1u8,2u8]};}"
                       "pub fn read(row:&crate::data::Row)->i32{return row.number;}"
                       "pub fn add(row:&mut Row,change:i32)->(){row.number=read(&*row)+change;return;}",
        }),
        ("repeated-shared-record-borrows", 0, True, {
            "main.ox": "struct Row{value:i32}"
                       "fn total(a:&Row,b:&Row)->i32{return a.value+b.value;}"
                       "fn main()->i32{let a=Row{value:14};let previous=total(&a,&a);"
                       "let renamed=a;return previous+renamed.value;}",
        }),
        ("duplicate-mutable-record-borrow", 65, True, {
            "main.ox": "struct Row{value:i32}"
                       "fn total(a:&mut Row,b:&mut Row)->i32{return a.value+b.value;}"
                       "fn main()->i32{let mut a=Row{value:21};return total(&mut a,&mut a);}",
        }),
        ("record-shared-borrow-cannot-write", 65, True, {
            "main.ox": "struct Row{value:i32}"
                       "fn update(row:&Row)->(){row.value=42;return;}"
                       "fn main()->i32{let row=Row{value:0};update(&row);return row.value;}",
        }),
        ("record-shared-borrow-cannot-upgrade", 65, True, {
            "main.ox": "struct Row{value:i32}"
                       "fn update(row:&mut Row)->(){row.value=42;return;}"
                       "fn wrap(row:&Row)->(){update(&mut *row);return;}"
                       "fn main()->i32{let row=Row{value:0};wrap(&row);return row.value;}",
        }),
        ("record-move-while-borrowed", 65, True, {
            "main.ox": "struct Row{value:i32}"
                       "fn take(row:Row)->i32{return row.value;}"
                       "fn total(row:&Row,amount:i32)->i32{return row.value+amount;}"
                       "fn main()->i32{let row=Row{value:21};return total(&row,take(row));}",
        }),
        ("record-borrow-return", 65, True, {
            "main.ox": "struct Row{value:i32}"
                       "fn escape(row:&Row)->&Row{return row;}fn main()->i32{return 42;}",
        }),
        ("private-field-through-record-borrow", 65, True, {
            "main.ox": "mod data;fn read(row:&crate::data::Row)->i32{return row.hidden;}"
                       "fn main()->i32{let row=crate::data::open();return read(&row);}",
            "data.ox": "pub struct Row{hidden:i32}pub fn open()->Row{return Row{hidden:42};}",
        }),
        ("private-record-import", 65, True, {
            "main.ox": "mod data;use crate::data::Row;fn main()->i32{return 42;}",
            "data.ox": "#[derive(Copy)]struct Row{pub number:i32}",
        }),
        ("private-qualified-record-type", 65, True, {
            "main.ox": "mod data;fn read(value:crate::data::Row)->i32{return 0;}fn main()->i32{return 42;}",
            "data.ox": "#[derive(Copy)]struct Row{pub number:i32}",
        }),
        ("private-record-field-read", 65, True, {
            "main.ox": "mod data;fn main()->i32{let row=crate::data::open();return row.hidden;}",
            "data.ox": "#[derive(Copy)]pub struct Row{hidden:i32}"
                       "pub fn open()->Row{return Row{hidden:42};}",
        }),
        ("private-record-field-write", 65, True, {
            "main.ox": "mod data;fn main()->i32{let mut row=crate::data::open();row.hidden=2;return 42;}",
            "data.ox": "#[derive(Copy)]pub struct Row{hidden:i32}"
                       "pub fn open()->Row{return Row{hidden:42};}",
        }),
        ("private-record-field-construction", 65, True, {
            "main.ox": "mod data;fn main()->i32{let row=crate::data::Row{hidden:42};return 42;}",
            "data.ox": "#[derive(Copy)]pub struct Row{hidden:i32}",
        }),
        ("utf8-module-source", 0, True, {
            "main.ox": "mod data;fn main()->i32{return crate::data::value();}",
            "data.ox": "// 雪\npub fn value()->i32{if '雪'<'🦀'{return 42;}return 0;}",
        }),
    )
    rows = []
    for name, expected, compare, files in cases:
        project = evidence / name
        project.mkdir()
        for relative, code in files.items():
            source_file = project / relative
            source_file.parent.mkdir(parents=True, exist_ok=True)
            source_file.write_text(code, encoding="utf-8")
        source = project / "main.ox"
        command = driver.compiler_command(compiler, source, target)
        compiled = run(command, evidence, timeout)
        row = {"name": name, "compiler": record(project, "compiler", command, compiled), "profiles": []}
        require(compiled.returncode == expected, name + ": unexpected module status")
        if expected == 65:
            check_source_rejection(name, compiled)
        elif expected == 66:
            require(compiled.stderr == INPUT_ERROR, name + ": expected an input diagnostic")
        elif expected == 68:
            require(compiled.stderr == RESOURCE_ERROR, name + ": expected a resource diagnostic")
        if compare:
            command = [str(reference), "run", str(source), "--edition=typed-preview"]
            observed = run(command, evidence, timeout, clear_environment=False)
            row["reference"] = record(project, "reference", command, observed)
            if expected == 0:
                check_native_behavior({"name": name, "kind": "value", "stdout": "42\n"}, observed)
            else:
                require(observed.returncode != 0, name + ": reference accepted invalid modules")
        if expected != 0:
            require(compiled.stdout == b"", name + ": rejected modules emitted LLVM")
            rows.append(row)
            continue
        driver.module_from_result(compiled, target)
        for profile in ("dev", "release"):
            binary = evidence / f"{name}-{profile}"
            receipt = driver.build(compiler, source, clang, runtime, binary,
                                   profile, timeout, project / profile)
            require(receipt["module_sha256"] == hashlib.sha256(compiled.stdout).hexdigest(),
                    name + ": module LLVM bytes changed with the working directory")
            row["profiles"].append(receipt)
        project.rename(evidence / f"{name}-retained")
        require(not project.exists(), name + ": module sources remain at their original path")
        for receipt in row["profiles"]:
            command = [receipt["output"]]
            executed = run(command, evidence, timeout)
            receipt["source_unavailable_run"] = record(evidence, f'{name}-{receipt["profile"]}', command, executed)
            check_native_behavior({"name": name, "kind": "value", "stdout": "42\n"}, executed)
        rows.append(row)
    return rows


def verify(args) -> dict:
    target = driver.host_target()
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
    report = {"status": "running", "self_hosting": False, "target": target, "cases": [],
              "compiler_sha256": hashlib.sha256(compiler.read_bytes()).hexdigest(),
              "runtime_sha256": hashlib.sha256(runtime.read_bytes()).hexdigest(),
              "driver_sha256": hashlib.sha256((ROOT / "build.py").read_bytes()).hexdigest(),
              "cases_sha256": hashlib.sha256((ROOT / "tests/cases.json").read_bytes()).hexdigest()}
    if runtime == driver.DEFAULT_RUNTIME:
        runtime_sources = ("native/typed_preview.c", "native/typed_character.c")
        report["runtime_sources_sha256"] = {
            name: hashlib.sha256((ROOT.parents[1] / name).read_bytes()).hexdigest()
            for name in runtime_sources
        }
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

            command = driver.compiler_command(compiler, source, target)
            compiled = run(command, work, args.timeout)
            row["compiler"] = record(evidence, "compiler", command, compiled)
            if case["kind"] == "reject":
                check_source_rejection(case["name"], compiled, source)
                rejected_destination = deploy / (case["name"] + "-rejected")
                try:
                    driver.build(compiler, source, clang, runtime, rejected_destination, "dev", args.timeout)
                except driver.BuildError:
                    pass
                else:
                    raise RuntimeError(case["name"] + ": invalid source built")
                require(not os.path.lexists(rejected_destination), "rejected source published an artifact")
            else:
                driver.module_from_result(compiled, target)
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
        if args.reference is not None:
            report["modules"] = verify_modules(compiler, clang, runtime, args.reference.resolve(strict=True),
                                               output, args.timeout)
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
    parser.add_argument("--runtime", type=Path, default=driver.DEFAULT_RUNTIME)
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
