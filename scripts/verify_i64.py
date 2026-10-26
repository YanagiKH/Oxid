#!/usr/bin/env python3
"""Independent bigint oracle for runtime i64 operands and source-free native ELFs.

Python computes every expected result without importing compiler internals.
Operands arrive through OS arguments after compilation, including for -O2.
All commands and exact output bytes are retained in commands.jsonl.
"""
import linux_native_target as native_target
import argparse
import hashlib
import json
import os
from pathlib import Path
import random
import re
import resource
import shutil
import subprocess
import sys

MIN = -(2**63)
MAX = 2**63 - 1
BOUNDARIES = (
    MIN, MIN + 1, -(2**53) - 1, -(2**53), -(2**31) - 1,
    -(2**31), -3037000500, -3037000499, -256, -1, 0, 1, 2,
    255, 256, 3037000499, 3037000500, 2**31 - 1, 2**31,
    2**53, 2**53 + 1, MAX - 1, MAX,
)
# The argument parser deliberately accumulates negative numbers negatively so
# MIN itself is admitted. Only valid decimal inputs from this controller enter.
INPUT = """use std::env::arg_bytes;
use std::env::ArgStatus;
fn number(argument:i32)->i64{
    let mut bytes=b".....................";
    let status=arg_bytes(argument,&mut bytes);
    let mut length=0;
    match status{
        ArgStatus::Copied(count)=>{length=count;},
        ArgStatus::TooSmall(required)=>{return 0i64;},
        ArgStatus::Missing=>{return 0i64;},
        ArgStatus::TooLong=>{return 0i64;},
    }
    let mut index=0;
    let first=bytes[0];
    let negative=first.to_i32()==45;
    if negative{index=1;}
    let mut value=0i64;
    while index<length{
        let byte=bytes[index];
        let narrow=byte.to_i32();
        let digit=narrow.to_i64()-48i64;
        if negative{value=value*10i64-digit;}else{value=value*10i64+digit;}
        index=index+1;
    }
    return value;
}
"""
OPERATIONS = {
    "identity": ("i64", "return a;", None),
    "add": ("i64", "return a+b;", "+"),
    "sub": ("i64", "return a-b;", "-"),
    "mul": ("i64", "return a*b;", "*"),
    "div": ("i64", "return a/b;", "/"),
    "rem": ("i64", "return a%b;", "%"),
    "neg": ("i64", "return -a;", "-"),
    "eq": ("bool", "return a==b;", "=="),
    "ne": ("bool", "return a!=b;", "!="),
    "lt": ("bool", "return a<b;", "<"),
    "le": ("bool", "return a<=b;", "<="),
    "gt": ("bool", "return a>b;", ">"),
    "ge": ("bool", "return a>=b;", ">="),
    "to_i32": ("i32", "return a.to_i32_checked();", "to_i32_checked"),
    "to_u8": ("i32", "let byte=a.to_u8_checked();return byte.to_i32();", "to_u8_checked"),
}


def expected(operation, a, b):
    """Use only Python arbitrary-precision arithmetic and explicit range checks."""
    if operation in ("to_i32", "to_u8"):
        low, high = (-(2**31), 2**31 - 1) if operation == "to_i32" else (0, 255)
        return (a, None) if low <= a <= high else (None, "E0610")
    if operation in ("div", "rem"):
        if b == 0:
            return None, "E0607"
        if a == MIN and b == -1:
            return None, "E0604"
        quotient = abs(a) // abs(b)
        if (a < 0) != (b < 0):
            quotient = -quotient
        return (quotient if operation == "div" else a - quotient * b), None
    comparisons = {
        "eq": lambda: a == b, "ne": lambda: a != b,
        "lt": lambda: a < b, "le": lambda: a <= b,
        "gt": lambda: a > b, "ge": lambda: a >= b,
    }
    if operation in comparisons:
        return comparisons[operation](), None
    value = {
        "identity": lambda: a, "add": lambda: a + b,
        "sub": lambda: a - b, "mul": lambda: a * b, "neg": lambda: -a,
    }[operation]()
    return (value, None) if MIN <= value <= MAX else (None, "E0604")


def cases(operation):
    if operation in ("identity", "neg", "to_i32", "to_u8"):
        return [(value, 0) for value in BOUNDARIES]
    pairs = {(a, b) for a in BOUNDARIES for b in (MIN, -1, 0, 1, 2, MAX)}
    pairs |= {(b, a) for a, b in pairs.copy()}
    rng = random.Random(0x164C0DE)
    pairs |= {(rng.randint(MIN, MAX), rng.randint(MIN, MAX)) for _ in range(24)}
    return sorted(pairs)


