#!/usr/bin/env python3
"""Execute hand-derived expression-component cases through public Oxid routes."""
import argparse
import hashlib
import json
from pathlib import Path
import shutil
import subprocess

ERRORS = ("ExpectedOperand", "ExpectedOperator", "UnexpectedClose", "UnclosedParen",
          "InvalidCharacter", "NodeLimit", "OperatorLimit", "OperandLimit", "InputLimit")


def source(case):
    codes = ", ".join(str(ord(c)) for c in case["input"])
    declaration = f"let codes = [{codes}];" if codes else "let codes: [i32; 0] = [];"
    body = ["mod arena;", "mod scanner;", "mod parser;", "mod evaluator;", "",
            "fn check_node(a: &crate::arena::Arena, i: i32, k: i32, v: i32, l: i32, r: i32) -> bool {",
            "    return a.kind[i] == k && a.value[i] == v && a.left[i] == l && a.right[i] == r;", "}",
            "fn main() -> i32 {", "    " + declaration,
            "    let mut nodes = crate::arena::new_arena();",
            "    let parsed = crate::parser::parse(&codes, &mut nodes);", "    match parsed {",
            "        crate::parser::ParseResult::Parsed(root) => {"]
    if "error" in case:
        body.append("            return 1;")
    else:
        count = len(case["nodes"]) if "nodes" in case else case.get("count")
        if count is not None:
            body.append(f"            if nodes.count != {count} || root != {count - 1} {{ return 2; }}")
        for i, row in enumerate(case.get("nodes", [])):
            body.append(f"            if !check_node(&nodes, {i}, {', '.join(map(str, row))}) {{ return 3; }}")
        body.extend(["            let evaluated = crate::evaluator::evaluate(&nodes, root);", "            match evaluated {",
                     "                crate::evaluator::EvalResult::Value(value) => {"])
        if "value" in case:
            body.append(f"                    if value != {case['value']} {{ return 4; }}")
        body.extend(["                    return 0;", "                },",
                     "                crate::evaluator::EvalResult::InvalidArena(position) => { return 5; },", "            }"])
    body.append("        },")
    for name in ERRORS:
        action = "return 6;"
        if case.get("error", [None])[0] == name:
            action = f"if position != {case['error'][1]} {{ return 7; }} return 0;"
        body.append(f"        crate::parser::ParseResult::{name}(position) => {{ {action} }},")
    body.extend(["    }", "}"])
    return "\n".join(body) + "\n"


def arena_source(case):
    body = ["mod arena;", "mod evaluator;", "fn main() -> i32 {",
            "    let mut nodes = crate::arena::new_arena();"]
    for index, row in enumerate(case["rows"]):
        for column, value in zip(("kind", "value", "left", "right"), row):
            body.append(f"    nodes.{column}[{index}] = {value};")
    body.extend([f"    nodes.count = {case['count']};",
                 f"    let result = crate::evaluator::evaluate(&nodes, {case['root']});",
                 "    match result {",
                 "        crate::evaluator::EvalResult::Value(value) => { return 1; },",
                 "        crate::evaluator::EvalResult::InvalidArena(position) => {",
                 f"            if position != {case['invalid']} {{ return 2; }}",
                 "            return 0;", "        },", "    }", "}"])
    return "\n".join(body) + "\n"


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--oxid", type=Path, required=True)
    ap.add_argument("--output", type=Path, required=True)
    ap.add_argument("--native", action="store_true")
    args = ap.parse_args()
    compiler = args.oxid.resolve(strict=True)
    fixture = Path(__file__).resolve().parents[1] / "fixtures/typed-expression-samples"
    corpus = json.loads((fixture / "cases.json").read_text())
    cases = corpus["cases"] + corpus["arenas"]
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=False)
    (output / "source-free").mkdir()
    manifest = {p.name: hashlib.sha256(p.read_bytes()).hexdigest() for p in fixture.iterdir() if p.is_file()}
    rows = []
    failed = False
    for case in cases:
        d = output / case["name"]
        d.mkdir()
        for name in ("arena.ox", "scanner.ox", "parser.ox", "evaluator.ox"):
            shutil.copyfile(fixture / name, d / name)
        entry = d / "main.ox"
        entry.write_text(arena_source(case) if "rows" in case else source(case))
        commands = [("check", [str(compiler), "check", str(entry), "--edition", "typed-preview"]),
                    ("reference", [str(compiler), "run", str(entry), "--edition", "typed-preview"])]
        if args.native:
            commands.extend([("compile", [str(compiler), "compile", str(entry), "--edition", "typed-preview", "--backend", "llvm", "--output", str(d / "program")]),
                             ("native", [str(d / "program")])])
        checks = []
        reference_stderr = None
        for stage, command in commands:
            native = stage == "native"
            result = subprocess.run(command, capture_output=True, cwd=output / "source-free" if native else d,
                                    env={"PATH": "/no-tools"} if native else None)
            (d / f"{stage}.stdout").write_bytes(result.stdout)
            (d / f"{stage}.stderr").write_bytes(result.stderr)
            if stage == "reference":
                reference_stderr = result.stderr
            if stage in ("reference", "native"):
                if "runtime_error" in case:
                    origin = str(d / case["diagnostic_source"]).encode() + b":"
                    ok = (result.returncode == 1 and result.stdout == b""
                          and case["runtime_error"].encode() in result.stderr and origin in result.stderr)
                else:
                    ok = result.returncode == 0 and result.stdout == b"0\n" and result.stderr == b""
            else:
                ok = result.returncode == 0 and result.stderr == b""
            if native and "runtime_error" in case:
                ok = ok and result.stderr == reference_stderr
            checks.append({"stage": stage, "command": command, "exit": result.returncode, "passed": ok})
            if not ok:
                failed = True
                break
        rows.append({"name": case["name"], "expectation": case, "checks": checks})
        print(case["name"], "PASS" if all(c["passed"] for c in checks) else "FAIL", flush=True)
    report = {"compiler_sha256": hashlib.sha256(compiler.read_bytes()).hexdigest(), "fixture_sha256": manifest,
              "native": args.native, "cases": rows, "passed": not failed,
              "native_execution": "Empty working directory and cleared environment; no filesystem isolation."}
    (output / "report.json").write_text(json.dumps(report, indent=2) + "\n")
    return int(failed)


if __name__ == "__main__":
    raise SystemExit(main())
