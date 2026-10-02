#!/usr/bin/env python3
"""Bind explicit current inputs to the byte-identical historical Unit2 runner.

Only a temporary derived source manifest and its containing package manifest
change. The observer, expectations, protocol and execution orchestration do not.
"""
import argparse
import datetime
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import uuid

HISTORICAL = Path(__file__).resolve().parent.parent / "typed_project_unit2_independent"
sys.path.insert(0, str(HISTORICAL))
import protocol as p


def encoded(value):
    return (json.dumps(value, indent=2, sort_keys=True) + "\n").encode()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo", type=Path, required=True)
    parser.add_argument("--source-manifest", type=Path, required=True,
                        help="Explicit current compiler manifest; no historical fallback")
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--cargo", default="cargo")
    parser.add_argument("--rustc", default="rustc")
    parser.add_argument("--prepare-only", action="store_true")
    args = parser.parse_args()
    repo, output = args.repo.resolve(strict=True), args.output.resolve()
    selected = args.source_manifest.resolve(strict=True)
    adapter = Path(__file__).resolve()
    adapter_sha = p.digest(adapter.read_bytes())
    p.require(output != repo and repo not in output.parents, "output must be outside repository")
    p.require(output != HISTORICAL and HISTORICAL not in output.parents,
              "output must be outside historical package")
    p.require(not output.exists(), "output must be new")

    historical_manifest_bytes = (HISTORICAL / "package-inputs.json").read_bytes()
    historical_manifest = json.loads(historical_manifest_bytes)
    p.check_files(HISTORICAL, historical_manifest["files"])
    selected_bytes = selected.read_bytes()
    selected_manifest = json.loads(selected_bytes)
    p.check_files(repo, selected_manifest["files"])
    # Preserve the original assembler's exact current source membership check.
    p.exact_ids([str(file.relative_to(repo)) for part in ("src", "native")
                 for file in (repo / part).rglob("*") if file.is_file()],
                [entry["path"] for entry in selected_manifest["files"]
                 if entry["path"].startswith(("src/", "native/"))])
    source_entries = [row for row in historical_manifest["files"] if row["path"] == "source-inputs.json"]
    p.require(len(source_entries) == 1, "historical package must bind exactly one source manifest")
    historical_source_sha = p.digest((HISTORICAL / "source-inputs.json").read_bytes())
    p.require(source_entries[0]["sha256"] == historical_source_sha,
              "historical source manifest binding differs")

    output.mkdir(parents=True)
    derived = output / "derived-package"
    derived.mkdir()
    for entry in historical_manifest["files"]:
        target = derived / entry["path"]
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copy2(p.relative_file(HISTORICAL, entry["path"]), target)
    (derived / "source-inputs.json").write_bytes(selected_bytes)
    derived_manifest = {**historical_manifest, "files": [
        {**entry, "bytes": len(selected_bytes), "sha256": p.digest(selected_bytes)}
        if entry["path"] == "source-inputs.json" else entry
        for entry in historical_manifest["files"]]}
    derived_manifest_bytes = encoded(derived_manifest)
    (derived / "package-inputs.json").write_bytes(derived_manifest_bytes)
    p.check_files(derived, derived_manifest["files"])
    contract = {
        "schema": 1, "invocation_id": uuid.uuid4().hex,
        "started_at_utc": datetime.datetime.now(datetime.timezone.utc).isoformat(),
        "source_manifest": str(selected), "source_inputs_sha256": p.digest(selected_bytes),
        "historical_source_inputs_sha256": historical_source_sha,
        "historical_package_inputs_sha256": p.digest(historical_manifest_bytes),
        "derived_package_inputs_sha256": p.digest(derived_manifest_bytes),
        "adapter_sha256": adapter_sha,
        "derived_changes": ["source-inputs.json", "package-inputs.json"],
    }
    (output / "invocation.json").write_bytes(encoded(contract))
    argv = [sys.executable, "-B", str(derived / "run.py"), "--repo", str(repo),
            "--output", str(output / "run"), "--cargo", args.cargo, "--rustc", args.rustc]
    if args.prepare_only:
        argv.append("--prepare-only")
    env = dict(os.environ, CARGO_INCREMENTAL="0", PYTHONDONTWRITEBYTECODE="1")
    env.pop("PYTHONOPTIMIZE", None)
    with (output / "runner.stdout").open("wb") as stdout, (output / "runner.stderr").open("wb") as stderr:
        result = subprocess.run(argv, cwd=repo, env=env, stdout=stdout, stderr=stderr)
    command = {"argv": argv, "cwd": str(repo), "exit_status": result.returncode,
               "stdout_sha256": p.digest((output / "runner.stdout").read_bytes()),
               "stderr_sha256": p.digest((output / "runner.stderr").read_bytes())}
    (output / "command.json").write_bytes(encoded(command))
    p.require(result.returncode == 0, "historical runner failed; see retained runner.stderr")
    p.check_files(HISTORICAL, historical_manifest["files"])
    p.unchanged_file(HISTORICAL / "package-inputs.json", contract["historical_package_inputs_sha256"])
    p.check_files(derived, derived_manifest["files"])
    p.unchanged_file(derived / "package-inputs.json", contract["derived_package_inputs_sha256"])
    p.unchanged_file(selected, contract["source_inputs_sha256"])
    p.unchanged_file(adapter, adapter_sha)
    p.check_files(repo, selected_manifest["files"])
    # Execution and all semantic/resource checks remain the historical runner's
    # responsibility. This envelope only binds its result to the derivation.
    artifact = "prepared.json" if args.prepare_only else "result.json"
    child_file = output / "run" / artifact
    child = json.loads(child_file.read_bytes())
    p.require(child["source_inputs_sha256"] == contract["source_inputs_sha256"], "wrong child source identity")
    p.require(child["package_inputs_sha256"] == contract["derived_package_inputs_sha256"], "wrong child package identity")
    p.require(child["status"] == ("prepared" if args.prepare_only else "passed"), "wrong child completion state")
    if args.prepare_only:
        p.require(child["compiler_executions"] == 0, "preparation must not execute compiler")
    envelope = {**contract, "status": child["status"], "exit_status": 0,
                "child_invocation_id": child["invocation_id"],
                "child_result": "run/" + artifact, "child_result_sha256": p.digest(child_file.read_bytes()),
                "command_sha256": p.digest((output / "command.json").read_bytes())}
    if args.prepare_only:
        envelope["compiler_executions"] = 0
    (output / artifact).write_bytes(encoded(envelope))
    print(json.dumps({"status": child["status"], "output": str(output),
                      "source_inputs_sha256": contract["source_inputs_sha256"]}))


if __name__ == "__main__":
    main()
