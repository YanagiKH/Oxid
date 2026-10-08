#!/usr/bin/env python3
"""Run the eight existing enum native gates and two frozen public CLI cases.

Linux x86_64 / LLVM 19.1.7, ordinary debug and release profiles only. This is a
small CI entry point, not a replacement for the existing Rust evidence harnesses.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess


RAW_PREFIX = "frontend::oir::owned::native::tests::enums::"
RAW_NAMES = tuple(RAW_PREFIX + name for name in (
    "native_enums_source_free_all_payloads_orders_sites_loops_and_every_fuel",
    "native_enums_source_free_all_transfer_faults_and_no_partial_write",
    "native_enums_source_free_changed_variants_and_later_owned_input_boundary",
    "native_enums_source_free_discard_and_inactive_moved_uninitialized_poison",
    "native_enums_source_free_guards_precede_every_binding_and_transfer_effect",
    "native_enums_source_free_invalid_dispatch_consume_and_active_bytes",
))
SOURCE_PREFIX = "frontend::oir::owned::source::enum_native_source_tests::"
# Success deliberately precedes the existing overflow/fuel gate.
SOURCE_NAMES = tuple(SOURCE_PREFIX + name for name in (
    "bounded_enum_pipeline_native_tiny_and_scanner_source_free",
    "bounded_enum_pipeline_native_scanner_overflow_and_tiny_fuel",
))
# Frozen tiny_relay_take from source-enum-independent-fixtures-d6249e7.json.
# Literal expected outputs are independent of any observed compiler output.
TINY = b"enum E{N,V(i32)} fn relay(x:E)->E{return x;} fn take(x:E)->i32{match x{E::N=>{return 0;},E::V(v)=>{return v;},}} fn main()->i32{return take(relay(E::V(7)));}"
TINY_SHA256 = "95bbfb95b733bc272e2b11e6772b66a74bf14507f4a86a7f3e9258d4c88d2220"
SCANNER_HASHES = {
    "main.ox": "768232ca66cd91a6439cf21393a8666f4ec10bcfac4d097caece354440278c3a",
    "scanner.ox": "d13847eaecd69667031e54bb6f3f329365427a483e0bff537d0b28882516ac14",
}
ELF_ENV = {"PATH": "/no-tools"}
# Reviewed native inventory successor executes the unchanged enum roster. The exact
# 237-member enum predecessor remains in enum-source.json; current execution
# uses the complete 330-member authority, never a caller-supplied subset.
REVIEWED_SOURCE_SHA256 = 'e9f92670a63edd0336bada42e03702f7e446706c9fe56fa80913c7428025da2c'


def require(condition, message):
    if not condition:
        raise ValueError(message)


def save_json(path, value):
    with path.open("x") as handle:
        json.dump(value, handle, indent=2, sort_keys=True)
        handle.write("\n")


def identity(path):
    with path.open("rb") as handle:
        digest = hashlib.file_digest(handle, "sha256").hexdigest()
    return {"path": str(path), "sha256": digest, "bytes": path.stat().st_size}


def command(root, label, args, *, cwd, env, timeout=1800, environment_mode="inherited"):
    """Keep original streams and invocation even when a command fails/times out."""
    args = [str(arg) for arg in args]
    error = None
    try:
        output = subprocess.run(args, cwd=cwd, env=env, capture_output=True,
                                timeout=timeout, check=False)
        stdout, stderr, status = output.stdout, output.stderr, output.returncode
    except (OSError, subprocess.TimeoutExpired) as caught:
        error = str(caught)
        stdout = getattr(caught, "stdout", None) or b""
        stderr = getattr(caught, "stderr", None) or b""
        status = None
    (root / (label + ".stdout")).write_bytes(stdout)
    (root / (label + ".stderr")).write_bytes(stderr)
    save_json(root / (label + ".json"), {
        "argv": args, "cwd": str(cwd), "status": status, "error": error,
        "environment": ELF_ENV if env == ELF_ENV else {
            "mode": "isolated-git-read", "values": env,
        } if environment_mode == "isolated-git-read" else {
            "mode": "inherited", "OXID_LLVM_BIN": env.get("OXID_LLVM_BIN"),
            "OXID_OWNED_NATIVE_EVIDENCE": env.get("OXID_OWNED_NATIVE_EVIDENCE"),
            **({"mode": "source-test-scoped-git", "git_environment": {
                key: value for key, value in env.items() if key.startswith("GIT_")
            }} if environment_mode == "source-test-scoped-git" else {}),
        },
    })
    require(status == 0, f"{label}: status {status}; see retained command streams ({error})")
    return stdout, stderr


def git_command(root, label, repo, *arguments, env):
    """Trust only this resolved checkout for an isolated, read-only Git child."""
    repo = repo.resolve(strict=True)
    git_env = {"PATH": "/usr/bin:/bin", "HOME": env["HOME"], "LC_ALL": "C",
               "GIT_CONFIG_NOSYSTEM": "1", "GIT_CONFIG_GLOBAL": "/dev/null",
               "GIT_OPTIONAL_LOCKS": "0"}
    return command(root, label, ["/usr/bin/git", "-c", "safe.directory=" + str(repo),
                   "-C", repo, *arguments], cwd=repo, env=git_env, timeout=60,
                   environment_mode="isolated-git-read")


def source_test_environment(repo, env):
    """Grant descendants an exact-checkout ownership exception; preserve HOME."""
    return {**{key: value for key, value in env.items() if not key.startswith("GIT_")},
            "GIT_CONFIG_NOSYSTEM": "1", "GIT_CONFIG_GLOBAL": "/dev/null",
            "GIT_CONFIG_COUNT": "1", "GIT_CONFIG_KEY_0": "safe.directory",
            "GIT_CONFIG_VALUE_0": str(repo.resolve(strict=True)), "GIT_OPTIONAL_LOCKS": "0"}


def native_test_command(root, unit, name, *, repo, env):
    scoped = name in SOURCE_NAMES
    return command(root, "execution", [unit, name, "--exact", "--ignored",
        "--nocapture", "--test-threads=1", "--color=never"], cwd=repo,
        env=source_test_environment(repo, env) if scoped else env,
        environment_mode="source-test-scoped-git" if scoped else "inherited")


def admit_listing(data, expected):
    lines = [line for line in data.decode("utf-8").splitlines() if line]
    require(lines and lines[-1] == f"{len(expected)} tests, 0 benchmarks",
            "wrong ignored-test discovery count")
    require(len(lines) == len(expected) + 1 and
            sorted(lines[:-1]) == sorted(name + ": test" for name in expected),
            "missing, duplicate, or unexpected ignored test")


def admit_execution(data, name):
    text = data.decode("utf-8")
    summaries = [line for line in text.splitlines() if line.startswith("test result:")]
    require(len(summaries) == 1 and re.fullmatch(
        r"test result: ok\. 1 passed; 0 failed; 0 ignored; 0 measured; "
        r"\d+ filtered out; finished in \d+(?:\.\d+)?s", summaries[0]),
        "test was not executed successfully exactly once")
    expected = ["running 1 test", f"test {name} ... ok"]
    if name in SOURCE_NAMES:
        marker = ("ENUM_SOURCE_NATIVE_SUCCESS artifacts=2 runs=2" if name == SOURCE_NAMES[0]
                  else "ENUM_SOURCE_NATIVE_BOUNDARIES artifacts=14 runs=21")
        expected = ["running 1 test", f"test {name} ... {marker}", "ok"]
    require([line for line in text.splitlines() if line] == [*expected, summaries[0]],
            "unexpected exact test name, completion, or source-native artifact/run count")


def admit_tool_version(name, marker, stdout):
    require(re.search(re.escape(marker) + r" 19\.1\.7(?:[ (]|$)", stdout.decode(), re.MULTILINE),
            name + ": expected LLVM 19.1.7")


def oxid_artifact(data, repo, profile, *, unit):
    rows = [json.loads(line) for line in data.splitlines() if line]
    require([row.get("success") for row in rows if row.get("reason") == "build-finished"]
            == [True], "missing or failed Cargo build completion")
    matches = [row for row in rows if row.get("reason") == "compiler-artifact"
               and row.get("target", {}).get("name") == "oxid"
               and row.get("target", {}).get("kind") == ["bin"]
               and row.get("profile", {}).get("test") is unit and row.get("executable")]
    require(len(matches) == 1, "missing or ambiguous oxid artifact")
    row = matches[0]
    require(row["profile"].get("opt_level") == ("0" if profile == "debug" else "3")
            and row["profile"].get("debug_assertions") is (profile == "debug")
            and row["profile"].get("overflow_checks") is (profile == "debug")
            and type(row["profile"].get("debuginfo")) is int
            and row["profile"]["debuginfo"] == (2 if profile == "debug" else 0),
            "nonstandard Cargo profile")
    path = Path(row["executable"]).resolve(strict=True)
    require((path.parent == repo / "target" / profile / "deps") if unit else
            (path == repo / "target" / profile / "oxid"), "unexpected oxid artifact path")
    return path


def admit_source_identity(repo, observed, manifest):
    selected = [row for row in manifest["files"] if row["path"].startswith(
        ("src/", "native/", "tests/fixtures/bounded_enum_scanner/"))
        or row["path"] in ("Cargo.toml", "Cargo.lock", "build.rs")]
    expected = {row["path"]: (row["bytes"], row["sha256"]) for row in selected}
    actual = {str(Path(row["path"]).relative_to(repo)): (row["bytes"], row["sha256"])
              for row in observed}
    require(bool(expected) and len(expected) == len(selected) and len(actual) == len(observed)
            and actual == expected, "compiler/scanner inputs differ from reviewed source manifest")


def read_reviewed_manifest(path):
    data = path.read_bytes()
    require(hashlib.sha256(data).hexdigest() == REVIEWED_SOURCE_SHA256,
            "reviewed source manifest differs from approved authority")
    return json.loads(data)


def reviewed_input_identities(repo, manifest):
    """Include retained array/composition fixtures embedded in the unit binary."""
    members = manifest["files"]
    require(members and len({row["path"] for row in members}) == len(members),
            "empty or duplicate reviewed input roster")
    observed = []
    for row in members:
        actual = identity(repo / row["path"])
        require((actual["bytes"], actual["sha256"]) == (row["bytes"], row["sha256"]),
                "reviewed input changed: " + row["path"])
        observed.append(actual)
    return observed


def public_cases(repo, root, binary, env):
    tiny_dir = root / "tiny-source"
    tiny_dir.mkdir()
    tiny = tiny_dir / "main.ox"
    tiny.write_bytes(TINY)
    require(identity(tiny)["sha256"] == TINY_SHA256, "frozen tiny source drift")
    scanner = repo / "tests/fixtures/bounded_enum_scanner"
    copied = root / "scanner-source"
    copied.mkdir()
    for filename, digest in SCANNER_HASHES.items():
        path = scanner / filename
        require(identity(path)["sha256"] == digest, "frozen scanner source drift")
        shutil.copy2(path, copied / filename)
    # Use the untouched repository scanner at the actual public CLI boundary.
    for name, source, expected in (("tiny", tiny, b"7\n"),
                                   ("scanner", scanner / "main.ox", b"115\n")):
        case = root / name
        case.mkdir()
        prefix = [binary, "check", source, "--edition=typed-preview"]
        stdout, stderr = command(case, "check", prefix, cwd=repo, env=env, timeout=60)
        require((stdout, stderr) == (b"typed-preview check ok (3 functions; check only)\n", b""),
                name + ": unexpected check streams")
        stdout, stderr = command(case, "run", [binary, "run", source, "--edition=typed-preview"],
                                 cwd=repo, env=env, timeout=60)
        require((stdout, stderr) == (expected, b""), name + ": wrong public run output")
        elf = case / "program.elf"
        stdout, stderr = command(case, "compile", [binary, "compile", source,
            "--edition=typed-preview", "--backend=llvm", "--output=" + str(elf),
            "--message-format=json"], cwd=repo, env=env, timeout=60)
        require(not stderr and json.loads(stdout) == {
            "schema_version": 1, "edition": "typed-preview", "kind": "compile-summary",
            "success": True, "errors": 0, "output": str(elf),
        }, name + ": unexpected public compile result")
        with elf.open("rb") as handle:
            require(handle.read(4) == b"\x7fELF", name + ": not an ELF")
        runtime = case / "source-free"
        runtime.mkdir()
        executable = runtime / "program.elf"
        shutil.copy2(elf, executable)
        require(list(runtime.iterdir()) == [executable], "runtime cwd contains unexpected files")
        stdout, stderr = command(case, "elf-run", [executable], cwd=runtime,
                                 env=ELF_ENV, timeout=60)
        require((stdout, stderr) == (expected, b""), name + ": wrong source-free ELF output")
        require(list(runtime.iterdir()) == [executable], "runtime cwd changed")
        original, copied_elf = identity(elf), identity(executable)
        require(original["sha256"] == copied_elf["sha256"], "ELF changed during copy/run")
        save_json(case / "artifacts.json", {"source": identity(source), "elf": original,
                  "executed_elf": copied_elf, "expected_stdout": expected.decode()})
    for filename, digest in SCANNER_HASHES.items():
        require(identity(scanner / filename)["sha256"] == digest, "canonical scanner changed")


def verify(args, root):
    repo = args.repo.resolve(strict=True)
    env = dict(os.environ, OXID_LLVM_BIN=str(args.llvm_bin.resolve(strict=True)))
    head, _ = git_command(root, "git-head", repo, "rev-parse", "HEAD", "HEAD^{tree}", env=env)
    require(head.decode().splitlines()[0] == args.expected_head, "checkout is not the exact CI head")
    status, _ = git_command(root, "git-status", repo, "status", "--porcelain=v1", env=env)
    require(not status, "current checkout must be clean")
    files, _ = git_command(root, "source-files", repo, "ls-files", "-z", "--", "src", "native",
        "Cargo.toml", "Cargo.lock", "build.rs", "tests/fixtures/bounded_enum_scanner", env=env)
    source_ids = [identity(repo / os.fsdecode(path)) for path in files.split(b"\0") if path]
    source_manifest = repo / "tests/fixtures/typed_project_source_binding/current-source.json"
    manifest = read_reviewed_manifest(source_manifest)
    admit_source_identity(repo, source_ids, manifest)
    reviewed_ids = reviewed_input_identities(repo, manifest)
    manifest_id = identity(source_manifest)
    shutil.copy2(source_manifest, root / "reviewed-source.json")
    save_json(root / "source-identity.json", {"head": args.expected_head, "event_sha": args.event_sha,
              "tree": head.decode().splitlines()[1], "files": source_ids,
              "reviewed_inputs": reviewed_ids,
              "scope": "Exact compiler/scanner roster plus every reviewed manifest input, including embedded fixtures",
              "reviewed_source_head": manifest["reviewed_source_head"],
              "reviewed_source_tree": manifest["source_only_tree"],
              "reviewed_source_manifest": manifest_id,
              "entrypoint": identity(Path(__file__).resolve())})
    tools = {}
    cargo = Path(args.cargo).resolve(strict=True)
    for name, tool in (("cargo", cargo), ("rustc", cargo.with_name("rustc"))):
        command(root, "tool-" + name, [tool, "--version", "--verbose"], cwd=repo, env=env)
        tools[name] = identity(tool)
    for name, marker in (("clang", "clang version"), ("llvm-as", "LLVM version"),
                         ("opt", "LLVM version"), ("ld.lld", "LLD")):
        tool = args.llvm_bin.resolve() / name
        stdout, _ = command(root, "tool-" + name, [tool, "--version"], cwd=repo, env=env)
        admit_tool_version(name, marker, stdout)
        tools[name] = identity(tool)
    save_json(root / "tools.json", tools)
    for profile in ("debug", "release"):
        evidence = root / profile
        evidence.mkdir()
        unit_build, _ = command(evidence, "unit-build", [args.cargo, "test", "--locked", "--bin",
            "oxid", "--no-run", "--message-format=json", *(["--release"] if profile == "release" else [])],
            cwd=repo, env=env)
        unit = oxid_artifact(unit_build, repo, profile, unit=True)
        cli_build, _ = command(evidence, "cli-build", [args.cargo, "build", "--locked", "--bin",
            "oxid", "--message-format=json", *(["--release"] if profile == "release" else [])],
            cwd=repo, env=env)
        binary = oxid_artifact(cli_build, repo, profile, unit=False)
        save_json(evidence / "binaries.json", {"profile": profile, "unit": identity(unit),
                  "production_cli": identity(binary)})
        for label, prefix, names in (("raw", RAW_PREFIX, RAW_NAMES),
                                     ("source", SOURCE_PREFIX, SOURCE_NAMES)):
            stdout, stderr = command(evidence, label + "-discovery",
                [unit, prefix, "--list", "--ignored", "--color=never"], cwd=repo, env=env)
            require(not stderr, "unexpected discovery stderr")
            admit_listing(stdout, names)
        save_json(evidence / "selected-tests.json", {"count": 8, "names": [*RAW_NAMES, *SOURCE_NAMES]})
        for index, name in enumerate((*RAW_NAMES, *SOURCE_NAMES)):
            group = evidence / f"test-{index + 1:02}"
            group.mkdir()
            artifacts = group / "artifacts"
            artifacts.mkdir()
            stdout, _ = native_test_command(group, unit, name, repo=repo,
                env=dict(env, OXID_OWNED_NATIVE_EVIDENCE=str(artifacts)))
            admit_execution(stdout, name)
        public = evidence / "public-cli"
        public.mkdir()
        public_cases(repo, public, binary, env)
        require(identity(unit) == json.loads((evidence / "binaries.json").read_text())["unit"],
                "unit binary changed during execution")
        require(identity(binary) == json.loads((evidence / "binaries.json").read_text())["production_cli"],
                "production CLI changed during execution")
    require(manifest_id == identity(source_manifest), "reviewed source manifest changed during execution")
    reviewed_after = reviewed_input_identities(repo, manifest)
    require(reviewed_ids == reviewed_after, "reviewed inputs changed during execution")
    files, _ = git_command(root, "source-files-after", repo, "ls-files", "-z", "--", "src", "native",
        "Cargo.toml", "Cargo.lock", "build.rs", "tests/fixtures/bounded_enum_scanner", env=env)
    source_after = [identity(repo / os.fsdecode(path)) for path in files.split(b"\0") if path]
    admit_source_identity(repo, source_after, manifest)
    save_json(root / "source-identity-after.json", {"reviewed_source_manifest": manifest_id,
              "reviewed_inputs": reviewed_after, "files": source_after})


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--cargo", required=True)
    parser.add_argument("--llvm-bin", type=Path, required=True)
    parser.add_argument("--expected-head", required=True)
    parser.add_argument("--event-sha", required=True)
    args = parser.parse_args()
    root = args.output.resolve()
    root.mkdir(parents=True, exist_ok=False)
    result = {"status": "failed", "scope": "8 existing native groups and 2 public CLI cases per profile; source-free cwd/env, not filesystem isolation"}
    try:
        verify(args, root)
        result["status"] = "passed"
    except Exception as error:
        result["error"] = str(error)
        raise
    finally:
        save_json(root / "result.json", result)
    print("bounded enum native: debug/release; 16 exact tests, 4 public CLI/ELF cases: PASS")


if __name__ == "__main__":
    main()
