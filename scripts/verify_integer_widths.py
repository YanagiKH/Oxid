#!/usr/bin/env python3
"""Compare runtime integer operations with Python bigints and source-free ELFs.

Operands are supplied after compilation. Every invocation retains exact output
bytes; native execution uses an empty environment and a 32 MiB address limit.
"""
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

TYPES = {
    "i8": (8, True), "u8": (8, False), "i16": (16, True),
    "i32": (32, True), "i64": (64, True), "u16": (16, False),
    "u32": (32, False), "u64": (64, False),
    "isize": (64, True), "usize": (64, False),
}
ADDED = ("i8", "i16", "u16", "u32", "u64", "isize", "usize")
ARITHMETIC = {"add": "+", "sub": "-", "mul": "*", "div": "/", "rem": "%"}
COMPARISONS = {"eq": "==", "ne": "!=", "lt": "<", "le": "<=", "gt": ">", "ge": ">="}
INPUT = """use std::env::arg_bytes;
use std::env::ArgStatus;
fn number(argument:i32)->NUMBER{
    let mut bytes=b".....................";
    let status=arg_bytes(argument,&mut bytes);
    let mut length=0;
    match status{
        ArgStatus::Copied(count)=>{length=count;},
        ArgStatus::TooSmall(required)=>{return ZERO;},
        ArgStatus::Missing=>{return ZERO;},
        ArgStatus::TooLong=>{return ZERO;},
    }
    let mut index=0;
    SIGN
    let mut value=ZERO;
    while index<length{
        let byte=bytes[index];
        let narrow=byte.to_i32();
        let digit=DIGIT;
        ACCUMULATE
        index=index+1;
    }
    return value;
}
"""


def bounds(ty):
    bits, signed = TYPES[ty]
    return (-(2 ** (bits - 1)), 2 ** (bits - 1) - 1) if signed else (0, 2 ** bits - 1)


def boundary_values(ty):
    low, high = bounds(ty)
    values = {low, low + 1, high - 1, high, -1, 0, 1, 2, 127, 128,
              255, 256, 32767, 32768, 65535, 65536, 2**31 - 1, 2**31,
              2**32 - 1, 2**32, 2**53, 2**53 + 1, 2**63 - 1, 2**63}
    return sorted(value for value in values if low <= value <= high)


def operations(ty):
    result = ["identity"]
    if ty in ADDED:
        result += list(ARITHMETIC) + list(COMPARISONS)
        if TYPES[ty][1]:
            result.append("neg")
    result += ["to_" + target for target in TYPES if not (ty == target == "u8")]
    return result


def cases(ty, operation):
    values = boundary_values(ty)
    if operation == "identity" or operation == "neg" or operation.startswith("to_"):
        return [(a, 0) for a in values]
    low, high = bounds(ty)
    special = {low, high, 0, 1, 2}
    if low < 0:
        special.add(-1)
    pairs = {(a, b) for a in values for b in special}
    pairs |= {(b, a) for a, b in pairs.copy()}
    rng = random.Random(0x641628 + list(TYPES).index(ty))
    pairs |= {(rng.randint(low, high), rng.randint(low, high)) for _ in range(12)}
    return sorted(pairs)


def expected(ty, operation, a, b):
    low, high = bounds(ty)
    if operation.startswith("to_"):
        low, high = bounds(operation[3:])
        return (a, None) if low <= a <= high else (None, "E0610")
    if operation in ("div", "rem"):
        if b == 0:
            return None, "E0607"
        if TYPES[ty][1] and a == low and b == -1:
            return None, "E0604"
        quotient = abs(a) // abs(b)
        if (a < 0) != (b < 0):
            quotient = -quotient
        return (quotient if operation == "div" else a - quotient * b), None
    if operation in COMPARISONS:
        return {"eq": a == b, "ne": a != b, "lt": a < b,
                "le": a <= b, "gt": a > b, "ge": a >= b}[operation], None
    value = {"identity": lambda: a, "neg": lambda: -a,
             "add": lambda: a + b, "sub": lambda: a - b,
             "mul": lambda: a * b}[operation]()
    return (value, None) if low <= value <= high else (None, "E0604")


