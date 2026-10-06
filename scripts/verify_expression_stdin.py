#!/usr/bin/env python3
"""Run hand-derived stdin cases through one public Oxid app and one compiled ELF."""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import shutil
import subprocess


MEMBERS = ("stdin.ox", "arena.ox", "scanner.ox", "parser.ox", "evaluator.ox")
ELF_ENV = {"PATH": "/no-tools"}
# Independent, literal expectations. This controller never parses expressions.
CASES = (
    ("tree-seven", b"12 + 3 * (4 + 5)", 39, None),
    ("second-expression", b"7*(8+1)", 63, None),
    ("empty", b"", -1, None),
    ("whitespace", b" \t\r\n", -1, None),
    ("trailing-newline", b"12 + 3 * (4 + 5)\r\n", 39, None),
    ("missing-operand", b"1+", -1, None),
    ("unclosed-parenthesis", b"(1", -1, None),
    ("unexpected-close", b")", -1, None),
    ("invalid-character", b"1+@", -1, None),
    ("nul", b"1\x00", -1, None),
    ("non-ascii", b"1\xff", -1, None),
    ("eof-127", b" " * 126 + b"1", 1, None),
    ("eof-128", b" " * 127 + b"1", 1, None),
    ("full-129", b" " * 128 + b"1", -4, None),
    ("full-130", b" " * 128 + b"12", -4, None),
    ("full-before-syntax", b"@" * 130, -4, None),
    ("node-capacity", b"1+2+3+4+5+6+7+8", 36, None),
    ("node-overflow", b"1+2+3+4+5+6+7+8+9", -1, None),
    ("operator-capacity", b"(((((((((((((((7)))))))))))))))", 7, None),
    ("operator-overflow", b"((((((((((((((((7))))))))))))))))", -1, None),
    ("maximum-i32", b"2147483647", 2147483647, None),
    ("syntax-before-evaluation", b"2147483647+1+", -1, None),
    ("literal-overflow", b"2147483648", None, "scanner.ox"),
    ("scan-before-grammar", b"1 2147483648", None, "scanner.ox"),
    ("addition-overflow", b"2147483647+1", None, "evaluator.ox"),
    ("multiplication-overflow", b"46341*46341", None, "evaluator.ox"),
    ("no-multiply-short-circuit", b"0*(2147483647+1)", None, "evaluator.ox"),
    ("closed-stdin", b"", -5, None),
)


