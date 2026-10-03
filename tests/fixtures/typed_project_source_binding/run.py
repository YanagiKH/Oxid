#!/usr/bin/env python3
"""Explicit current-input and archived-input adapters; no language oracle."""
import argparse
import datetime
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import re
import stat
import subprocess
import sys
import uuid

sys.dont_write_bytecode = True
PACKAGE = Path(__file__).resolve().parent
U2 = "tests/fixtures/typed_project_unit2_independent"
U3 = "tests/fixtures/typed_project_unit3_independent"
COMPAT = "tests/fixtures/typed_project_unit3_compatibility/run.py"
EXTRA = {"src/frontend/parser/activation_tests.rs", "tests/typed_frontend.rs",
         "tests/typed_project_dispatch.rs"}
RESOURCE = "archive/resource/parser-resource-review-tests.rs"
OLD_SEAM = b"mode:SourceMode::ProjectCandidate,tokens,cursor:0"
NEW_SEAM = b"mode:SourceMode::ProjectCandidate,project_recovery:false,tokens,cursor:0"
PATCH_SHA = "04f0588360aac12b96cd69a34b282329ea696eb69d7b979c8ffc385b7a42aab8"


class BindingError(ValueError):
    pass


def require(condition, message):
    if not condition:
        raise BindingError(message)


def digest(data):
    return hashlib.sha256(data).hexdigest()


def encoded(value):
    return (json.dumps(value, indent=2, sort_keys=True) + "\n").encode()


def read_json(path):
    return json.loads(path.read_bytes())


def write_json(path, value):
    path.write_bytes(encoded(value))


def relative(name):
    require(isinstance(name, str) and name, "empty member")
    value = PurePosixPath(name)
    require(not value.is_absolute() and str(value) == name
            and all(x not in (".", "..") for x in value.parts), "noncanonical member")
    return value


def regular(root, name):
    path = Path(root)
    require(path.is_dir() and not path.is_symlink(), "invalid input root")
    for part in relative(name).parts:
        path = path / part
        require(not path.is_symlink(), "symlink input: " + name)
    require(path.is_file() and stat.S_ISREG(path.stat().st_mode), "missing regular input: " + name)
    return path


def entry(name, data):
    return {"path": name, "bytes": len(data), "sha256": digest(data)}


def members(root):
    result = []
    for parent, directories, files in os.walk(root, followlinks=False):
        for name in directories + files:
            path = Path(parent) / name
            require(not path.is_symlink(), "symlink member: " + str(path))
        result.extend((Path(parent) / name).relative_to(root).as_posix() for name in files)
    return sorted(result)


def check_entries(root, entries, exact=False):
    require(isinstance(entries, list) and entries, "zero input members")
    names = [x["path"] for x in entries]
    require(len(names) == len(set(names)), "duplicate member")
    if exact:
        require(members(root) == sorted(names), "missing or extra member")
    result = {}
    for item in entries:
        data = regular(root, item["path"]).read_bytes()
        require(entry(item["path"], data) == item, "changed input: " + item["path"])
        result[item["path"]] = data
    return result


def check_bytes(inputs, entries):
    require(set(inputs) == {x["path"] for x in entries}, "missing or extra reconstructed member")
    require(len(inputs) == len(entries), "duplicate reconstructed member")
    for item in entries:
        require(entry(item["path"], inputs[item["path"]]) == item,
                "changed reconstructed input: " + item["path"])


