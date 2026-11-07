#!/usr/bin/env python3
"""Link LLVM emitted by the standalone Oxid scalar compiler.

This driver does not parse, typecheck, lower or emit Oxid. It accepts a complete
success stream from an explicitly supplied native compiler and invokes Clang
with the repository's scalar I/O adapter. An existing output is never replaced.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import stat
import subprocess
import tempfile

PREFIX = b'; Oxid scalar compiler 1\ntarget triple = "x86_64-unknown-linux-gnu"\n'
SUFFIX = b'; Oxid scalar compiler end\n'
MAX_MODULE_BYTES = 8 * 1024 * 1024


class BuildError(RuntimeError):
    pass


def require(condition: bool, message: str) -> None:
    if not condition:
        raise BuildError(message)


def module_from_result(result: subprocess.CompletedProcess[bytes]) -> bytes:
    require(result.returncode == 0, f"compiler failed with status {result.returncode}")
    require(not result.stderr, "compiler emitted diagnostics on its success path")
    data = result.stdout
    require(0 < len(data) <= MAX_MODULE_BYTES, "compiler output size is invalid")
    require(data.startswith(PREFIX) and data.endswith(SUFFIX), "incomplete compiler output framing")
    require(data.count(PREFIX) == 1 and data.count(SUFFIX) == 1, "ambiguous compiler output framing")
    require(all(byte in (9, 10, 13) or 32 <= byte < 127 for byte in data), "non-ASCII compiler output")
    # Never remove a trailing main result, diagnostic line, banner or prefix.
    # LLVM's verifier/Clang must independently accept the exact retained bytes.
    return data


def invoke(command: list[str], timeout: int, cwd: Path,
           environment: dict[str, str] | None = None) -> subprocess.CompletedProcess[bytes]:
    try:
        return subprocess.run(command, cwd=cwd, stdin=subprocess.DEVNULL,
                              stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                              timeout=timeout, check=False, env=environment)
    except (OSError, subprocess.TimeoutExpired) as error:
        raise BuildError(f"cannot complete {Path(command[0]).name}: {error}") from error


def build(compiler: Path, source: Path, clang: Path, runtime: Path,
          destination: Path, profile: str, timeout: int = 120, evidence: Path | None = None) -> dict:
    require(platform.system() == "Linux" and platform.machine() in ("x86_64", "AMD64"),
            "this compiler currently targets Linux x86_64")
    compiler = compiler.resolve(strict=True)
    source = source.resolve(strict=True)
    clang = clang.resolve(strict=True)
    runtime = runtime.resolve(strict=True)
    destination = destination.absolute()
    require(profile in ("dev", "release"), "unknown build profile")
    require(source.is_file() and runtime.is_file(), "source and runtime must be regular files")
    require(compiler.is_file() and os.access(compiler, os.X_OK), "compiler is not executable")
    require(clang.is_file() and os.access(clang, os.X_OK), "Clang is not executable")
    require(destination.parent.is_dir(), "output parent does not exist")
    require(not os.path.lexists(destination), "output already exists")
    if evidence is not None:
        evidence.mkdir(parents=True, exist_ok=False)
    result = invoke([str(compiler), str(source)], timeout, source.parent, environment={})
    if evidence is not None:
        (evidence / "compiler.stdout").write_bytes(result.stdout)
        (evidence / "compiler.stderr").write_bytes(result.stderr)
        (evidence / "compiler.json").write_text(json.dumps({
            "command": result.args, "status": result.returncode,
            "source_sha256": hashlib.sha256(source.read_bytes()).hexdigest(),
        }, indent=2) + "\n")
    module = module_from_result(result)
    with tempfile.TemporaryDirectory(prefix=".oxid-scalar-build-", dir=destination.parent) as directory:
        staging = Path(directory)
        module_path = staging / "program.ll"
        binary = staging / "program"
        module_path.write_bytes(module)
        command = [str(clang), "--no-default-config", "--target=x86_64-unknown-linux-gnu",
                   "-O0" if profile == "dev" else "-O2", "-Werror", "-fuse-ld=lld",
                   str(module_path), str(runtime), "-o", str(binary)]
        linked = invoke(command, timeout, staging)
        if evidence is not None:
            (evidence / "program.ll").write_bytes(module)
            (evidence / "clang.stdout").write_bytes(linked.stdout)
            (evidence / "clang.stderr").write_bytes(linked.stderr)
            (evidence / "clang.json").write_text(json.dumps({
                "command": command, "status": linked.returncode,
            }, indent=2) + "\n")
        require(linked.returncode == 0, "Clang rejected the emitted module: "
                + linked.stderr.decode("utf-8", errors="replace")[-4096:])
        require(binary.is_file() and not binary.is_symlink(), "Clang did not produce a regular executable")
        mode = stat.S_IMODE(binary.stat().st_mode)
        require(bool(mode & 0o111), "Clang output is not executable")
        with binary.open("rb") as stream:
            os.fsync(stream.fileno())
            digest = hashlib.file_digest(stream, "sha256").hexdigest()
        # Same-filesystem hard-link publication is atomic and fails if any
        # process creates a file/symlink at destination during compilation.
        try:
            os.link(binary, destination, follow_symlinks=False)
        except FileExistsError as error:
            raise BuildError("output appeared during compilation; it was preserved") from error
        directory_fd = os.open(destination.parent, os.O_RDONLY | os.O_DIRECTORY)
        try:
            os.fsync(directory_fd)
        finally:
            os.close(directory_fd)
    return {"output": str(destination), "sha256": digest, "profile": profile,
            "module_sha256": hashlib.sha256(module).hexdigest(),
            "compiler_sha256": hashlib.sha256(compiler.read_bytes()).hexdigest(),
            "runtime_sha256": hashlib.sha256(runtime.read_bytes()).hexdigest(),
            "self_hosting": False}


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("source", type=Path)
    parser.add_argument("output", type=Path)
    parser.add_argument("--compiler", type=Path, required=True,
                        help="native oxid-scalar-compiler built in process entry mode")
    parser.add_argument("--clang", type=Path, required=True)
    parser.add_argument("--runtime", type=Path, default=Path(__file__).resolve().parents[2] / "native/typed_preview.c")
    parser.add_argument("--profile", choices=("dev", "release"), default="dev")
    parser.add_argument("--evidence", type=Path)
    args = parser.parse_args()
    try:
        receipt = build(args.compiler, args.source, args.clang, args.runtime, args.output,
                        args.profile, evidence=args.evidence)
    except (BuildError, OSError) as error:
        parser.exit(1, f"build failed: {error}\n")
    print(json.dumps(receipt, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
