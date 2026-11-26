#!/usr/bin/env python3
"""Check queued file jobs, cancellation and offline native builds."""
import argparse
import errno
import fcntl
import hashlib
import json
import os
from pathlib import Path
import platform
import resource
import selectors
import shutil
import signal
import subprocess
import struct
import termios
import time

import linux_native_target
from verify_macos_native import macho

ROOT = Path(__file__).resolve().parents[1]
PROJECT = ROOT / "apps/file-worker"
MACOS_CPU = {"x86_64": 0x1000007, "arm64": 0x100000C, "aarch64": 0x100000C}
TEST_REPORT = b'test "records": ok (exit status: 0)\ntests: 1 passed; 0 failed\n'


def require(condition, message):
    if not condition:
        raise ValueError(message)


def run(argv, cwd, environment, *, payload=b"", status=0, stdout=None, stderr=b"", preexec=None):
    result = subprocess.run(list(map(str, argv)), cwd=cwd, env=environment,
                            input=payload, capture_output=True, timeout=60, preexec_fn=preexec)
    require(result.returncode == status,
            f"{argv} returned {result.returncode}, expected {status}: {result.stderr!r}")
    if stdout is not None:
        require(result.stdout == stdout, f"{argv} returned unexpected output: {result.stdout[:256]!r}")
    if stderr is not None:
        require(result.stderr == stderr, f"{argv} returned unexpected diagnostics: {result.stderr[:256]!r}")
    return result.stdout


def read_line(child, timeout):
    result = bytearray()
    deadline = time.monotonic() + timeout
    with selectors.DefaultSelector() as selector:
        selector.register(child.stdout, selectors.EVENT_READ)
        while not result.endswith(b"\n"):
            remaining = deadline - time.monotonic()
            require(remaining > 0 and selector.select(remaining), "worker did not finish its first job")
            byte = os.read(child.stdout.fileno(), 1)
            require(byte and len(result) < 128, "worker closed stdout or sent an oversized response")
            result.extend(byte)
    return result


def stop_worker(command, cwd, environment, signum, partial):
    with subprocess.Popen(list(map(str, command)), cwd=cwd, env=environment,
                          stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE) as child:
        try:
            child.stdin.write(b"empty\n")
            child.stdin.flush()
            require(read_line(child, 15) == b"OK 0 1\n", "worker's first job failed")
            if partial:
                child.stdin.write(b"empty")
                child.stdin.flush()
            child.send_signal(signum)
            require(child.wait(timeout=5) == 130, "worker failed to stop with input still open")
            require(child.stdout.read() == b"" and child.stderr.read() == b"",
                    "worker emitted a response for an unfinished job")
        finally:
            if child.poll() is None:
                child.kill()
                child.wait()


def limit_files():
    _, hard = resource.getrlimit(resource.RLIMIT_NOFILE)
    resource.setrlimit(resource.RLIMIT_NOFILE, (24, hard))


def closed_output(command, cwd, environment):
    read_fd, write_fd = os.pipe()
    os.close(read_fd)
    try:
        result = subprocess.run(list(map(str, command)), cwd=cwd, env=environment,
                                input=b"empty\n", stdout=write_fd, stderr=subprocess.PIPE,
                                timeout=10)
        require(result.returncode == 74, f"closed output returned {result.returncode}")
    finally:
        os.close(write_fd)



def stop_blocked_output(command, cwd, environment):
    pending = cwd / "pending.txt"
    pending.write_bytes(b"empty\n" * 200000)
    with pending.open("rb") as jobs, subprocess.Popen(
        list(map(str, command)), cwd=cwd, env=environment, stdin=jobs,
        stdout=subprocess.PIPE, stderr=subprocess.PIPE,
    ) as child:
        try:
            descriptor = child.stdout.fileno()
            capacity = fcntl.fcntl(descriptor, fcntl.F_GETPIPE_SZ)
            deadline = time.monotonic() + 10
            previous = 0
            while time.monotonic() < deadline:
                available, = struct.unpack("i", fcntl.ioctl(descriptor, termios.FIONREAD, bytes(4)))
                # Short writes can leave unused space at the end of each pipe page.
                if available >= capacity * 3 // 4 and available == previous:
                    break
                require(child.poll() is None, "worker exited before filling stdout")
                previous = available
                time.sleep(0.05)
            else:
                raise ValueError("worker did not fill stdout")
            child.send_signal(signal.SIGTERM)
            require(child.wait(timeout=5) == 130, "worker failed to cancel while stdout was full")
            require(child.stderr.read() == b"", "worker wrote diagnostics during cancellation")
        finally:
            if child.poll() is None:
                child.kill()
                child.wait()