def inverse_patch(inputs, patch):
    """Apply the pinned git patch backwards with exact offsets and byte context."""
    require(digest(patch) == PATCH_SHA and len(patch) == 28881, "wrong transition patch")
    lines = patch.splitlines(keepends=True)
    at, touched = 0, []
    result = dict(inputs)
    while at < len(lines):
        match = re.fullmatch(rb"diff --git a/(\S+) b/(\S+)\n", lines[at])
        require(match is not None and match[1] == match[2], "invalid git patch header")
        name = match[1].decode("utf-8")
        relative(name)
        require(name in result and name not in touched, "missing or duplicate transition member")
        touched.append(name)
        at += 1
        added = lines[at] == b"new file mode 100644\n"
        if added:
            at += 1
        require(re.fullmatch(rb"index [0-9a-f]+\.\.[0-9a-f]+(?: 100644)?\n", lines[at]),
                "invalid git patch index")
        at += 1
        require(lines[at] == (b"--- /dev/null\n" if added else b"--- a/" + name.encode() + b"\n"),
                "invalid transition old path")
        require(lines[at + 1] == b"+++ b/" + name.encode() + b"\n", "invalid transition new path")
        at += 2
        before, after, cursor, hunks = result[name].splitlines(keepends=True), [], 0, 0
        while at < len(lines) and lines[at].startswith(b"@@ "):
            match = re.fullmatch(rb"@@ -(\d+)(?:,(\d+))? \+(\d+)(?:,(\d+))? @@[^\n]*\n", lines[at])
            require(match is not None, "invalid transition hunk")
            old_line, old_count, new_line, new_count = [int(v or b"1") for v in match.groups()]
            offset = max(new_line - 1, 0)
            require(cursor <= offset <= len(before), "transition offset out of bounds")
            after.extend(before[cursor:offset])
            cursor = offset
            require(len(after) == max(old_line - 1, 0), "transition old offset differs")
            at += 1
            removed = inserted = 0
            hunks += 1
            while at < len(lines) and not lines[at].startswith((b"@@ ", b"diff --git ")):
                line = lines[at]
                require(line[:1] in (b" ", b"+", b"-"), "unsupported transition record")
                if line[:1] in (b" ", b"+"):
                    require(cursor < len(before) and before[cursor] == line[1:],
                            "transition current context differs: " + name)
                    cursor += 1
                    removed += 1
                if line[:1] in (b" ", b"-"):
                    after.append(line[1:])
                    inserted += 1
                at += 1
            require(removed == new_count and inserted == old_count, "transition hunk cardinality differs")
        require(hunks > 0, "zero transition hunks")
        after.extend(before[cursor:])
        if added:
            require(not after, "inverse addition retained bytes")
            del result[name]
        else:
            result[name] = b"".join(after)
    require(len(touched) == 9, "wrong transition scope")
    return result, touched


def preflight(repo, package=PACKAGE):
    require(sys.flags.optimize == 0 and __debug__, "optimized Python is not supported")
    package_manifest = regular(package, "package-manifest.json").read_bytes()
    package_entries = json.loads(package_manifest)["files"]
    require(members(package) == sorted([x["path"] for x in package_entries] + ["package-manifest.json"]),
            "missing or extra adapter member")
    package_bytes = check_entries(package, package_entries)
    authority = json.loads(package_bytes["authority.json"])
    references = check_entries(repo, authority["repository_inputs"])
    historical = json.loads(references[U2 + "/package-inputs.json"])
    historical_bytes = check_entries(repo / U2, historical["files"])
    require(members(repo / U2) == sorted([x["path"] for x in historical["files"]] + ["package-inputs.json"]),
            "missing or extra historical Unit2 member")
    current = json.loads(package_bytes["current-source.json"])
    selected = json.loads(references[U3 + "/manifests/selected-current.json"])
    require(len(current["files"]) == 120 and len(selected["files"]) == 117, "wrong source count")
    require({x["path"] for x in current["files"]} == {x["path"] for x in selected["files"]} | EXTRA,
            "unexpected current source membership")
    require(digest(package_bytes["current-source.json"]) == authority["current_source_sha256"],
            "stale current manifest")
    inputs = check_entries(repo, current["files"])
    actual = [part + "/" + name for part in ("src", "native") for name in members(repo / part)]
    expected = [x for x in inputs if x.startswith(("src/", "native/"))]
    require(sorted(actual) == sorted(expected), "missing or extra compiler source member")
    reconstructed, touched = inverse_patch(inputs, package_bytes["source-transition.patch"])
    archived_extra = authority["inverse_only_inputs"]
    for item in archived_extra:
        require(entry(item["path"], reconstructed.pop(item["path"])) == item,
                "inverse integration-test identity differs")
    check_bytes(reconstructed, selected["files"])
    resource = historical_bytes[RESOURCE]
    require(resource.count(OLD_SEAM) == 1 and NEW_SEAM not in resource, "resource seam drift")
    adapted_resource = resource.replace(OLD_SEAM, NEW_SEAM)
    require(entry(RESOURCE, adapted_resource) == authority["derived_resource"], "derived resource drift")
    return {"current": current, "selected": selected, "historical": historical,
            "inputs": inputs, "archived": reconstructed, "references": references,
            "historical_bytes": historical_bytes, "resource": adapted_resource,
            "package_bytes": package_bytes, "package_manifest": package_manifest,
            "touched": touched, "authority": authority}


