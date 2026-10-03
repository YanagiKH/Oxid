#!/usr/bin/env python3
"""Pure comparison protocol. There is deliberately no compiler adapter here."""
from __future__ import annotations
import argparse
import json
from pathlib import Path


def projection(fixture):
    """Data that a future adapter must derive from real compiler hooks/HIR.

    Model-only reasons, permission representative tables, and deferred renderer
    wording are never disguised as actual compiler trace fields.
    """
    e = fixture["expected"]
    definitions = e["source_declaration_predictions"]
    diagnostics = []
    for d in e["diagnostics"]:
        item = {k: d[k] for k in ("code", "stage", "phase", "order", "primary", "secondary")}
        if d["message"] is not None:
            item["message"] = d["message"]
        diagnostics.append(item)
    out = {
        "syntax_flavor": e["syntax_flavor"],
        "selected_route": e["selected_route"],
        "modules": definitions["modules"],
        "function_declarations": definitions["functions"],
        "record_declarations": [{k: v for k, v in d.items() if k != "canonical_name"}
                                for d in definitions["records"]],
        "final_index_available": e["final_index_available"],
        "first_stopping_phase": e["first_stopping_phase"],
        "diagnostics": diagnostics,
        "import_events": e["import_events"],
        "observed_targets": [{k: op[k] for k in ("phase", "owner", "kind", "origin", "target", "field_id") if k in op}
                             for op in e["reached_operations"] if "target" in op],
    }
    if e["final_index_available"]:
        out["index_expectations"] = e["index_expectations"]
    return out


def compare(expected, actual, path="$", failures=None):
    """Closed structural comparison with only diagnostic prose left flexible.

    Extra message/label/note rendering may appear in actual diagnostic objects.
    No other missing, extra, reordered or unexpected result is accepted.
    """
    failures = [] if failures is None else failures
    if isinstance(expected, dict):
        if not isinstance(actual, dict):
            failures.append(f"{path}: expected object")
            return failures
        extra = set(actual) - set(expected)
        if ".diagnostics[" in path and path.count(".") == 1:
            extra -= {"message", "labels", "notes"}
        if extra:
            failures.append(f"{path}: unexpected keys {sorted(extra)}")
        for key, value in expected.items():
            if key not in actual:
                failures.append(f"{path}.{key}: missing")
            else:
                compare(value, actual[key], path + "." + key, failures)
    elif isinstance(expected, list):
        if not isinstance(actual, list):
            failures.append(f"{path}: expected list")
            return failures
        if len(expected) != len(actual):
            failures.append(f"{path}: expected {len(expected)} elements, found {len(actual)}")
        for i, (want, got) in enumerate(zip(expected, actual)):
            compare(want, got, f"{path}[{i}]", failures)
    elif type(expected) is not type(actual) or expected != actual:
        failures.append(f"{path}: expected {expected!r}, found {actual!r}")
    return failures


def check_observation(fixture, observation):
    required = ("candidate_source_manifest_sha256", "adapter_source_sha256", "binary_sha256",
                "rustc_version", "host", "profile", "argv", "exit_status", "stdout_sha256",
                "stderr_sha256", "started_at_utc", "finished_at_utc", "source_files")
    errors = []
    metadata = observation.get("provenance", {})
    for key in required:
        if key not in metadata:
            errors.append(f"provenance.{key}: required")
    if metadata.get("profile") not in ("debug", "release"):
        errors.append("provenance.profile: must be debug or release")
    if metadata.get("source_files") != fixture["files"]:
        errors.append("source hashes/byte lengths must exactly match frozen fixture files")
    if observation.get("case") != fixture["case"]:
        errors.append("case identity differs")
    if observation.get("status") != "actual-compiler-observation":
        errors.append("status must identify an actual compiler observation")
    errors.extend(compare(projection(fixture), observation.get("result")))
    return errors


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("fixture", type=Path)
    parser.add_argument("observation", type=Path)
    args = parser.parse_args()
    failures = check_observation(json.loads(args.fixture.read_text()),
                                 json.loads(args.observation.read_text()))
    print(json.dumps({"match": not failures, "failures": failures}, indent=2))
    raise SystemExit(bool(failures))


if __name__ == "__main__":
    main()
