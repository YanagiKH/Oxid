#!/usr/bin/env python3
"""Explicit current-input and archived-input adapters; no language oracle."""
import argparse
import datetime
import hashlib
import importlib.util
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
EXTRA = {
    'src/frontend/oir/owned/array_native_resource_tests.rs',
    'src/frontend/oir/owned/array_native_tests.rs',
    'src/frontend/oir/owned/array_observe.rs',
    'src/frontend/oir/owned/array_reference_boundary_tests.rs',
    'src/frontend/oir/owned/array_reference_tests.rs',
    'src/frontend/oir/owned/array_tests.rs',
    'src/frontend/oir/owned/reviewer_array_observer_tests.rs',
    'src/frontend/oir/owned/reviewer_array_reference_tests.rs',
    'src/frontend/oir/owned_types/array_tests.rs',
    'src/frontend/parser/activation_tests.rs',
    'tests/typed_frontend.rs',
    'tests/typed_project_dispatch.rs',
}
RESOURCE = "archive/resource/parser-resource-review-tests.rs"
OLD_SEAM = b"mode:SourceMode::ProjectCandidate,tokens,cursor:0"
NEW_SEAM = b"mode:SourceMode::ProjectCandidate,project_recovery:false,tokens,cursor:0"
CURRENT_SOURCE_SHA = '7c3de8673eca2bf2267251a9b3235a123bcefb1538785f3400a1fa0d073c5bb8'
PATCH_SHA = '63055a4b1a2cb63ce6a160a53e5c8131c4c288c198cd9af6ea421b5c2931fc18'
PATCH_BYTES = 605300
PATCH_PREFIX_BYTES = 28881
PATCH_PREFIX_SHA = "04f0588360aac12b96cd69a34b282329ea696eb69d7b979c8ffc385b7a42aab8"
SOURCE_DELTA_BASE = "0ef3be1df3643febdff1f859a4eb1ce567ab8164"
SOURCE_DELTA_RECIPE = "git diff --binary --no-ext-diff --no-renames --abbrev=7 BASE CHECKPOINT -- PATHS"
PATCH_PATHS = (
    'src/frontend/driver.rs',
    'src/frontend/oir/project.rs',
    'src/frontend/parser.rs',
    'src/frontend/parser/activation_tests.rs',
    'src/frontend/parser/project_tests.rs',
    'src/frontend/project.rs',
    'src/frontend/project/unit2_tests.rs',
    'tests/typed_frontend.rs',
    'tests/typed_project_dispatch.rs',
    'src/frontend/declaration_index.rs',
    'src/frontend/declaration_index/tests.rs',
    'src/frontend/diagnostic.rs',
    'src/frontend/oir/owned/array_native_resource_tests.rs',
    'src/frontend/oir/owned/array_native_tests.rs',
    'src/frontend/oir/owned/array_observe.rs',
    'src/frontend/oir/owned/array_reference_boundary_tests.rs',
    'src/frontend/oir/owned/array_reference_tests.rs',
    'src/frontend/oir/owned/array_tests.rs',
    'src/frontend/oir/owned/budget.rs',
    'src/frontend/oir/owned/cfg.rs',
    'src/frontend/oir/owned/consumer_fixtures.rs',
    'src/frontend/oir/owned/consumer_pilot.rs',
    'src/frontend/oir/owned/denial_tests.rs',
    'src/frontend/oir/owned/execute.rs',
    'src/frontend/oir/owned/execute_tests.rs',
    'src/frontend/oir/owned/flow.rs',
    'src/frontend/oir/owned/mod.rs',
    'src/frontend/oir/owned/native.rs',
    'src/frontend/oir/owned/native_heldout_review.rs',
    'src/frontend/oir/owned/native_tests.rs',
    'src/frontend/oir/owned/oracle_tests.rs',
    'src/frontend/oir/owned/origin_tests.rs',
    'src/frontend/oir/owned/plan.rs',
    'src/frontend/oir/owned/reviewer_array_observer_tests.rs',
    'src/frontend/oir/owned/reviewer_array_reference_tests.rs',
    'src/frontend/oir/owned/reviewer_heldout.rs',
    'src/frontend/oir/owned/reviewer_origins.rs',
    'src/frontend/oir/owned/reviewer_reference_tests.rs',
    'src/frontend/oir/owned/shape.rs',
    'src/frontend/oir/owned/source/association.rs',
    'src/frontend/oir/owned/source/budget.rs',
    'src/frontend/oir/owned/source/candidate_adapter.rs',
    'src/frontend/oir/owned/source/candidate_mutations.rs',
    'src/frontend/oir/owned/source/candidate_native.rs',
    'src/frontend/oir/owned/source/diagnostic.rs',
    'src/frontend/oir/owned/source/hir.rs',
    'src/frontend/oir/owned/source/lower.rs',
    'src/frontend/oir/owned/source/resolve.rs',
    'src/frontend/oir/owned/source/reviewer_heldout.rs',
    'src/frontend/oir/owned/source/reviewer_source.rs',
    'src/frontend/oir/owned/source/tests.rs',
    'src/frontend/oir/owned/source/typeck.rs',
    'src/frontend/oir/owned/tests.rs',
    'src/frontend/oir/owned/verified.rs',
    'src/frontend/oir/owned_types.rs',
    'src/frontend/oir/owned_types/array_tests.rs',
)