def materialize(root, inputs):
    require(not root.exists(), "materialization already exists")
    root.mkdir(parents=True)
    for name, data in sorted(inputs.items()):
        target = root / relative(name)
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_bytes(data)
    check_entries(root, [entry(name, data) for name, data in sorted(inputs.items())], exact=True)


def assert_unchanged(repo, captured, package=PACKAGE):
    fresh = preflight(repo, package)
    require(fresh == captured, "input identity changed during operation")


def prepare_archived(output, captured):
    materialize(output / "archived-selected", captured["archived"])
    return {"status": "prepared-archived-selected", "compiler_executions": 0,
            "semantic_pass": False, "direct_current_execution": False,
            "label": "Archived selected-current inputs reconstructed by pinned inverse source changes",
            "archived_root": str(output / "archived-selected"),
            "archived_files": captured["selected"]["files"],
            "archived_manifest_sha256": digest(captured["references"][U3 + "/manifests/selected-current.json"]),
            "inverse_patch_sha256": PATCH_SHA, "inverse_touched": captured["touched"]}


def prepare_unit2(output, captured):
    inputs = dict(captured["historical_bytes"])
    inputs[RESOURCE] = captured["resource"]
    manifest = {**captured["historical"], "files": [entry(name, data) for name, data in sorted(inputs.items())]}
    inputs["package-inputs.json"] = encoded(manifest)
    root = output / "compatibility" / "typed_project_unit2_independent"
    materialize(root, inputs)
    compat = output / "compatibility" / "typed_project_unit3_compatibility"
    materialize(compat, {"run.py": captured["references"][COMPAT]})
    return {"resource_package_root": str(root), "resource_package_inputs_sha256": digest(inputs["package-inputs.json"]),
            "resource_package_changes": [RESOURCE, "package-inputs.json"],
            "resource_before": next(x for x in captured["historical"]["files"] if x["path"] == RESOURCE),
            "resource_after": captured["authority"]["derived_resource"],
            "compatibility_runner": str(compat / "run.py"), "files": manifest["files"]}


