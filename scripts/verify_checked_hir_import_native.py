#!/usr/bin/env python3
"""Opt-in Linux x86_64 qualification of the bounded private Emit artifact.

Build the test binary on a clean commit first and retain its build receipt.
This driver records the executable hash; the current Git head alone is not
proof that an arbitrary supplied binary was compiled from that head.
No provider, production route, source pins or LLVM text are modified.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import signal
import subprocess
import time

ROOT = Path(__file__).resolve().parents[1]
EXPORT_TEST = "frontend::oir::source::hir_import::emit_tests::native_tests::checked_hir_import_emit_export_native_artifacts"
CASES = ("rich", "overflow", "division")
VERSION = "19.1.7"
TARGET = "x86_64-unknown-linux-gnu"


def save(path: Path, data: bytes) -> None:
    with path.open("xb") as stream:
        stream.write(data)


def digest(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as stream:
        for chunk in iter(lambda: stream.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


def child_limits() -> None:
    import resource  # Linux-only gate; keep module importable by other CI hosts.
    # Per-file bound includes LLVM artifacts and retained streams. No core dumps.
    resource.setrlimit(resource.RLIMIT_CORE, (0, 0))
    resource.setrlimit(resource.RLIMIT_FSIZE, (64 << 20, 64 << 20))


def run(root: Path, name: str, argv: list[str], *, cwd: Path,
        env: dict[str, str], seconds: float = 30) -> tuple[int, bytes, bytes]:
    """No shell or stdin. Retain streams even on failure; reap on timeout.

    Output goes to disk while running, not an unbounded PIPE; each child file
    is capped at 64 MiB. These three closed programs have small outputs. External processes are
    outside the private compiler's work/heap tariff; this is not an OS sandbox.
    """
    started = time.monotonic()
    timed_out = False
    with (root / f"{name}.stdout").open("xb") as stdout, (root / f"{name}.stderr").open("xb") as stderr:
        process = subprocess.Popen(argv, cwd=cwd, env=env, stdin=subprocess.DEVNULL,
                                   stdout=stdout, stderr=stderr, start_new_session=True,
                                   preexec_fn=child_limits)
        try:
            status = process.wait(timeout=seconds)
        except subprocess.TimeoutExpired:
            timed_out = True
            os.killpg(process.pid, signal.SIGKILL)
            status = process.wait()
    receipt = {"argv": argv, "cwd": str(cwd), "env": env, "status": status,
               "timeout_seconds": seconds, "timed_out": timed_out,
               "elapsed_seconds": time.monotonic() - started}
    save(root / f"{name}.json", (json.dumps(receipt, indent=2) + "\n").encode())
    if timed_out:
        raise RuntimeError(f"{name} exceeded {seconds}s; process group killed; evidence retained")
    # Bound the in-process read, independently of retained output size.
    paths = [root / f"{name}.{stream}" for stream in ("stdout", "stderr")]
    if any(path.stat().st_size > 1 << 20 for path in paths):
        raise RuntimeError(f"{name} output exceeded 1 MiB; evidence retained")
    return status, paths[0].read_bytes(), paths[1].read_bytes()


def git_read(root: Path, name: str, *arguments: str) -> tuple[int, bytes, bytes]:
    """Trust only the validated checkout, for this isolated read-only child."""
    repo = ROOT.resolve(strict=True)
    if not (repo / ".git").exists():
        raise RuntimeError("Git identity requires the checkout root")
    env = {"PATH": "/usr/bin:/bin", "LC_ALL": "C",
           "GIT_CONFIG_NOSYSTEM": "1", "GIT_CONFIG_GLOBAL": "/dev/null",
           "GIT_OPTIONAL_LOCKS": "0"}
    return run(root, name, ["/usr/bin/git", "-c", "safe.directory=" + str(repo),
                           "-C", str(repo), *arguments], cwd=repo, env=env)


def success(result: tuple[int, bytes, bytes], label: str) -> None:
    if result[0] != 0:
        raise RuntimeError(f"{label} failed with status {result[0]}; inspect retained stderr")


def verify_elf(path: Path) -> None:
    with path.open("rb") as stream:
        header = stream.read(20)
    if header[:6] != b"\x7fELF\x02\x01" or header[16:20] != b"\x03\x00\x3e\x00":
        raise RuntimeError("expected little-endian x86_64 ELF64 PIE")


def evidence_destination(path: Path) -> Path:
    resolved = path.resolve()
    if resolved.is_relative_to(ROOT.resolve()):
        raise RuntimeError("evidence directory must be outside the checkout")
    return resolved


def qualify(binary: Path, llvm: Path, root: Path) -> None:
    if platform.system() != "Linux" or platform.machine() != "x86_64":
        raise RuntimeError("private native gate is qualified only for Linux x86_64")
    binary, llvm = binary.resolve(strict=True), llvm.resolve(strict=True)
    root = evidence_destination(root)
    root.mkdir()  # fresh, no overwrite or deletion of old evidence
    env = {"PATH": f"{llvm}:/usr/bin:/bin", "LC_ALL": "C"}
    metadata: dict[str, object] = {"test_binary": str(binary), "test_binary_sha256": digest(binary),
                                 "llvm_bin": str(llvm), "scope": "private test-only; not public provider qualification"}
    save(root / "driver.py", Path(__file__).read_bytes())
    try:
        head = git_read(root, "git-head", "rev-parse", "HEAD")
        status = git_read(root, "git-status", "status", "--porcelain=v1")
        success(head, "Git identity")
        success(status, "Git status")
        metadata["head"] = head[1].decode().strip()
        if status[1]:
            raise RuntimeError("qualification requires a clean checkout")
        for tool, marker in (("clang", "clang version"), ("opt", "LLVM version"),
                             ("llvm-as", "LLVM version"), ("ld.lld", "LLD")):
            result = run(root, f"version-{tool}", [str(llvm / tool), "--version"], cwd=root, env=env)
            success(result, tool)
            # Match a complete version token, never 19.1.70 or an unrelated banner.
            if not any(f"{marker} {VERSION}" in line and
                       line.split(f"{marker} {VERSION}", 1)[1][:1] in ("", " ", "(")
                       for line in result[1].decode().splitlines()):
                raise RuntimeError(f"{tool} must report {marker} {VERSION}")
            metadata[tool + "_sha256"] = digest(llvm / tool)
        export_env = {**env, "OXID_HIR_IMPORT_NATIVE_EVIDENCE": str(root)}
        result = run(root, "export", [str(binary), EXPORT_TEST, "--exact", "--ignored", "--nocapture"],
                     cwd=ROOT, env=export_env, seconds=120)
        success(result, "private artifact export")
        if b"1 passed; 0 failed" not in result[1]:
            raise RuntimeError("exact exporter test did not run once")
        clang, opt, assembler = (str(llvm / name) for name in ("clang", "opt", "llvm-as"))
        for case in CASES:
            directory = root / case
            private = (directory / "private.ll").read_bytes()
            if private != (directory / "ordinary.ll").read_bytes():
                raise RuntimeError(f"{case}: owned private and ordinary LLVM differ")
            expected = (int((directory / "expected.status").read_text()),
                        (directory / "expected.stdout").read_bytes(),
                        (directory / "expected.stderr").read_bytes())
            for route in ("private", "ordinary"):
                ir = directory / f"{route}.ll"
                success(run(directory, f"{route}-assemble", [assembler, str(ir), "-o", str(directory / f"{route}.bc")], cwd=directory, env=env), "LLVM assembly")
                success(run(directory, f"{route}-verify", [opt, "-passes=verify", "-disable-output", str(ir)], cwd=directory, env=env), "LLVM verification")
                run_dir = directory / f"{route}-run"
                run_dir.mkdir()
                executable = run_dir / "program.elf"
                # Mirrors the preview's O0 PIE target/runtime/link flags. This
                # direct external check does not call the production CLI or
                # prove its publication behavior. No text rewriting is allowed.
                args = [clang, "--no-default-config", f"--target={TARGET}", "-O0", "-fPIE",
                        "-pie", "-Wl,-z,noexecstack", "-Wl,-z,relro", "-Wl,-z,now",
                        f"--ld-path={llvm / 'ld.lld'}", str(ir), str(root / "runtime.c"),
                        "-std=c11", "-Wall", "-Wextra", "-Werror", "-o", str(executable)]
                success(run(directory, f"{route}-compile", args, cwd=directory, env=env, seconds=60), "native compile/link")
                verify_elf(executable)
                actual = run(directory, f"{route}-execute", [str(executable)], cwd=run_dir,
                             env={"PATH": "/no-tools", "LC_ALL": "C"}, seconds=5)
                if actual != expected:
                    raise RuntimeError(f"{case}/{route}: status/stdout/stderr differ from frozen source reference")
                if list(run_dir.iterdir()) != [executable]:
                    raise RuntimeError("unexpected runtime files")
            # Real independent rejection: corrupt only a disposable copy, never
            # the qualified private artifact, and require LLVM to diagnose it.
            malformed = directory / "malformed.ll"
            save(malformed, private + b"\nthis is invalid LLVM\n")
            result = run(directory, "malformed-verify", [opt, "-passes=verify", "-disable-output", str(malformed)], cwd=directory, env=env)
            if result[0] <= 0 or b"error:" not in result[2] or result[1]:
                raise RuntimeError("malformed LLVM did not produce a nonzero diagnostic-only rejection")
        final_head = git_read(root, "git-head-after", "rev-parse", "HEAD")
        final_status = git_read(root, "git-status-after", "status", "--porcelain=v1")
        if final_head != head or final_status != status or digest(binary) != metadata["test_binary_sha256"]:
            raise RuntimeError("source or test executable changed during qualification")
        metadata["result"] = "passed"
    except BaseException as error:
        metadata["result"] = "failed"
        metadata["error"] = str(error)
        raise
    finally:
        metadata["artifacts"] = {str(path.relative_to(root)): digest(path)
                                 for path in sorted(root.rglob("*")) if path.is_file()}
        save(root / "manifest.json", (json.dumps(metadata, indent=2) + "\n").encode())


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--test-binary", type=Path, required=True)
    parser.add_argument("--llvm-bin", type=Path, required=True)
    parser.add_argument("--evidence", type=Path, required=True)
    args = parser.parse_args()
    qualify(args.test_binary, args.llvm_bin, args.evidence)


if __name__ == "__main__":
    main()