OBSERVER = 'semantic/observer.rs'
OBSERVER_ADAPTER_VERSION = 'unit2-record-aggregate-observer-v1'
OBSERVER_ORIGINAL_SHA = 'f2403aace53b6255a94b8b3ec0290db025638c5af94571d5729355672681fb00'
OBSERVER_DERIVED_SHA = 'ddeff8bd0acfaa9af0301c74f3aabd26ef69954eb5a216fdd9cc21e1837901a3'
OBSERVER_SEAMS = (
    (b'use oir::owned_types::{BorrowKind, FieldId, ParameterTy, ValueTy};', b'use oir::owned_types::{AggregateTy, BorrowKind, FieldId, ParameterTy, ValueTy};'),
    (b'fn value_type(value: ValueTy) -> String {', b'fn current_unit2_record_ordinal(aggregate: AggregateTy) -> usize {\n    match aggregate {\n        AggregateTy::Record(record) => record.0,\n        AggregateTy::FixedArray(_) => panic!("current Unit2 observer excludes fixed-array projection"),\n    }\n}\n\n#[test]\nfn current_unit2_aggregate_adapter_preserves_scalar_and_record_json() {\n    use oir::owned_types::RecordId;\n    assert_eq!(value_type(ValueTy::Scalar(hir::Ty::I32)), "{\\"kind\\":\\"scalar\\",\\"scalar\\":\\"I32\\"}");\n    assert_eq!(value_type(ValueTy::Owned(AggregateTy::Record(RecordId(7)))), "{\\"kind\\":\\"owned\\",\\"record\\":7}");\n    assert_eq!(parameter_type(ParameterTy::Reference { aggregate: AggregateTy::Record(RecordId(7)), kind: BorrowKind::Shared }), "{\\"kind\\":\\"shared\\",\\"record\\":7}");\n    assert_eq!(parameter_type(ParameterTy::Reference { aggregate: AggregateTy::Record(RecordId(7)), kind: BorrowKind::Exclusive }), "{\\"kind\\":\\"exclusive\\",\\"record\\":7}");\n}\n#[test]\n#[should_panic(expected = "current Unit2 observer excludes fixed-array projection")]\nfn current_unit2_aggregate_adapter_denies_owned_array() {\n    let array = oir::owned_types::FixedArrayTy::check(hir::Ty::I32, 1).unwrap();\n    value_type(ValueTy::Owned(AggregateTy::FixedArray(array)));\n}\n#[test]\n#[should_panic(expected = "current Unit2 observer excludes fixed-array projection")]\nfn current_unit2_aggregate_adapter_denies_shared_array() {\n    let array = oir::owned_types::FixedArrayTy::check(hir::Ty::I32, 0).unwrap();\n    parameter_type(ParameterTy::Reference { aggregate: AggregateTy::FixedArray(array), kind: BorrowKind::Shared });\n}\n#[test]\n#[should_panic(expected = "current Unit2 observer excludes fixed-array projection")]\nfn current_unit2_aggregate_adapter_denies_exclusive_array() {\n    let array = oir::owned_types::FixedArrayTy::check(hir::Ty::Bool, 1024).unwrap();\n    parameter_type(ParameterTy::Reference { aggregate: AggregateTy::FixedArray(array), kind: BorrowKind::Exclusive });\n}\n\nfn value_type(value: ValueTy) -> String {'),
    (b'ValueTy::Owned(record) => object(vec![("kind",q("owned")),("record",number(record.0))]),', b'ValueTy::Owned(aggregate) => object(vec![("kind",q("owned")),("record",number(current_unit2_record_ordinal(aggregate)))]),'),
    (b'ParameterTy::Reference { record, kind } => object(vec![("kind",q(match kind { BorrowKind::Shared=>"shared",BorrowKind::Exclusive=>"exclusive"})),("record",number(record.0))]),', b'ParameterTy::Reference { aggregate, kind } => object(vec![("kind",q(match kind { BorrowKind::Shared=>"shared",BorrowKind::Exclusive=>"exclusive"})),("record",number(current_unit2_record_ordinal(aggregate)))]),'),
)


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
    require(digest(patch) == PATCH_SHA and len(patch) == PATCH_BYTES, "wrong transition patch")
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
    require(tuple(touched) == PATCH_PATHS, "wrong transition scope")
    return result, touched


