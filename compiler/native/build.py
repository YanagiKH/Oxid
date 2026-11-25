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
import shutil
import stat
import subprocess
import tempfile

DEFAULT_TARGET = "x86_64-unknown-linux-gnu"
ARM64_TARGET = "aarch64-unknown-linux-gnu"
MACOS_X64_TARGET = "x86_64-apple-macosx11.0.0"
MACOS_ARM64_TARGET = "arm64-apple-macosx11.0.0"
TARGETS = {
    ("Linux", "x86_64"): DEFAULT_TARGET, ("Linux", "arm64"): ARM64_TARGET,
    ("Darwin", "x86_64"): MACOS_X64_TARGET, ("Darwin", "arm64"): MACOS_ARM64_TARGET,
}
MACOS_TARGETS = (MACOS_X64_TARGET, MACOS_ARM64_TARGET)
SUFFIX = b'; Oxid scalar compiler end\n'
MAX_MODULE_BYTES = 8 * 1024 * 1024
DEFAULT_RUNTIME = Path(__file__).resolve().with_name("runtime.c")


class BuildError(RuntimeError):
    pass


def require(condition: bool, message: str) -> None:
    if not condition:
        raise BuildError(message)


def host_target() -> str:
    machine = platform.machine().lower()
    arch = {"amd64": "x86_64", "aarch64": "arm64"}.get(machine, machine)
    target = TARGETS.get((platform.system(), arch))
    if target is None:
        raise BuildError("this compiler requires Linux or macOS on x86-64 or ARM64")
    return target


def linker_name(target: str) -> str:
    return "ld64.lld" if target in MACOS_TARGETS else "ld.lld"


def macos_link_flags(clang: Path, cwd: Path, timeout: int,
                     environment: dict[str, str] | None) -> list[str]:
    sdk_setting = (environment or {}).get("SDKROOT") or os.environ.get("SDKROOT")
    if sdk_setting:
        sdk = Path(sdk_setting)
    else:
        selected = invoke(["/usr/bin/xcrun", "--sdk", "macosx", "--show-sdk-path"], timeout, cwd)
        require(selected.returncode == 0 and not selected.stderr, "xcrun could not select the macOS SDK")
        sdk = Path(os.fsdecode(selected.stdout.rstrip(b"\r\n")))
    require(sdk.is_absolute() and (sdk / "usr/lib/libSystem.tbd").is_file(),
            "set SDKROOT to a macOS SDK containing usr/lib/libSystem.tbd")
    linker = None
    if environment is not None and "PATH" in environment:
        linker = shutil.which("ld64.lld", path=environment["PATH"])
    if linker is None:
        linker = Path(os.environ.get("OXID_LLVM_BIN", clang.parent)) / "ld64.lld"
    linker = Path(linker)
    require(linker.is_absolute() and linker.is_file() and os.access(linker, os.X_OK),
            "LLVM ld64.lld is required alongside Clang or in OXID_LLVM_BIN")
    return ["-isysroot", str(sdk), f"--ld-path={linker}", "-Wl,-pie", "-Wl,-oso_prefix,."]


def module_prefix(target: str, entry_mode: str = "result") -> bytes:
    prefix = f'; Oxid scalar compiler 1\ntarget triple = "{target}"\n'.encode("ascii")
    if entry_mode == "process":
        return prefix + b"; entry mode = process\n"
    return prefix


PREFIX = module_prefix(DEFAULT_TARGET)


def compiler_command(compiler: Path, source: Path, target: str, entry_mode: str = "result") -> list[str]:
    command = [str(compiler), str(source)]
    if entry_mode == "process" or target != DEFAULT_TARGET:
        command.append(f"--target={target}")
    if entry_mode == "process":
        command.append("--entry-mode=process")
    return command


