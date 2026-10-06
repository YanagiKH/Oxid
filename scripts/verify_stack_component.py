#!/usr/bin/env python3
"""Verify independently authored stack-code rows and malformed programs."""
import argparse
import hashlib
import json
from pathlib import Path
import shutil
import subprocess

import verify_expression_component as expression

MODULES = ("arena.ox", "scanner.ox", "parser.ox", "evaluator.ox", "stack_code.ox", "lowering.ox")


def initialize(rows, count):
    physical = rows + [[0, 0]] * (15 - len(rows))
    opcode = ", ".join(str(row[0]) for row in physical)
    operand = ", ".join(str(row[1]) for row in physical)
    return [f"    let mut code = crate::stack_code::Code {{ opcode: [{opcode}], operand: [{operand}], count: {count} }};"]


def compare(rows, count):
    body = [f"    if code.count != {count} {{ return 21; }}"]
    for index, (opcode, operand) in enumerate(rows):
        body.append(f"    if !check_row(&code, {index}, {opcode}, {operand}) {{ return 22; }}")
    return body


def execute(case):
    invalid = case.get("invalid")
    good = f"if value != {case['value']} {{ return 23; }} return 0;" if "value" in case else "return 24;"
    bad = f"if position != {invalid} {{ return 25; }} return 0;" if invalid is not None else "return 26;"
    return ["    let result = crate::stack_code::execute(&code);", "    match result {",
            f"        crate::stack_code::CodeResult::Value(value) => {{ {good} }},",
            f"        crate::stack_code::CodeResult::InvalidCode(position) => {{ {bad} }},", "    }"]


def source(case, initial, arenas):
    category = case["category"]
    if category == "execute":
        return "\n".join(["mod stack_code;", "fn main() -> i32 {", *initialize(case["rows"], case["count"]), *execute(case), "}"]) + "\n"
    body = [*("mod " + Path(name).stem + ";" for name in MODULES),
            "fn check_row(code: &crate::stack_code::Code, index: i32, opcode: i32, operand: i32) -> bool {",
            "    return code.opcode[index] == opcode && code.operand[index] == operand;", "}",
            "fn main() -> i32 {",
            "    let mut nodes = crate::arena::new_arena();", *initialize(initial["rows"], initial["count"])]
    if category == "lower":
        codes = ", ".join(str(ord(c)) for c in case["text"])
        body += [f"    let input = [{codes}];", "    let parsed = crate::parser::parse(&input, &mut nodes);", "    match parsed {",
                 "        crate::parser::ParseResult::Parsed(root) => {"]
        expected = case["rows"] + initial["rows"][case["count"]:]
        body += ["    let lowered = crate::lowering::lower(&nodes, root, &mut code);", "    match lowered {",
                 "        crate::lowering::LowerResult::Lowered(count) => {", f"    if count != {case['count']} {{ return 27; }}",
                 *compare(expected, case["count"]), *execute(case), "        },",
                 "        crate::lowering::LowerResult::InvalidArena(position) => { return 28; },", "    }", "        },"]
        body += [f"        crate::parser::ParseResult::{name}(position) => {{ return 29; }}," for name in expression.ERRORS]
        body += ["    }"]
    else:
        arena = arenas[case["arena_case"]]
        for index, row in enumerate(arena["rows"]):
            for column, value in zip(("kind", "value", "left", "right"), row):
                body.append(f"    nodes.{column}[{index}] = {value};")
        body += [f"    nodes.count = {arena['count']};", f"    let lowered = crate::lowering::lower(&nodes, {arena['root']}, &mut code);",
                 "    match lowered {", "        crate::lowering::LowerResult::Lowered(count) => { return 30; },",
                 "        crate::lowering::LowerResult::InvalidArena(position) => {",
                 f"    if position != {arena['invalid']} {{ return 31; }}", *compare(initial["rows"], initial["count"]),
                 "    return 0;", "        },", "    }"]
    return "\n".join([*body, "}"]) + "\n"


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--oxid", type=Path, required=True)
    ap.add_argument("--output", type=Path, required=True)
    ap.add_argument("--native", action="store_true")
    args = ap.parse_args()
    compiler = args.oxid.resolve(strict=True)
    fixture = Path(__file__).resolve().parents[1] / "fixtures/typed-expression-samples"
    corpus = json.loads((fixture / "stack-cases.json").read_text())
    arenas = {c["name"]: c for c in json.loads((fixture / "cases.json").read_text())["arenas"]}
    output = args.output.resolve(); output.mkdir(parents=True, exist_ok=False)
    empty = output / "empty"; empty.mkdir()
    report = {"compiler_sha256": hashlib.sha256(compiler.read_bytes()).hexdigest(), "native": args.native,
              "inputs": {n: hashlib.sha256((fixture / n).read_bytes()).hexdigest() for n in (*MODULES, "stack-cases.json", "cases.json")},
              "cases": [], "passed": False}
    failed = False
    for case in corpus["cases"]:
        directory = output / case["name"]; directory.mkdir()
        for name in (("stack_code.ox",) if case["category"] == "execute" else MODULES):
            shutil.copyfile(fixture / name, directory / name)
        entry = directory / "main.ox"; entry.write_text(source(case, corpus["initial_code"], arenas))
        program = directory / "program"
        commands = [("check", [compiler, "check", entry, "--edition", "typed-preview"]),
                    ("reference", [compiler, "run", entry, "--edition", "typed-preview"])]
        if args.native:
            commands += [("compile", [compiler, "compile", entry, "--edition", "typed-preview", "--backend", "llvm", "--output", program]),
                         ("native", [program])]
        checks = []; reference_error = None
        for stage, argv in commands:
            native = stage == "native"; argv = list(map(str, argv))
            result = subprocess.run(argv, capture_output=True, cwd=empty if native else directory,
                                    env={"PATH": "/no-tools"} if native else None, timeout=120)
            (directory / (stage + ".stdout")).write_bytes(result.stdout)
            (directory / (stage + ".stderr")).write_bytes(result.stderr)
            if stage == "reference": reference_error = result.stderr
            if stage in ("reference", "native"):
                if "runtime_error" in case:
                    ok = result.returncode == 1 and result.stdout == b"" and b"error[E0604]" in result.stderr and (str(directory / "stack_code.ox") + ":").encode() in result.stderr
                    if native: ok = ok and result.stderr == reference_error
                else: ok = result.returncode == 0 and result.stdout == b"0\n" and result.stderr == b""
            else: ok = result.returncode == 0 and result.stderr == b""
            checks.append({"stage": stage, "argv": argv, "status": result.returncode, "passed": bool(ok)})
            if not ok: failed = True; break
        row = {"name": case["name"], "expectation": case, "checks": checks}
        if program.exists(): row["program_sha256"] = hashlib.sha256(program.read_bytes()).hexdigest()
        report["cases"].append(row)
        (output / "report.json").write_text(json.dumps(report, indent=2) + "\n")
        print(case["name"], "PASS" if all(x["passed"] for x in checks) else "FAIL", flush=True)
    report["passed"] = not failed
    (output / "report.json").write_text(json.dumps(report, indent=2) + "\n")
    return int(failed)


if __name__ == "__main__":
    raise SystemExit(main())
