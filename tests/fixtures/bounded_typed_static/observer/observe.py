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


def observe(observer, canonical, source, evidence):
    evidence = Path(evidence).resolve()
    evidence.mkdir(parents=True, exist_ok=False)
    (evidence / "stdin.ox").write_bytes(source)
    result = run([str(Path(observer).resolve())], evidence, "observer", source)
    if result.returncode != 0 or result.stderr:
        raise ValueError("observer did not produce a complete observation; inspect observer streams")
    value = json.loads(result.stdout)
    if value.get("schema") != "canonical-static-observation-1":
        raise ValueError("unknown observation schema")
    if value.get("status") == "public_route_required":
        if value.get("source_name") != "stdin.ox":
            raise ValueError("malformed public-route marker")
        route = value.get("route"), value.get("purpose")
        if route == ("project_scalar", "scalar_project_facts"):
            result = run([str(Path(observer).resolve()), "--project-source"], evidence,
                         "project-observer", source)
            if result.returncode != 0 or result.stderr:
                raise ValueError("project observer did not produce a complete observation")
            projected = json.loads(result.stdout)
            if projected.get("schema") != value["schema"]:
                raise ValueError("project observation schema drifted")
            if projected.get("status") == "ok":
                if projected.get("route") != "project_scalar" or projected.get("ast") != value["ast"]:
                    raise ValueError("project observer changed route or original AST projection")
            elif projected.get("status") == "diagnostic":
                if projected.get("phase") not in ("resolve", "type") or "typed_hir" in projected:
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
    (evidence / "observation.json").write_text(json.dumps(value, indent=2) + "\n")
    return value


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--observer", type=Path, required=True)
    parser.add_argument("--canonical", type=Path,
                        help="existing independently qualified CLI; not built by this controller")
    parser.add_argument("--evidence", type=Path, required=True, help="fresh per-input directory")
    args = parser.parse_args()
    try:
        value = observe(args.observer, args.canonical, sys.stdin.buffer.read(129), args.evidence)
    except (OSError, ValueError, subprocess.SubprocessError) as error:
        print("canonical static observation failed: " + str(error), file=sys.stderr)
        return 1
    print(json.dumps(value))
    return 0


if __name__ == "__main__":
    sys.exit(main())