def source(operation):
    ty, body, token = OPERATIONS[operation]
    prefix = INPUT + f"fn apply(a:i64,b:i64)->{ty}{{\n    "
    text = prefix + body + "\n}\n" + (
        f"fn main()->{ty}{{let a=number(1);let b=number(2);return apply(a,b);}}\n"
    )
    offset = len(prefix) + body.index(token) if token else None
    origin = None if offset is None else {
        "start": offset, "end": offset + len(token),
        "line": text[:offset].count("\n") + 1,
        "column": len(text[:offset].rsplit("\n", 1)[-1]) + 1,
    }
    return text, origin


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def limited_native():
    resource.setrlimit(resource.RLIMIT_AS, (32 * 1024 * 1024,) * 2)


def require(condition, message):
    if not condition:
        raise ValueError(message)


def verify(result, value, error, origin, json_output=False, result_type="i64"):
    if json_output:
        require(not result.stderr, "unexpected JSON stderr")
        rows = [json.loads(line) for line in result.stdout.splitlines()]
        require(result.returncode == (1 if error else 0), "wrong JSON exit status")
        require(rows[-1]["kind"] == "run-summary", "missing run summary")
        if error:
            require(len(rows) == 2 and rows[0]["code"] == error, "wrong JSON diagnostic")
            require(rows[0]["stage"] == "oir-run", "failure did not occur during execution")
            primary = rows[0]["primary"]
            require(all(primary[k] == v for k, v in origin.items()), "wrong JSON failure origin")
            require(rows[-1]["result"] is None and not rows[-1]["success"], "failure returned a value")
        else:
            exact = str(value) if result_type == "i64" else value
            require(len(rows) == 1 and rows[0]["success"], "unexpected JSON diagnostic")
            require(rows[0]["result"] == {"type": result_type, "value": exact}, "inexact JSON result")
            if result_type == "i64":
                require(type(rows[0]["result"]["value"]) is str, "i64 JSON lost decimal string")
        return
    require(result.returncode == (1 if error else 0), "wrong exit status")
    if error:
        require(not result.stdout, "failure printed a partial result")
        text = result.stderr.decode("utf-8", errors="strict")
        require(f"error[{error}] (oir-run):" in text, "wrong runtime diagnostic")
        require("E0500" not in text, "internal verifier error")
        require(re.search(r"main\.ox:" + str(origin["line"]) + ":" + str(origin["column"]) + r"(?:\s|$)", text),
                "wrong runtime failure origin")
    else:
        rendered = str(value).lower() if type(value) is bool else str(value)
        require(result.stdout == (rendered + "\n").encode() and not result.stderr, "wrong exact result")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("compilers", nargs="+")
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--llvm-bin", type=Path)
    parser.add_argument("--reference-only", action="store_true")
    args = parser.parse_args()
    require(native_target.host() is not None, "argument execution requires Linux x86_64 or ARM64")
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=False)
    compilers = [Path(path).resolve(strict=True) for path in args.compilers]
    pins = [{"path": str(path), "bytes": path.stat().st_size, "sha256": sha(path)} for path in compilers]
    receipt = {
        "schema": 1, "status": "running", "controller_sha256": sha(Path(__file__)),
        "compilers": pins, "reference_only": args.reference_only,
        "source_cases": sum(len(cases(op)) for op in OPERATIONS),
        "commands": 0, "reference_cases": 0, "native_cases": 0,
        "successful_cases": 0, "error_cases": 0, "native_elfs": [],
    }
    env = dict(os.environ, OXID_CACHE_DIR=str(output / "cache"))
    env.pop("OXID_PATH", None)
    if args.llvm_bin:
        env["OXID_LLVM_BIN"] = str(args.llvm_bin.resolve(strict=True))
    with (output / "commands.jsonl").open("w") as transcript:
        def invoke(argv, cwd, metadata, native=False):
            result = subprocess.run(
                [str(arg) for arg in argv], cwd=cwd, env={} if native else env,
                stdin=subprocess.DEVNULL, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                timeout=90 if metadata["kind"] == "build" else 15,
                preexec_fn=limited_native if native else None,
            )
            transcript.write(json.dumps({
                **metadata, "argv": [str(arg) for arg in argv], "cwd": str(cwd),
                "environment": {} if native else "compiler environment; independent cache; OXID_PATH removed",
                "memory_limit_bytes": 32 * 1024 * 1024 if native else None,
                "status": result.returncode, "stdout_hex": result.stdout.hex(), "stderr_hex": result.stderr.hex(),
            }, sort_keys=True) + "\n")
            transcript.flush()
            receipt["commands"] += 1
            return result

        try:
            for compiler_index, compiler in enumerate(compilers):
                for policy in ("bounded", "hosted"):
                    for operation, (result_type, _, _) in OPERATIONS.items():
                        name = f"compiler-{compiler_index}/{policy}/{operation}"
                        project = output / "projects" / name
                        (project / "src").mkdir(parents=True)
                        text, origin = source(operation)
                        (project / "src/main.ox").write_text(text)
                        manifest = ("[project]\nname=\"integer\"\nedition=\"typed-preview\"\n"
                                    "entry=\"src/main.ox\"\n[build]\nentry-mode=\"result\"\n"
                                    f"[execution]\npolicy=\"{policy}\"\n")
                        (project / "oxid.toml").write_text(manifest)
                        proof_source = output / "sources" / name
                        proof_source.mkdir(parents=True)
                        (proof_source / "main.ox").write_text(text)
                        (proof_source / "oxid.toml").write_text(manifest)
                        rows = cases(operation)
                        for a, b in rows:
                            value, error = expected(operation, a, b)
                            metadata = {"kind": "reference", "compiler": compiler_index, "policy": policy,
                                        "operation": operation, "a": str(a), "b": str(b),
                                        "expected": value, "expected_error": error, "origin": origin}
                            result = invoke([compiler, "run", "--", a, b], project, metadata)
                            verify(result, value, error, origin, result_type=result_type)
                            receipt["reference_cases"] += 1
                            receipt["error_cases" if error else "successful_cases"] += 1
                        # Check JSON precision above 2**53 and negative limits,
                        # plus runtime error positions without native formatting.
                        for a, b in ((MAX, 1), (MIN, -1), (2**53 + 1, 0)):
                            value, error = expected(operation, a, b)
                            result = invoke([compiler, "run", "--message-format=json", "--", a, b], project,
                                            {"kind": "json", "operation": operation, "policy": policy,
                                             "compiler": compiler_index, "a": str(a), "b": str(b)})
                            verify(result, value, error, origin, True, result_type)
                        deployments = []
                        if not args.reference_only:
                            for profile in ("dev", "release"):
                                built = invoke([compiler, "build", "--profile", profile], project,
                                               {"kind": "build", "operation": operation, "policy": policy,
                                                "compiler": compiler_index, "profile": profile})
                                require(built.returncode == 0 and not built.stderr, "native build failed")
                                deployment = output / "deployment" / name / profile
                                deployment.mkdir(parents=True)
                                executable = deployment / "integer"
                                shutil.copy2(project / ".oxid/bin/integer", executable)
                                require(executable.read_bytes()[:4] == b"\x7fELF", "not a native ELF")
                                pin = {"path": str(executable.relative_to(output)), "sha256": sha(executable),
                                       "bytes": executable.stat().st_size, "profile": profile,
                                       "policy": policy, "compiler": compiler_index, "operation": operation,
                                       "source_sha256": sha(proof_source / "main.ox")}
                                receipt["native_elfs"].append(pin)
                                deployments.append((deployment, executable, pin))
                        # Native commands have no project, compiler, source,
                        # interpreter, cache or environment in their directory.
                        shutil.rmtree(project)
                        for deployment, executable, pin in deployments:
                            require(list(deployment.iterdir()) == [executable], "deployment contains source or build inputs")
                            for a, b in rows:
                                value, error = expected(operation, a, b)
                                metadata = {"kind": "native", "operation": operation, "compiler": compiler_index,
                                            "policy": policy, "profile": pin["profile"],
                                            "a": str(a), "b": str(b), "expected": value,
                                            "expected_error": error, "origin": origin}
                                result = invoke([executable, a, b], deployment, metadata, native=True)
                                verify(result, value, error, origin, result_type=result_type)
                                receipt["native_cases"] += 1
                                receipt["error_cases" if error else "successful_cases"] += 1
                            require(sha(executable) == pin["sha256"], "native ELF changed")
                        print(json.dumps({"completed": name, "commands": receipt["commands"]}), flush=True)
            require(pins == [{"path": str(path), "bytes": path.stat().st_size, "sha256": sha(path)} for path in compilers],
                    "compiler changed during qualification")
            receipt["status"] = "passed"
        except Exception as error:
            receipt["status"] = "failed"
            receipt["failure"] = f"{type(error).__name__}: {error}"
            raise
        finally:
            receipt["commands_sha256"] = sha(output / "commands.jsonl")
            (output / "result.json").write_text(json.dumps(receipt, indent=2, sort_keys=True) + "\n")
    print(json.dumps({key: value for key, value in receipt.items() if key != "native_elfs"}, sort_keys=True))


if __name__ == "__main__":
    main()
