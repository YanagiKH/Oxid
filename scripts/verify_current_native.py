#!/usr/bin/env python3
"""Run existing source-independent native regressions on a clean current checkout.

The oracle and Rust test bodies are unchanged. This runner records their actual
builds, commands and results; it does not reissue historical Unit3/Unit4 proofs
or establish v1 readiness. Requires Linux x86_64, Rust and LLVM 19.1.7.
"""
import argparse
import hashlib
import json
import math
import os
from pathlib import Path
import platform
import re
import signal
import subprocess
import sys
import time


EXACT_TESTS = (
    ("boolean", "frontend::oir::logical_tests::raw_bool_merge_uses_real_llvm"),
    ("mutable", "frontend::oir::mutable_tests::raw_mutable_places_use_real_llvm"),
    ("cyclic", "frontend::oir::verify::cyclic_tests::execution::raw_cyclic_witnesses_use_real_llvm"),
    ("loop-control", "frontend::oir::loop_control_tests::loop_control_uses_real_llvm"),
    ("array-consumers", "frontend::oir::owned::source::array_consumer_tests::source_array_consumers_use_real_llvm"),
    ("heldout-source", "frontend::oir::owned::source::reviewer_source::reviewer_source_native_heldout_successes_and_failures"),
)
GROUP_TESTS = (
    ("negation", "raw_negation", "--include-ignored", "OXID_UNARY_NATIVE_EVIDENCE"),
    ("slices", "native_slices", "--ignored", "OXID_OWNED_NATIVE_EVIDENCE"),
    ("composition", "native_composition", "--include-ignored", "OXID_OWNED_NATIVE_EVIDENCE"),
    ("inventory", "native_inventory", "--include-ignored", "OXID_OWNED_NATIVE_EVIDENCE"),
    ("projected-slices", "native_projected_slices", "--include-ignored", "OXID_OWNED_NATIVE_EVIDENCE"),
)
COMPONENTS = (
    ("expression", "verify_expression_component.py", ()),
    ("expression-stdin", "verify_expression_stdin.py", ()),
    ("stack", "verify_stack_component.py", ()),
    ("stack-stdin", "verify_expression_stdin.py", ("--component", "stack")),
)


def require(condition, message):
    if not condition:
        raise ValueError(message)


def digest(path):
    with Path(path).open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def save(path, value):
    path.write_text(json.dumps(value, indent=2, sort_keys=True) + "\n")


def fresh_output(repo, output):
    require(not output.exists() and not output.is_symlink(), "output must be new")
    output = output.resolve()
    require(output != repo and repo not in output.parents, "output must be outside repository")
    output.mkdir(parents=True)
    return output


def built_executable(log, test):
    rows = [json.loads(line) for line in log.read_text().splitlines() if line.strip()]
    require([r.get("success") for r in rows if r.get("reason") == "build-finished"] == [True],
            "missing successful Cargo build receipt")
    matches = [r["executable"] for r in rows
               if r.get("reason") == "compiler-artifact"
               and r.get("target", {}).get("name") == "oxid"
               and r.get("target", {}).get("kind") == ["bin"]
               and r.get("profile", {}).get("test") is test and r.get("executable")]
    require(len(matches) == 1, "Cargo must identify exactly one requested executable")
    binary = Path(matches[0]).resolve(strict=True)
    require(binary.is_file() and os.access(binary, os.X_OK), "built executable is not runnable")
    return binary


def selected_tests(body, exact=None):
    names = re.findall(r"^(\S+): test$", body, re.M)
    require(names and len(names) == len(set(names)), "empty or duplicate Rust test selection")
    if exact is not None:
        require(names == [exact], "exact Rust test selection changed")
    return names


def check_test_result(body, names):
    summaries = re.findall(r"^test result: ok\. (\d+) passed; 0 failed; 0 ignored;", body, re.M)
    require(summaries == [str(len(names))], "Rust test count/result differs from selection")


def native_artifacts(directory):
    require(directory.is_dir() and any(directory.rglob("*.ll"))
            and any(directory.rglob("*.elf")), "missing retained LLVM/ELF evidence: " + str(directory))