def verify_unit2_result(output, captured, seam, prepare_only):
    compatibility = output / "unit2"
    run = compatibility / "run"
    artifact = "prepared.json" if prepare_only else "result.json"
    outer, child = read_json(compatibility / artifact), read_json(run / artifact)
    invocation = read_json(run / "invocation.json")
    expected_source = digest(captured["package_bytes"]["current-source.json"])
    final_manifest = {**captured["historical"], "files": [
        entry(item["path"], captured["package_bytes"]["current-source.json"])
        if item["path"] == "source-inputs.json" else item for item in seam["files"]]}
    final_manifest_bytes = encoded(final_manifest)
    package_sha = digest(final_manifest_bytes)
    final_package = compatibility / "derived-package"
    require(regular(final_package, "package-inputs.json").read_bytes() == final_manifest_bytes,
            "unexpected derived Unit2 package manifest")
    require(members(final_package) == sorted([x["path"] for x in final_manifest["files"]] + ["package-inputs.json"]),
            "missing or extra derived Unit2 package member")
    check_entries(final_package, final_manifest["files"])
    require(outer["historical_package_inputs_sha256"] == seam["resource_package_inputs_sha256"], "wrong seam package")
    require(outer["derived_package_inputs_sha256"] == package_sha, "wrong final Unit2 package")
    require(outer["child_result_sha256"] == digest((run / artifact).read_bytes()), "stale child result")
    require(outer["child_invocation_id"] == child["invocation_id"] == invocation["invocation_id"], "stale child invocation")
    for field, expected in (("source_inputs_sha256", expected_source), ("package_inputs_sha256", package_sha)):
        require(child[field] == invocation[field] == expected, "stale child binding: " + field)
    require(outer["source_inputs_sha256"] == expected_source, "wrong compatibility source")
    if prepare_only:
        require(outer["status"] == child["status"] == "prepared", "preparation claimed execution")
        require(outer["compiler_executions"] == child["compiler_executions"] == 0, "preparation executed compiler")
        require(not (run / "result.json").exists() and not (compatibility / "result.json").exists(),
                "preparation emitted pass")
    else:
        require(outer["status"] == child["status"] == "passed", "missing terminal Unit2 pass")
        require(child["profiles"] == ["debug", "release"] and child["semantic_cases_per_profile"] == 3603
                and child["resource_tests_per_profile"] == 21, "zero or partial Unit2 result")
        require([x["profile"] for x in child["receipts"]] == ["debug", "release"], "missing or duplicate profiles")
        for row in child["receipts"]:
            profile = row["profile"]
            receipt_file = run / (profile + "-receipt.json")
            require(row["sha256"] == digest(receipt_file.read_bytes()), "stale profile receipt")
            receipt = read_json(receipt_file)
            require(receipt["status"] == "passed" and receipt["exit_status"] == 0
                    and receipt["semantic_cases"] == 3603 and receipt["resource_tests"] == 21,
                    "zero or partial profile execution")
            for field in ("invocation_id", "source_inputs_sha256", "package_inputs_sha256", "queue_sha256", "queue_tsv_sha256"):
                require(receipt[field] == child[field], "stale profile binding: " + field)
            require(receipt["profile"] == profile, "wrong profile")
            binary = Path(receipt["binary"])
            require(binary.is_absolute() and (run / "target") in binary.parents
                    and not binary.is_symlink(), "test binary outside isolated build")
            require(digest(binary.read_bytes()) == receipt["binary_sha256"], "changed built binary")
            check_entries(run, receipt["artifacts"])
        check_entries(run, child["evidence"])
    check_entries(Path(seam["resource_package_root"]), seam["files"])
    require(digest((Path(seam["resource_package_root"]) / "package-inputs.json").read_bytes())
            == seam["resource_package_inputs_sha256"], "seam package changed")
    require((Path(seam["compatibility_runner"])).read_bytes() == captured["references"][COMPAT], "compatibility runner changed")
    return {"status": "prepared-current-unit2" if prepare_only else "passed-current-unit2",
            "semantic_pass": not prepare_only, "direct_current_execution": not prepare_only,
            "compiler_executions": 0 if prepare_only else "see retained Cargo build commands",
            "test_function_executions": 0 if prepare_only else 2 * (1 + 21),
            "semantic_cases_per_profile": 0 if prepare_only else 3603,
            "resource_tests_per_profile": 0 if prepare_only else 21,
            "child_result": str(run / artifact), "child_result_sha256": digest((run / artifact).read_bytes()),
            "compatibility_result_sha256": digest((compatibility / artifact).read_bytes()),
            "unit2_package_inputs_sha256": package_sha}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("action", choices=("preflight", "prepare-archived", "run-unit2"))
    parser.add_argument("--repo", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--cargo", default="cargo")
    parser.add_argument("--rustc", default="rustc")
    parser.add_argument("--prepare-only", action="store_true")
    args = parser.parse_args()
    repo = args.repo.resolve(strict=True)
    output = args.output.resolve()
    require(output != repo and repo not in output.parents and output != PACKAGE and PACKAGE not in output.parents,
            "output must be outside input repository and adapter")
    require(not output.exists(), "output must be fresh; previous evidence is retained")
    output.mkdir(parents=True)
    result = {"schema": "oxid-current-archive-binding-v1", "invocation_id": uuid.uuid4().hex,
              "action": args.action, "started_at_utc": datetime.datetime.now(datetime.timezone.utc).isoformat(),
              "status": "failed", "semantic_pass": False, "compiler_executions": 0,
              "controller_sha256": digest(Path(__file__).read_bytes())}
    try:
        captured = preflight(repo)
        result.update(current_source_sha256=digest(captured["package_bytes"]["current-source.json"]),
                      adapter_package_sha256=digest(captured["package_manifest"]),
                      authority_sha256=digest(captured["package_bytes"]["authority.json"]))
        plan = {**result, "status": "planned", "repository": str(repo),
                "current_source_members": 120, "archive_members": 117,
                "unit2_semantic_cases_per_profile": 3603, "unit2_resource_tests_per_profile": 21}
        write_json(output / "plan.json", plan)
        result["plan_sha256"] = digest((output / "plan.json").read_bytes())
        if args.action == "preflight":
            result.update(status="verified-inputs", semantic_pass=False)
        elif args.action == "prepare-archived":
            result.update(prepare_archived(output, captured))
        else:
            seam = prepare_unit2(output, captured)
            write_json(output / "resource-seam.json", seam)
            result["resource_seam_sha256"] = digest((output / "resource-seam.json").read_bytes())
            assert_unchanged(repo, captured)
            argv = [sys.executable, "-B", seam["compatibility_runner"], "--repo", str(repo),
                    "--source-manifest", str(PACKAGE / "current-source.json"), "--output", str(output / "unit2"),
                    "--cargo", args.cargo, "--rustc", args.rustc]
            if args.prepare_only:
                argv.append("--prepare-only")
            env = dict(os.environ, CARGO_INCREMENTAL="0", CARGO_BUILD_JOBS="2", PYTHONDONTWRITEBYTECODE="1")
            env.pop("PYTHONOPTIMIZE", None)
            result["compiler_executions"] = 0 if args.prepare_only else "unverified-see-child-commands"
            with (output / "runner.stdout").open("wb") as stdout, (output / "runner.stderr").open("wb") as stderr:
                completed = subprocess.run(argv, cwd=repo, env=env, stdout=stdout, stderr=stderr)
            command = {"argv": argv, "cwd": str(repo), "exit_status": completed.returncode,
                       "plan_sha256": result["plan_sha256"],
                       "stdout": entry("runner.stdout", (output / "runner.stdout").read_bytes()),
                       "stderr": entry("runner.stderr", (output / "runner.stderr").read_bytes())}
            write_json(output / "command.json", command)
            result["command_sha256"] = digest((output / "command.json").read_bytes())
            require(completed.returncode == 0, "retained Unit2 gate failed; see runner.stderr and unit2 evidence")
            result.update(verify_unit2_result(output, captured, seam, args.prepare_only))
        assert_unchanged(repo, captured)
        if args.action == "prepare-archived":
            check_entries(output / "archived-selected", captured["selected"]["files"], exact=True)
        require(result["plan_sha256"] == digest((output / "plan.json").read_bytes()), "execution plan changed")
    except (Exception, KeyboardInterrupt) as exc:
        result.update(status="failed", semantic_pass=False, error=f"{type(exc).__name__}: {exc}")
        print(result["error"], file=sys.stderr)
    finally:
        result["finished_at_utc"] = datetime.datetime.now(datetime.timezone.utc).isoformat()
        # Preparation and identity verification are never named result.json.
        artifact = "result.json" if result.get("semantic_pass") else "failure.json" if result["status"] == "failed" else "prepared.json"
        write_json(output / artifact, result)
    print(json.dumps({"status": result["status"], "output": str(output), "semantic_pass": result["semantic_pass"]}))
    return 1 if result["status"] == "failed" else 0


if __name__ == "__main__":
    raise SystemExit(main())
