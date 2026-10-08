#!/usr/bin/env python3
"""Build and qualify exact-head local HIR producers in both ordinary profiles.

The output directory is new and outside the checkout. Every command retains
streams and status; failure leaves an explicitly incomplete top-level receipt.
No dependency installation, source rebinding or shell command execution occurs.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys
import time


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def require(condition, message):
    if not condition:
        raise RuntimeError(message)


def select_executable(stdout, target_dir, profile):
    """Admit the real non-test oxid binary from this successful Cargo build."""
    records = [json.loads(line) for line in stdout.splitlines() if line.strip()]
    require(any(item.get("reason") == "build-finished" and item.get("success") is True
                for item in records), "Cargo did not report successful build completion")
    binaries = [item for item in records if item.get("reason") == "compiler-artifact"
                and item.get("target", {}).get("name") == "oxid"
                and item.get("target", {}).get("kind") == ["bin"]
                and item.get("profile", {}).get("test") is False
                and item.get("executable") is not None]
    require(len(binaries) == 1, "expected exactly one non-test oxid executable receipt")
    record = binaries[0]
    require(record["profile"].get("opt_level") == ("0" if profile == "debug" else "3"),
            "Cargo receipt is not the selected ordinary profile")
    executable = Path(record["executable"])
    require(not executable.is_symlink(), "Cargo executable must not be a symlink")
    expected = target_dir / profile / "oxid"
    require(executable.resolve(strict=True) == expected.resolve(strict=True),
            "Cargo executable escaped this profile's isolated target directory")
    require(executable.is_file(), "Cargo executable is not a regular file")
    return executable.resolve(), record


class Runner:
    def __init__(self, args):
        self.repo = Path(__file__).resolve().parents[1]
        self.output = args.output.resolve()
        require(not self.output.is_relative_to(self.repo), "evidence must be outside the checkout")
        self.output.mkdir()
        self.args = args
        self.env = dict(os.environ)
        forbidden = [name for name in self.env if name.startswith("CARGO_PROFILE_")
                     or name in ("RUSTFLAGS", "CARGO_ENCODED_RUSTFLAGS", "RUSTC_WRAPPER",
                                 "RUSTC_WORKSPACE_WRAPPER", "RUSTC", "RUSTDOC",
                                 "CARGO_BUILD_RUSTFLAGS", "CARGO_BUILD_RUSTC_WRAPPER",
                                 "CARGO_BUILD_RUSTC_WORKSPACE_WRAPPER")]
        self.forbidden = forbidden
        self.env.update(CARGO_BUILD_JOBS="2", CARGO_INCREMENTAL="0")
        self.target = self.output / "target"
        self.receipt = dict(schema_version=1, complete=False, expected_head=args.expected_head,
                            event_sha=args.event_sha, commands=[], profiles={})
        self.save()

    def save(self):
        (self.output / "receipt.json").write_text(json.dumps(self.receipt, indent=2) + "\n")

    def call(self, name, command, timeout=1200):
        command = [str(value) for value in command]
        started = time.monotonic()
        try:
            result = subprocess.run(command, cwd=self.repo, env=self.env,
                                    stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=timeout)
            stdout, stderr, status = result.stdout, result.stderr, result.returncode
        except subprocess.TimeoutExpired as error:
            stdout, stderr, status = error.stdout or b"", error.stderr or b"", "timeout"
        except OSError as error:
            stdout, stderr, status = b"", str(error).encode(), "spawn-failed"
        out, err = self.output / (name + ".stdout"), self.output / (name + ".stderr")
        out.write_bytes(stdout)
        err.write_bytes(stderr)
        self.receipt["commands"].append(dict(name=name, argv=command, status=status,
            elapsed_seconds=time.monotonic() - started, stdout_sha256=digest(out), stderr_sha256=digest(err)))
        self.save()
        require(status == 0, name + " failed; see preserved streams and receipt")
        return stdout

    def identity(self, label):
        head = self.call(label + "-head", ["git", "rev-parse", "HEAD"]).decode().strip()
        require(head == self.args.expected_head, "checkout HEAD differs from expected exact head")
        status = self.call(label + "-status", ["git", "status", "--porcelain=v1", "--untracked-files=all"])
        require(not status, "qualification requires a clean source checkout")
        tree = self.call(label + "-tree", ["git", "rev-parse", "HEAD^{tree}"]).decode().strip()
        return dict(head=head, tree=tree)

    def run(self):
        require(not self.forbidden, "ordinary-profile qualification forbids overrides: " + ", ".join(self.forbidden))
        initial = self.identity("initial")
        self.receipt["source"] = initial
        harness = self.repo / "scripts/qualify_hir_producers.py"
        self.receipt["harness_sha256"] = digest(harness)
        self.receipt["wrapper_sha256"] = digest(Path(__file__))
        rust = self.call("rust-version", ["rustc", "-Vv"]).decode()
        require(re.search(r"^release: 1\.99\.0$", rust, re.MULTILINE), "requires qualified Rust 1.99.0")
        self.call("cargo-version", [self.args.cargo, "-V"])
        for tool in ("clang", "llc", "ld.lld"):
            version = self.call(tool + "-version", [self.args.llvm_bin / tool, "--version"]).decode()
            require("19.1.7" in version, "requires qualified LLVM 19.1.7 " + tool)
        self.call("cc-version", [self.args.cc, "--version"])
        # CI explicitly fetches the full lockfile first. Builds cannot resolve a
        # new dependency or use the network to mask an incomplete locked cache.
        for profile in ("debug", "release"):
            command = [self.args.cargo, "build", "--locked", "--offline", "--bin", "oxid",
                       "--message-format=json", "--target-dir", self.target]
            if profile == "release":
                command.append("--release")
            cargo_output = self.call(profile + "-build", command)
            executable, artifact = select_executable(cargo_output, self.target, profile)
            binary_hash = digest(executable)
            retained = self.output / (profile + "-compiler")
            shutil.copy2(executable, retained)
            require(digest(retained) == binary_hash, "retained compiler copy differs from Cargo executable")
            entry = dict(executable=str(executable), retained_executable=str(retained),
                         executable_sha256=binary_hash,
                         cargo_artifact=artifact, source=initial, complete=False)
            self.receipt["profiles"][profile] = entry
            self.save()
            evidence = self.output / (profile + "-qualification")
            self.call(profile + "-qualification", [sys.executable, "-B", harness,
                      "--compiler", executable, "--llvm-bin", self.args.llvm_bin,
                      "--cc", self.args.cc, "--output", evidence])
            summary = json.loads((evidence / "summary.json").read_text())
            require(summary.get("result") == "passed" and summary.get("negative_cli_probes") == 76,
                    "qualification summary did not complete its exact recipe")
            require(summary.get("source_head") == initial["head"]
                    and summary.get("compiler_sha256") == binary_hash
                    and summary.get("harness_sha256") == self.receipt["harness_sha256"],
                    "qualification identities differ from the selected build")
            require(digest(executable) == binary_hash and digest(retained) == binary_hash,
                    "compiler changed during qualification")
            require(self.identity(profile + "-final") == initial, "source changed during qualification")
            entry.update(complete=True, summary_sha256=digest(evidence / "summary.json"))
            self.save()
        require(digest(harness) == self.receipt["harness_sha256"], "qualification harness changed")
        self.receipt["complete"] = True
        self.save()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--expected-head", required=True)
    parser.add_argument("--event-sha", required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--llvm-bin", type=Path, required=True)
    parser.add_argument("--cargo", default="cargo")
    parser.add_argument("--cc", default="cc")
    args = parser.parse_args()
    for name in ("expected_head", "event_sha"):
        if not re.fullmatch(r"[0-9a-f]{40}", getattr(args, name)):
            parser.error(name + " must be an exact lowercase commit SHA")
    args.llvm_bin = args.llvm_bin.resolve(strict=True)
    runner = None
    try:
        runner = Runner(args)
        runner.run()
    except Exception as error:
        if runner is not None:
            runner.receipt["error"] = str(error)
            runner.save()
        print(str(error), file=sys.stderr)
        return 1
    print(json.dumps({"complete": True, "receipt": str(runner.output / "receipt.json")}))
    return 0


if __name__ == "__main__":
    sys.exit(main())
