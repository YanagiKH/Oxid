#!/usr/bin/env python3
"""Verify character text access against Python UTF-8, including every scalar."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import random
import resource
import shutil
import subprocess

ROOT = Path(__file__).resolve().parents[1]
EXAMPLE = Path("examples/typed-unicode-range")
LIBRARY = Path("packages/utf8_text")


def digest(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def expected_range(first, end):
    return "".join(chr(code) for code in range(first, end)
                   if not 0xd800 <= code <= 0xdfff).encode("utf-8")


def scalar_ranges():
    for start in range(0, 0x110000, 128):
        yield start, min(start + 128, 0x110000)


def reference_ranges():
    ranges = {(0, 0), (0, 128), (0x7f, 0x82), (0x7fe, 0x802),
              (0xd7fe, 0xd802), (0xd800, 0xe000), (0xdffe, 0xe002),
              (0xfffe, 0x10002), (0x10fffc, 0x110000)}
    rng = random.Random(0x7e87)
    for _ in range(32):
        start = rng.randrange(0x110000)
        ranges.add((start, min(start + 4, 0x110000)))
    return sorted(ranges)


def native_limits():
    resource.setrlimit(resource.RLIMIT_AS, (32 * 1024 * 1024,) * 2)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("compilers", nargs="+", type=Path)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--llvm-bin", type=Path)
    parser.add_argument("--reference-only", action="store_true")
    args = parser.parse_args()
    if not args.reference_only and args.llvm_bin is None:
        raise ValueError("native verification requires --llvm-bin")
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=False)
    compilers = [p.resolve(strict=True) for p in args.compilers]
    pins = [{"path": str(p), "bytes": p.stat().st_size, "sha256": digest(p)} for p in compilers]
    sources = [EXAMPLE / "oxid.toml", EXAMPLE / "src/main.ox",
               LIBRARY / "oxid.toml", LIBRARY / "src/lib.ox"]
    source_pins = {str(p): digest(ROOT / p) for p in sources}
    receipt = {"status": "running", "compilers": pins, "sources": source_pins,
               "controller_sha256": digest(__file__), "commands": 0,
               "reference_cases": 0, "native_cases": 0, "unicode_scalars_per_native_profile": 1112064,
               "native_elfs": [], "reference_only": args.reference_only}
    environment = dict(os.environ, OXID_CACHE_DIR=str(output / "cache"))
    environment.pop("OXID_PATH", None)
    if args.llvm_bin:
        environment["OXID_LLVM_BIN"] = str(args.llvm_bin.resolve(strict=True))
    artifacts = {}
    with (output / "commands.jsonl").open("x") as transcript:
        def invoke(argv, cwd, metadata, expected, native=False, status=0, closed=False):
            descriptor = None
            if closed:
                reader, descriptor = os.pipe()
                os.close(reader)
            try:
                result = subprocess.run([str(a) for a in argv], cwd=cwd,
                    env={} if native else environment, stdin=subprocess.DEVNULL,
                    stdout=descriptor if closed else subprocess.PIPE, stderr=subprocess.PIPE,
                    preexec_fn=native_limits if native else None,
                    timeout=90 if metadata["kind"] == "build" else 30)
            finally:
                if descriptor is not None:
                    os.close(descriptor)
            transcript.write(json.dumps({**metadata, "argv": [str(a) for a in argv],
                "cwd": str(cwd), "environment": {} if native else "isolated compiler cache",
                "memory_limit_bytes": 32 * 1024 * 1024 if native else None,
                "status": result.returncode, "stdout_hex": (result.stdout or b"").hex(),
                "stderr_hex": result.stderr.hex()}, sort_keys=True) + "\n")
            transcript.flush()
            receipt["commands"] += 1
            if result.returncode != status or result.stderr:
                raise ValueError("command status or stderr differs: " + repr(metadata))
            if expected is not None and (result.stdout or b"") != expected:
                raise ValueError("exact UTF-8 output differs: " + repr(metadata))
            return result

        try:
            for index, compiler in enumerate(compilers):
                for policy in ("bounded", "hosted"):
                    name = f"compiler-{index}/{policy}"
                    project_root = output / "projects" / name
                    for source in sources:
                        destination = project_root / source
                        destination.parent.mkdir(parents=True, exist_ok=True)
                        shutil.copy2(ROOT / source, destination)
                    manifest = project_root / EXAMPLE / "oxid.toml"
                    configuration = manifest.read_text()
                    if configuration.count('policy = "hosted"') != 1:
                        raise ValueError("execution policy template changed")
                    manifest.write_text(configuration.replace('policy = "hosted"', f'policy = "{policy}"', 1))
                    project = project_root / EXAMPLE
                    proof = output / "sources" / name
                    shutil.copytree(project_root, proof)
                    meta = {"compiler": index, "policy": policy}
                    for first, end in reference_ranges():
                        invoke([compiler, "run", "--", first, end], project,
                               {**meta, "kind": "reference", "first": first, "end": end}, expected_range(first, end))
                        receipt["reference_cases"] += 1
                    invalid = [[], ["1"], ["1", "2", "3"], ["-1", "2"], ["+1", "2"],
                               ["2", "1"], ["0", "1114113"], ["1x", "2"], ["", "2"],
                               ["１", "2"], ["9999999", "1114112"]]
                    for values in invalid:
                        invoke([compiler, "run", "--", *values], project,
                               {**meta, "kind": "invalid-reference"}, b"", status=64)
                    deployments = []
                    if not args.reference_only:
                        for profile in ("dev", "release"):
                            for repeat in range(2):
                                invoke([compiler, "build", "--profile", profile], project,
                                       {**meta, "kind": "build", "profile": profile, "repeat": repeat}, None)
                            deployment = output / "deployment" / name / profile
                            deployment.mkdir(parents=True)
                            binary = deployment / "unicode-range"
                            shutil.copy2(project / ".oxid/bin/unicode-range", binary)
                            if binary.read_bytes()[:4] != b"\x7fELF":
                                raise ValueError("native output is not an ELF")
                            key = (policy, profile)
                            if key in artifacts and artifacts[key] != digest(binary):
                                raise ValueError("reproducible ELF differs across compiler profiles")
                            artifacts[key] = digest(binary)
                            receipt["native_elfs"].append({"path": str(binary), "sha256": digest(binary),
                                                          "policy": policy, "profile": profile})
                            deployments.append((profile, deployment, binary))
                    shutil.rmtree(project_root)
                    for profile, deployment, binary in deployments:
                        if list(deployment.iterdir()) != [binary]:
                            raise ValueError("deployment contains source or build inputs")
                        native_meta = {**meta, "profile": profile}
                        for first, end in scalar_ranges():
                            invoke([binary, first, end], deployment,
                                   {**native_meta, "kind": "native", "first": first, "end": end},
                                   expected_range(first, end), native=True)
                            receipt["native_cases"] += 1
                        for values in invalid:
                            invoke([binary, *values], deployment, {**native_meta, "kind": "invalid-native"},
                                   b"", native=True, status=64)
                        invoke([binary, 65, 91], deployment, {**native_meta, "kind": "closed-output"},
                               b"", native=True, status=74, closed=True)
                        print(json.dumps({"completed": name + "/" + profile,
                                          "commands": receipt["commands"]}), flush=True)
            if not args.reference_only:
                for profile in ("dev", "release"):
                    if artifacts[("bounded", profile)] == artifacts[("hosted", profile)]:
                        raise ValueError("execution policies produced identical native programs")
            if source_pins != {str(p): digest(ROOT / p) for p in sources}:
                raise ValueError("source changed during verification")
            if pins != [{"path": str(p), "bytes": p.stat().st_size, "sha256": digest(p)} for p in compilers]:
                raise ValueError("compiler changed during verification")
            if digest(__file__) != receipt["controller_sha256"]:
                raise ValueError("controller changed during verification")
            receipt["status"] = "passed"
        except BaseException as error:
            receipt.update(status="failed", error=type(error).__name__ + ": " + str(error))
            raise
        finally:
            receipt["commands_sha256"] = digest(output / "commands.jsonl")
            (output / "result.json").write_text(json.dumps(receipt, indent=2, sort_keys=True) + "\n")
    print(json.dumps({k: v for k, v in receipt.items() if k != "native_elfs"}, sort_keys=True))


if __name__ == "__main__":
    main()