def adapt_unit2_observer(original):
    """Versioned current-only aggregate projection; frozen scalar/record JSON is retained."""
    require(digest(original) == OBSERVER_ORIGINAL_SHA, "wrong original Unit2 observer")
    result = original
    require(len(OBSERVER_SEAMS) == 4, "wrong Unit2 observer substitution count")
    for old, new in OBSERVER_SEAMS:
        require(result.count(old) == 1 and new not in result, "Unit2 observer seam drift")
        result = result.replace(old, new)
    require(digest(result) == OBSERVER_DERIVED_SHA, "wrong derived Unit2 observer")
    return result


def preflight(repo, package=PACKAGE):
    require(sys.flags.optimize == 0 and __debug__, "optimized Python is not supported")
    package_manifest = regular(package, "package-manifest.json").read_bytes()
    package_entries = json.loads(package_manifest)["files"]
    require(members(package) == sorted([x["path"] for x in package_entries] + ["package-manifest.json"]),
            "missing or extra adapter member")
    package_bytes = check_entries(package, package_entries)
    require(digest(package_bytes["current-source.json"]) == CURRENT_SOURCE_SHA,
            "unapproved current source manifest")
    authority = json.loads(package_bytes["authority.json"])
    require(authority["current_source_sha256"] == CURRENT_SOURCE_SHA, "stale current manifest authority")
    patch = package_bytes["source-transition.patch"]
    require(authority["transition_patch_sha256"] == PATCH_SHA
            and authority["transition_patch_bytes"] == PATCH_BYTES
            and authority["transition_touched_paths"] == list(PATCH_PATHS), "stale transition authority")
    require(authority["transition_patch_prefix"] == {"bytes": PATCH_PREFIX_BYTES, "sha256": PATCH_PREFIX_SHA}
            and digest(patch[:PATCH_PREFIX_BYTES]) == PATCH_PREFIX_SHA, "changed activation patch prefix")
    delta = authority["transition_source_delta"]
    require(delta["bytes"] == len(patch) - PATCH_PREFIX_BYTES
            and delta["sha256"] == digest(patch[PATCH_PREFIX_BYTES:])
            and delta["paths"] == list(PATCH_PATHS[9:])
            and delta["base_head"] == SOURCE_DELTA_BASE
            and delta["recipe"] == SOURCE_DELTA_RECIPE, "stale source delta authority")
    references = check_entries(repo, authority["repository_inputs"])
    historical = json.loads(references[U2 + "/package-inputs.json"])
    historical_bytes = check_entries(repo / U2, historical["files"])
    require(members(repo / U2) == sorted([x["path"] for x in historical["files"]] + ["package-inputs.json"]),
            "missing or extra historical Unit2 member")
    current = json.loads(package_bytes["current-source.json"])
    selected = json.loads(references[U3 + "/manifests/selected-current.json"])
    require(len(current["files"]) == 129 and len(selected["files"]) == 117, "wrong source count")
    require(delta["reviewed_source_head"] == current["reviewed_source_head"]
            and delta["source_only_tree"] == current["source_only_tree"], "stale source checkpoint provenance")
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
    adapted_observer = adapt_unit2_observer(historical_bytes[OBSERVER])
    require(authority["unit2_observer_adapter"] == {
        "version": OBSERVER_ADAPTER_VERSION,
        "original": entry(OBSERVER, historical_bytes[OBSERVER]),
        "derived": entry(OBSERVER, adapted_observer),
        "substitutions": [{"old_sha256": digest(old), "new_sha256": digest(new), "count": 1}
                          for old, new in OBSERVER_SEAMS],
        "scope": "Four exact substitutions in an isolated current Unit2 copy: AggregateTy import, fail-closed record projection and four compiled adapter controls, Owned projection, Reference aggregate projection. Scalar/record JSON and frozen expectations remain unchanged; FixedArray projection panics.",
    }, "stale Unit2 observer adapter authority")
    return {"current": current, "selected": selected, "historical": historical,
            "inputs": inputs, "archived": reconstructed, "references": references,
            "historical_bytes": historical_bytes, "resource": adapted_resource,
            "observer": adapted_observer,
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
    inputs[OBSERVER] = captured["observer"]
    require({name for name, data in inputs.items() if data != captured["historical_bytes"][name]}
            == {RESOURCE, OBSERVER}, "unexpected current Unit2 adapter member changes")
    manifest = {**captured["historical"], "files": [entry(name, data) for name, data in sorted(inputs.items())]}
    inputs["package-inputs.json"] = encoded(manifest)
    root = output / "compatibility" / "typed_project_unit2_independent"
    materialize(root, inputs)
    compat = output / "compatibility" / "typed_project_unit3_compatibility"
    materialize(compat, {"run.py": captured["references"][COMPAT]})
    return {"resource_package_root": str(root), "resource_package_inputs_sha256": digest(inputs["package-inputs.json"]),
            "resource_package_changes": [RESOURCE, OBSERVER, "package-inputs.json"],
            "observer_adapter": captured["authority"]["unit2_observer_adapter"],
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


OBSERVER_CONTROL_FILTER = "current_unit2_aggregate_adapter_"
OBSERVER_CONTROL_NAMES = tuple("frontend::oir::unit2_observer::" + OBSERVER_CONTROL_FILTER + name for name in (
    "preserves_scalar_and_record_json", "denies_owned_array", "denies_shared_array", "denies_exclusive_array"))


def verify_observer_control_output(protocol, listing, execution):
    protocol.rust_listing(listing, list(OBSERVER_CONTROL_NAMES))
    # Rust's human test format explicitly labels registered should_panic tests.
    expected = [name + (" - should panic" if "_denies_" in name else "") for name in OBSERVER_CONTROL_NAMES]
    return protocol.rust_success(execution, expected)


def run_unit2_observer_controls(repo, output, captured, seam):
    """Execute current-only adapter controls on the two already verified test binaries."""
    run = output / "unit2/run"
    controls = output / "observer-adapter-controls"
    require(not controls.exists(), "observer adapter controls output exists")
    controls.mkdir()
    protocol_path = regular(Path(seam["resource_package_root"]), "protocol.py")
    require(protocol_path.read_bytes() == captured["historical_bytes"]["protocol.py"], "observer control protocol changed")
    spec = importlib.util.spec_from_file_location("_current_unit2_control_protocol", protocol_path)
    protocol = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(protocol)
    receipts = []
    for profile in ("debug", "release"):
        assert_unchanged(repo, captured)
        source = run / "source"
        assembly = read_json(run / "assembly.json")
        check_entries(source, assembly["files"], exact=True)
        original_receipt = read_json(run / (profile + "-receipt.json"))
        binary = Path(original_receipt["binary"])
        require(binary.is_absolute() and (run / "target") in binary.parents and not binary.is_symlink(),
                "observer control binary outside isolated build")
        require(digest(binary.read_bytes()) == original_receipt["binary_sha256"], "observer control binary changed")
        commands, streams = [], {}
        for label, filters in (("list", ["--list", OBSERVER_CONTROL_FILTER]),
                               ("run", [OBSERVER_CONTROL_FILTER, "--test-threads=1", "--color", "never"])):
            argv = [str(binary), *filters]
            started = datetime.datetime.now(datetime.timezone.utc).isoformat()
            out, err = controls / (profile + "-" + label + ".stdout"), controls / (profile + "-" + label + ".stderr")
            env = dict(os.environ, PYTHONDONTWRITEBYTECODE="1")
            env.pop("PYTHONOPTIMIZE", None)
            status, timeout = None, False
            with out.open("wb") as stdout, err.open("wb") as stderr:
                try:
                    status = subprocess.run(argv, cwd=source, env=env, stdout=stdout, stderr=stderr, timeout=60).returncode
                except subprocess.TimeoutExpired:
                    timeout = True
            command = {"argv": argv, "cwd": str(source), "started_at_utc": started,
                       "finished_at_utc": datetime.datetime.now(datetime.timezone.utc).isoformat(),
                       "exit_status": status, "timed_out": timeout,
                       "stdout": entry(out.name, out.read_bytes()), "stderr": entry(err.name, err.read_bytes())}
            commands.append(command)
            write_json(controls / (profile + "-commands.json"), commands)
            require(status == 0 and not timeout, "current Unit2 observer controls failed; see retained streams")
            streams[label] = out.read_text(encoding="utf-8")
            require(digest(binary.read_bytes()) == original_receipt["binary_sha256"], "observer control binary changed")
        result = verify_observer_control_output(protocol, streams["list"], streams["run"])
        check_entries(source, assembly["files"], exact=True)
        assert_unchanged(repo, captured)
        record = {"schema": "oxid-current-unit2-observer-controls-v1", "profile": profile,
                  "status": "passed", "tests": list(OBSERVER_CONTROL_NAMES), "result": result,
                  "source_inputs_sha256": CURRENT_SOURCE_SHA, "observer_adapter": seam["observer_adapter"],
                  "original_unit2_receipt": entry(profile + "-receipt.json", (run / (profile + "-receipt.json")).read_bytes()),
                  "binary": entry(str(binary), binary.read_bytes()),
                  "assembly": entry("assembly.json", (run / "assembly.json").read_bytes()),
                  "commands": entry(profile + "-commands.json", (controls / (profile + "-commands.json")).read_bytes())}
        path = controls / (profile + "-receipt.json")
        write_json(path, record)
        receipts.append(entry(str(path.relative_to(output)), path.read_bytes()))
    return {"observer_adapter_version": OBSERVER_ADAPTER_VERSION,
            "observer_control_tests_per_profile": 4, "observer_control_receipts": receipts,
            "test_function_executions": 2 * (1 + 21 + 4)}


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
                "current_source_members": 129, "archive_members": 117,
                "unit2_semantic_cases_per_profile": 3603, "unit2_resource_tests_per_profile": 21,
                "unit2_current_observer_controls_per_profile": 4}
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
            verified = verify_unit2_result(output, captured, seam, args.prepare_only)
            if not args.prepare_only:
                verified.update(run_unit2_observer_controls(repo, output, captured, seam))
                verify_unit2_result(output, captured, seam, False)
            result.update(verified)
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
