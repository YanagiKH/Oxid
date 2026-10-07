#!/usr/bin/env python3
"""Complete an explicit public-route marker with an externally qualified CLI."""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import sys


def sha256(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def run(argv, cwd, prefix, input_bytes=None):
    result = subprocess.run(argv, input=input_bytes, cwd=cwd, capture_output=True, timeout=15)
    (cwd / (prefix + ".stdout")).write_bytes(result.stdout)
    (cwd / (prefix + ".stderr")).write_bytes(result.stderr)
    (cwd / (prefix + ".receipt.json")).write_text(json.dumps({
        "argv": argv, "cwd": str(cwd), "exit_code": result.returncode,
        "executable_sha256": sha256(Path(argv[0])),
        "stdin_sha256": hashlib.sha256(input_bytes).hexdigest() if input_bytes is not None else None,
        "stdout": prefix + ".stdout", "stderr": prefix + ".stderr",
    }, indent=2) + "\n")
    return result


def public_check(canonical, evidence):
    result = run([str(Path(canonical).resolve()), "check", "stdin.ox", "--edition",
                  "typed-preview", "--message-format", "json"], evidence, "public")
    if result.returncode not in (0, 1) or result.stderr:
        raise ValueError("public CLI check failed outside language diagnostics; inspect public streams")
    records = [json.loads(line) for line in result.stdout.splitlines()]
    if not records or records[-1].get("kind") != "check-summary":
        raise ValueError("public CLI did not finish with a check summary")
    diagnostics = records[:-1]
    summary = records[-1]
    if any(record.get("kind") != "diagnostic" for record in diagnostics):
        raise ValueError("public CLI emitted unexpected non-diagnostic records")
    if (summary.get("errors") != len(diagnostics)
            or summary.get("success") != (result.returncode == 0)
            or (result.returncode == 0) != (not diagnostics)):
        raise ValueError("public CLI diagnostic/status/summary disagreement")
    return diagnostics, summary


def validate_observation(value, resolve_only):
    schema = "canonical-resolution-observation-1" if resolve_only else "canonical-static-observation-1"
    if value.get("schema") != schema:
        raise ValueError("unknown or wrong-mode observation schema")
    if resolve_only:
        if "typed_hir" in value or value.get("phase") == "type":
            raise ValueError("resolution observation contains typed facts or a type-check phase")
        if value.get("status") == "ok":
            raise ValueError("resolution success must explicitly identify pending typing")
        if value.get("status") == "resolved":
            if value.get("phase") != "resolve" or value.get("typing") != "pending" or "resolved_hir" not in value:
                raise ValueError("resolution observation did not finish whole-program resolution")
            for function in value["resolved_hir"]["functions"]:
                if any("ty" in item for key in ("locals", "expressions") for item in function[key]):
                    raise ValueError("resolution observation contains checked types")
                if any("flow" in block for block in function["blocks"]):
                    raise ValueError("resolution observation contains checked flow")
        elif "resolved_hir" in value:
            raise ValueError("failed resolution observation contains partial HIR")
    elif "resolved_hir" in value:
        raise ValueError("typed observation contains resolution-only facts")


def observe(observer, canonical, source, evidence, *, resolve_only=False):
    evidence = Path(evidence).resolve()
    evidence.mkdir(parents=True, exist_ok=False)
    (evidence / "stdin.ox").write_bytes(source)
    mode_args = ["--resolve-only"] if resolve_only else []
    result = run([str(Path(observer).resolve()), *mode_args], evidence, "observer", source)
    if result.returncode != 0 or result.stderr:
        raise ValueError("observer did not produce a complete observation; inspect observer streams")
    value = json.loads(result.stdout)
    validate_observation(value, resolve_only)
    if value.get("status") == "public_route_required":
        if value.get("source_name") != "stdin.ox":
            raise ValueError("malformed public-route marker")
        route = value.get("route"), value.get("purpose")
        if route == ("project_scalar", "scalar_project_facts"):
            result = run([str(Path(observer).resolve()), "--project-source", *mode_args], evidence,
                         "project-observer", source)
            if result.returncode != 0 or result.stderr:
                raise ValueError("project observer did not produce a complete observation")
            projected = json.loads(result.stdout)
            validate_observation(projected, resolve_only)
            if projected.get("status") == ("resolved" if resolve_only else "ok"):
                if projected.get("route") != "project_scalar" or projected.get("ast") != value["ast"]:
                    raise ValueError("project observer changed route or original AST projection")
            elif projected.get("status") == "diagnostic":
                phases = ("resolve",) if resolve_only else ("resolve", "type")
                if projected.get("phase") not in phases or "typed_hir" in projected or "resolved_hir" in projected:
                    raise ValueError("project diagnostic observation escaped static phases")
            else:
                raise ValueError("project observer did not finish scalar static observation")
            value = projected
        elif route == ("owned", "diagnostic_only"):
            if canonical is None:
                raise ValueError("an externally qualified --canonical CLI is required for this input")
            diagnostics, summary = public_check(canonical, evidence)
            if not diagnostics or summary["success"]:
                raise ValueError("bounded unknown-type public route unexpectedly accepted input")
            if any(d["stage"] != "resolve" or d["code"] not in ("E0201", "E0202") for d in diagnostics):
                raise ValueError("bounded unknown-type public route escaped its diagnostic-only contract")
            value = {"schema": value["schema"], "status": "diagnostic", "phase": "resolve",
                     "route": "public_owned", "diagnostic": diagnostics[0]}
        else:
            raise ValueError("malformed public-route marker")
    validate_observation(value, resolve_only)
    (evidence / "observation.json").write_text(json.dumps(value, indent=2) + "\n")
    return value


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--observer", type=Path, required=True)
    parser.add_argument("--canonical", type=Path,
                        help="existing independently qualified CLI; not built by this controller")
    parser.add_argument("--evidence", type=Path, required=True, help="fresh per-input directory")
    parser.add_argument("--resolve-only", action="store_true",
                        help="observe complete resolved HIR before type checking, using a separate schema")
    args = parser.parse_args()
    try:
        value = observe(args.observer, args.canonical, sys.stdin.buffer.read(129), args.evidence,
                        resolve_only=args.resolve_only)
    except (OSError, ValueError, subprocess.SubprocessError) as error:
        print("canonical static observation failed: " + str(error), file=sys.stderr)
        return 1
    print(json.dumps(value))
    return 0


if __name__ == "__main__":
    sys.exit(main())
