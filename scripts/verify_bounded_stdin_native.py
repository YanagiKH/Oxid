#!/usr/bin/env python3
"""Current bounded stdin qualification using already built ordinary Rust profiles.

The historical enum wrapper's eight groups/two CLI cases remain independent.
Default CI runs both profiles; --profiles debug permits an honestly labeled local
rehearsal. Cargo is never invoked here. Clang 19.1.7 emits bounded native fixtures.
"""
from __future__ import annotations

import argparse
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import platform
import re
import sys

import verify_bounded_enum_native as enum_gate

HELPERS = Path(__file__).resolve().parents[1] / "tests/qualification/bounded_stdin_current"
spec = importlib.util.spec_from_file_location("bounded_stdin_controls", HELPERS / "controls.py")
controls = importlib.util.module_from_spec(spec)
spec.loader.exec_module(controls)
require = controls.require
REVIEWED_SOURCE_SHA256 = '9e9e65c9ba3b034074ca22cb0967d6820ff5b5f8907613fcb36720b97519b0f8'
REVIEWED_SOURCE_MEMBERS = 376
REVIEWED_COMPILER_BODIES = 291
MANIFEST_PATH = "tests/fixtures/typed_project_source_binding/current-source.json"
CORE_PATHS = ("src", "native", "Cargo.toml", "Cargo.lock", "build.rs")
HELPER_FILES = ("controls.py", "read_retry_shim.c", "calibration_probe.c", "smoke.txt", "test_controller.py")


def source_manifest(repo, digest):
    require(digest == REVIEWED_SOURCE_SHA256, "source seal is not the reviewed current authority")
    data = controls.read_file(repo / MANIFEST_PATH, 1024 * 1024)
    require(hashlib.sha256(data).hexdigest() == digest, "current source manifest seal differs")
    manifest = json.loads(data)
    rows = manifest["files"]
    require(len(rows) == REVIEWED_SOURCE_MEMBERS and len({row["path"] for row in rows}) == REVIEWED_SOURCE_MEMBERS,
            "current source authority requires the exact reviewed unique membership")
    for row in rows:
        path = Path(row["path"])
        require(not path.is_absolute() and path.as_posix() == row["path"]
                and ".." not in path.parts, "unsafe manifest path")
    return manifest


def source_identity(repo, manifest, tracked_core):
    expected = {row["path"]: row for row in manifest["files"]}
    core = {name for name in expected if name.startswith(("src/", "native/"))
            or name in ("Cargo.toml", "Cargo.lock", "build.rs")}
    actual_core = [os.fsdecode(name) for name in tracked_core.split(b"\0") if name]
    require(len(core) == REVIEWED_COMPILER_BODIES and len(actual_core) == len(set(actual_core))
            and set(actual_core) == core, "compiler/native tracked source closure differs")
    observed = []
    for name, row in expected.items():
        current = controls.identity(repo / name)
        require((current["bytes"], current["sha256"]) == (row["bytes"], row["sha256"]),
                "reviewed input changed: " + name)
        observed.append(current)
    return observed


def admit_build_artifact(data, repo, target, profile, unit):
    rows = [json.loads(line) for line in data.splitlines() if line]
    require([row.get("success") for row in rows if row.get("reason") == "build-finished"] == [True],
            "missing successful Cargo build completion")
    matches = [row for row in rows if row.get("reason") == "compiler-artifact"
        and row.get("target", {}).get("name") == "oxid" and row.get("target", {}).get("kind") == ["bin"]
        and row.get("profile", {}).get("test") is unit and row.get("executable")]
    require(len(matches) == 1, "missing or ambiguous prebuilt oxid artifact")
    row = matches[0]
    flags = row["profile"]
    require(flags.get("opt_level") == ("0" if profile == "debug" else "3")
        and flags.get("debug_assertions") is (profile == "debug")
        and flags.get("overflow_checks") is (profile == "debug")
        and type(flags.get("debuginfo")) is int and flags["debuginfo"] == (2 if profile == "debug" else 0),
        "prebuilt artifact does not use the ordinary selected Cargo profile")
    executable = controls.no_symlinks(Path(row["executable"]))
    require((executable.parent == target / profile / "deps" and re.fullmatch(r"oxid-[a-f0-9]{16}", executable.name))
        if unit else executable == target / profile / "oxid", "unexpected prebuilt artifact location")
    require(Path(row["target"].get("src_path", "")) == repo / "src/cli.rs",
            "prebuilt Cargo artifact is from a different checkout")
    require(controls.read_file(executable).startswith(b"\x7fELF"), "prebuilt artifact is not an ELF")
    return executable, flags