def sha256(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def close_stdin():
    os.close(0)


def invoke(directory, label, command, data, mode, *, cwd, env=None, timeout=120):
    """Retain streams and independently observe unread bytes on the shared fd."""
    command = [str(part) for part in command]
    (directory / (label + ".input")).write_bytes(data)
    fd = None
    writer = None
    input_file = None
    result = None
    failure = None
    stdout = b""
    stderr = b""
    remaining = None
    try:
        if mode == "pipe":
            fd, writer = os.pipe()
            # Every case fits within PIPE_BUF; finish writing before launching.
            count = os.write(writer, data)
            if count != len(data):
                raise OSError("incomplete controller pipe write")
            os.close(writer)
            writer = None
        elif mode == "file":
            input_file = (directory / (label + ".input")).open("rb", buffering=0)
            fd = input_file.fileno()
        elif mode != "closed":
            raise ValueError("unknown input mode")
        result = subprocess.run(
            command, stdin=subprocess.DEVNULL if mode == "closed" else fd,
            preexec_fn=close_stdin if mode == "closed" else None,
            cwd=cwd, env=env, capture_output=True, timeout=timeout, check=False,
        )
        stdout, stderr = result.stdout, result.stderr
        if fd is not None:
            remaining = b""
            while True:
                chunk = os.read(fd, 4096)
                if not chunk:
                    break
                remaining += chunk
    except (OSError, subprocess.TimeoutExpired) as error:
        failure = str(error)
        stdout = getattr(error, "stdout", None) or b""
        stderr = getattr(error, "stderr", None) or b""
    finally:
        if writer is not None:
            os.close(writer)
        if input_file is not None:
            input_file.close()
        elif fd is not None:
            os.close(fd)
    (directory / (label + ".stdout")).write_bytes(stdout)
    (directory / (label + ".stderr")).write_bytes(stderr)
    if remaining is not None:
        (directory / (label + ".remaining")).write_bytes(remaining)
    record = {
        "command": command, "cwd": str(cwd), "mode": mode,
        "environment": ELF_ENV if env == ELF_ENV else "inherited",
        "exit": None if result is None else result.returncode,
        "failure": failure, "input_bytes": len(data),
        "remaining_bytes": None if remaining is None else len(remaining),
        "input_sha256": hashlib.sha256(data).hexdigest(),
    }
    return record, stdout, stderr, remaining


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--oxid", type=Path, required=True)
    ap.add_argument("--output", type=Path, required=True)
    ap.add_argument("--native", action="store_true")
    args = ap.parse_args()
    if platform.system() != "Linux" or platform.machine() != "x86_64":
        ap.error("bounded stdin execution is qualified only on Linux x86_64")
    compiler = args.oxid.resolve(strict=True)
    fixture = Path(__file__).resolve().parents[1] / "fixtures/typed-expression-samples"
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=False)
    project = output / "project"
    project.mkdir()
    source_free = output / "source-free"
    source_free.mkdir()
    for name in MEMBERS:
        shutil.copyfile(fixture / name, project / name)
    entry = project / "stdin.ox"
    program = output / "expression-stdin"
    report = {
        "compiler_sha256": sha256(compiler),
        "fixture_sha256": {name: sha256(project / name) for name in MEMBERS},
        "native": args.native, "setup": [], "cases": [], "passed": False,
        "application_compilations": 0,
        "native_execution": "One unchanged ELF; empty working directory and cleared environment. No filesystem isolation.",
    }

    def save_report():
        (output / "report.json").write_text(json.dumps(report, indent=2) + "\n")

    commands = [("check", [compiler, "check", entry, "--edition", "typed-preview"])]
    if args.native:
        commands.append(("compile", [compiler, "compile", entry, "--edition", "typed-preview",
                                      "--backend", "llvm", "--output", program]))
    for label, command in commands:
        sentinel = b"check-and-compile-must-not-read-stdin"
        row, _, stderr, remaining = invoke(output, label, command, sentinel, "pipe", cwd=project)
        row["passed"] = row["exit"] == 0 and not stderr and remaining == sentinel
        report["setup"].append(row)
        if label == "compile":
            report["application_compilations"] += 1
        save_report()
        if not row["passed"]:
            print(label, "FAIL; see", output / "report.json", flush=True)
            return 1
    if args.native:
        if not program.is_file() or program.read_bytes()[:4] != b"\x7fELF":
            report["artifact_failure"] = "compile did not produce an ELF"
            save_report()
            return 1
        report["program_sha256"] = sha256(program)
        report["program_bytes"] = program.stat().st_size
    failed = False
    for name, data, value, diagnostic_source in CASES:
        directory = output / name
        directory.mkdir()
        expected_remaining = data[129:]
        expected_stdout = b"" if diagnostic_source else f"{value}\n".encode()
        expectation = {
            "value": value, "error": "E0604" if diagnostic_source else None,
            "diagnostic_source": diagnostic_source,
            "consumed_bytes": min(len(data), 129),
            "remaining_hex": expected_remaining.hex(),
        }
        rows = []
        modes = ("closed",) if name == "closed-stdin" else ("pipe", "file")
        for mode in modes:
            reference_stderr = None
            stages = [("reference", [compiler, "run", entry, "--edition", "typed-preview"])]
            if args.native:
                stages.append(("native", [program]))
            for stage, command in stages:
                native = stage == "native"
                before = sha256(program) if native else None
                row, stdout, stderr, remaining = invoke(
                    directory, stage + "-" + mode, command, data, mode,
                    cwd=source_free if native else project, env=ELF_ENV if native else None,
                )
                expected_exit = 1 if diagnostic_source else 0
                ok = (row["failure"] is None and row["exit"] == expected_exit
                      and stdout == expected_stdout)
                if diagnostic_source:
                    origin = str(project / diagnostic_source).encode() + b":"
                    ok = ok and b"error[E0604]" in stderr and origin in stderr
                else:
                    ok = ok and stderr == b""
                if mode != "closed":
                    ok = ok and remaining == expected_remaining
                if native:
                    after = sha256(program)
                    row["program_sha256_before"] = before
                    row["program_sha256_after"] = after
                    ok = ok and before == after == report["program_sha256"]
                    ok = ok and stderr == reference_stderr
                else:
                    reference_stderr = stderr
                row["passed"] = bool(ok)
                failed = failed or not ok
                rows.append(row)
        report["cases"].append({"name": name, "expectation": expectation, "checks": rows})
        save_report()
        print(name, "PASS" if all(row["passed"] for row in rows) else "FAIL", flush=True)
    report["passed"] = not failed
    save_report()
    return int(failed)


if __name__ == "__main__":
    raise SystemExit(main())
