"""Run the frozen Unit1 compatibility matrix without writing into the repository."""
import argparse
import base64
import hashlib
import itertools
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile

sys.dont_write_bytecode = True
PACKAGE = Path(__file__).resolve().parent


def require(condition, message):
    if not condition:
        raise ValueError(message)


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def read(path):
    return json.loads(path.read_text(encoding="utf-8"))


def write(path, value):
    path.write_text(json.dumps(value, indent=2) + "\n", encoding="utf-8")


def inventory():
    return {p.name: sha(p) for p in PACKAGE.iterdir() if p.is_file()}


def load_package():
    metadata = read(PACKAGE / "package-manifest.json")
    actual = inventory()
    require(set(actual) == set(metadata["files"]) | {"package-manifest.json"}, "unexpected or missing package files")
    for name, item in metadata["files"].items():
        require(actual[name] == item["sha256"], f"package hash mismatch: {name}")
        require((PACKAGE / name).stat().st_size == item["bytes"], f"package size mismatch: {name}")
    require(not any(PACKAGE.rglob("*.ox")), "installed package must contain no Oxid fixtures")
    intent = read(PACKAGE / "suite-intent.json")
    cases = intent["cases"]
    names = [c["name"] for c in cases]
    require(len(cases) == len(set(names)) == 35, "expected exactly 35 unique frozen cases")
    require(sum(c["group"] == "OriginalSingleFile" for c in cases) == 31, "expected 31 OriginalSingleFile cases")
    require(sum(c["group"] == "DeliberateFutureMigration" for c in cases) == 4, "expected four migration cases")
    for c in cases:
        require(c["path"] == f"fixtures/{c['name']}.ox", "source argv changed")
    expected_file = read(PACKAGE / "expected-results.json")
    require(expected_file["complete_baseline_manifest_sha256"] == metadata["provenance"]["complete_baseline_manifest_sha256"], "baseline provenance mismatch")
    require(expected_file["source_intent_sha256"] == actual["suite-intent.json"], "intent provenance mismatch")
    expected = {}
    sources = {c["name"]: c["sha256"] for c in cases}
    for row in expected_file["canonical_results"]:
        key = (row["case"], row["operation"], row["format"])
        require(key not in expected, f"duplicate canonical result: {key}")
        require(row["source_sha256"] == sources.get(row["case"]), f"canonical source mismatch: {key}")
        require(type(row["exit_code"]) is int, f"invalid expected exit: {key}")
        streams = []
        for stream in ("stdout", "stderr"):
            data = base64.b64decode(row[stream + "_base64"], validate=True)
            require(hashlib.sha256(data).hexdigest() == row[stream + "_sha256"], f"canonical stream hash mismatch: {key}")
            streams.append(data)
        expected[key] = (row["exit_code"], *streams)
    require(set(expected) == set(itertools.product(names, ("check", "run"), ("json", "text"))), "canonical results are incomplete")
    require(len(expected) == 140, "expected exactly 140 canonical results")
    return metadata, cases, expected, actual


def forbidden_roots(binaries):
    roots = {PACKAGE}
    if PACKAGE.parent.name == "fixtures" and PACKAGE.parent.parent.name == "tests":
        roots.add(PACKAGE.parent.parent.parent)
    for path in (PACKAGE, *binaries.values()):
        for parent in (path, *path.parents):
            if (parent / ".git").exists():
                roots.add(parent)
                break
    return roots