def source(ty, operation):
    signed = TYPES[ty][1]
    number = "i64" if signed else "u64"
    parser = INPUT.replace("NUMBER", number).replace("ZERO", "0" + number)
    parser = parser.replace("SIGN", "let first=bytes[0];let negative=first.to_i32()==45;if negative{index=1;}" if signed else "")
    parser = parser.replace("DIGIT", "narrow.to_i64()-48i64" if signed else "narrow.to_u64_checked()-48u64")
    parser = parser.replace("ACCUMULATE", "if negative{value=value*10i64-digit;}else{value=value*10i64+digit;}" if signed else "value=value*10u64+digit;")
    result_type, marker = ty, None
    if operation in ARITHMETIC or operation in COMPARISONS:
        marker = (ARITHMETIC | COMPARISONS)[operation]
        body = f"return a{marker}b;"
        if operation in COMPARISONS:
            result_type = "bool"
    elif operation == "neg":
        marker, body = "-", "return -a;"
    elif operation.startswith("to_"):
        target = operation[3:]
        marker = operation + "_checked"
        result_type = "i32" if target == "u8" else target
        body = f"let byte=a.{marker}();return byte.to_i32();" if target == "u8" else f"return a.{marker}();"
    else:
        body = "return a.to_i32();" if ty == "u8" else "return a;"
        if ty == "u8":
            result_type = "i32"
    convert = "" if ty == number else f".to_{ty}_checked()"
    prefix = parser + f"fn apply(a:{ty},b:{ty})->{result_type}{{\n    "
    text = prefix + body + f"\n}}\nfn main()->{result_type}{{let parsed_a=number(1);let parsed_b=number(2);let a=parsed_a{convert};let b=parsed_b{convert};return apply(a,b);}}\n"
    offset = len(prefix.encode()) + len(body[:body.index(marker)].encode()) if marker else None
    origin = None if offset is None else {"start": offset, "end": offset + len(marker),
        "line": text[:offset].count("\n") + 1,
        "column": len(text[:offset].rsplit("\n", 1)[-1]) + 1}
    return text, result_type, origin


def require(condition, message):
    if not condition:
        raise ValueError(message)


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def limited_native():
    resource.setrlimit(resource.RLIMIT_AS, (32 * 1024 * 1024,) * 2)