class Runner:
    def __init__(self, repo, output, llvm, timeout):
        self.repo, self.output, self.timeout = repo, output, timeout
        self.environment = {"OXID_LLVM_BIN": str(llvm), "PYTHONDONTWRITEBYTECODE": "1",
                            "CARGO_TERM_COLOR": "never"}
        self.commands = []
        (output / "commands").mkdir()

    def command(self, name, argv, extra=None):
        require(name not in [r["name"] for r in self.commands], "duplicate command name")
        directory = self.output / "commands" / name
        directory.mkdir()
        environment = {**self.environment, **(extra or {})}
        row = {"name": name, "argv": [str(a) for a in argv], "cwd": str(self.repo),
               "environment_overrides": environment, "status": "running",
               "timeout_seconds": self.timeout}
        self.commands.append(row)
        save(directory / "command.json", row)
        started = time.monotonic()
        process = None
        print(name + ": running", flush=True)
        try:
            with (directory / "stdout").open("xb") as stdout, (directory / "stderr").open("xb") as stderr:
                process = subprocess.Popen(row["argv"], cwd=self.repo, stdin=subprocess.DEVNULL,
                                           stdout=stdout, stderr=stderr,
                                           env={**os.environ, **environment}, start_new_session=True)
                row["exit_code"] = process.wait(timeout=self.timeout)
            require(row["exit_code"] == 0, name + ": command failed; inspect " + str(directory))
            row["status"] = "passed"
        except BaseException as error:
            if process is not None and process.poll() is None:
                os.killpg(process.pid, signal.SIGTERM)
                try:
                    process.wait(timeout=3)
                except subprocess.TimeoutExpired:
                    os.killpg(process.pid, signal.SIGKILL)
                    process.wait()
            row.update(status="failed", error=str(error))
            raise
        finally:
            row["elapsed_seconds"] = time.monotonic() - started
            for stream in ("stdout", "stderr"):
                path = directory / stream
                if path.is_file():
                    row[stream + "_sha256"] = digest(path)
            save(directory / "command.json", row)
        print(name + ": passed", flush=True)
        return directory

    def checkout(self, stage):
        head = self.command("head-" + stage, ["git", "rev-parse", "HEAD"])
        status = self.command("status-" + stage, ["git", "status", "--porcelain=v1", "--untracked-files=all"])
        require(not (status / "stdout").read_bytes(), "native regression requires a clean checkout")
        return (head / "stdout").read_text().strip()

    def rust_tests(self, name, binary, selection, extra=None, exact=None):
        listed = self.command(name + "-list", [binary, *selection, "--list"], extra)
        names = selected_tests((listed / "stdout").read_text(), exact)
        result = self.command(name, [binary, *selection, "--test-threads=1", "--nocapture"], extra)
        check_test_result((result / "stdout").read_text(), names)
        save(result / "selection.json", names)


