#!/usr/bin/env python3
"""Check source-free native file copies at both optimization levels.

Every deployed process has a 32 MiB address-space ceiling. The 64 MiB case
checks streaming through a 1024-byte buffer against exact independent bytes.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import resource
import shutil
import subprocess
import sys

ROOT = Path(__file__).resolve().parents[1]
FIXTURE = "tests/fixtures/native_files/copy.ox.in"
ADDRESS_SPACE = 32 * 1024 * 1024
LARGE_BYTES = 64 * 1024 * 1024
MANIFEST = ('[project]\nname="copy"\nedition="typed-preview"\nentry="src/main.ox"\n'
            '[build]\nentry-mode="result"\n[execution]\npolicy="hosted"\n')


def require(condition, message):
    if not condition:
        raise ValueError(message)


def identity(path):
    with path.open("rb") as stream:
        digest = hashlib.file_digest(stream, "sha256").hexdigest()
    return {"path": str(path), "bytes": path.stat().st_size, "sha256": digest}


def save(path, value):
    path.write_text(json.dumps(value, indent=2, sort_keys=True) + "\n")


def bounded_process():
    resource.setrlimit(resource.RLIMIT_AS, (ADDRESS_SPACE, ADDRESS_SPACE))


def materialize(path, size, samples):
    with path.open("xb") as stream:
        stream.truncate(size)
        for offset, data in samples:
            stream.seek(offset)
            stream.write(data)


def equal_files(left, right):
    with left.open("rb") as first, right.open("rb") as second:
        while True:
            block = first.read(1024 * 1024)
            if block != second.read(1024 * 1024):
                return False
            if not block:
                return True


def run(args):
    require(platform.system() == "Linux" and platform.machine() == "x86_64",
            "native file qualification requires Linux x86_64")
    output = args.output.resolve()
    require(not output.exists() and output != ROOT and ROOT not in output.parents,
            "output must be new and outside the repository")
    output.mkdir(parents=True)
    compilers = [path.resolve(strict=True) for path in args.compilers]
    pinned = [identity(path) for path in compilers]
    source = (ROOT / FIXTURE).read_bytes()
    (output / "copy.ox").write_bytes(source)
    report = {"schema": "oxid-native-files-streaming-v1", "status": "running",
              "compilers": pinned, "source": identity(output / "copy.ox"),
              "address_space_limit_bytes": ADDRESS_SPACE, "large_input_bytes": LARGE_BYTES,
              "buffer_bytes": 1024, "commands": [], "cases": []}

    def command(argv, cwd, label, env, deployed=False):
        result = subprocess.run(argv, cwd=cwd, env=env, stdin=subprocess.DEVNULL,
                                stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                                timeout=180, preexec_fn=bounded_process if deployed else None)
        out, err = output / (label + ".stdout"), output / (label + ".stderr")
        out.write_bytes(result.stdout)
        err.write_bytes(result.stderr)
        report["commands"].append({"argv": [str(arg) for arg in argv], "cwd": str(cwd),
                                   "exit_status": result.returncode, "stdout": identity(out),
                                   "stderr": identity(err), "environment_empty": deployed,
                                   "address_space_limit_bytes": ADDRESS_SPACE if deployed else None})
        save(output / "result.json", report)
        require(result.returncode == 0 and not result.stderr, "command failed: " + label)
        return result.stdout

    try:
        env = dict(os.environ, OXID_LLVM_BIN=str(args.llvm_bin.resolve(strict=True)))
        for index, compiler in enumerate(compilers):
            for profile in ("dev", "release"):
                prefix = f"compiler-{index}-{profile}"
                build = output / (prefix + "-build")
                (build / "src").mkdir(parents=True)
                (build / "src/main.ox").write_bytes(source)
                (build / "oxid.toml").write_text(MANIFEST)
                command([compiler, "build", "--profile", profile], build, prefix + "-build", env)
                artifact = output / (prefix + ".elf")
                shutil.copy2(build / ".oxid/bin/copy", artifact)
                require(artifact.read_bytes().startswith(b"\x7fELF"), "not an ELF artifact")
                shutil.rmtree(build)
                binary = bytes(range(256)) * 5 + "Oxid\0\u0000\U0001f44b".encode()
                cases = (("empty", 0, []), ("binary", len(binary), [(0, binary)]),
                         ("large", LARGE_BYTES, [(0, binary), (1021, b"\xff\0Oxid"),
                          (LARGE_BYTES // 2 - 3, binary), (LARGE_BYTES - 4, b"Oxid")]),
                         ("missing-input", None, []))
                for label, size, samples in cases:
                    location = output / (prefix + "-" + label)
                    location.mkdir()
                    executable = location / "copy"
                    shutil.copy2(artifact, executable)
                    incoming, outgoing = location / "input.bin", location / "output.bin"
                    if size is not None:
                        materialize(incoming, size, samples)
                        expected = str(size).encode() + b"\n"
                    else:
                        outgoing.write_bytes(b"preserve-existing-output")
                        expected = b"-1\n"
                    before = identity(incoming) if size is not None else None
                    actual = command([executable], location, prefix + "-" + label, {}, True)
                    require(actual == expected, "wrong result bytes: " + label)
                    if size is not None:
                        require(identity(incoming) == before, "input changed")
                        require(equal_files(incoming, outgoing), "copied bytes differ")
                    else:
                        require(outgoing.read_bytes() == b"preserve-existing-output", "early error changed output")
                    # Preserve exact logical bytes while avoiding dense storage
                    # of already-verified zero runs in large evidence artifacts.
                    if label == "large" and shutil.which("fallocate"):
                        before_output = identity(outgoing)
                        command([shutil.which("fallocate"), "--dig-holes", outgoing], location,
                                prefix + "-sparsify", dict(os.environ))
                        require(identity(outgoing) == before_output, "sparse representation changed bytes")
                    require(not any((location / path).exists() for path in ("src", "oxid.toml", ".oxid")),
                            "application sources survived deployment")
                    report["cases"].append({"compiler": index, "optimization": profile, "case": label,
                                            "artifact": identity(executable), "input": before,
                                            "output": identity(outgoing), "source_removed": True})
                    save(output / "result.json", report)
        require([identity(path) for path in compilers] == pinned, "compiler changed")
        require((ROOT / FIXTURE).read_bytes() == source, "source fixture changed")
        report["status"] = "passed"
        save(output / "result.json", report)
        print(json.dumps({"status": "passed", "deployed_cases": len(report["cases"]), "output": str(output)}))
        return 0
    except Exception as error:
        report.update(status="failed", error=f"{type(error).__name__}: {error}")
        save(output / "result.json", report)
        print(report["error"], file=sys.stderr)
        return 1


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("compilers", type=Path, nargs="+", help="current debug and release compilers")
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--llvm-bin", type=Path, required=True)
    return run(parser.parse_args())


if __name__ == "__main__":
    raise SystemExit(main())