def reused_binaries(evidence, destination, repo, target, profile):
    previous = evidence / profile
    receipt = json.loads(controls.read_file(previous / "binaries.json", 1024 * 1024))
    require(receipt["profile"] == profile, "prebuilt profile label differs")
    found = {}
    retained = []
    for kind, unit in (("unit", True), ("cli", False)):
        stream = previous / (kind + "-build.stdout")
        data = controls.read_file(stream, controls.MAX_STREAM)
        binary, flags = admit_build_artifact(data, repo, target, profile, unit)
        current = controls.identity(binary)
        require(current == receipt["unit" if unit else "production_cli"], "prebuilt binary identity changed")
        (destination / (kind + "-build.stdout")).write_bytes(data)
        # Preserve the actual original invocation, status, and stderr as well.
        for extension in ("json", "stderr"):
            source = previous / (kind + "-build." + extension)
            raw = controls.read_file(source, controls.MAX_STREAM)
            (destination / source.name).write_bytes(raw)
            if extension == "json":
                command = json.loads(raw)
                require(command.get("status") == 0, "prebuilt Cargo command failed")
                expected = ["test", "--locked", "--bin", "oxid", "--no-run", "--message-format=json"] if unit else ["build", "--locked", "--bin", "oxid", "--message-format=json"]
                if profile == "release":
                    expected.append("--release")
                require(command.get("argv", [])[1:] == expected, "nonstandard prebuilt Cargo invocation")
                require(command.get("cwd") == str(repo), "prebuilt Cargo invocation used another checkout")
        found[kind] = binary
        retained.append({"kind": kind, "identity": current, "cargo_profile": flags,
                         "cargo_artifact_stream": controls.identity(stream)})
    controls.save_json(destination / "prebuilt-binaries.json", {"profile": profile, "binaries": retained})
    return found


def build_retry_instrumentation(root, clang, env):
    target = controls.fresh(root / "retry-instrumentation")
    shim, probe = target / "libstdin_retry.so", target / "calibration_probe"
    flags = ["-std=c11", "-O2", "-Wall", "-Wextra", "-Werror"]
    for name, source, output, extra in (
        ("shim", "read_retry_shim.c", shim, ["-fPIC", "-fvisibility=hidden", "-shared", "-Wl,-z,defs", "-Wl,-z,relro,-z,now"]),
        ("probe", "calibration_probe.c", probe, []),
    ):
        result, unread = controls.run(target / ("compile-" + name),
            [clang, *flags, *extra, HELPERS / source, "-o", output], env, timeout=120)
        require((result.returncode, result.stdout, result.stderr, unread) == (0, b"", b"", b""),
                "retry instrumentation compilation failed")
    controls.calibrate(target / "calibration", probe, shim)
    controls.save_json(target / "artifacts.json", {"shim": controls.identity(shim), "probe": controls.identity(probe),
        "sources": [controls.identity(HELPERS / name) for name in ("read_retry_shim.c", "calibration_probe.c")]})
    return shim


