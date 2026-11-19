#!/usr/bin/env python3
"""Qualify native Mach-O builds on an actual macOS x86_64 or ARM64 host."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import re
import shutil
import signal
import struct
import subprocess
import time


ROOT = Path(__file__).resolve().parents[1]


def require(condition, message):
    if not condition:
        raise ValueError(message)


def sha(path):
    with Path(path).open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def macho(data, machine):
    require(len(data) >= 32, "truncated Mach-O header")
    magic, cpu, _, kind, count, size, flags, _ = struct.unpack_from("<8I", data)
    require(magic == 0xFEEDFACF and cpu == machine and kind == 2,
            "executable has the wrong Mach-O format or CPU")
    require(flags & 0x200000, "Mach-O executable is not position independent")
    end = 32 + size
    require(end <= len(data) and count <= size // 8, "invalid load-command region")
    offset, libraries, signature = 32, [], False
    for _ in range(count):
        require(offset + 8 <= end, "truncated load command")
        command, length = struct.unpack_from("<2I", data, offset)
        require(length >= 8 and length % 8 == 0 and offset + length <= end,
                "invalid load-command length")
        if command in (0xC, 0x80000018, 0x8000001F, 0x80000023):
            require(length >= 24, "truncated dylib command")
            name = struct.unpack_from("<I", data, offset + 8)[0]
            require(24 <= name < length, "invalid dylib name offset")
            text = data[offset + name:offset + length]
            require(b"\0" in text, "unterminated dylib name")
            library = text.split(b"\0", 1)[0].decode("utf-8")
            require(library.startswith(("/usr/lib/", "/System/Library/")),
                    "executable depends on a non-system dynamic library: " + library)
            libraries.append(library)
        if command == 0x1D:
            require(length == 16, "invalid code-signature command")
            start, length = struct.unpack_from("<2I", data, offset + 8)
            require(length > 0 and start + length <= len(data), "invalid code-signature extent")
            signature = True
        offset += struct.unpack_from("<I", data, offset + 4)[0]
    require(offset == end and libraries, "incomplete load commands or missing system runtime")
    if machine == 0x100000C:
        require(signature, "ARM64 executable has no code signature")
    return {"cpu": cpu, "libraries": libraries, "code_signature": signature}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("compilers", type=Path, nargs=2, metavar=("DEBUG", "RELEASE"))
    parser.add_argument("--llvm-bin", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    args = parser.parse_args()
    machine = {"x86_64": 0x1000007, "arm64": 0x100000C, "aarch64": 0x100000C}.get(platform.machine())
    require(platform.system() == "Darwin" and machine is not None and struct.calcsize("P") == 8,
            "qualification requires an actual macOS x86_64 or ARM64 host")
    output = args.output.absolute()
    output.mkdir(parents=True, exist_ok=False)
    report = {"status": "running", "platform": platform.platform(), "machine": machine,
              "commands": [], "compilers": [], "builds": []}

    def save():
        (output / "result.json").write_text(json.dumps(report, indent=2) + "\n")

    def command(label, argv, cwd, environment, expected=0):
        folder = output / "commands" / (str(len(report["commands"])) + "-" + label)
        folder.mkdir(parents=True)
        started = time.monotonic()
        with (folder / "stdout").open("wb") as stdout, (folder / "stderr").open("wb") as stderr:
            process = subprocess.Popen(list(map(str, argv)), cwd=cwd, env=environment,
                                       stdin=subprocess.DEVNULL, stdout=stdout, stderr=stderr,
                                       start_new_session=True)
            try:
                status = process.wait(timeout=300)
            finally:
                if process.poll() is None:
                    os.killpg(process.pid, signal.SIGKILL)
                    process.wait()
        result = {"label": label, "argv": list(map(str, argv)), "cwd": str(cwd),
                  "exit": status, "seconds": time.monotonic() - started,
                  "stdout": str(folder / "stdout"), "stderr": str(folder / "stderr"),
                  "stdout_sha256": sha(folder / "stdout"), "stderr_sha256": sha(folder / "stderr")}
        report["commands"].append(result)
        save()
        require(status == expected, label + " failed; see " + str(folder))
        return (folder / "stdout").read_bytes(), (folder / "stderr").read_bytes()

    save()
    try:
        environment = dict(os.environ, OXID_LLVM_BIN=str(args.llvm_bin.resolve(strict=True)))
        environment.pop("CCC_OVERRIDE_OPTIONS", None)
        require(environment.get("SDKROOT") and Path(environment["SDKROOT"]).is_dir(),
                "set SDKROOT to xcrun --sdk macosx --show-sdk-path")
        report["sdk"] = environment["SDKROOT"]
        for tool in ("clang", "opt", "ld64.lld"):
            stdout, _ = command("version-" + tool, [args.llvm_bin / tool, "--version"], output, environment)
            require(b"19.1.7" in stdout, "requires LLVM 19.1.7")
        fixture = ROOT / "tests/native_files.rs"
        match = re.search(r'const ROUND_TRIP: &str = r#"(.*?)"#;', fixture.read_text(), re.DOTALL)
        require(match is not None, "missing native file round-trip fixture")
        report["file_fixture"] = {"path": "tests/native_files.rs", "sha256": sha(fixture)}
        cases = [
            ("scalar", "fn choose(flag:bool,n:i32)->i32{if flag{return n+2;}else{return n;}}"
             "fn main()->i32{return choose(true,40);}", b"42\n", None),
            ("integer128", "fn main()->i128{return 170141183460469231731687303715884105727i128;}",
             b"170141183460469231731687303715884105727\n", None),
            ("float-record", "struct Packet{values:[f32;2],scale:f64}"
             "fn scaled(packet:&Packet)->f64{let first=packet.values[0];let value=first.to_f64();"
             "return value*packet.scale;}fn main()->f64{let mut packet="
             "Packet{values:[0.5f32,-0.25f32],scale:8f64};packet.values[0]=1.5f32;return scaled(&packet);}",
             b"12\n", None),
            ("file-round-trip", match.group(1), b"11\n", b"Oxid\0\xff\n" + "👋".encode()),
            ("modules", "mod local;fn main()->i32{return crate::local::answer();}", b"42\n", None),
            ("dependency", "fn main()->i32{return crate::math::answer();}", b"42\n", None),
            ("process", "use std::io::write_stdout as emit;use std::io::WriteStatus as Written;"
             "fn main()->i32{let bytes=[0,65,255];let written=emit(&bytes);match written{"
             "Written::Complete=>{return 39;},Written::InvalidInput=>{return 2;},"
             "Written::IoError(n)=>{return n+3;},}}", bytes((0, 65, 255)), None),
        ]
        for compiler_profile, source in zip(("debug", "release"), args.compilers):
            binary = output / "compilers" / compiler_profile / "oxid"
            binary.parent.mkdir(parents=True)
            shutil.copy2(source.resolve(strict=True), binary)
            binary.chmod(0o555)
            report["compilers"].append({"profile": compiler_profile, "path": str(binary), "sha256": sha(binary)})
            command("doctor", [binary, "doctor", "--native", "--message-format=json"], output, environment)
            for policy in ("bounded", "hosted"):
                for profile in ("dev", "release"):
                    for name, text, expected, file_bytes in cases:
                        label = "-".join((compiler_profile, policy, profile, name))
                        project = output / "projects" / label
                        relocated = output / "projects" / (label + "-relocated-遷移")
                        (project / "src").mkdir(parents=True)
                        (project / "src/main.ox").write_text(text)
                        mode = "process" if name == "process" else "result"
                        exit_status = 39 if name == "process" else 0
                        (project / "oxid.toml").write_text(
                            '[project]\nname="app"\nedition="typed-preview"\nentry="src/main.ox"\n'
                            '[build]\nentry-mode="' + mode + '"\n[execution]\npolicy="' + policy + '"\n')
                        if name == "modules":
                            (project / "src/local.ox").write_text("pub fn answer()->i32{return 42;}")
                        if name == "dependency":
                            dependency = project / "vendor/math"
                            (dependency / "src").mkdir(parents=True)
                            (dependency / "src/lib.ox").write_text("pub fn answer()->i32{return 42;}")
                            (dependency / "oxid.toml").write_text(
                                '[project]\nname="math"\nkind="lib"\nedition="typed-preview"\nentry="src/lib.ox"\n')
                            with (project / "oxid.toml").open("a") as manifest:
                                manifest.write('[dependencies]\nmath="vendor/math"\n')
                        inputs = output / "inputs" / label
                        inputs.parent.mkdir(exist_ok=True)
                        shutil.copytree(project, inputs)
                        shutil.copytree(project, relocated)
                        stdout, stderr = command(label + "-reference", [binary, "run"], project,
                                                 environment, expected=exit_status)
                        require(stdout == expected and not stderr, label + " reference differs from its independent oracle")
                        artifacts = []
                        for directory, variant in ((project, "cold"), (project, "repeat"), (relocated, "relocated")):
                            stdout, stderr = command(label + "-" + variant,
                                [binary, "build", "--profile", profile, "--reproducible", "--incremental", "--message-format=json"],
                                directory, environment)
                            rows = [json.loads(line) for line in stdout.splitlines()]
                            require(any(row.get("kind") == "build-cache" and row.get("status") == "unavailable"
                                        for row in rows), "macOS must report unavailable cache honestly")
                            built = directory / ".oxid/bin/app"
                            data = built.read_bytes()
                            details = macho(data, machine)
                            artifact = output / "artifacts" / (label + "-" + variant)
                            artifact.parent.mkdir(exist_ok=True)
                            shutil.copy2(built, artifact)
                            artifacts.append(artifact)
                            report["builds"].append({"label": label, "variant": variant,
                                "artifact": str(artifact), "sha256": sha(artifact), **details})
                            save()
                        require(len({sha(path) for path in artifacts}) == 1,
                                label + " raw Mach-O bytes differ after repeat or relocation")
                        shutil.rmtree(project)
                        shutil.rmtree(relocated)
                        for index, artifact in enumerate(artifacts):
                            deployment = output / "deployments" / (label + "-" + str(index))
                            deployment.mkdir(parents=True)
                            stdout, stderr = command(label + "-source-free", [artifact], deployment, {},
                                                     expected=exit_status)
                            require(stdout == expected and not stderr, label + " deployed output differs")
                            if file_bytes is not None:
                                require((deployment / "data.bin").read_bytes() == file_bytes,
                                        label + " file bytes differ")
            project = output / "debug-refusal" / compiler_profile
            (project / "src").mkdir(parents=True)
            (project / "src/main.ox").write_text("fn main()->i32{return 42;}")
            (project / "oxid.toml").write_text('[project]\nname="app"\nedition="typed-preview"\n')
            _, stderr = command("debug-refusal", [binary, "build", "--debug-info"], project,
                                dict(environment, OXID_LLVM_BIN=str(project / "absent-tools")), expected=1)
            require(b"native debug information currently requires Linux" in stderr,
                    "unsupported debug settings must fail before tool selection")
            require(not (project / ".oxid").exists(), "debug refusal published build state")
        report["status"] = "passed"
        save()
    except BaseException as error:
        report.update(status="failed", failure=type(error).__name__ + ": " + str(error))
        save()
        raise


if __name__ == "__main__":
    main()
