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

from qualify_hir_producers_v2 import validate_summary as validate_v2_summary


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


# Reviewed af436c1 source closure. An internally consistent edited manifest is
# still a different recipe and must not silently broaden this qualification.
V2_SOURCE_MANIFEST_SHA256 = '08629aee51a25e05fd7bf4b121ff8a7d882683687425f4e5469f8b508618d608'
V2_SOURCE_MANIFEST_BYTES = 5461


def validate_source_manifest(path):
    require(path.stat().st_size == V2_SOURCE_MANIFEST_BYTES and digest(path) == V2_SOURCE_MANIFEST_SHA256,
            'v2 source manifest differs from the reviewed frozen closure')


def validate_v1_summary(summary, source, compiler_hash, harness_hash):
    require(isinstance(summary, dict) and summary.get('result') == 'passed'
            and type(summary.get('negative_cli_probes')) is int
            and summary['negative_cli_probes'] == 76,
            'v1 qualification summary did not complete its exact recipe')
    require(summary.get('source_head') == source['head']
            and summary.get('compiler_sha256') == compiler_hash
            and summary.get('harness_sha256') == harness_hash,
            'v1 qualification identities differ from the selected build')


def validate_edge_summary(evidence, compiler, parser_hash):
    summary = json.loads((evidence / 'summary.json').read_text())
    require(isinstance(summary, dict) and summary.get('status') == 'passed', 'invalid v2 edge summary')
    probe = summary.get('actual_parser_probe_io_error')
    require(isinstance(probe, dict) and probe.get('parser_sha256') == parser_hash,
            'v2 edge parser identity mismatch')
    for key, value in (('unread_bytes', 0), ('fd', 0), ('requested_bytes', 1), ('status', 74)):
        require(type(probe.get(key)) is int and probe[key] == value, 'incomplete v2 EOF-probe evidence')
    require(probe.get('syscall') == 'read', 'v2 EOF-probe did not observe read')
    span = summary.get('synthetic_secondary_span')
    require(isinstance(span, dict) and type(span.get('start')) is int and span['start'] == 254
            and type(span.get('end')) is int and span['end'] == 255, 'incomplete v2 secondary-span evidence')
    require(span.get('executable_sha256') == digest(evidence / 'secondary-boundary')
            and span.get('source_sha256') == digest(evidence / 'sources/secondary_boundary.ox'),
            'v2 secondary-span identity mismatch')
    command = span.get('argv')
    require(isinstance(command, list) and command and command[0] == str(compiler),
            'v2 edge selected compiler mismatch')
    require((evidence / 'secondary.stdout').read_bytes() == b'STF2' + bytes([1, 0, 3, 1, 2, 254, 255, 1, 0, 0, 0, 0]),
            'v2 secondary-span output mismatch')
    for name in ('probe-error.stdout', 'probe-error.stderr', 'secondary.stderr'):
        require(not (evidence / name).read_bytes(), 'v2 edge emitted unexpected output')
    return summary


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
        v2_harness = self.repo / "scripts/qualify_hir_producers_v2.py"
        edge_harness = self.repo / "scripts/verify_hir_v2_edge_controls.py"
        v2_paths = dict(harness_sha256=v2_harness,
                        builder_sha256=self.repo / "scripts/build_hir_producers_v2.py",
                        source_manifest_sha256=self.repo / "fixtures/typed-frontend-v2/sources.json")
        validate_source_manifest(v2_paths['source_manifest_sha256'])
        self.receipt["v2_inputs"] = {key: digest(path) for key, path in v2_paths.items()}
        self.receipt["edge_harness_sha256"] = digest(edge_harness)
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
            validate_v1_summary(summary, initial, binary_hash, self.receipt['harness_sha256'])
            v2_evidence = self.output / (profile + '-v2-qualification')
            self.call(profile + '-v2-qualification', [sys.executable, '-B', v2_harness,
                      '--compiler', executable, '--llvm-bin', self.args.llvm_bin,
                      '--output', v2_evidence])
            expected_v2 = dict(self.receipt['v2_inputs'], source_head=initial['head'],
                               source_tree=initial['tree'], compiler_sha256=binary_hash)
            v2_summary = validate_v2_summary(v2_evidence, expected_v2)
            edge_evidence = self.output / (profile + '-v2-edge-controls')
            self.call(profile + '-v2-edge-controls', [sys.executable, '-B', edge_harness,
                      '--compiler', executable, '--llvm-bin', self.args.llvm_bin,
                      '--bundle', v2_evidence / 'v2-build/bundle', '--output', edge_evidence])
            validate_edge_summary(edge_evidence, executable, v2_summary['executable_sha256']['parser'])
            validate_v2_summary(v2_evidence, expected_v2)
            require(digest(executable) == binary_hash and digest(retained) == binary_hash,
                    "compiler changed during qualification")
            require(self.identity(profile + "-final") == initial, "source changed during qualification")
            entry.update(complete=True, summary_sha256=digest(evidence / "summary.json"),
                         v2_summary_sha256=digest(v2_evidence / 'summary.json'),
                         v2_edge_summary_sha256=digest(edge_evidence / 'summary.json'))
            self.save()
        require(digest(harness) == self.receipt["harness_sha256"], "qualification harness changed")
        require({key: digest(path) for key, path in v2_paths.items()} == self.receipt['v2_inputs'],
                'v2 qualification inputs changed')
        require(digest(edge_harness) == self.receipt['edge_harness_sha256'], 'v2 edge harness changed')
        require(digest(Path(__file__)) == self.receipt['wrapper_sha256'], 'qualification wrapper changed')
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