def audit_observations(manifest, cases, expected, output, workspace, binaries, binary_hashes):
    """Fail closed on empty, partial, duplicate, altered or extra observations."""
    names = [c["name"] for c in cases]
    wanted = set(itertools.product(names, ("debug", "release"), ("check", "run"), ("json", "text"), (1, 2)))
    rows = manifest["rows"]
    key = lambda r: (r["case"], r["profile"], r["operation"], r["format"], r["repeat"])
    require(bool(wanted) and len(rows) == len(wanted), "zero or partial observation matrix")
    require({key(r) for r in rows} == wanted, "duplicate, missing or unexpected observation")
    require(manifest["cases"] == len(cases) and manifest["invocations"] == len(wanted), "incorrect observation counts")
    require(not manifest["semantic_failures"], "independent semantic expectation failed")
    require(manifest["runner_sha256"] == sha(PACKAGE / "cli_engine.py"), "CLI engine identity changed")
    require(manifest["source_intent_sha256"] == sha(PACKAGE / "suite-intent.json"), "source intent identity changed")
    case_map = {c["name"]: c for c in cases}
    artifacts, groups, records_count, labels_count = set(), {}, 0, 0
    for row in rows:
        c = case_map[row["case"]]
        require(row["source"] == c["path"] and row["source_sha256"] == c["sha256"], "observation source mismatch")
        require(row["group"] == c["group"] and row["route"] == c["route"], "observation classification changed")
        require(row["binary"] == str(binaries[row["profile"]]) and row["binary_sha256"] == binary_hashes[row["profile"]], "observation binary mismatch")
        argv = [str(binaries[row["profile"]]), row["operation"], "--edition", "typed-preview", "--message-format", row["format"], c["path"]]
        require(row["argv"] == argv and row["cwd"] == str(workspace), "relative argv or fixture cwd changed")
        require(row["argv_sha256"] == hashlib.sha256(json.dumps(argv, separators=(",", ":")).encode()).hexdigest(), "argv hash mismatch")
        require(not row["semantic_failures"], "per-invocation semantic failure")
        stem = f"{row['case']}-{row['profile']}-{row['operation']}-{row['format']}-r{row['repeat']}"
        streams = []
        for stream in ("stdout", "stderr"):
            name = f"{stem}.{stream}"
            require(row[stream + "_file"] == name and name not in artifacts, "duplicate or invalid stream artifact")
            artifacts.add(name)
            path = output / name
            require(path.is_file() and sha(path) == row[stream + "_sha256"], f"missing or altered stream: {name}")
            streams.append(path.read_bytes())
        canonical_key = (row["case"], row["operation"], row["format"])
        observed = (row["exit_code"], *streams)
        require(observed == expected[canonical_key], f"exact predecessor mismatch: {key(row)}")
        groups.setdefault(canonical_key, []).append(observed)
        if row["format"] == "json":
            records = [json.loads(line) for line in streams[0].splitlines()]
            require(records == row["records"] and bool(records), "recorded JSON differs from actual stream")
            require(records[-1]["kind"] == row["operation"] + "-summary", "missing terminal summary")
            require(all(r["kind"] == "diagnostic" for r in records[:-1]), "unexpected JSON record")
            contract = c["expected"][row["operation"]]
            require([[r["code"], r["stage"]] for r in records[:-1]] == contract.get("diagnostics", []), "ordered diagnostic contract mismatch")
            summary = records[-1]
            require(summary["success"] is (contract["exit"] == 0) and summary["errors"] == len(records) - 1, "summary result mismatch")
            field = "functions" if row["operation"] == "check" else "result"
            require(summary[field] == contract.get(field), "independent semantic result mismatch")
            records_count += len(records)
            labels_count += sum(bool(r["primary"]) + len(r["secondary"]) for r in records[:-1])
    actual_artifacts = {p.name for p in output.glob("*.stdout")} | {p.name for p in output.glob("*.stderr")}
    require(actual_artifacts == artifacts and len(artifacts) == len(wanted) * 2, "stream artifact count mismatch")
    require(len(groups) == len(cases) * 4 and all(len(g) == 4 and len(set(g)) == 1 for g in groups.values()), "repeat/profile equality failed")
    require(len(manifest["comparison"]) == len(wanted) and {key(r) for r in manifest["comparison"]} == wanted and all(r["equal"] is True for r in manifest["comparison"]), "incomplete comparison ledger")
    require(manifest["preservation_identical"] is True and manifest["repeated_and_profiles_identical"] is True, "CLI engine did not confirm preservation")
    return dict(status="PASS", cases=len(cases), invocations=len(rows), streams=len(artifacts),
                exact_comparisons=len(rows), repeat_profile_groups=len(groups), json_records=records_count,
                diagnostic_labels=labels_count)


