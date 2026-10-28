#!/usr/bin/env python3
"""Qualify bounded bool/i32/unit MSVC x64 native programs on actual Windows."""
import argparse
import ctypes
from ctypes import wintypes as W
import hashlib
import json
import os
from pathlib import Path
import platform
import re
import shutil
import struct
import time

from windows_native_process import kernel, run

ROOT = Path(__file__).resolve().parents[1]


def require(condition, message):
    if not condition:
        raise ValueError(message)


def sha(path):
    with Path(path).open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def pe(data):
    """Independent on-disk PE reader, including RVA-based imported DLL names."""
    require(len(data) >= 64 and data[:2] == b"MZ", "not a DOS/PE image")
    start = struct.unpack_from("<I", data, 60)[0]
    require(64 <= start <= len(data) - 24 and data[start:start + 4] == b"PE\0\0", "invalid PE signature")
    machine, count, timestamp, _, _, optional_bytes, flags = struct.unpack_from("<HHIIIHH", data, start + 4)
    require(machine == 0x8664 and 0 < count <= 96 and flags & 2 and not flags & (0x2000 | 1),
            "requires AMD64 executable COFF with relocations retained")
    optional = start + 24
    require(optional_bytes >= 240 and optional + optional_bytes + count * 40 <= len(data), "truncated PE headers")
    require(struct.unpack_from("<H", data, optional)[0] == 0x20B, "requires PE32+")
    subsystem, dll = struct.unpack_from("<HH", data, optional + 68)
    stack, committed = struct.unpack_from("<QQ", data, optional + 72)
    require(subsystem == 3 and dll & 0x160 == 0x160, "requires console, ASLR, high entropy and NX")
    require(stack == 8 * 1024 * 1024 and 0 < committed <= stack, "requires an 8 MiB reserved stack")
    directories = struct.unpack_from("<I", data, optional + 108)[0]
    require(2 <= directories <= (optional_bytes - 112) // 8, "invalid PE data directories")
    sections = []
    for index in range(count):
        at = optional + optional_bytes + index * 40
        virtual_size, virtual, size, offset = struct.unpack_from("<4I", data, at + 8)
        require(offset <= len(data) and size <= len(data) - offset, "section exceeds file")
        sections.append((virtual, virtual_size, offset, size))

    def extent(rva, length):
        matches = [(offset + rva - virtual, size - (rva - virtual))
                   for virtual, virtual_size, offset, size in sections
                   if virtual <= rva < virtual + max(virtual_size, size)
                   and rva - virtual <= size and length <= size - (rva - virtual)]
        require(len(matches) == 1, "unmapped, ambiguous or truncated PE RVA")
        return matches[0]

    imports_rva, imports_length = struct.unpack_from("<II", data, optional + 120)
    require(imports_rva and 20 <= imports_length <= 1024 * 1024, "missing or oversized import table")
    imports, terminated = [], False
    at, _ = extent(imports_rva, imports_length)
    for cursor in range(at, at + imports_length - 19, 20):
        descriptor = struct.unpack_from("<5I", data, cursor)
        if not any(descriptor):
            terminated = True
            break
        name, available = extent(descriptor[3], 1)
        end = data.find(b"\0", name, name + min(available, 261))
        require(end > name, "invalid DLL import name")
        library = data[name:end].decode("ascii").lower()
        require(re.fullmatch(r"[a-z0-9_.-]+\.dll", library) is not None, "non-basename DLL import")
        require(library in ("kernel32.dll", "ntdll.dll", "advapi32.dll", "user32.dll", "shell32.dll")
                or library.startswith(("api-ms-win-core-", "ext-ms-win-")),
                "non-system or dynamic CRT DLL dependency: " + library)
        imports.append(library)
    require(terminated and imports, "unterminated or empty import table")
    require(directories >= 6, "missing base relocation directory")
    relocation_rva, relocation_length = struct.unpack_from("<II", data, optional + 152)
    require(relocation_rva and relocation_length >= 8, "ASLR requires retained base relocations")
    extent(relocation_rva, relocation_length)
    return {"machine": machine, "format": "PE32+", "dll_flags": dll,
            "stack_reserve": stack, "stack_commit": committed, "timestamp": timestamp,
            "imports": imports}


def diagnostic(source, marker, code, message):
    prefix = source[:source.index(marker)]
    line, column = prefix.count("\n") + 1, len(prefix.rsplit("\n", 1)[-1]) + 1
    return f"error[{code}] (oir-run): {message}\n  --> src/main.ox:{line}:{column}\n".encode()


def case_list():
    result = [
        ("flow", "fn step(flag:bool,n:i32)->i32{if flag{return n+1;}return n;}"
         "fn done()->(){return;}fn main()->i32{let mut n=0;while n<42{n=step(true,n);"
         "if n==40{continue;}}done();return n;}", b"42\n", b"", 0),
        ("bool", "fn main()->bool{return false&&(1/0==0)||true;}", b"true\n", b"", 0),
        ("unit", "fn main()->(){return;}", b"()\n", b"", 0),
        ("min-i32", "fn main()->i32{return -2147483648;}", b"-2147483648\n", b"", 0),
        ("division", "fn main()->i32{return (-17/5)*10+(-17%5);}", b"-32\n", b"", 0),
        ("modules", "mod local;fn main()->i32{return crate::local::answer();}", b"42\n", b"", 0),
    ]
    for name, source, marker, code, message in [
        ("overflow", "fn main()->i32{let unused=2147483647+1;return 7;}", "+1", "E0604", "checked i32 arithmetic overflow"),
        ("zero", "fn main()->i32{let unused=1/0;return 7;}", "/0", "E0607", "checked i32 division by zero"),
        ("first-error", "fn take(a:i32,b:i32)->i32{return 7;}fn main()->i32{return take(2147483647+1,1/0);}",
         "+1", "E0604", "checked i32 arithmetic overflow"),
        ("remainder-overflow", "fn main()->i32{return -2147483648%-1;}", "%-1", "E0604", "checked i32 arithmetic overflow"),
    ]:
        result.append((name, source, b"", diagnostic(source, marker, code, message), 1))
    return result


def changed(environment, **values):
    names = {key.upper() for key in values}
    return {**{key: value for key, value in environment.items() if key.upper() not in names}, **values}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("compilers", type=Path, nargs=2, metavar=("DEBUG", "RELEASE"))
    parser.add_argument("--llvm-bin", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    args = parser.parse_args()
    require(platform.system() == "Windows" and platform.machine().upper() in ("AMD64", "X86_64")
            and struct.calcsize("P") == 8, "requires actual Windows MSVC x64 qualification")
    output = args.output.absolute()
    output.mkdir(parents=True, exist_ok=False)
    tools = args.llvm_bin.resolve(strict=True)
    report = {"status": "running", "scope": "bounded-result-bool-i32-unit-msvc-x64",
              "platform": platform.platform(), "commands": [], "builds": []}

    def save():
        (output / "result.json").write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")

    def command(label, argv, cwd, environment, expected=0, stdin=None, tick=None, timeout=300):
        folder = output / "commands" / (str(len(report["commands"])) + "-" + label)
        folder.mkdir(parents=True)
        row = {"label": label, "argv": list(map(str, argv)), "cwd": str(cwd),
               "stdout": str(folder / "stdout"), "stderr": str(folder / "stderr")}
        report["commands"].append(row)
        save()
        try:
            status, duration = run(argv, cwd, environment, folder / "stdout", folder / "stderr",
                                   timeout=timeout, stdin=stdin, tick=tick)
            row.update(exit=status, seconds=duration, stdout_sha256=sha(folder / "stdout"),
                       stderr_sha256=sha(folder / "stderr"))
            save()
            require(status == expected, label + " failed; see " + str(folder))
            return (folder / "stdout").read_bytes(), (folder / "stderr").read_bytes()
        except BaseException as error:
            row["failure"] = type(error).__name__ + ": " + str(error)
            save()
            raise

    save()
    try:
        environment = changed(dict(os.environ), OXID_LLVM_BIN=str(tools))
        environment = {key: value for key, value in environment.items()
                       if key.upper() != "CCC_OVERRIDE_OPTIONS"}
        require(any(key.upper() == "INCLUDE" and value for key, value in environment.items())
                and any(key.upper() == "LIB" and value for key, value in environment.items()),
                "initialize installed x64 Visual Studio and Windows SDK first")
        for tool, marker in (("clang.exe", "clang version"), ("opt.exe", "LLVM version"), ("lld-link.exe", "LLD")):
            stdout, _ = command("version-" + tool, [tools / tool, "--version"], output, environment)
            require(re.search(re.escape(marker.encode()) + rb" 19\.1\.7(?:[ (\r\n]|$)", stdout),
                    "requires exact LLVM 19.1.7: " + tool)
        link_flags = ["-fms-runtime-lib=static", "--ld-path=" + str(tools / "lld-link.exe"),
                      "-Wl,/Brepro", "-Wl,/DYNAMICBASE", "-Wl,/HIGHENTROPYVA", "-Wl,/NXCOMPAT",
                      "-Wl,/SUBSYSTEM:CONSOLE", "-Wl,/STACK:8388608"]
        c_flags = ["--no-default-config", "--target=x86_64-pc-windows-msvc",
                   "-std=c11", "-Wall", "-Wextra", "-Werror"]
        helper = output / "probe-helper.exe"
        command("compile-probe-helper", [tools / "clang.exe", *c_flags, "-O2",
                ROOT / "tests/native_windows_probe_helper.c", *link_flags, "-o", helper], output, environment)
        pe(helper.read_bytes())
        fake_tools = output / "tools"
        fake_tools.mkdir()
        for name, source in (("clang.exe", helper), ("opt.exe", tools / "opt.exe"), ("lld-link.exe", helper)):
            try:
                os.link(source, fake_tools / name)
            except OSError:
                shutil.copy2(source, fake_tools / name)
        fake_environment = changed(environment, OXID_LLVM_BIN=str(fake_tools),
                                   OXID_NATIVE_REAL_CLANG=str(tools / "clang.exe"))
        # LLVM tool DLLs, when present, remain in their authenticated installation.
        path_key = next((key for key in environment if key.upper() == "PATH"), "PATH")
        fake_environment = changed(fake_environment, PATH=str(tools) + os.pathsep + environment.get(path_key, ""))
        for compiler_profile, original in zip(("debug", "release"), args.compilers):
            binary = output / "compilers" / compiler_profile / "oxid.exe"
            binary.parent.mkdir(parents=True)
            shutil.copy2(original.resolve(strict=True), binary)
            stdout, stderr = command("doctor-" + compiler_profile,
                [binary, "doctor", "--native", "--message-format=json"], output, environment)
            health = json.loads(stdout)
            require(not stderr and health["toolchain_ready"] and health["target"] == "x86_64-pc-windows-msvc",
                    "doctor did not qualify the actual MSVC SDK/link/execute chain")
            for profile in ("dev", "release"):
                for name, source, expected_out, expected_error, expected_status in case_list():
                    label = "-".join((compiler_profile, profile, name))
                    project = output / "projects" / label
                    relocated = output / "projects" / (label + "-遷移")
                    (project / "src").mkdir(parents=True)
                    (project / "src/main.ox").write_text(source, encoding="utf-8")
                    (project / "oxid.toml").write_text(
                        '[project]\nname="app"\nedition="typed-preview"\nentry="src/main.ox"\n'
                        '[build]\nentry-mode="result"\n[execution]\npolicy="bounded"\n', encoding="utf-8")
                    if name == "modules":
                        (project / "src/local.ox").write_text("pub fn answer()->i32{return 42;}", encoding="utf-8")
                    inputs = output / "inputs" / label
                    inputs.parent.mkdir(exist_ok=True)
                    shutil.copytree(project, inputs)
                    shutil.copytree(project, relocated)
                    stdout, stderr = command(label + "-reference", [binary, "run", "src/main.ox", "--edition=typed-preview"], project, environment,
                                             expected=expected_status)
                    require(stdout == expected_out and stderr == expected_error, "independent reference oracle differs: " + label)
                    variants = []
                    for directory, variant in ((project, "cold"), (project, "repeat"), (relocated, "relocated")):
                        stdout, _ = command(label + "-" + variant, [binary, "build", "--profile", profile,
                            "--reproducible", "--incremental", "--message-format=json"], directory, environment)
                        rows = [json.loads(line) for line in stdout.splitlines()]
                        require(any(row.get("kind") == "build-cache" and row.get("status") == "unavailable" for row in rows),
                                "COFF object cache must be unavailable")
                        built = directory / ".oxid/bin/app.exe"
                        details = pe(built.read_bytes())
                        artifact = output / "artifacts" / (label + "-" + variant + ".exe")
                        artifact.parent.mkdir(exist_ok=True)
                        shutil.copy2(built, artifact)
                        variants.append(artifact)
                        report["builds"].append({"label": label, "variant": variant, "path": str(artifact),
                                                  "sha256": sha(artifact), **details})
                        save()
                    require(len({sha(path) for path in variants}) == 1, "raw PE bytes differ after repeat/Unicode relocation: " + label)
                    shutil.rmtree(project)
                    shutil.rmtree(relocated)
                    for index, artifact in enumerate(variants):
                        deployment = output / "deployments" / (label + "-" + str(index))
                        deployment.mkdir(parents=True)
                        stdout, stderr = command(label + "-source-free", [artifact], deployment, {}, expected=expected_status)
                        require(stdout == expected_out and stderr == expected_error, "source-free exact byte behavior differs: " + label)
            failure_controls(binary, compiler_profile, output, environment, fake_environment, command, report, save)
        io_controls(output, tools, environment, c_flags, link_flags, command)
        report["status"] = "passed"
        save()
    except BaseException as error:
        report.update(status="failed", failure=type(error).__name__ + ": " + str(error))
        save()
        raise


def failure_controls(binary, label, output, environment, fake_environment, command, report, save):
    project = output / "failure-controls" / label
    (project / "src").mkdir(parents=True)
    (project / "src/main.ox").write_text("fn main()->i32{return 42;}", encoding="utf-8")
    (project / "oxid.toml").write_text('[project]\nname="app"\nedition="typed-preview"\nentry="src/main.ox"\n', encoding="utf-8")
    command(label + "-old-output", [binary, "build"], project, environment)
    built = project / ".oxid/bin/app.exe"
    old = built.read_bytes()
    (project / "src/main.ox").write_text("fn main()->i32{return 43;}", encoding="utf-8")
    for mode in ("wrong-version", "missing-sdk-headers", "link-failure", "missing-tools"):
        changed_env = changed(fake_environment, OXID_NATIVE_PROBE_MODE=mode)
        if mode == "missing-tools":
            changed_env = changed(changed_env, OXID_LLVM_BIN=str(project / "absent-tools"))
        stdout, stderr = command(label + "-" + mode, [binary, "build", "--message-format=json"],
                                 project, changed_env, expected=1)
        require(not stderr and b'"code":"E0701"' in stdout and b'"stage":"native-toolchain"' in stdout,
                mode + " must fail as a native toolchain diagnostic")
        require(built.read_bytes() == old, mode + " replaced an existing executable")
        require(list(built.parent.iterdir()) == [built], mode + " leaked a staged output")
    stdout, stderr = command(label + "-no-overwrite", [binary, "compile", "src/main.ox", "--edition=typed-preview",
        "--backend=llvm", "--output", built, "--message-format=json"], project, environment, expected=1)
    require(not stderr and b"already exists" in stdout and built.read_bytes() == old, "explicit output overwrote a file")
    api = kernel()
    api.CreateFileW.argtypes = [W.LPCWSTR, W.DWORD, W.DWORD, W.LPVOID, W.DWORD, W.DWORD, W.HANDLE]
    api.CreateFileW.restype = W.HANDLE
    held = api.CreateFileW(str(built), 0x80000000, 3, None, 3, 0, None)  # read/write sharing, no DELETE sharing
    require(held not in (None, ctypes.c_void_p(-1).value), "cannot hold output without delete sharing")
    try:
        stdout, stderr = command(label + "-locked-output", [binary, "build", "--message-format=json"],
                                 project, environment, expected=1)
        require(not stderr and b'"code":"E0701"' in stdout and b"native artifact I/O failed" in stdout
                and built.read_bytes() == old,
                "locked output must retain the prior bytes and report the native publication error")
    finally:
        api.CloseHandle(held)
    require(list(built.parent.iterdir()) == [built], "locked output failure leaked a stage")
    command(label + "-replace-after-unlock", [binary, "build"], project, environment)
    require(built.read_bytes() != old, "successful rebuild did not replace the old program")
    stdout, stderr = command(label + "-replacement-run", [built], project, {})
    require(stdout == b"43\n" and not stderr, "replacement executable differs")
    for mode in ("stdout-flood", "stderr-flood", "sleep-tree", "exit-tree", "wrong-version"):
        state = output / "probe-state" / (label + "-" + mode)
        state.mkdir(parents=True)
        probe_env = changed(fake_environment, OXID_NATIVE_PROBE_MODE=mode, OXID_NATIVE_PROBE_STATE=str(state))
        observed = []

        def observe():
            pid_file = state / "child.pid"
            if pid_file.exists() and not observed:
                try:
                    pid = int(pid_file.read_text(encoding="ascii"))
                except ValueError:
                    return
                handle = api.OpenProcess(0x00100000 | 0x1000, False, pid)
                require(handle, "cannot retain an independent descendant process handle")
                observed.append(handle)
                (state / "observer.ready").write_bytes(b"ready")

        try:
            started = time.monotonic()
            stdout, stderr = command(label + "-doctor-" + mode,
                [binary, "doctor", "--native", "--message-format=json"], state, probe_env,
                expected=1, tick=observe, timeout=30)
            health = json.loads(stdout)
            require(not stderr and not health["toolchain_ready"] and not health["success"], "adversarial doctor reported ready")
            checks = {item["id"]: item for item in health["checks"]}
            if mode in ("stdout-flood", "stderr-flood"):
                require(checks["clang"]["status"] == "failed" and "exceeded 65536" in checks["clang"]["message"],
                        "doctor did not enforce its per-stream byte cap")
            if mode == "sleep-tree":
                require(checks["clang"]["status"] == "failed" and "timed out" in checks["clang"]["message"]
                        and time.monotonic() - started < 12, "doctor timeout was not bounded")
            if mode in ("sleep-tree", "exit-tree"):
                require(observed, "missing independent live-descendant witness")
                require(api.WaitForSingleObject(observed[0], 0) == 0, "doctor leaked its descendant process")
                require(not (state / "escaped-child").exists(), "descendant escaped doctor cleanup")
            if mode == "exit-tree":
                require(checks["clang"]["status"] == "passed", "exited tool did not yield its complete validated output")
            report.setdefault("doctor_controls", []).append({"compiler": label, "mode": mode,
                "seconds": time.monotonic() - started, "independent_child_handle_observed": bool(observed)})
            save()
        finally:
            for handle in observed:
                api.CloseHandle(handle)


def io_controls(output, tools, environment, c_flags, link_flags, command):
    payload = bytes((0, 13, 10, 26, 255))
    incoming = output / "raw-input"
    incoming.write_bytes(payload)
    for optimization in ("-O0", "-O2"):
        program = output / ("raw-io-" + optimization[1:] + ".exe")
        command("compile-raw-io-" + optimization[1:], [tools / "clang.exe", *c_flags, optimization,
                ROOT / "native/typed_preview.c", ROOT / "tests/native_windows_io.c", *link_flags, "-o", program], output, environment)
        pe(program.read_bytes())
        stdout, stderr = command("raw-bytes", [program, "raw"], output, {}, expected=1)
        require(stdout == payload and stderr == payload, "Win32 runtime translated CRLF/Ctrl-Z/NUL/non-ASCII bytes")
        stdout, stderr = command("raw-input", [program, "read"], output, {}, stdin=incoming)
        require(not stdout and not stderr, "Win32 input bytes changed or Ctrl-Z became EOF")
        stdout, stderr = command("broken-pipe", [program, "broken-pipe"], output, {})
        require(not stdout and not stderr, "broken-pipe control wrote a diagnostic or terminated abnormally")
        stdout, stderr = command("raw-print", [program, "print"], output, {})
        require(stdout == b"-2147483648\n2147483647\ntrue\nfalse\n()\n" and not stderr,
                "runtime scalar output differs under LLP64")


if __name__ == "__main__":
    main()
