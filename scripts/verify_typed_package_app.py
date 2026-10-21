#!/usr/bin/env python3
"""Check the real byte-count package application against independent byte oracles.

The optional native mode also rebuilds edited dependencies, preserves a previous
executable on a failed build, and runs a deployed executable after deleting the
copied source tree. This is R1 regression evidence, not v1 acceptance.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import time

ROOT = Path(__file__).resolve().parents[1]
APPLICATION = ROOT / "apps/byte-count"
CASES = (
    ("empty", b""),
    ("one-byte", b"x"),
    ("newline", b"\n"),
    ("text", b"hello\nworld\n"),
    ("unterminated", b"first\nlast"),
    ("short-chunk", b"\n" * 31),
    ("full-chunk", b"\n" * 32),
    ("next-chunk", b"\n" * 33),
    ("binary", bytes(range(256))),
    ("many-chunks", (b"abc\x00\xff\n" * 257) + b"tail"),
)


def require(condition, message):
    if not condition:
        raise ValueError(message)


def digest(path):
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def save(path, value):
    path.write_text(json.dumps(value, indent=2, sort_keys=True) + "\n")


def expected(payload, multiplier=1):
    return f"{payload.count(bytes([10])) * multiplier} {len(payload)}\n".encode()


class Runner:
    def __init__(self, output, compiler, report):
        self.output, self.compiler, self.report = output, compiler, report
        self.commands = report["commands"] = []

    def command(self, name, argv, cwd, payload=b"", failure=False, env=None):
        directory = self.output / name
        directory.mkdir()
        (directory / "stdin.bin").write_bytes(payload)
        row = {"name": name, "argv": [str(a) for a in argv], "cwd": str(cwd),
               "environment_overrides": env or {}, "expected_failure": failure,
               "stdin_sha256": digest(directory / "stdin.bin"), "status": "running"}
        self.commands.append(row)
        started = time.monotonic()
        try:
            result = subprocess.run(row["argv"], cwd=cwd, input=payload, capture_output=True,
                                    env={**os.environ, **(env or {})}, timeout=180)
            (directory / "stdout.bin").write_bytes(result.stdout)
            (directory / "stderr.bin").write_bytes(result.stderr)
            row.update(exit_code=result.returncode, stdout_sha256=digest(directory / "stdout.bin"),
                       stderr_sha256=digest(directory / "stderr.bin"))
            require(result.returncode != 0 if failure else result.returncode == 0,
                    name + ": unexpected exit status; inspect " + str(directory))
            row["status"] = "passed"
            return result
        except (OSError, ValueError, subprocess.TimeoutExpired) as error:
            row.update(status="failed", error=str(error))
            raise
        finally:
            row["elapsed_seconds"] = time.monotonic() - started
            save(directory / "command.json", row)

    def cases(self, prefix, argv, cwd, multiplier=1, env=None):
        for name, payload in CASES:
            result = self.command(prefix + "-" + name, argv, cwd, payload, env=env)
            require(result.stdout == expected(payload, multiplier) and not result.stderr,
                    prefix + "-" + name + ": byte oracle mismatch")

    def check(self, native):
        project = self.output / "application"
        shutil.copytree(APPLICATION, project, ignore=shutil.ignore_patterns(".oxid", "oxid.lock"))
        self.command("check", [self.compiler, "check"], project)
        tested = self.command("test", [self.compiler, "test", "--message-format=json"], project)
        rows = [json.loads(line) for line in tested.stdout.splitlines() if line.strip()]
        results = [row for row in rows if row.get("kind") == "test-result"]
        summaries = [row for row in rows if row.get("kind") == "test-summary"]
        require(len(rows) == 2 and len(results) == len(summaries) == 1
                and results[0].get("name") == "counts" and results[0].get("success") is True
                and results[0].get("exit_status") == 0 and summaries[0].get("success") is True
                and summaries[0].get("passed") == 1 and summaries[0].get("failed") == 0
                and not tested.stderr, "declared application test did not pass")
        self.cases("interpreted", [self.compiler, "run"], project)
        if not native:
            return
        self.command("build", [self.compiler, "build"], project)
        executable = project / ".oxid/bin/byte-count"
        require(executable.read_bytes().startswith(b"\x7fELF"), "build must produce a native ELF")
        self.report["initial_executable_sha256"] = digest(executable)
        self.cases("native", [executable], project)

        # A real dependency edit must reach both execution paths and the next
        # managed build. Double newline counting while retaining the byte oracle.
        dependency = project / "packages/counts/src/lib.ox"
        original = dependency.read_text()
        old = "totals.lines = totals.lines + 1;"
        require(original.count(old) == 1, "dependency edit location changed")
        changed = original.replace(old, "totals.lines = totals.lines + 2;")
        dependency.write_text(changed)
        self.cases("edited-interpreted", [self.compiler, "run"], project, multiplier=2)
        self.command("rebuild", [self.compiler, "build"], project)
        rebuilt = digest(executable)
        require(rebuilt != self.report["initial_executable_sha256"], "dependency edit did not change executable")
        self.report["edited_executable_sha256"] = rebuilt
        self.cases("edited-native", [executable], project, multiplier=2)

        dependency.write_text(changed + "\nfn broken(\n")
        failed = self.command("failed-build", [self.compiler, "build", "--message-format=json"],
                              project, failure=True)
        rejected = [json.loads(line) for line in failed.stdout.splitlines() if line.strip()]
        require(failed.returncode == 1 and not failed.stderr
                and any(row.get("kind") == "diagnostic" and row.get("code") == "E0100"
                        and row.get("stage") == "parse" for row in rejected)
                and any(row.get("success") is False and row.get("kind") == "compile-summary"
                        for row in rejected), "broken dependency must fail with a typed parse diagnostic")
        require(digest(executable) == rebuilt, "failed build replaced the last successful executable")
        self.cases("preserved-native", [executable], project, multiplier=2)

        deployment = self.output / "deployment"
        deployment.mkdir()
        deployed = deployment / "byte-count"
        shutil.copy2(executable, deployed)
        shutil.rmtree(project)
        require(not project.exists(), "source tree remains present during deployment check")
        self.cases("deployed", [deployed], deployment, multiplier=2, env={"PATH": ""})
        self.report["deployed_executable_sha256"] = digest(deployed)
        require(digest(deployed) == rebuilt, "deployed executable changed")


def verify(args):
    output = args.output.resolve()
    require(not output.exists() and not args.output.is_symlink(), "output directory must be new")
    require(output != ROOT and ROOT not in output.parents, "output must be outside the repository")
    compilers = [path.resolve(strict=True) for path in args.compilers]
    require(len(compilers) == len(set(compilers)), "compiler paths must be distinct")
    require(all(path.is_file() and os.access(path, os.X_OK) for path in compilers), "compiler must be executable")
    output.mkdir(parents=True)
    sources = {path.relative_to(APPLICATION).as_posix(): digest(path)
               for path in sorted(APPLICATION.rglob("*"))
               if path.is_file() and ".oxid" not in path.parts and path.name != "oxid.lock"}
    report = {"schema": "oxid-typed-package-application-v1", "status": "running",
              "scope": "R1 application regression; not v1 acceptance", "native": args.native,
              "source_sha256": sources, "controller_sha256": digest(Path(__file__)),
              "cases_per_execution": len(CASES), "compilers": []}
    try:
        for index, compiler in enumerate(compilers):
            directory = output / f"compiler-{index}"
            directory.mkdir()
            row = {"path": str(compiler), "sha256": digest(compiler), "status": "running"}
            report["compilers"].append(row)
            Runner(directory, compiler, row).check(args.native)
            require(digest(compiler) == row["sha256"], "compiler changed during verification")
            row["status"] = "passed"
        require(all(digest(APPLICATION / name) == sha for name, sha in sources.items()),
                "application sources changed during verification")
        report["status"] = "passed"
        print(json.dumps({"status": "passed", "compilers": len(compilers), "native": args.native,
                          "cases_per_execution": len(CASES), "output": str(output)}))
        return 0
    except (OSError, ValueError, subprocess.TimeoutExpired) as error:
        report.update(status="failed", error=str(error))
        print(str(error), file=sys.stderr)
        return 1
    finally:
        save(output / "result.json", report)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("compilers", nargs="+", type=Path)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--native", action="store_true")
    args = parser.parse_args()
    try:
        return verify(args)
    except (OSError, ValueError) as error:
        print(str(error), file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