def main():
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--debug", required=True, type=Path, help="existing debug CLI binary; never built here")
    ap.add_argument("--release", required=True, type=Path, help="existing release CLI binary; never built here")
    ap.add_argument("--evidence-root", type=Path, help="existing external parent for a fresh retained temporary directory; default system temp")
    ap.add_argument("--original-only", action="store_true", help="future activation only: exclude four deliberate migrations, run 31 cases/496 invocations")
    args = ap.parse_args()
    workspace, receipt = None, {"status": "FAIL"}
    try:
        metadata, all_cases, expected, package_before = load_package()
        binaries = {p: getattr(args, p).resolve(strict=True) for p in ("debug", "release")}
        for binary in binaries.values():
            require(binary.is_file() and os.access(binary, os.X_OK), f"CLI is not executable: {binary}")
        binary_hashes = {p: sha(b) for p, b in binaries.items()}
        parent = (args.evidence_root or Path(tempfile.gettempdir())).resolve(strict=True)
        require(parent.is_dir(), "evidence root must be an existing directory")
        for root in forbidden_roots(binaries):
            require(not parent.is_relative_to(root), f"evidence root is inside an input repository/package: {parent}")
        workspace = Path(tempfile.mkdtemp(prefix="oxid-unit1-compatibility-", dir=parent))
        print(f"Evidence retained at {workspace}", flush=True)
        (workspace / "fixtures").mkdir()
        for name in ("generate_fixtures.py", "cli_engine.py"):
            shutil.copyfile(PACKAGE / name, workspace / name)
        cases = [c for c in all_cases if not args.original_only or c["group"] == "OriginalSingleFile"]
        receipt.update(workspace=str(workspace), original_only=args.original_only,
                       planned_cases=len(cases), planned_invocations=len(cases) * 16,
                       package_hashes_before=package_before, binaries={p: str(b) for p, b in binaries.items()},
                       binary_hashes_before=binary_hashes, baseline_provenance=metadata["provenance"])
        write(workspace / "receipt.json", receipt)
        env = os.environ.copy()
        env.update(PYTHONDONTWRITEBYTECODE="1", LC_ALL="C", LANG="C", TZ="UTC")
        generator_argv = [sys.executable, "-B", str(workspace / "generate_fixtures.py")]
        with (workspace / "generator.stdout").open("wb") as out, (workspace / "generator.stderr").open("wb") as err:
            generated = subprocess.run(generator_argv, cwd=workspace, env=env, stdout=out, stderr=err, timeout=60)
        receipt.update(generator_argv=generator_argv, generator_exit=generated.returncode)
        require(generated.returncode == 0, "frozen fixture generator failed")
        require((workspace / "suite-intent.json").read_bytes() == (PACKAGE / "suite-intent.json").read_bytes(), "generator changed independent intent")
        source_hashes = {c["path"]: sha(workspace / c["path"]) for c in all_cases}
        require(all(source_hashes[c["path"]] == c["sha256"] for c in all_cases), "generated fixture bytes changed")
        receipt["fixture_hashes_before"] = source_hashes
        # Expand only reference metadata. Canonical stream bytes are stored once
        # in the package. These generated rows are expectations, not executions.
        compare_rows = []
        for (case, op, fmt), result in expected.items():
            for profile, repeat in itertools.product(("debug", "release"), (1, 2)):
                compare_rows.append(dict(case=case, operation=op, format=fmt, profile=profile, repeat=repeat,
                                         exit_code=result[0], stdout_sha256=hashlib.sha256(result[1]).hexdigest(),
                                         stderr_sha256=hashlib.sha256(result[2]).hexdigest()))
        write(workspace / "canonical-reference-matrix.json", {"scope": "Derived expected comparison keys; zero executed observations", "rows": compare_rows})
        argv = [sys.executable, "-B", str(workspace / "cli_engine.py"), "--debug", str(binaries["debug"]), "--release", str(binaries["release"]),
                "--label", "matrix", "--compare", str(workspace / "canonical-reference-matrix.json")]
        if args.original_only:
            argv.append("--original-only")
        receipt["engine_argv"] = argv
        write(workspace / "receipt.json", receipt)
        with (workspace / "engine.stdout").open("wb") as out, (workspace / "engine.stderr").open("wb") as err:
            executed = subprocess.run(argv, cwd=workspace, env=env, stdout=out, stderr=err)
        receipt["engine_exit"] = executed.returncode
        receipt["binary_hashes_after"] = {p: sha(b) for p, b in binaries.items()}
        receipt["fixture_hashes_after"] = {c["path"]: sha(workspace / c["path"]) for c in all_cases}
        receipt["package_hashes_after"] = inventory()
        require(receipt["binary_hashes_after"] == binary_hashes, "CLI changed during execution")
        require(receipt["fixture_hashes_after"] == source_hashes, "fixture changed during execution")
        require(receipt["package_hashes_after"] == package_before, "installed package changed during execution")
        require(not (PACKAGE / "__pycache__").exists(), "installed helper bytecode was created")
        require(executed.returncode == 0, "CLI matrix failed; inspect retained engine logs and partial artifacts")
        output = workspace / "evidence/matrix"
        verdict = audit_observations(read(output / "manifest.json"), cases, expected, output, workspace, binaries, binary_hashes)
        write(workspace / "audit.json", verdict)
        receipt.update(status="PASS", audit=verdict, matrix_manifest_sha256=sha(output / "manifest.json"))
    except (Exception, KeyboardInterrupt) as exc:
        receipt["error"] = f"{type(exc).__name__}: {exc}"
        print(receipt["error"], file=sys.stderr)
    finally:
        if workspace is not None:
            receipt["log_hashes"] = {p.name: sha(p) for p in workspace.glob("*.stdout")}
            receipt["log_hashes"].update({p.name: sha(p) for p in workspace.glob("*.stderr")})
            receipt["stream_artifacts"] = [dict(path=str(p.relative_to(workspace)), sha256=sha(p), bytes=p.stat().st_size)
                                           for pattern in ("*.stdout", "*.stderr")
                                           for p in sorted((workspace / "evidence/matrix").glob(pattern))]
            write(workspace / "receipt.json", receipt)
    print(json.dumps({k: receipt[k] for k in ("status", "workspace", "audit", "error") if k in receipt}, sort_keys=True))
    return 0 if receipt["status"] == "PASS" else 1


if __name__ == "__main__":
    raise SystemExit(main())