def exercise(command, cwd, environment, *, native):
    (cwd / "empty").touch()
    (cwd / "data with spaces").write_bytes(bytes(range(256)) * 513)
    (cwd / "directory").mkdir()
    os.mkfifo(cwd / "pipe")
    (cwd / "file-link").symlink_to("data with spaces")
    (cwd / "pipe-link").symlink_to("pipe")
    payload = (b"x" * 4093 + b"\0\n" + b"data with spaces\r\n"
               + b"missing\n" + b"directory\n\r\n" + b"y" * 4097
               + b"\r\n" + b"empty")
    expected = (
        b"ERR invalid-path 0\nOK 131328 1\n"
        + f"ERR io:{errno.ENOENT} 3\nERR io:{errno.EISDIR} 1\n".encode()
        + b"ERR invalid-path 0\nERR path-too-long 0\nOK 0 1\n"
    )
    started = time.monotonic()
    run(command, cwd, environment, payload=payload, stdout=expected)
    require(time.monotonic() - started >= 0.45, "missing file retries returned without waiting")
    absent = f"ERR io:{errno.EINVAL} 1\n".encode()
    run(command, cwd, environment, payload=b"pipe\npipe-link\nfile-link\n",
        stdout=absent * 2 + b"OK 131328 1\n")
    run(command, cwd, environment, stdout=b"")
    usage = b"Usage: file-worker [--help] < paths.txt\n"
    run([*command, "--help"], cwd, environment, stdout=usage)
    run([*command, "unexpected"], cwd, environment, status=64, stdout=b"", stderr=usage)
    stop_worker(command, cwd, environment, signal.SIGTERM, partial=False)
    stop_worker(command, cwd, environment, signal.SIGINT, partial=True)
    cases = ["records-and-retries", "file-and-pipe-links", "eof", "help", "usage", "sigterm", "sigint"]
    if native:
        run(command, cwd, environment, payload=b"empty\npipe\nempty\n" * 500,
            stdout=(b"OK 0 1\n" + absent + b"OK 0 1\n") * 500, preexec=limit_files)
        closed_output(command, cwd, environment)
        cases += ["1500-jobs-under-24-descriptors", "closed-output"]
        if platform.system() == "Linux":
            stop_blocked_output(command, cwd, environment)
            cases.append("sigterm-while-stdout-full")
    return cases


def verify(compiler, llvm, output):
    output = output.resolve()
    require(ROOT not in (output, *output.parents), "use an output directory outside the repository")
    output.mkdir(parents=True, exist_ok=False)
    project = output / "source"
    shutil.copytree(PROJECT, project, ignore=shutil.ignore_patterns(".oxid", "oxid.lock"))
    environment = dict(os.environ, OXID_LLVM_BIN=str(llvm), OXID_CACHE_DIR=str(output / "cache"))
    environment.pop("OXID_PATH", None)
    run([compiler, "check"], project, environment)
    run([compiler, "test"], project, environment, stderr=TEST_REPORT)
    report = {"host": platform.platform(), "compiler_sha256": digest(compiler),
              "interpreter": exercise([compiler, "run", "--"], project, environment, native=False)}

    run([compiler, "lock", "--offline"], project, environment)
    archive = output / "file-worker.oxpkg.tar"
    run([compiler, "package", "--frozen", "--output", archive], project, environment)
    checksum = digest(archive)
    run([compiler, "package", "--verify", archive, "--sha256", checksum], output, environment)
    shutil.move(project, output / "source-before-extraction")
    extracted = output / "unpacked"
    run([compiler, "package", "--extract", archive, "--sha256", checksum, "--output", extracted],
        output, environment)
    project = extracted / "source"
    run([compiler, "test", "--frozen"], project, environment, stderr=TEST_REPORT)
    report["archive_sha256"] = checksum
    builds = {}
    for profile in ("dev", "release"):
        run([compiler, "build", "--frozen", "--profile", profile], project, environment)
        deployed = output / profile
        deployed.mkdir()
        executable = deployed / "file-worker"
        shutil.copy2(project / ".oxid/bin/file-worker", executable)
        body = executable.read_bytes()
        if platform.system() == "Darwin":
            macho(body, MACOS_CPU[platform.machine()])
        else:
            require(body.startswith(b"\x7fELF\x02\x01\x01")
                    and body[16:20] == linux_native_target.elf_identity(), "wrong host ELF")
        builds[profile] = executable

    shutil.move(project, output / "extracted-source")
    for profile, executable in builds.items():
        report[profile] = {"binary_sha256": digest(executable),
                           "cases": exercise([executable], executable.parent, {}, native=True)}
    (output / "result.json").write_text(json.dumps(report, indent=2) + "\n")
    print(f"File worker passed {sum(len(report[k]['cases']) for k in builds) + len(report['interpreter'])} cases")


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("compiler", type=Path)
    parser.add_argument("--llvm-bin", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    args = parser.parse_args()
    verify(args.compiler.resolve(strict=True), args.llvm_bin.resolve(strict=True), args.output)


if __name__ == "__main__":
    main()