def verify(result, ty, value, error, origin, json_output=False):
    require(result.returncode == (1 if error else 0), "wrong exit status")
    if json_output:
        require(not result.stderr, "unexpected JSON stderr")
        rows = [json.loads(line) for line in result.stdout.splitlines()]
        require(rows[-1]["kind"] == "run-summary", "missing run summary")
        if error:
            require(len(rows) == 2 and rows[0]["code"] == error and rows[0]["stage"] == "oir-run", "wrong JSON diagnostic")
            require(all(rows[0]["primary"][key] == val for key, val in origin.items()), "wrong JSON failure origin")
            require(rows[-1]["result"] is None and not rows[-1]["success"], "failure returned a result")
        else:
            exact = str(value) if ty in TYPES and TYPES[ty][0] == 64 else value
            require(len(rows) == 1 and rows[0]["success"], "unexpected JSON diagnostic")
            require(rows[0]["result"] == {"type": ty, "value": exact}, "inexact JSON value or type")
            require(type(rows[0]["result"]["value"]) is type(exact), "wrong JSON value representation")
        return
    if error:
        require(not result.stdout, "failure printed a partial result")
        text = result.stderr.decode("utf-8", errors="strict")
        require(f"error[{error}] (oir-run):" in text and "E0500" not in text, "wrong runtime diagnostic")
        require(re.search(r"main\.ox:" + str(origin["line"]) + ":" + str(origin["column"]) + r"(?:\s|$)", text), "wrong runtime failure origin")
    else:
        rendered = str(value).lower() if type(value) is bool else str(value)
        require(result.stdout == (rendered + "\n").encode() and not result.stderr, "wrong exact result")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("compilers", nargs="+")
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--llvm-bin", type=Path)
    parser.add_argument("--reference-only", action="store_true")
    parser.add_argument("--types", nargs="+", choices=TYPES, default=list(TYPES))
    parser.add_argument("--operations", nargs="+", choices=sorted({op for ty in TYPES for op in operations(ty)}))
    args = parser.parse_args()
    selected = {ty: [op for op in operations(ty) if args.operations is None or op in args.operations]
                for ty in args.types}
    require(all(selected.values()), "each selected type must have at least one selected operation")
    require(sys.platform == "linux" and os.uname().machine == "x86_64", "requires Linux x86_64")
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=False)
    compilers = [Path(path).resolve(strict=True) for path in args.compilers]
    pins = [{"path": str(path), "bytes": path.stat().st_size, "sha256": sha(path)} for path in compilers]
    receipt = {"schema": 1, "status": "running", "controller_sha256": sha(Path(__file__)),
        "compilers": pins, "reference_only": args.reference_only,
        "selected_operations": selected,
        "source_cases": sum(len(cases(ty, op)) for ty in selected for op in selected[ty]),
        "commands": 0, "reference_cases": 0, "native_cases": 0,
        "successful_cases": 0, "error_cases": 0, "native_elfs": []}
    env = dict(os.environ, OXID_CACHE_DIR=str(output / "cache"))
    env.pop("OXID_PATH", None)
    if args.llvm_bin:
        env["OXID_LLVM_BIN"] = str(args.llvm_bin.resolve(strict=True))
    with (output / "commands.jsonl").open("w") as transcript:
        def invoke(argv, cwd, metadata, native=False):
            result = subprocess.run([str(arg) for arg in argv], cwd=cwd, env={} if native else env,
                stdin=subprocess.DEVNULL, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                timeout=90 if metadata["kind"] == "build" else 15,
                preexec_fn=limited_native if native else None)
            transcript.write(json.dumps({**metadata, "argv": [str(arg) for arg in argv], "cwd": str(cwd),
                "environment": {} if native else "independent compiler cache; OXID_PATH removed",
                "memory_limit_bytes": 32 * 1024 * 1024 if native else None,
                "status": result.returncode, "stdout_hex": result.stdout.hex(), "stderr_hex": result.stderr.hex()}, sort_keys=True) + "\n")
            transcript.flush()
            receipt["commands"] += 1
            return result
        try:
            for compiler_index, compiler in enumerate(compilers):
                for policy in ("bounded", "hosted"):
                    for ty in selected:
                        for operation in selected[ty]:
                            name = f"compiler-{compiler_index}/{policy}/{ty}/{operation}"
                            project = output / "projects" / name
                            (project / "src").mkdir(parents=True)
                            text, result_type, origin = source(ty, operation)
                            manifest = "[project]\nname=\"integer\"\nedition=\"typed-preview\"\nentry=\"src/main.ox\"\n[build]\nentry-mode=\"result\"\n" + f"[execution]\npolicy=\"{policy}\"\n"
                            (project / "src/main.ox").write_text(text)
                            (project / "oxid.toml").write_text(manifest)
                            proof = output / "sources" / name
                            proof.mkdir(parents=True)
                            (proof / "main.ox").write_text(text)
                            (proof / "oxid.toml").write_text(manifest)
                            metadata = {"compiler": compiler_index, "policy": policy, "type": ty, "operation": operation, "result_type": result_type, "origin": origin}
                            rows = cases(ty, operation)
                            for a, b in rows:
                                value, error = expected(ty, operation, a, b)
                                meta = {**metadata, "kind": "reference", "a": str(a), "b": str(b), "expected": value, "expected_error": error}
                                verify(invoke([compiler, "run", "--", a, b], project, meta), result_type, value, error, origin)
                                receipt["reference_cases"] += 1
                                receipt["error_cases" if error else "successful_cases"] += 1
                            for a in boundary_values(ty):
                                b = 0
                                value, error = expected(ty, operation, a, b)
                                meta = {**metadata, "kind": "json", "a": str(a), "b": str(b), "expected": value, "expected_error": error}
                                verify(invoke([compiler, "run", "--message-format=json", "--", a, b], project, meta), result_type, value, error, origin, True)
                            deployments = []
                            if not args.reference_only:
                                for profile in ("dev", "release"):
                                    built = invoke([compiler, "build", "--profile", profile], project, {**metadata, "kind": "build", "profile": profile})
                                    require(built.returncode == 0 and not built.stderr, "native build failed")
                                    deployment = output / "deployment" / name / profile
                                    deployment.mkdir(parents=True)
                                    executable = deployment / "integer"
                                    shutil.copy2(project / ".oxid/bin/integer", executable)
                                    require(executable.read_bytes()[:4] == b"\x7fELF", "not a native ELF")
                                    pin = {**metadata, "path": str(executable.relative_to(output)), "sha256": sha(executable), "bytes": executable.stat().st_size, "profile": profile, "source_sha256": sha(proof / "main.ox")}
                                    receipt["native_elfs"].append(pin)
                                    deployments.append((deployment, executable, pin))
                            shutil.rmtree(project)
                            for deployment, executable, pin in deployments:
                                require(list(deployment.iterdir()) == [executable], "deployment includes source or build inputs")
                                for a, b in rows:
                                    value, error = expected(ty, operation, a, b)
                                    meta = {**metadata, "kind": "native", "profile": pin["profile"], "a": str(a), "b": str(b), "expected": value, "expected_error": error}
                                    verify(invoke([executable, a, b], deployment, meta, native=True), result_type, value, error, origin)
                                    receipt["native_cases"] += 1
                                    receipt["error_cases" if error else "successful_cases"] += 1
                                require(sha(executable) == pin["sha256"], "native ELF changed")
                            print(json.dumps({"completed": name, "commands": receipt["commands"]}), flush=True)
            require(pins == [{"path": str(path), "bytes": path.stat().st_size, "sha256": sha(path)} for path in compilers], "compiler changed during verification")
            require(sha(Path(__file__)) == receipt["controller_sha256"], "controller changed during verification")
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