def module_from_result(result: subprocess.CompletedProcess[bytes], target: str = DEFAULT_TARGET,
                       entry_mode: str = "result") -> bytes:
    if result.returncode != 0:
        raw_detail = result.stderr[:8192].decode("utf-8", errors="replace")
        detail = "".join(character if character.isprintable() else " " for character in raw_detail).strip()
        suffix = f"\n{detail}" if detail else ""
        raise BuildError(f"compiler failed with status {result.returncode}{suffix}")
    require(not result.stderr, "compiler emitted diagnostics on its success path")
    data = result.stdout
    require(0 < len(data) <= MAX_MODULE_BYTES, "compiler output size is invalid")
    require(entry_mode in ("result", "process"), "unknown entry mode")
    require(data.startswith(module_prefix(target)) and data.endswith(SUFFIX),
            "incomplete compiler output or target mismatch")
    require(data.startswith(module_prefix(target, "process")) == (entry_mode == "process"),
            "compiler output entry mode mismatch")
    require(data.count(b"; Oxid scalar compiler 1\n") == 1 and data.count(SUFFIX) == 1,
            "ambiguous compiler output framing")
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
          destination: Path, profile: str, timeout: int = 120, evidence: Path | None = None,
          entry_mode: str = "result", linker_environment: dict[str, str] | None = None) -> dict:
    target = host_target()
    compiler = compiler.resolve(strict=True)
    source = source.resolve(strict=True)
    clang = clang.resolve(strict=True)
    runtime = runtime.resolve(strict=True)
    destination = destination.absolute()
    require(profile in ("dev", "release"), "unknown build profile")
    require(entry_mode in ("result", "process"), "unknown entry mode")
    require(source.is_file() and runtime.is_file(), "source and runtime must be regular files")
    require(compiler.is_file() and os.access(compiler, os.X_OK), "compiler is not executable")
    require(clang.is_file() and os.access(clang, os.X_OK), "Clang is not executable")
    require(destination.parent.is_dir(), "output parent does not exist")
    require(not os.path.lexists(destination), "output already exists")
    if evidence is not None:
        evidence.mkdir(parents=True, exist_ok=False)
    result = invoke(compiler_command(compiler, source, target, entry_mode), timeout, source.parent, environment={})
    if evidence is not None:
        (evidence / "compiler.stdout").write_bytes(result.stdout)
        (evidence / "compiler.stderr").write_bytes(result.stderr)
        (evidence / "compiler.json").write_text(json.dumps({
            "command": result.args, "status": result.returncode,
            "source_sha256": hashlib.sha256(source.read_bytes()).hexdigest(),
        }, indent=2) + "\n")
    module = module_from_result(result, target, entry_mode)
    with tempfile.TemporaryDirectory(prefix=".oxid-scalar-build-", dir=destination.parent) as directory:
        staging = Path(directory)
        module_path = staging / "program.ll"
        binary = staging / "program"
        module_path.write_bytes(module)
        command = [str(clang), "--no-default-config", f"--target={target}",
                   "-O0" if profile == "dev" else "-O2", "-Werror"]
        if target in MACOS_TARGETS:
            command.extend(macos_link_flags(clang, staging, timeout, linker_environment))
        else:
            command.append("-fuse-ld=lld")
        if runtime == DEFAULT_RUNTIME and b"\ndeclare void @__oxid_bytebuf_start()\n" in module:
            command.append("-DOXID_SCALAR_HEAP")
            if b"\n@__oxid_record_vector_schemas = " in module:
                command.append("-DOXID_RECORD_VECTOR_SCHEMAS")
        command.extend([str(module_path), str(runtime), "-o", str(binary)])
        linked = invoke(command, timeout, staging, environment=linker_environment)
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
        if target not in MACOS_TARGETS:
            directory_fd = os.open(destination.parent, os.O_RDONLY | os.O_DIRECTORY)
            try:
                os.fsync(directory_fd)
            finally:
                os.close(directory_fd)
    return {"output": str(destination), "sha256": digest, "profile": profile, "target": target,
            "entry_mode": entry_mode,
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
    parser.add_argument("--runtime", type=Path, default=DEFAULT_RUNTIME)
    parser.add_argument("--profile", choices=("dev", "release"), default="dev")
    parser.add_argument("--entry-mode", choices=("result", "process"), default="result")
    parser.add_argument("--evidence", type=Path)
    args = parser.parse_args()
    try:
        receipt = build(args.compiler, args.source, args.clang, args.runtime, args.output,
                        args.profile, evidence=args.evidence, entry_mode=args.entry_mode)
    except (BuildError, OSError) as error:
        parser.exit(1, f"build failed: {error}\n")
    print(json.dumps(receipt, sort_keys=True))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
