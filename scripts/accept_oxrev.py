"""Focused ordinary-source oxrev acceptance; artifacts belong outside checkout.

Example: python3 scripts/accept_oxrev.py target/debug/oxid --engine interpreter
         --evidence /tmp/oxrev-reference
Native mode builds and packages, then runs only the deployed binary. LLVM must
already be installed; this script never downloads or installs tools.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import subprocess
import tarfile

ROOT = Path(__file__).resolve().parents[1]


def sha(data):
    return hashlib.sha256(data).hexdigest()


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("compiler", type=Path)
    parser.add_argument("--engine", choices=["interpreter", "native"], required=True)
    parser.add_argument("--evidence", type=Path, required=True)
    args = parser.parse_args()
    compiler = args.compiler.resolve(strict=True)
    evidence = args.evidence.resolve()
    evidence.mkdir(parents=True, exist_ok=False)
    manifest = ROOT / "examples/projects/oxrev/oxid.toml"
    identity = subprocess.check_output(["git", "rev-parse", "HEAD", "HEAD^{tree}"], cwd=ROOT, text=True).splitlines()
    receipt = {"engine": args.engine, "platform": platform.platform(),
               "machine": platform.machine(), "head": identity[0], "head_tree": identity[1],
               "compiler_sha256": sha(compiler.read_bytes()), "commands": []}

    def invoke(label, command, cwd, data=b""):
        result = subprocess.run([str(part) for part in command], cwd=cwd, input=data, capture_output=True)
        (evidence / (label + ".stdout")).write_bytes(result.stdout)
        (evidence / (label + ".stderr")).write_bytes(result.stderr)
        row = {"label": label, "command": [str(part) for part in command], "cwd": str(cwd),
               "exit": result.returncode, "input_bytes": len(data), "input_sha256": sha(data),
               "stdout_bytes": len(result.stdout), "stdout_sha256": sha(result.stdout),
               "stderr_sha256": sha(result.stderr)}
        receipt["commands"].append(row)
        (evidence / "receipt.json").write_text(json.dumps(receipt, indent=2) + "\n")
        assert result.returncode == 0, row
        assert result.stderr == b"", (row, result.stderr)
        return result.stdout

    base = [compiler, "--manifest-path", manifest, "--package", "oxrev"]
    invoke("check", [base[0], "check", *base[1:]], ROOT)
    deploy = evidence / "deploy"
    deploy.mkdir()
    if args.engine == "native":
        llvm = Path(os.environ["OXID_LLVM_BIN"]).resolve(strict=True)
        for tool in ["clang", "opt", "llvm-as", "ld.lld"]:
            version = invoke("tool-" + tool, [llvm / tool, "--version"], ROOT)
            assert b"19.1.7" in version, (tool, version)
        invoke("build", [base[0], "build", *base[1:], "--entry-mode", "process"], ROOT)
        archive = evidence / "oxrev.tar"
        invoke("package", [base[0], "package", *base[1:], "--entry-mode", "process", "--output", archive], ROOT)
        receipt["archive_sha256"] = sha(archive.read_bytes())
        with tarfile.open(archive) as package:
            package.extractall(deploy, filter="data")
        executable = deploy / "bin/oxrev"
        receipt["native_sha256"] = sha(executable.read_bytes())
        command = [executable]
    else:
        command = [base[0], "run", *base[1:], "--engine", "interpreter", "--entry-mode", "process"]
    for length in [0, 1, 3, 1023, 1024, 1025, 8199]:
        data = bytes([0, 128, 255]) if length == 3 else bytes((i * 73 + 19) % 256 for i in range(length))
        expected = bytes(reversed(data))
        (evidence / f"input-{length}.bin").write_bytes(data)
        (evidence / f"expected-{length}.bin").write_bytes(expected)
        output = invoke(f"reverse-{length}", command, deploy, data)
        assert output == expected, (length, sha(output), sha(expected))
        print(f"{args.engine} bytes={length} exit=0 sha256={sha(output)}")
    (evidence / "receipt.json").write_text(json.dumps(receipt, indent=2) + "\n")


if __name__ == "__main__":
    main()
