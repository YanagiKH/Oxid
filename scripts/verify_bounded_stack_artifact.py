#!/usr/bin/env python3
"""Verify saved OXS1 artifacts using one public producer and one standalone loader.

Uses a supplied compiler without building it. Literal wire bytes and results are
independently authored. Publication requires producer termination and validation;
failed or partial output remains evidence. Atomic rename is not crash durability.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import shutil
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]
CORPUS = ROOT / "tests/fixtures/bounded_stack_artifact"
SOURCES = ROOT / "fixtures/typed-expression-samples"
PRODUCER_MEMBERS = ("artifact_main.ox", "artifact_writer.ox", "arena.ox", "scanner.ox",
                    "parser.ox", "evaluator.ox", "stack_code.ox", "lowering.ox")
LOADER_MEMBERS = ("artifact_load.ox", "artifact_reader.ox")
# These outcomes are input-boundary requirements, not inferred from Oxid output.
REJECTIONS = (
    ("empty", b"", 64, None),
    ("syntax", b"1+", 64, None),
    ("nul", b"1\x00", 64, None),
    ("non-ascii", b"1\xff", 64, None),
    ("node-capacity", b"1+2+3+4+5+6+7+8+9", 64, None),
    ("operator-capacity", b"(" * 16 + b"7" + b")" * 16, 64, None),
    ("syntax-before-arithmetic", b"2147483647+1+", 64, None),
    ("literal-overflow", b"2147483648", 1, "scanner.ox"),
    ("scan-before-grammar", b"1 2147483648", 1, "scanner.ox"),
    ("input-129", b" " * 128 + b"0", 64, None),
    ("input-130", b" " * 128 + b"01", 64, None),
    ("stdin-error", b"", 74, None),
)


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def save_json(path, value):
    path.write_text(json.dumps(value, indent=2) + "\n")


def invoke(directory, command, *, data=b"", input_path=None, mode="file", cwd,
           env=None, timeout=120):
    """Wait for exit with stdout saved directly, preserving failures and unread input."""
    directory.mkdir(parents=True, exist_ok=False)
    command = [str(value) for value in command]
    if input_path is None:
        input_path = directory / "input.bin"
        input_path.write_bytes(data)
    else:
        input_path = Path(input_path)
        data = input_path.read_bytes()
    stdout_path = directory / "stdout.partial"
    stderr_path = directory / "stderr"
    record = {"command": command, "cwd": str(cwd), "input": str(input_path),
              "input_sha256": digest(input_path), "input_bytes": len(data),
              "environment": "cleared" if env == {} else "inherited", "mode": mode,
              "status": None, "failure": None, "remaining_hex": None}
    fd = writer = None
    try:
        if mode == "directory":
            fd = os.open(directory, os.O_RDONLY | os.O_DIRECTORY)
        elif mode == "file":
            fd = os.open(input_path, os.O_RDONLY)
        elif mode == "pipe":
            fd, writer = os.pipe()
            if os.write(writer, data) != len(data):
                raise OSError("incomplete input write")
            os.close(writer)
            writer = None
        else:
            raise ValueError("unknown input mode")
        with stdout_path.open("xb") as stdout, stderr_path.open("xb") as stderr:
            result = subprocess.run(command, stdin=fd, stdout=stdout, stderr=stderr,
                                    cwd=cwd, env=env, timeout=timeout, check=False)
        record["status"] = result.returncode
        if mode != "directory":
            remaining = bytearray()
            while chunk := os.read(fd, 4096):
                remaining.extend(chunk)
            (directory / "remaining.bin").write_bytes(remaining)
            record["remaining_hex"] = remaining.hex()
    except (OSError, subprocess.TimeoutExpired) as error:
        record["failure"] = str(error)
    finally:
        if writer is not None:
            os.close(writer)
        if fd is not None:
            os.close(fd)
        save_json(directory / "command.json", record)
    return record, stdout_path.read_bytes() if stdout_path.exists() else b"", stderr_path.read_bytes() if stderr_path.exists() else b""


def expect_decoder(record, stdout, stderr, expected):
    if record["failure"]:
        return False
    try:
        if expected["kind"] == "value":
            return (record["status"] == 0 and not stderr and json.loads(stdout) ==
                    {"count": expected["count"], "rows": expected["rows"], "value": expected["value"]})
        if expected["kind"] == "overflow":
            return (record["status"] == 1 and not stdout and json.loads(stderr) ==
                    {"error": "E0604", "row": expected["row"]})
        return (record["status"] == 65 and not stdout and json.loads(stderr) ==
                {"error": expected["reason"], "offset": expected["offset"]})
    except (ValueError, KeyError):
        return False


def expect_loader(record, stdout, stderr, expected, source, data, mode="file"):
    ok = (record["failure"] is None and record["status"] == expected["loader_status"]
          and stdout == expected["loader_stdout"].encode())
    if expected["kind"] == "overflow":
        ok = ok and b"error[E0604]" in stderr and (str(source) + ":").encode() in stderr
    else:
        ok = ok and stderr == b""
    return bool(ok and (mode == "directory" or record["remaining_hex"] == data[81:].hex()))


def publish(partial, destination, producer, expected_bytes, *, consumers_passed):
    """A failed producer or invalid bytes can never replace a published artifact."""
    if (producer["failure"] is not None or producer["status"] != 0 or not consumers_passed
            or (partial.parent / "stderr").read_bytes() or not partial.is_file()
            or len(expected_bytes) != 80 or partial.read_bytes() != expected_bytes):
        return False
    if destination.exists():
        raise FileExistsError(destination)
    # Both names live in the fresh evidence tree on the same filesystem.
    os.replace(partial, destination)
    return True


def verify(args, output):
    compiler = args.oxid.resolve(strict=True)
    empty = output / "empty-cwd"
    empty.mkdir()
    published = output / "published"
    published.mkdir()
    # Persist the independent inputs and decoder exactly; do not give the
    # decoder source text, fixture names, expected values, or the case manifest.
    shutil.copytree(CORPUS, output / "corpus")
    corpus = output / "corpus"
    cases = json.loads((corpus / "artifact-cases.json").read_bytes())["cases"]
    if len(cases) != 28 or len({case["name"] for case in cases}) != 28:
        raise ValueError("expected 28 unique independently authored artifact cases")
    for case in cases:
        raw = (corpus / case["file"]).read_bytes()
        if (len(raw) != case["bytes"] or raw.hex() != case["hex"]
                or digest(corpus / case["file"]) != case["sha256"]):
            raise ValueError("literal fixture identity differs: " + case["name"])
    report = {"passed": False, "native": args.native, "compiler": str(compiler),
              "compiler_sha256": digest(compiler), "application_compilations": 0,
              "setup": [], "artifact_cases": [], "producer_cases": [], "rejection_cases": [],
              "publication": "After producer exit 0, exact bytes, external decoder and loader validation; atomic rename, no durability claim",
              "consumer_execution": "Saved-file input, empty cwd and cleared native/decoder environment; no filesystem isolation"}
    def save():
        save_json(output / "report.json", report)
    def record(row, ok):
        row["passed"] = bool(ok)
        save()
        return bool(ok)
    entries, programs, hashes = {}, {}, {}
    for kind, members in (("producer", PRODUCER_MEMBERS), ("loader", LOADER_MEMBERS)):
        project = output / (kind + "-source")
        project.mkdir()
        for name in members:
            shutil.copyfile(SOURCES / name, project / name)
        entries[kind] = project / members[0]
        programs[kind] = output / kind
        report[kind + "_source_sha256"] = {name: digest(project / name) for name in members}
        commands = [("check", [compiler, "check", entries[kind], "--edition", "typed-preview"])]
        if args.native:
            command = [compiler, "compile", entries[kind], "--edition", "typed-preview",
                       "--backend", "llvm", "--output", programs[kind]]
            if kind == "producer":
                command += ["--entry-mode", "process"]
            commands.append(("compile", command))
        for action, command in commands:
            sentinel = b"setup-must-not-consume-input"
            row, _, stderr = invoke(output / (kind + "-" + action), command,
                                    data=sentinel, mode="pipe", cwd=empty)
            report["setup"].append(row)
            if action == "compile":
                report["application_compilations"] += 1
            if not record(row, row["status"] == 0 and not stderr and row["failure"] is None
                          and row["remaining_hex"] == sentinel.hex()):
                return False
        if args.native:
            if programs[kind].read_bytes()[:4] != b"\x7fELF":
                raise ValueError("compiler did not produce an ELF: " + kind)
            hashes[kind] = digest(programs[kind])
    report["program_sha256"] = hashes
    save()

    def stages(kind):
        command = [compiler, "run", entries[kind], "--edition", "typed-preview"]
        if kind == "producer":
            command += ["--entry-mode", "process"]
        result = [("reference", command, None)]
        if args.native:
            result.append(("native", [programs[kind]], {}))
        return result

    def consumers(directory, artifact, expected, modes=("file",)):
        directory.mkdir(parents=True, exist_ok=False)
        data = artifact.read_bytes()
        row, stdout, stderr = invoke(directory / "external", [sys.executable, "-I", "-B",
                                    corpus / "decode_oxs1.py", artifact], cwd=empty, env={})
        row["passed"] = expect_decoder(row, stdout, stderr, expected)
        rows = [row]
        for mode in modes:
            reference_stderr = None
            for stage, command, env in stages("loader"):
                row, stdout, stderr = invoke(directory / (stage + "-" + mode), command,
                                            input_path=artifact, mode=mode, cwd=empty, env=env)
                ok = expect_loader(row, stdout, stderr, expected, entries["loader"].parent / "artifact_reader.ox", data, mode)
                if stage == "native":
                    ok = ok and stderr == reference_stderr and digest(programs["loader"]) == hashes["loader"]
                else:
                    reference_stderr = stderr
                row["passed"] = bool(ok)
                rows.append(row)
        return rows

    for case in cases:
        rows = consumers(output / "artifact-cases" / case["name"], corpus / case["file"],
                         case["expected"], ("pipe", "file"))
        report["artifact_cases"].append({"name": case["name"], "checks": rows})
        save()
        print("artifact", case["name"], "PASS" if all(row["passed"] for row in rows) else "FAIL", flush=True)

    producer_cases = [case for case in cases if "producer_input" in case]
    # The existing 128-byte input boundary uses the independently authored zero
    # artifact; adding whitespace changes no grammar or serializer expectation.
    zero = next(case for case in cases if case["name"] == "zero-push")
    for width in (127, 128):
        producer_cases.append(dict(zero, name="input-" + str(width), input_bytes=b" " * (width - 1) + b"0"))
    for case in producer_cases:
        data = case.get("input_bytes")
        if data is None:
            data = (corpus / case["producer_input"]).read_bytes()
        expected = (corpus / case["file"]).read_bytes()
        rows = []
        reference_stderr = None
        for stage, command, env in stages("producer"):
            directory = output / "producer-cases" / case["name"] / stage
            row, stdout, stderr = invoke(directory, command, data=data, cwd=empty, env=env)
            ok = (row["failure"] is None and row["status"] == 0 and not stderr
                  and stdout == expected and row["remaining_hex"] == data[129:].hex())
            if stage == "native":
                ok = ok and stderr == reference_stderr and digest(programs["producer"]) == hashes["producer"]
            else:
                reference_stderr = stderr
            validations = []
            if ok:
                # invoke has returned only after the producer exited and closed
                # its output. Consumers receive this saved file, never a pipe.
                validations = consumers(directory / "consumers", directory / "stdout.partial", case["expected"])
                ok = all(result["passed"] for result in validations)
            destination = published / (case["name"] + "-" + stage + ".oxs")
            did_publish = publish(directory / "stdout.partial", destination, row, expected, consumers_passed=bool(ok))
            row.update(passed=bool(ok and did_publish), consumers=validations,
                       published=str(destination) if did_publish else None)
            rows.append(row)
        report["producer_cases"].append({"name": case["name"], "checks": rows})
        save()
        print("producer", case["name"], "PASS" if all(row["passed"] for row in rows) else "FAIL", flush=True)

    for name, data, status, origin in REJECTIONS:
        rows = []
        previous_stderr = None
        mode = "directory" if name == "stdin-error" else "file"
        for stage, command, env in stages("producer"):
            row, stdout, stderr = invoke(output / "rejections" / name / stage, command,
                                        data=data, mode=mode, cwd=empty, env=env)
            ok = row["failure"] is None and row["status"] == status and stdout == b""
            if origin:
                ok = ok and b"error[E0604]" in stderr and (str(entries["producer"].parent / origin) + ":").encode() in stderr
            else:
                ok = ok and stderr == b""
            if mode != "directory":
                ok = ok and row["remaining_hex"] == data[129:].hex()
            if stage == "native":
                ok = ok and stderr == previous_stderr and digest(programs["producer"]) == hashes["producer"]
            else:
                previous_stderr = stderr
            row["passed"] = bool(ok)
            rows.append(row)
        report["rejection_cases"].append({"name": name, "checks": rows})
        save()
        print("rejection", name, "PASS" if all(row["passed"] for row in rows) else "FAIL", flush=True)

    # The loader's input failure is distinct from malformed or truncated bytes.
    rows = []
    for stage, command, env in stages("loader"):
        row, stdout, stderr = invoke(output / "loader-io-error" / stage, command,
                                    mode="directory", cwd=empty, env=env)
        row["passed"] = row["failure"] is None and row["status"] == 0 and stdout == b"-5\n" and not stderr
        rows.append(row)
    report["loader_io_error"] = rows
    report["identities_unchanged"] = (digest(compiler) == report["compiler_sha256"]
        and all(digest(programs[kind]) == sha for kind, sha in hashes.items())
        and all(digest(output / (kind + "-source") / name) == sha
                for kind in ("producer", "loader") for name, sha in report[kind + "_source_sha256"].items()))
    report["passed"] = (report["identities_unchanged"] and all(row["passed"] for row in rows)
        and all(row["passed"] for group in ("artifact_cases", "producer_cases", "rejection_cases")
                for case in report[group] for row in case["checks"]))
    save()
    return report["passed"]


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--oxid", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--native", action="store_true", help="compile each app once and run the unchanged ELFs")
    args = parser.parse_args()
    if platform.system() != "Linux" or platform.machine() != "x86_64":
        parser.error("qualified only on Linux x86_64")
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=False)
    try:
        return 0 if verify(args, output) else 1
    except (OSError, ValueError, KeyError) as error:
        save_json(output / "failure.json", {"error": str(error)})
        print("FAIL:", error, file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
