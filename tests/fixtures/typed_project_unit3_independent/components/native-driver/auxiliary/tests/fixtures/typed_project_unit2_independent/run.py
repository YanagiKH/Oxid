#!/usr/bin/env python3
"""Run bounded Unit2 source-only observation and independent resource controls."""
import argparse
import datetime
import json
import os
from pathlib import Path
import subprocess
import sys
import uuid

from assemble import assemble, inventory
import protocol as p


OBSERVER = "frontend::oir::unit2_observer::observe_source_queue"
RESOURCE_FILTER = "independent_unit2_resource_review"


def encoded(value):
    return (json.dumps(value, indent=2, sort_keys=True) + "\n").encode()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--cargo", default="cargo")
    parser.add_argument("--rustc", default="rustc")
    parser.add_argument("--prepare-only", action="store_true")
    args = parser.parse_args()
    package = Path(__file__).resolve().parent
    repo, output = args.repo.resolve(), args.output.resolve()
    p.require(output != repo and repo not in output.parents, "output must be outside input repository")
    p.require(output != package and package not in output.parents, "output must be outside test package")
    p.require(not output.exists(), "output must be new")
    package_manifest = package / "package-inputs.json"
    package_manifest_bytes = package_manifest.read_bytes()
    package_entries = json.loads(package_manifest_bytes)["files"]
    source_manifest_bytes = (package / "source-inputs.json").read_bytes()
    source_manifest = json.loads(source_manifest_bytes)
    p.check_files(package, package_entries)
    p.require([entry["sha256"] for entry in package_entries if entry["path"] == "source-inputs.json"]
              == [p.digest(source_manifest_bytes)], "source manifest does not match captured package")
    output.mkdir(parents=True)
    invocation = uuid.uuid4().hex
    contract = {"schema": 1, "invocation_id": invocation,
                "source_inputs_sha256": p.digest(source_manifest_bytes),
                "package_inputs_sha256": p.digest(package_manifest_bytes),
                "started_at_utc": datetime.datetime.now(datetime.timezone.utc).isoformat()}
    (output / "invocation.json").write_bytes(encoded(contract))
    commands = []

    def command(label, argv, cwd, env):
        out, err = output / (label + ".stdout"), output / (label + ".stderr")
        p.require(not out.exists() and not err.exists(), "command output already exists")
        start = datetime.datetime.now(datetime.timezone.utc).isoformat()
        with out.open("wb") as stdout, err.open("wb") as stderr:
            result = subprocess.run(argv, cwd=cwd, env=env, stdout=stdout, stderr=stderr)
        record = {"label": label, "argv": argv, "cwd": str(cwd), "started_at_utc": start,
                  "finished_at_utc": datetime.datetime.now(datetime.timezone.utc).isoformat(),
                  "exit_status": result.returncode,
                  "stdout_sha256": p.digest(out.read_bytes()), "stderr_sha256": p.digest(err.read_bytes())}
        commands.append(record)
        (output / "commands.json").write_bytes(encoded(commands))
        p.require(result.returncode == 0, label + " failed; see retained output")
        return out.read_text()

    env = dict(os.environ, CARGO_BUILD_JOBS="2", CARGO_TARGET_DIR=str(output / "target"),
               RUSTC=args.rustc, PYTHONDONTWRITEBYTECODE="1")
    # The unchanged exported scripts use assertions for checked corpus invariants.
    # Child Python commands must retain them even when the caller uses -O.
    env.pop("PYTHONOPTIMIZE", None)
    for name in ("tmp", "resource-fixtures"):
        (output / name).mkdir()
    env.update(TMPDIR=str(output / "tmp"), OXID_RESOURCE_FIXTURES=str(output / "resource-fixtures"))
    source = output / "source"
    assembly = assemble(repo, source, package, source_manifest)
    (output / "assembly.json").write_bytes(encoded(assembly))
    assertion_mode = command("python-assertions", [sys.executable, "-B", "-c",
                             "import sys; print(sys.flags.optimize); assert __debug__"], output, env)
    p.require(assertion_mode == "0\n", "exported Python checks must run with assertions enabled")
    inputs = output / "observer-inputs"
    command("materialize", [sys.executable, "-B", str(package / "semantic/materialize.py"), str(inputs)], output, env)
    queue = json.loads((inputs / "queue.json").read_text())
    ids = [row["cohort"] + "/" + row["case"] for row in queue["cases"]]
    p.unique_ids(ids)
    corpus = json.loads((package / "semantic/corpus-manifest.json").read_text())
    p.require(len(ids) == corpus["cases"] == 3603, "wrong frozen case count")
    p.require(p.digest(("\n".join(ids) + "\n").encode()) == corpus["case_ids_sha256"], "wrong frozen case identities")
    def check_materialized():
        expected_paths = ["queue.json", "queue.tsv"]
        for row in queue["cases"]:
            request_file = p.relative_file(inputs, row["request"])
            p.require(p.digest(request_file.read_bytes()) == row["request_sha256"], "changed request")
            request = json.loads(request_file.read_text())
            p.require(row["source_files"] == request["source_files"], "source request mismatch")
            p.check_files(request_file.parent / "files", row["source_files"])
            expected_paths.append(row["request"])
            expected_paths.extend(str(Path(row["request"]).parent / "files" / entry["path"])
                                  for entry in row["source_files"])
        p.exact_ids([str(file.relative_to(inputs)) for file in inputs.rglob("*") if file.is_file()],
                    expected_paths)

    check_materialized()
    contract["queue_sha256"] = p.digest((inputs / "queue.json").read_bytes())
    contract["queue_tsv_sha256"] = p.digest((inputs / "queue.tsv").read_bytes())
    (output / "invocation.json").write_bytes(encoded(contract))
    if args.prepare_only:
        (output / "prepared.json").write_bytes(encoded({**contract, "status": "prepared", "compiler_executions": 0}))
        print(json.dumps({"status": "prepared", "cases": len(ids), "source_files": len(assembly["files"])}))
        return

    command("rustc-version", [args.rustc, "-vV"], source, env)
    names = json.loads((package / "resource-test-names.json").read_text())
    p.require(len(p.unique_ids(names)) == 21, "wrong resource-test contract")
    receipts = []
    for profile in ("debug", "release"):
        build = [args.cargo, "test", "--offline", "--locked", "--no-run", "--bin", "oxid",
                 "-j", "2", "--message-format=json"] + (["--release"] if profile == "release" else [])
        stdout = command(profile + "-build", build, source, env)
        events = [json.loads(line) for line in stdout.splitlines() if line.startswith("{")]
        p.require(any(row.get("reason") == "build-finished" and row.get("success") is True for row in events),
                  "missing Cargo terminal success")
        binaries = [row["executable"] for row in events if row.get("reason") == "compiler-artifact"
                    and row.get("executable") and row.get("profile", {}).get("test")]
        p.require(len(binaries) == 1, "expected exactly one test executable")
        binary = Path(binaries[0]).resolve()
        p.require((output / "target") in binary.parents, "test binary outside isolated target")
        binary_sha = p.digest(binary.read_bytes())
        binding = {**contract, "profile": profile, "binary_sha256": binary_sha}

        def run_test(label, filters):
            p.require(p.digest(binary.read_bytes()) == binary_sha, "changed test executable")
            return command(profile + "-" + label, [str(binary), *filters], source, env)

        p.rust_listing(run_test("observer-list", ["--list", "--exact", OBSERVER]), [OBSERVER])
        p.rust_listing(run_test("resource-list", ["--list", RESOURCE_FILTER]), names)
        raw = output / (profile + "-raw.jsonl")
        p.require(not raw.exists(), "raw observations already exist")
        env.update(OXID_OBSERVER_QUEUE=str(inputs / "queue.tsv"), OXID_OBSERVER_OUTPUT=str(raw))
        p.rust_success(run_test("observe", ["--exact", OBSERVER, "--test-threads=1"]), [OBSERVER])
        p.observations(raw, ids)
        p.rust_success(run_test("resource", [RESOURCE_FILTER, "--test-threads=1", "--show-output"]), names)
        normalized = output / (profile + "-normalized.jsonl")
        command(profile + "-normalize", [sys.executable, "-B", str(package / "semantic/normalize.py"),
                                         str(raw), str(inputs / "queue.json"), str(normalized)], output, env)
        p.observations(normalized, ids)
        compared = output / (profile + "-comparison.json")
        command(profile + "-compare", [sys.executable, "-B", str(package / "semantic/compare.py"),
                                       profile, str(normalized), str(compared)], output, env)
        p.comparison(json.loads(compared.read_text()), profile, ids)
        p.require(p.digest(binary.read_bytes()) == binary_sha, "changed test executable")
        expected_artifacts = [profile + "-" + label + extension
                              for label in ("build", "observer-list", "resource-list", "observe", "resource", "normalize", "compare")
                              for extension in (".stdout", ".stderr")]
        expected_artifacts += [profile + "-raw.jsonl", profile + "-normalized.jsonl", profile + "-comparison.json"]
        artifacts = []
        for name in sorted(expected_artifacts):
            file = p.relative_file(output, name)
            artifacts.append({"path": name, "bytes": file.stat().st_size, "sha256": p.digest(file.read_bytes())})
        receipt = {**binding, "schema": 1, "status": "passed", "exit_status": 0,
                   "binary": str(binary), "semantic_cases": len(ids), "resource_tests": len(names),
                   "artifacts": artifacts}
        p.run_receipt(receipt, binding, output, expected_artifacts)
        (output / (profile + "-receipt.json")).write_bytes(encoded(receipt))
        receipts.append(receipt)
        print(profile + ": 3603 unique semantic cases and 21 resource tests passed", flush=True)

    p.both_profiles(receipts)
    p.check_files(source, assembly["files"])
    p.exact_ids([entry["path"] for entry in inventory(source)], [entry["path"] for entry in assembly["files"]])
    p.check_files(repo, source_manifest["files"])
    p.check_files(package, package_entries)
    p.unchanged_file(package / "source-inputs.json", contract["source_inputs_sha256"])
    p.unchanged_file(package_manifest, contract["package_inputs_sha256"])
    p.require(p.digest((inputs / "queue.json").read_bytes()) == contract["queue_sha256"], "changed queue")
    p.require(p.digest((inputs / "queue.tsv").read_bytes()) == contract["queue_tsv_sha256"], "changed queue TSV")
    check_materialized()
    evidence = []
    for name in ("invocation.json", "assembly.json", "commands.json", "rustc-version.stdout",
                 "rustc-version.stderr", "materialize.stdout", "materialize.stderr",
                 "python-assertions.stdout", "python-assertions.stderr"):
        file = p.relative_file(output, name)
        evidence.append({"path": name, "bytes": file.stat().st_size, "sha256": p.digest(file.read_bytes())})
    (output / "result.json").write_bytes(encoded({**contract, "status": "passed", "profiles": ["debug", "release"],
                                                "semantic_cases_per_profile": 3603, "resource_tests_per_profile": 21,
                                                "evidence": evidence,
                                                "receipts": [{"profile": profile, "sha256": p.digest((output / (profile + "-receipt.json")).read_bytes())}
                                                             for profile in ("debug", "release")]}))


if __name__ == "__main__":
    main()