def verify(args):
    require(__debug__, "native regression refuses optimized Python")
    require(platform.system() == "Linux" and platform.machine() == "x86_64",
            "native regression requires Linux x86_64")
    require(math.isfinite(args.timeout) and args.timeout > 0, "timeout must be positive and finite")
    repo = args.repo.resolve(strict=True)
    require(Path(__file__).resolve() == repo / "scripts/verify_current_native.py",
            "run the controller from the selected checkout")
    llvm = args.llvm_bin.resolve(strict=True)
    output = fresh_output(repo, args.output)
    runner = Runner(repo, output, llvm, args.timeout)
    report = {"schema": "oxid-current-native-regression-v1", "status": "running",
              "scope": "current native regression; not full Unit3/Unit4 or v1 qualification",
              "controller_sha256": digest(Path(__file__)), "commands": runner.commands,
              "binaries": {}}
    save(output / "result.json", report)
    python = [sys.executable, "-B"]
    try:
        report["head"] = runner.checkout("before")
        for profile in ("debug", "release"):
            profile_flags = ["--release"] if profile == "release" else []
            evidence = output / profile
            evidence.mkdir()
            build = runner.command(profile + "-build", ["cargo", "build", "--locked", "--bin", "oxid",
                                                       "--message-format=json", *profile_flags])
            binary = built_executable(build / "stdout", False)
            build = runner.command(profile + "-test-build", ["cargo", "test", "--locked", "--bin", "oxid",
                                                            "--no-run", "--message-format=json", *profile_flags])
            test_binary = built_executable(build / "stdout", True)
            report["binaries"][profile] = {"compiler": str(binary), "compiler_sha256": digest(binary),
                                            "tests": str(test_binary), "tests_sha256": digest(test_binary)}
            for label, test in EXACT_TESTS:
                extra = ({"OXID_REVIEWER_SOURCE_NATIVE_EVIDENCE": str(evidence / label)}
                         if label == "heldout-source" else None)
                runner.rust_tests(profile + "-" + label, test_binary, [test, "--ignored", "--exact"],
                                  extra, exact=test)
                if extra:
                    native_artifacts(evidence / label)
            for label, test, mode, variable in GROUP_TESTS:
                runner.rust_tests(profile + "-" + label, test_binary, [test, mode],
                                  {variable: str(evidence / label)})
                native_artifacts(evidence / label)
            hir = evidence / "checked-hir"
            runner.command(profile + "-checked-hir", [*python, "scripts/verify_checked_hir_import_native.py",
                           "--test-binary", test_binary, "--llvm-bin", llvm, "--evidence", hir])
            manifest = json.loads((hir / "manifest.json").read_bytes())
            require(manifest.get("result") == "passed" and manifest.get("head") == report["head"]
                    and manifest.get("test_binary_sha256") == digest(test_binary), "checked-HIR evidence mismatch")
            native_artifacts(hir)
            for label, script, options in COMPONENTS:
                destination = evidence / label
                runner.command(profile + "-" + label, [*python, "scripts/" + script, "--oxid", binary,
                               "--output", destination, "--native", *options])
                result = json.loads((destination / "report.json").read_bytes())
                require(result.get("passed") is True and result.get("native") is True and result.get("cases")
                        and result.get("compiler_sha256") == digest(binary), "public component evidence mismatch")
        binaries = [report["binaries"][p]["compiler"] for p in ("debug", "release")]
        suite = runner.command("native-suite", [*python, "scripts/verify_native_suite.py", *binaries, "--jobs", "2"])
        require((suite / "stdout").read_text().splitlines()[-2:] == [
                    "native suite: 7/7 passed; jobs=2; exit=0",
                    "native suite compiler SHA256: " + " ".join(digest(p) for p in binaries)],
                "missing complete native-suite result")
        owned = output / "owned-cli"
        runner.command("owned-cli", [*python, "scripts/verify_owned_source.py", *binaries, "--mode", "cli",
                       "--cli-amendment", "projected-array-slices-v1", "--jobs", "2", "--evidence", owned])
        result = json.loads((owned / "production-cli-result.json").read_bytes())
        require(result.get("status") == "CLI_ELF_PASS" and result.get("full_qualification") is False,
                "missing owned-source CLI result")
        require([p.get("profile") for p in result.get("profiles", [])] == ["debug", "release"],
                "missing owned-source profile results")
        for row in result["profiles"]:
            require(row.get("compiler_sha256") == report["binaries"][row["profile"]]["compiler_sha256"]
                    and row.get("counts", {}).get("native_executions", 0) > 0, "owned-source compiler/run mismatch")
        require(runner.checkout("after") == report["head"], "checkout changed during native regression")
        for row in report["binaries"].values():
            for role in ("compiler", "tests"):
                require(digest(row[role]) == row[role + "_sha256"], "built executable changed")
        report["status"] = "passed"
        return 0
    except (Exception, KeyboardInterrupt) as error:
        report.update(status="failed", error=str(error))
        print("current native regression failed: " + str(error), file=sys.stderr)
        return 1
    finally:
        save(output / "result.json", report)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--llvm-bin", type=Path, required=True)
    parser.add_argument("--timeout", type=float, default=3600, help="per-command timeout in seconds")
    args = parser.parse_args()
    try:
        return verify(args)
    except (OSError, ValueError) as error:
        print("current native regression failed: " + str(error), file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