def verify(args, root):
    require(platform.system() == "Linux" and platform.machine() == "x86_64", "Linux x86_64 required")
    repo = controls.no_symlinks(args.repo).resolve(strict=True)
    llvm = args.llvm_bin.resolve(strict=True)
    evidence = controls.no_symlinks(args.build_evidence).resolve(strict=True)
    target = controls.no_symlinks(args.target_dir or repo / "target").resolve(strict=True)
    require(not os.environ.get("RUSTFLAGS") and not os.environ.get("CARGO_ENCODED_RUSTFLAGS"),
            "ordinary profiles require no additional Rust flags")
    env = enum_gate.source_test_environment(repo, controls.child_env(dict(os.environ, OXID_LLVM_BIN=str(llvm))))
    head, _ = enum_gate.git_command(root, "git-head", repo, "rev-parse", "HEAD", "HEAD^{tree}", env=env)
    observed_head, tree = head.decode().splitlines()
    require(observed_head == args.expected_head, "checkout is not the exact requested head")
    status, _ = enum_gate.git_command(root, "git-status", repo, "status", "--porcelain=v1", env=env)
    require(not status, "current checkout must be clean")
    manifest = source_manifest(repo, args.source_manifest_sha256)
    core, _ = enum_gate.git_command(root, "source-files", repo, "ls-files", "-z", "--", *CORE_PATHS, env=env)
    identities = source_identity(repo, manifest, core)
    previous = json.loads(controls.read_file(evidence / "source-identity.json", controls.MAX_STREAM))
    require(previous.get("head") == observed_head and previous.get("tree") == tree
        and previous.get("reviewed_source_manifest", {}).get("sha256") == args.source_manifest_sha256,
        "prebuilt evidence is not bound to this exact head/tree/source authority")
    current_manifest = controls.identity(repo / MANIFEST_PATH)
    (root / "reviewed-source.json").write_bytes(controls.read_file(repo / MANIFEST_PATH))
    helpers = [controls.identity(HELPERS / name) for name in HELPER_FILES]
    entrypoint = controls.identity(Path(__file__))
    controls.save_json(root / "source-identity.json", {"head": observed_head, "tree": tree, "event_sha": args.event_sha,
        "profiles": args.profiles, "reviewed_source_manifest": current_manifest, "reviewed_inputs": identities,
        "helper_inputs": helpers, "entrypoint": entrypoint, "prebuilt_source_receipt": controls.identity(evidence / "source-identity.json")})
    tools = {}
    for name, marker in (("clang", "clang version"), ("llvm-as", "LLVM version"), ("opt", "LLVM version"), ("ld.lld", "LLD")):
        tool = llvm / name
        result, _ = controls.run(root / ("tool-" + name), [tool, "--version"], env)
        require(result.returncode == 0 and not result.stderr, "native tool version failed")
        enum_gate.admit_tool_version(name, marker, result.stdout)
        tools[name] = enum_gate.identity(tool.resolve(strict=True))
    controls.save_json(root / "tools.json", tools)
    shim = build_retry_instrumentation(root, llvm / "clang", env)
    for profile in args.profiles:
        directory = controls.fresh(root / profile)
        binaries = reused_binaries(evidence, directory, repo, target, profile)
        before = {name: controls.identity(binary) for name, binary in binaries.items()}
        for name in (controls.RAW_TEST, controls.SOURCE_TEST):
            label = "raw" if name == controls.RAW_TEST else "source"
            result, _ = controls.run(directory / (label + "-discovery"),
                [binaries["unit"], name, "--exact", "--list", "--color=never"], env)
            require(result.returncode == 0 and not result.stderr, "child discovery failed")
            controls.admit_listing(result.stdout, name)
            # No effect selectors: prove both ordinary nonignored children inert.
            result, _ = controls.run(directory / (label + "-inert"),
                controls.test_command(binaries["unit"], name), controls.child_env(env), b"UNCHANGED")
            require(controls.admit_child(result, name, ()) == {} and not result.stderr,
                    "ordinary child was not inert")
            require((directory / (label + "-inert") / "remaining.bin").read_bytes() == b"UNCHANGED",
                    "inert child consumed stdin")
        suite = controls.Suite(directory, binaries["unit"], binaries["cli"], env, shim)
        suite.raw_and_shapes()
        suite.fuel()
        suite.sources()
        suite.finish()
        require(before == {name: controls.identity(binary) for name, binary in binaries.items()},
                "prebuilt binaries changed during execution")
    core, _ = enum_gate.git_command(root, "source-files-after", repo, "ls-files", "-z", "--", *CORE_PATHS, env=env)
    require(source_identity(repo, manifest, core) == identities and controls.identity(repo / MANIFEST_PATH) == current_manifest,
            "source authority changed during execution")
    require(helpers == [controls.identity(HELPERS / name) for name in HELPER_FILES]
            and controls.identity(Path(__file__)) == entrypoint, "controller inputs changed during execution")
    after, _ = enum_gate.git_command(root, "git-head-after", repo, "rev-parse", "HEAD", "HEAD^{tree}", env=env)
    status, _ = enum_gate.git_command(root, "git-status-after", repo, "status", "--porcelain=v1", env=env)
    require(after == head and not status, "checkout changed during execution")
    controls.save_json(root / "source-identity-after.json", {"head": observed_head, "tree": tree,
        "reviewed_source_manifest": current_manifest, "reviewed_inputs": identities})


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ("repo", "output", "build-evidence", "llvm-bin"):
        parser.add_argument("--" + name, type=Path, required=True)
    for name in ("expected-head", "event-sha", "source-manifest-sha256"):
        parser.add_argument("--" + name, required=True)
    parser.add_argument("--target-dir", type=Path)
    parser.add_argument("--profiles", choices=("debug", "release", "debug,release"), default="debug,release")
    args = parser.parse_args()
    args.profiles = args.profiles.split(",")
    root = controls.fresh(args.output)
    result = {"status": "failed", "profiles": args.profiles, "paired_cases_per_profile": 77,
        "roster_per_profile": controls.ROSTER, "scope": "current bounded stdin; historical enum groups remain separate"}
    try:
        verify(args, root)
        result["status"] = "passed"
    except Exception as error:
        result["error"] = str(error)
        raise
    finally:
        controls.save_json(root / "result.json", result)
    print(f"bounded stdin native: {','.join(args.profiles)}; {77 * len(args.profiles)} paired cases: PASS")


if __name__ == "__main__":
    main()
