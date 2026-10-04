#!/usr/bin/env python3
"""Current-source successor using immutable historical Unit4 helpers and semantics."""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import platform
import re
import shutil
import struct
import subprocess
import sys
import time
import types
import uuid

HERE = Path(__file__).resolve().parent
REPOSITORY = HERE.parents[2]
FROZEN = REPOSITORY / "tests/fixtures/typed_project_unit4_parser_portable/frozen/v3"
sys.dont_write_bytecode = True
HISTORICAL_AUTHORITY_SHA = "02b72b3dcf45c695e5c523d71bb1c83e15c082556cf36029829fefc7a71571b0"
CURRENT_PATHS = (
    'src/frontend/ast.rs',
    'src/frontend/declaration_index.rs',
    'src/frontend/declaration_index/source_owner.rs',
    'src/frontend/declaration_index/tests.rs',
    'src/frontend/diagnostic.rs',
    'src/frontend/driver.rs',
    'src/frontend/format.rs',
    'src/frontend/format/ast_tests.rs',
    'src/frontend/format/resource_tests.rs',
    'src/frontend/format_cli.rs',
    'src/frontend/hir.rs',
    'src/frontend/mod.rs',
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
    'src/frontend/oir/owned/source/array_consumer_tests.rs',
    'src/frontend/oir/owned/source/array_pipeline.rs',
    'src/frontend/oir/owned/source/array_pipeline_rows.rs',
    'src/frontend/oir/owned/source/array_pipeline_tests.rs',
    'src/frontend/oir/owned/source/array_pipeline_transport.rs',
    'src/frontend/oir/owned/source/array_type_controls.rs',
    'src/frontend/oir/owned/source/array_types_tests.rs',
    'src/frontend/oir/owned/source/association.rs',
    'src/frontend/oir/owned/source/budget.rs',
    'src/frontend/oir/owned/source/candidate_adapter.rs',
    'src/frontend/oir/owned/source/candidate_mutations.rs',
    'src/frontend/oir/owned/source/candidate_native.rs',
    'src/frontend/oir/owned/source/diagnostic.rs',
    'src/frontend/oir/owned/source/hir.rs',
    'src/frontend/oir/owned/source/lower.rs',
    'src/frontend/oir/owned/source/mod.rs',
    'src/frontend/oir/owned/source/program.rs',
    'src/frontend/oir/owned/source/resolve.rs',
    'src/frontend/oir/owned/source/reviewer_heldout.rs',
    'src/frontend/oir/owned/source/reviewer_source.rs',
    'src/frontend/oir/owned/source/tests.rs',
    'src/frontend/oir/owned/source/typeck.rs',
    'src/frontend/oir/owned/tests.rs',
    'src/frontend/oir/owned/verified.rs',
    'src/frontend/oir/owned_types.rs',
    'src/frontend/oir/owned_types/array_tests.rs',
    'src/frontend/options.rs',
    'src/frontend/parser.rs',
    'src/frontend/parser/array_syntax_tests.rs',
    'src/frontend/parser/arrays.rs',
    'src/frontend/parser/project_tests.rs',
    'src/frontend/project.rs',
    'src/frontend/project/array_syntax_tests.rs',
    'src/frontend/project/budget.rs',
    'src/frontend/source.rs',
    'src/main.rs',
    'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/empty-call-context-excluded/main.ox',
    'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/empty-no-context/main.ox',
    'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/empty-nonzero-annotation/main.ox',
    'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/empty-reassignment-context-excluded/main.ox',
    'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/empty-return-context-excluded/main.ox',
    'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/empty-scalar-context/main.ox',
    'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/grouped-complete-access-and-index/main.ox',
    'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/literal-first-heterogeneous-element/main.ox',
    'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/literal-length-max-trailing-comma/main.ox',
    'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/literal-nested-nonempty-is-nonscalar/main.ox',
    'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/nested-empty-does-not-inherit-context/main.ox',
    'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/read-array-bad-index/main.ox',
    'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/read-base-kind-before-index-kind/main.ox',
    'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/read-index-internal-type-first/main.ox',
    'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/read-index-literal-range-before-base-type/main.ox',
    'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/read-index-resolution-before-base-type/main.ox',
    'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/read-unknown-base-first/main.ox',
    'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/scalar-bool-length-0/main.ox',
    'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/scalar-bool-length-1/main.ox',
    'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/scalar-i32-length-0/main.ox',
    'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/scalar-i32-length-1/main.ox',
    'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/scalar-unit-length-0/main.ox',
    'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/scalar-unit-length-1/main.ox',
    'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/structural-identities-pairwise-distinct/main.ox',
    'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/utf8-read-primary/main.ox',
    'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/whole-program-resolution-before-earlier-function-type/main.ox',
    'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/write-base-kind-before-index-kind/main.ox',
    'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/write-both-subtree-errors-rhs-wins/main.ox',
    'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/write-element-before-mutability/main.ox',
    'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/write-index-internal-type-before-base/main.ox',
    'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/write-index-kind-before-rhs-element/main.ox',
    'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/write-one-conflict-element/main.ox',
    'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/write-one-conflict-index/main.ox',
    'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/write-owner-mutability-last/main.ox',
    'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/write-resolve-base-before-both-operands/main.ox',
    'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/write-resolve-rhs-before-index/main.ox',
    'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/write-rhs-internal-type-first/main.ox',
    'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/write-whole-resolution-before-rhs-typing/main.ox',
    'tests/fixtures/fixed_array_source_unit3/typing-contracts-v1/fixtures/guard-array-free/main.ox',
    'tests/fixtures/fixed_array_source_unit3/typing-contracts-v1/fixtures/guard-empty/main.ox',
    'tests/fixtures/fixed_array_source_unit3/typing-contracts-v1/fixtures/guard-record-only/main.ox',
    'tests/fixtures/fixed_array_source_unit3/typing-contracts-v1/fixtures/reference-access-modes/main.ox',
)
CURRENT_ADDED_PATHS = (
    'src/frontend/format.rs',
    'src/frontend/format/ast_tests.rs',
    'src/frontend/format/resource_tests.rs',
    'src/frontend/format_cli.rs',
    'src/frontend/oir/owned/array_native_resource_tests.rs',
    'src/frontend/oir/owned/array_native_tests.rs',
    'src/frontend/oir/owned/array_observe.rs',
    'src/frontend/oir/owned/array_reference_boundary_tests.rs',
    'src/frontend/oir/owned/array_reference_tests.rs',
    'src/frontend/oir/owned/array_tests.rs',
    'src/frontend/oir/owned/reviewer_array_observer_tests.rs',
    'src/frontend/oir/owned/reviewer_array_reference_tests.rs',
    'src/frontend/oir/owned/source/array_consumer_tests.rs',
    'src/frontend/oir/owned/source/array_pipeline.rs',
    'src/frontend/oir/owned/source/array_pipeline_rows.rs',
    'src/frontend/oir/owned/source/array_pipeline_tests.rs',
    'src/frontend/oir/owned/source/array_pipeline_transport.rs',
    'src/frontend/oir/owned/source/array_type_controls.rs',
    'src/frontend/oir/owned/source/array_types_tests.rs',
    'src/frontend/oir/owned_types/array_tests.rs',
    'src/frontend/parser/array_syntax_tests.rs',
    'src/frontend/parser/arrays.rs',
    'src/frontend/project/array_syntax_tests.rs',
    'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/empty-call-context-excluded/main.ox',
    'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/empty-no-context/main.ox',
    'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/empty-nonzero-annotation/main.ox',
    'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/empty-reassignment-context-excluded/main.ox',
    'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/empty-return-context-excluded/main.ox',
    'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/empty-scalar-context/main.ox',
    'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/grouped-complete-access-and-index/main.ox',
    'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/literal-first-heterogeneous-element/main.ox',
    'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/literal-length-max-trailing-comma/main.ox',
    'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/literal-nested-nonempty-is-nonscalar/main.ox',
    'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/nested-empty-does-not-inherit-context/main.ox',
    'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/read-array-bad-index/main.ox',
    'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/read-base-kind-before-index-kind/main.ox',
    'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/read-index-internal-type-first/main.ox',
    'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/read-index-literal-range-before-base-type/main.ox',
    'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/read-index-resolution-before-base-type/main.ox',
    'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/read-unknown-base-first/main.ox',
    'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/scalar-bool-length-0/main.ox',
    'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/scalar-bool-length-1/main.ox',
    'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/scalar-i32-length-0/main.ox',
    'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/scalar-i32-length-1/main.ox',
    'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/scalar-unit-length-0/main.ox',
    'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/scalar-unit-length-1/main.ox',
    'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/structural-identities-pairwise-distinct/main.ox',
    'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/utf8-read-primary/main.ox',
    'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/whole-program-resolution-before-earlier-function-type/main.ox',
    'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/write-base-kind-before-index-kind/main.ox',
    'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/write-both-subtree-errors-rhs-wins/main.ox',
    'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/write-element-before-mutability/main.ox',
    'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/write-index-internal-type-before-base/main.ox',
    'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/write-index-kind-before-rhs-element/main.ox',
    'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/write-one-conflict-element/main.ox',
    'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/write-one-conflict-index/main.ox',
    'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/write-owner-mutability-last/main.ox',
    'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/write-resolve-base-before-both-operands/main.ox',
    'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/write-resolve-rhs-before-index/main.ox',
    'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/write-rhs-internal-type-first/main.ox',
    'tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/write-whole-resolution-before-rhs-typing/main.ox',
    'tests/fixtures/fixed_array_source_unit3/typing-contracts-v1/fixtures/guard-array-free/main.ox',
    'tests/fixtures/fixed_array_source_unit3/typing-contracts-v1/fixtures/guard-empty/main.ox',
    'tests/fixtures/fixed_array_source_unit3/typing-contracts-v1/fixtures/guard-record-only/main.ox',
    'tests/fixtures/fixed_array_source_unit3/typing-contracts-v1/fixtures/reference-access-modes/main.ox',
)
ARRAY_INSTRUMENTATION_PATHS = ("src/frontend/ast.rs", "src/frontend/parser.rs", "src/frontend/project/budget.rs")
AUTHORITY_SHA = "fd049b7473ff96d3420f24f3924c5f30479241d7caf2cba26361982e8e9b0458"
COMPARATOR_SHA = "7c40e4782bee8082dc41534227348c26f952f3b870904cda9e71862b0be42a6b"
PREFIX_START = "    manifest = read_json(path)\n"
PREFIX_END = "    cases = {c[\"id\"]: c for c in contract[\"cases\"]}\n"


class Rejected(ValueError):
    pass


def require(condition, message):
    if not condition:
        raise Rejected(message)


def same(actual, expected, message):
    require(type(actual) is type(expected), message)
    if isinstance(expected, dict):
        require(actual.keys() == expected.keys(), message)
        for key in expected:
            same(actual[key], expected[key], message)
    elif isinstance(expected, list):
        require(len(actual) == len(expected), message)
        for left, right in zip(actual, expected):
            same(left, right, message)
    else:
        require(actual == expected, message)


def sha(raw):
    return hashlib.sha256(raw).hexdigest()


def load(raw):
    def pairs(items):
        result = {}
        for key, value in items:
            require(key not in result, "duplicate JSON key: " + key)
            result[key] = value
        return result
    return json.loads(raw, object_pairs_hook=pairs,
                      parse_constant=lambda x: (_ for _ in ()).throw(Rejected("nonfinite JSON")))


def read(path):
    return load(Path(path).read_bytes())


def write(path, value):
    Path(path).write_text(json.dumps(value, sort_keys=True, indent=2) + "\n")


def identity(path):
    path = Path(path).absolute()
    regular(path)
    raw = path.read_bytes()
    return {"path": str(path), "bytes": len(raw), "sha256": sha(raw)}


def regular(path):
    path = Path(path).absolute()
    require(not any(p.is_symlink() for p in [path, *path.parents]), "symlink ancestry: " + str(path))
    require(path.is_file(), "missing regular file: " + str(path))


def artifact(record, expected_path=None):
    require(set(record) == {"path", "bytes", "sha256"}, "artifact fields")
    path = Path(record["path"])
    require(path.is_absolute(), "artifact must have absolute path")
    if expected_path is not None:
        same(str(path), str(Path(expected_path).absolute()), "artifact at wrong bound path")
    same(identity(path), record, "artifact content differs: " + str(path))
    return path.read_bytes()


def safe_relative(name):
    require(isinstance(name, str) and name and "\\" not in name, "invalid relative path")
    path = PurePosixPath(name)
    require(not path.is_absolute() and str(path) == name and all(x not in ("", ".", "..") for x in path.parts),
            "unsafe relative path: " + name)
    return name


def verify_map(root, records, exact=False, extras=()):
    root = Path(root).absolute()
    names = [safe_relative(x["path"]) for x in records]
    require(len(set(names)) == len(names), "duplicate file map member")
    for entry in records:
        require(set(entry) == {"path", "bytes", "sha256"}, "relative identity fields")
        got = identity(root / entry["path"])
        same({k: got[k] for k in ("bytes", "sha256")},
             {k: entry[k] for k in ("bytes", "sha256")}, "file bytes differ: " + entry["path"])
    if exact:
        files = []
        for path in root.rglob("*"):
            require(not path.is_symlink(), "symlink in complete tree")
            require(path.is_file() or path.is_dir(), "special file in complete tree")
            if path.is_file():
                files.append(path.relative_to(root).as_posix())
        same(sorted(files), sorted(names + list(extras)), "complete file inventory differs")


def authority():
    raw = (HERE / "authority.json").read_bytes()
    same(sha(raw), AUTHORITY_SHA, "unreviewed current parser authority")
    active = load(raw)
    same(active["schema"], "oxid-unit4-current-parser-authority-v1", "current authority schema")
    same(active["historical_authority"]["sha256"], HISTORICAL_AUTHORITY_SHA, "historical authority pin")
    verify_map(REPOSITORY, [active["historical_authority"], active["historical_portable"], active["current_source_manifest"], active["formatter_transition_patch"], active["combined_transition_patch"], active["source_binding_runner"]])
    same(active["historical_authority"]["path"], "tests/fixtures/typed_project_unit4_parser_portable/frozen/v3/authority.json", "historical authority path")
    same(active["historical_portable"]["path"], "tests/fixtures/typed_project_unit4_parser_portable/frozen/v3/portable.py", "historical adapter path")
    same(active["current_source_manifest"]["path"], "tests/fixtures/typed_project_source_binding/current-source.json", "current source authority path")
    same(active["formatter_transition_patch"]["path"], "tests/fixtures/typed_project_source_binding/formatter-transition.patch", "formatter transition patch path")
    result = read(FROZEN / "authority.json")
    same(len(result["original_files"]), 283, "historical input count")
    same(len(result["derived_files"]), 286, "historical derived input count")
    same(len(result["control_derived_files"]), 286, "historical control input count")
    verify_map(FROZEN, result["package_files"])
    verify_map(FROZEN / "frozen/helpers", result["helper_files"], exact=True)
    current = read(REPOSITORY / active["current_source_manifest"]["path"])
    same(len(current["files"]), 185, "complete current source count")
    same(current["reviewed_source_head"], active["reviewed_source_head"], "reviewed source checkpoint")
    same(current["source_only_tree"], active["source_only_tree"], "reviewed source tree")
    before = {row["path"]: row for row in result["original_files"]}
    after = {row["path"]: row for row in current["files"]}
    same(len(before), 283, "duplicate historical member")
    same(len(after), 185, "duplicate current member")
    historical_compiler = {name for name in before if name.startswith(("src/", "native/"))
                           or name in ("Cargo.toml", "Cargo.lock", "build.rs")}
    require(historical_compiler <= after.keys(), "current transition deletes historical compiler input")
    changes = [{"path": name, "before": before.get(name), "after": row}
               for name, row in after.items() if before.get(name) != row]
    same([row["path"] for row in changes], list(CURRENT_PATHS), "unexpected current transition scope")
    same(changes, active["source_delta"], "current transition before/after identities")
    same(sorted(set(CURRENT_PATHS).intersection(row["path"] for row in result["instrumentation"])),
         [*ARRAY_INSTRUMENTATION_PATHS, "src/frontend/source.rs"], "transition overlaps instrumentation outside exact current composition")
    same(sorted(set(CURRENT_PATHS).intersection(row["path"] for row in result["control_instrumentation"])),
         ["src/frontend/ast.rs", "src/frontend/parser.rs"], "transition overlaps control instrumentation outside exact current composition")
    same([row["path"] for row in changes if row["before"] is None], list(CURRENT_ADDED_PATHS), "unexpected transition additions")
    merged = before | after
    base = [merged[name] for name in sorted(merged)]
    same(len(base), 348, "current base count")
    same(base, active["current_base_files"], "current base map must be derived from frozen inputs")
    result["current"] = active
    result["current_source"] = current
    candidate = (json.dumps(current_candidate(result), sort_keys=True, indent=2) + "\n").encode()
    same(sha(candidate), active["current_candidate_source_manifest_sha256"], "current candidate manifest identity")
    candidate_row = {"path": "candidate-source-manifest.json", "bytes": len(candidate), "sha256": sha(candidate)}
    for field in ("derived_files", "control_derived_files"):
        derived = {row["path"]: row for row in result[field]}
        derived.update({row["path"]: row["after"] for row in changes})
        control = field == "control_derived_files"
        for name in ARRAY_INSTRUMENTATION_PATHS:
            if control and name == "src/frontend/project/budget.rs":
                continue
            raw = compose_array_instrumentation(result, name, (REPOSITORY / name).read_bytes(), control)
            derived[name] = {"path": name, "bytes": len(raw), "sha256": sha(raw)}
        if not control:
            name = "src/frontend/source.rs"
            raw = compose_source_read(result, (REPOSITORY / name).read_bytes())
            derived[name] = {"path": name, "bytes": len(raw), "sha256": sha(raw)}
        name = "src/frontend/parser/unit4_observer.rs"
        raw = compose_observer_initializer(result)
        derived[name] = {"path": name, "bytes": len(raw), "sha256": sha(raw)}
        derived[candidate_row["path"]] = candidate_row
        ordered = [derived[name] for name in sorted(derived, key=lambda name: PurePosixPath(name).parts)]
        same(len(ordered), 351, "current derived count")
        same(ordered, active["current_" + field], "unapproved current derived map")
    return result


def compose_source_read(a, raw):
    """Compose only the pinned formatter accessor with unchanged historical hooks."""
    name = "src/frontend/source.rs"
    current = next(row for row in a["current"]["source_delta"] if row["path"] == name)
    same({"path": name, "bytes": len(raw), "sha256": sha(raw)}, current["after"], "composition current source identity")
    patch = a["current"]["formatter_transition_patch"]
    verify_map(REPOSITORY, [patch])
    parts = (REPOSITORY / patch["path"]).read_bytes().split(b"diff --git a/src/frontend/source.rs b/src/frontend/source.rs\n")
    same(len(parts), 2, "exact source-read formatter patch section")
    section = parts[1].split(b"\ndiff --git ", 1)[0]
    require(not any(line.startswith(b"-") and not line.startswith(b"---") for line in section.splitlines()),
            "source-read formatter patch must be insertion only")
    addition = b"".join(line[1:] for line in section.splitlines(keepends=True)
                       if line.startswith(b"+") and not line.startswith(b"+++"))
    same(len(addition.splitlines()), 7, "exact seven-line formatter accessor")
    same(raw.count(addition), 1, "exact formatter accessor occurrence")
    original = raw.replace(addition, b"", 1)
    historical = next(row for row in a["original_files"] if row["path"] == name)
    same(current["before"], historical, "composition historical source identity")
    same({"path": name, "bytes": len(original), "sha256": sha(original)}, historical,
         "formatter accessor must leave exact historical source")
    observer = b"crate::frontend::parser::unit4_observer"
    for before, after in (
        (b"    pub(super) fn try_text(&self, span: Span) -> Option<&str> {\n        if span.file",
         b"    pub(super) fn try_text(&self, span: Span) -> Option<&str> {\n        " + observer + b"::source_read(span.end.saturating_sub(span.start));\n        if span.file"),
        (b"    pub fn text(&self) -> &str {\n        &self.text",
         b"    pub fn text(&self) -> &str {\n        " + observer + b"::source_read(self.text.len());\n        &self.text"),
    ):
        same(raw.count(before), 1, "exact historical source-read hook location")
        raw = raw.replace(before, after, 1)
    historical_instrumented = next(row for row in a["derived_files"] if row["path"] == name)
    original_instrumented = raw.replace(addition, b"", 1)
    same({"path": name, "bytes": len(original_instrumented), "sha256": sha(original_instrumented)},
         historical_instrumented, "composition must preserve exact historical instrumentation")
    return raw


def compose_array_instrumentation(a, name, raw, control=False):
    """Apply the frozen hooks only at three explicitly bound array-overlap paths."""
    require(name in ARRAY_INSTRUMENTATION_PATHS, "unapproved array instrumentation path")
    require(not control or name != "src/frontend/project/budget.rs", "unapproved control composition")
    active = a["current"]
    current = next(row for row in active["source_delta"] if row["path"] == name)
    same({"path": name, "bytes": len(raw), "sha256": sha(raw)}, current["after"], "composition current array identity")
    runner = active["source_binding_runner"]
    transition = active["combined_transition_patch"]
    same(runner["path"], "tests/fixtures/typed_project_source_binding/run.py", "source binding runner path")
    same(transition["path"], "tests/fixtures/typed_project_source_binding/combined-transition.patch", "combined transition path")
    verify_map(REPOSITORY, [runner, transition])
    module = types.ModuleType("unit4_current_source_binding")
    module.__file__ = str(REPOSITORY / runner["path"])
    exec(compile((REPOSITORY / runner["path"]).read_bytes(), module.__file__, "exec"), module.__dict__)
    sections = (REPOSITORY / transition["path"]).read_bytes().split(b"diff --git ")
    prefix = ("a/" + name + " b/" + name + "\n").encode()
    selected = [b"diff --git " + part for part in sections[1:] if part.startswith(prefix)]
    same(len(selected), 1, "exact combined instrumentation source section")
    patch = selected[0]
    restored, touched = module.apply_inverse_patch({name: raw}, patch, sha(patch), len(patch), (name,))
    same(touched, [name], "exact instrumentation inverse scope")
    historical = next(row for row in a["original_files"] if row["path"] == name)
    same(current["before"], historical, "composition historical array source identity")
    original = restored[name]
    same({"path": name, "bytes": len(original), "sha256": sha(original)}, historical,
         "array transition must recover exact historical source")
    helper_path = FROZEN / "frozen/helpers/prepare.py"
    verify_map(helper_path.parent, [next(row for row in a["helper_files"] if row["path"] == "prepare.py")])
    helper = types.ModuleType("unit4_frozen_instrumentation")
    helper.__file__ = str(helper_path)
    exec(compile(helper_path.read_bytes(), str(helper_path), "exec"), helper.__dict__)
    def transform(body):
        text = body.decode()
        if name == "src/frontend/ast.rs":
            text = helper.replace(text, 'impl Program {', 'impl Program {\n    #[cfg(test)]\n    pub(super) fn unit4_source_handle(&self) -> (u64,usize,usize) { crate::frontend::parser::unit4_observer::provenance_handle(&self.source) }')
        elif name == "src/frontend/parser.rs":
            text = text + '\n#[cfg(test)]\npub(super) mod unit4_observer;\n' if control else helper.parser_overlay(text)
        else:
            text = helper.replace(text, '        #[cfg(test)]\n        self.trace.push(ReserveEvent {', '        #[cfg(test)]\n        ' + helper.J + '::reserve(kind, length, element_bytes, success);\n        #[cfg(test)]\n        self.trace.push(ReserveEvent {')
        return text.encode()
    historical_composed = transform(original)
    expected = next(row for row in a["control_derived_files" if control else "derived_files"] if row["path"] == name)
    same({"path": name, "bytes": len(historical_composed), "sha256": sha(historical_composed)}, expected,
         "array composition must preserve exact historical instrumentation")
    return transform(raw)


def compose_observer_initializer(a):
    """Keep the direct path-overflow probe on the original closed-array policy."""
    path = FROZEN / "frozen/helpers/observer.rs"
    original = path.read_bytes()
    row = next(row for row in a["helper_files"] if row["path"] == "observer.rs")
    same({"path": "observer.rs", "bytes": len(original), "sha256": sha(original)}, row, "historical observer identity")
    before = b"            mode,\n            project_recovery: false,"
    after = b"            mode,\n            arrays: ArraySyntaxPolicy::Closed,\n            project_recovery: false,"
    same(original.count(before), 1, "exact direct observer initializer")
    require(after not in original, "historical observer already adapted")
    result = original.replace(before, after, 1)
    same(result.replace(after, before, 1), original, "observer successor must preserve every historical byte")
    same(a["current"]["observer_initializer_adapter"], {
        "version": "unit4-direct-parser-closed-array-v1", "original": row,
        "derived": {"path": "src/frontend/parser/unit4_observer.rs", "bytes": len(result), "sha256": sha(result)},
        "old_seam_sha256": sha(before), "new_seam_sha256": sha(after), "substitutions": 1,
    }, "unapproved observer initializer successor")
    return result


def current_candidate(a):
    active = a["current"]
    return {"schema": "oxid-unit4-current-candidate-source-manifest-v1", "historical_commit": a["base_commit"],
            "reviewed_source_head": active["reviewed_source_head"], "source_only_tree": active["source_only_tree"],
            "current_source_manifest_sha256": active["current_source_manifest"]["sha256"], "files": active["current_base_files"]}

def fresh(path):
    path = Path(path).absolute()
    require(not path.exists() and not path.is_symlink(), "fresh destination required: " + str(path))
    require(path.parent.is_dir() and not any(x.is_symlink() for x in path.parents), "unsafe destination parent")
    path.mkdir()
    return path


def measured_host():
    u = platform.uname()
    machine = u.machine.lower()
    return {"os": {"Linux": "linux", "Darwin": "macos", "Windows": "windows"}.get(u.system, u.system.lower()),
            "architecture": {"amd64": "x86_64", "arm64": "aarch64"}.get(machine, machine),
            "python_pointer_width": struct.calcsize("P") * 8,
            "uname": {"system": u.system, "release": u.release, "version": u.version, "machine": u.machine},
            "python_executable": sys.executable, "python_version": sys.version}


def check_host(value, a):
    u = value["uname"]
    require(all(isinstance(u[k], str) and u[k] for k in ("system", "machine", "release", "version")), "measured uname required")
    same(u["system"], "Linux", "unsupported measured system")
    require(u["machine"].lower() in ("x86_64", "amd64"), "unsupported measured machine")
    same({"os": value["os"], "architecture": value["architecture"], "pointer_width": value["python_pointer_width"]},
         a["recipe"]["host"], "unsupported measured host")


def no_cargo_configs(source):
    for root in [Path(source), *Path(source).parents]:
        for name in ("config", "config.toml"):
            require(not (root / ".cargo" / name).exists(), "ancestor Cargo configuration forbidden")


def git(repo, *args):
    repo = Path(repo).absolute()
    require(repo.is_dir() and not any(p.is_symlink() for p in [repo, *repo.parents]), "intended Git directory must be real")
    env = {"PATH": "/usr/bin:/bin", "LANG": "C.UTF-8", "LC_ALL": "C.UTF-8",
           "HOME": "/nonexistent/oxid-parser-git-home", "GIT_CONFIG_NOSYSTEM": "1",
           "GIT_CONFIG_GLOBAL": "/dev/null", "GIT_NO_REPLACE_OBJECTS": "1", "GIT_TERMINAL_PROMPT": "0"}
    command = ["/usr/bin/git", "-c", "core.hooksPath=/dev/null", "-c", "core.fsmonitor=false", "-C", str(repo)]
    top = subprocess.check_output([*command, "rev-parse", "--show-toplevel"], env=env, timeout=30).decode().strip()
    same(Path(top), repo, "Git worktree differs from intended directory")
    return subprocess.check_output([*command, *args], env=env, timeout=30)


def compiler_map(a):
    return [x for x in a["current"]["current_base_files"] if x["path"].startswith(("src/", "native/"))
            or x["path"] in ("Cargo.toml", "Cargo.lock", "build.rs")]

def verify_checkout(repo, a):
    repo = Path(repo).absolute()
    wanted = compiler_map(a)
    same(len(wanted), 136, "current compiler body count")
    verify_map(repo, [a["current"]["current_source_manifest"]])
    verify_map(repo, a["current_source"]["files"])
    names = []
    for sub in ("src", "native"):
        for path in (repo / sub).rglob("*"):
            require(not path.is_symlink(), "checkout compiler symlink")
            if path.is_file():
                names.append(path.relative_to(repo).as_posix())
    names += ["Cargo.toml", "Cargo.lock", "build.rs"]
    same(sorted(names), [x["path"] for x in wanted], "checkout complete current compiler roster")
    head = git(repo, "rev-parse", "HEAD").decode().strip()
    tree = git(repo, "rev-parse", "HEAD^{tree}").decode().strip()
    for row in [*a["current_source"]["files"], a["current"]["current_source_manifest"]]:
        same(sha(git(repo, "show", head + ":" + row["path"])), row["sha256"], "checkout Git current input bytes")
    return {"path": str(repo), "head": head, "tree": tree, "compiler_files": wanted,
            "historical_source_equivalent": False, "current_source_bound": True,
            "current_source_manifest_sha256": a["current"]["current_source_manifest"]["sha256"],
            "reviewed_source_head": a["current"]["reviewed_source_head"], "source_only_tree": a["current"]["source_only_tree"]}

def minimal_env(home, cargo_home=None, toolchain=None, executable_view=None):
    result = {"HOME": str(home), "PATH": "/usr/bin:/bin", "LANG": "C.UTF-8", "LC_ALL": "C.UTF-8",
              "PYTHONDONTWRITEBYTECODE": "1", "PYTHONNOUSERSITE": "1"}
    if toolchain is not None:
        require(executable_view is not None, "verified narrow executable view required")
        verify_executable_view(executable_view, toolchain)
        result["PATH"] = str(Path(executable_view)) + ":/usr/bin:/bin"
        result["CARGO_HOME"] = str(cargo_home)
        for key, name in (("CC", "cc"), ("CXX", "c++"), ("AR", "ar"), ("CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER", "cc")):
            found = shutil.which(name, path="/usr/bin:/bin")
            require(found is not None, "required native tool unavailable: " + name)
            result[key] = str(Path(found).resolve())
    return result


def process_tools(env):
    result = {"python": {"binary": identity(Path(sys.executable).resolve()), "version": sys.version}}
    for name in ("which", "as"):
        found = shutil.which(name, path=env["PATH"])
        require(found is not None, "required host tool unavailable: " + name)
        require(Path(found).parent in (Path("/usr/bin"), Path("/bin")), "host tool shadowed by compiler view")
        result[name] = {"binary": identity(Path(found).resolve())}
    for key in ("CC", "CXX", "AR", "CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER"):
        if key in env:
            path = Path(env[key])
            version = subprocess.check_output([str(path), "--version"], env=env, text=True, timeout=15)
            result[key] = {"binary": identity(path), "version": version}
    if "CC" in env:
        for key, driver, option in (("c_compiler_body", "CC", "cc1"), ("cpp_compiler_body", "CXX", "cc1plus"), ("native_linker", "CC", "ld")):
            found = subprocess.check_output([env[driver], "-print-prog-name=" + option], env=env, text=True, timeout=15).strip()
            path = Path(found)
            if not path.is_absolute():
                found = shutil.which(found, path=env["PATH"])
                require(found is not None, "native compiler body unavailable")
                path = Path(found)
            result[key] = {"binary": identity(path.resolve())}
    return result


def verify_process_tools(receipt):
    same(receipt["tools"], process_tools(receipt["environment"]), "actual process Python/native tool identities")


def invoke(argv, cwd, env, out, host):
    actual_tools = process_tools(env)
    start = time.time_ns()
    with (out / "driver.stdout").open("wb") as stdout, (out / "driver.stderr").open("wb") as stderr:
        result = subprocess.run(argv, cwd=cwd, env=env, stdout=stdout, stderr=stderr)
    end = time.time_ns()
    same(process_tools(env), actual_tools, "process tools changed during invocation")
    receipt = {"argv": argv, "cwd": str(cwd), "environment": env, "started_ns": start, "finished_ns": end,
               "exit_code": result.returncode, "stdout": identity(out / "driver.stdout"),
               "stderr": identity(out / "driver.stderr"), "host": host, "tools": actual_tools}
    write(out / "invocation.json", receipt)
    require(result.returncode == 0, "helper process failed; inspect " + str(out / "driver.stderr"))
    return receipt


def prepare(repo, checkout, output):
    a = authority()
    host = measured_host(); check_host(host, a)
    repo, checkout = Path(repo).absolute(), Path(checkout).absolute()
    same(git(repo, "rev-parse", "HEAD").decode().strip(), a["base_commit"], "historical checkout HEAD")
    verify_map(repo, a["original_files"])
    for row in a["original_files"]:
        same(sha(git(repo, "show", a["base_commit"] + ":" + row["path"])), row["sha256"], "historical Git input")
    current = verify_checkout(checkout, a)
    out = fresh(output)
    helpers = out / "helpers"; helpers.mkdir()
    for row in a["helper_files"]:
        shutil.copyfile(FROZEN / "frozen/helpers" / row["path"], helpers / row["path"])
    home = out / "home"; home.mkdir()
    no_cargo_configs(out)
    transitions = []
    for control in (False, True):
        invocation = out / ("prepare-control" if control else "prepare"); invocation.mkdir()
        source = out / ("control-source" if control else "source")
        argv = [sys.executable, "-B", str(helpers / "prepare.py"), "--repo", str(repo), "--output", str(source)]
        if control:
            argv.append("--control")
        invoke(argv, out, minimal_env(home), invocation, host)
        verify_historical_overlay(out, a, control)
        historical = {}
        for key, filename in (("candidate", "candidate-source-manifest.json"), ("overlay", "overlay-manifest.json")):
            target = invocation / ("historical-" + filename)
            shutil.copyfile(source / filename, target)
            historical[key] = identity(target)
        for change in a["current"]["source_delta"]:
            target = source / change["path"]
            name = change["path"]
            overlap = name in ARRAY_INSTRUMENTATION_PATHS and (not control or name != "src/frontend/project/budget.rs")
            if overlap or (not control and name == "src/frontend/source.rs"):
                verify_map(source, [next(row for row in a["control_derived_files" if control else "derived_files"] if row["path"] == name)])
                raw = (checkout / name).read_bytes()
                target.write_bytes(compose_array_instrumentation(a, name, raw, control) if overlap else compose_source_read(a, raw))
                continue
            if change["before"] is None:
                require(not target.exists(), "new current source already exists")
            else:
                verify_map(source, [change["before"]])
            verify_map(checkout, [change["after"]])
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(checkout / change["path"], target)
        observer = source / "src/frontend/parser/unit4_observer.rs"
        verify_map(source, [next(row for row in a["derived_files"] if row["path"] == "src/frontend/parser/unit4_observer.rs")])
        observer.write_bytes(compose_observer_initializer(a))
        write(source / "candidate-source-manifest.json", current_candidate(a))
        write(source / "overlay-manifest.json", current_overlay(source, a, control))
        verify_overlay(out, a, control)
        transitions.append({"control": control, "historical_candidate": historical["candidate"],
                            "historical_overlay": historical["overlay"], "current_candidate": identity(source / "candidate-source-manifest.json"),
                            "current_overlay": identity(source / "overlay-manifest.json"), "changes": a["current"]["source_delta"]})
    for source, target in ((HERE / "authority.json", "current-authority.json"), (FROZEN / "authority.json", "historical-authority.json"),
                           (checkout / a["current"]["current_source_manifest"]["path"], "current-source-manifest.json")):
        shutil.copyfile(source, out / target)
    same(current, verify_checkout(checkout, a), "current inputs changed during preparation")
    session = {"schema": "oxid-unit4-current-parser-session-v1", "authority_sha256": AUTHORITY_SHA,
               "run_id": uuid.uuid4().hex, "root": str(out), "prepared_ns": time.time_ns(),
               "historical_repo": str(repo), "checkout": current, "host": host,
               "prepare_invocation": identity(out / "prepare/invocation.json"),
               "control_prepare_invocation": identity(out / "prepare-control/invocation.json"),
               "overlay": identity(out / "source/overlay-manifest.json"),
               "control_overlay": identity(out / "control-source/overlay-manifest.json"),
               "adapter": identity(Path(__file__)), "authority": identity(HERE / "authority.json"),
               "transition_authority": identity(out / "current-authority.json"), "historical_authority": identity(out / "historical-authority.json"),
               "current_source_manifest": identity(out / "current-source-manifest.json"), "transitions": transitions}
    verify_transition_records(session, a)
    write(out / "session.json", session)
    return identity(out / "session.json")


def current_overlay(source, a, control):
    active = a["current"]
    instrumentation = [dict(row) for row in a["control_instrumentation" if control else "instrumentation"]]
    for row in instrumentation:
        if row["path"] in (*ARRAY_INSTRUMENTATION_PATHS, "src/frontend/source.rs"):
            row["before_sha256"] = next(item["sha256"] for item in active["current_base_files"] if item["path"] == row["path"])
            row["after_sha256"] = next(item["sha256"] for item in active["current_control_derived_files" if control else "current_derived_files"] if item["path"] == row["path"])
    return {"schema": "oxid-unit4-current-observer-overlay-v1", "historical_base_commit": a["base_commit"], "control": control,
            "source": str(source), "reviewed_source_head": active["reviewed_source_head"], "source_only_tree": active["source_only_tree"],
            "current_source_manifest_sha256": active["current_source_manifest"]["sha256"], "transition_authority_sha256": AUTHORITY_SHA,
            "candidate_source_manifest_sha256": active["current_candidate_source_manifest_sha256"],
            "observer_source_sha256": a["helper_manifest_sha256"], "observer_files": a["helper_files"],
            "instrumentation": instrumentation,
            "files": active["current_control_derived_files" if control else "current_derived_files"]}


def verify_overlay(root, a, control=False):
    source = Path(root) / ("control-source" if control else "source")
    expected = current_overlay(source, a, control)
    verify_map(source, expected["files"], exact=True, extras=("observer-source-manifest.json", "overlay-manifest.json"))
    same(read(source / "overlay-manifest.json"), expected, "current overlay must match reviewed transition exactly")
    same(read(source / "candidate-source-manifest.json"), current_candidate(a), "current ordered348 base map")
    same(sha((source / "observer-source-manifest.json").read_bytes()), a["helper_manifest_sha256"], "unchanged helper manifest bytes")
    verify_map(Path(root) / "helpers", a["helper_files"], exact=True)
    no_cargo_configs(source)
    return expected


def verify_transition_records(session, a, resolve=artifact):
    """Shared strict metadata predicate; source bodies are separately verified live."""
    root = Path(session["root"])
    same(session["schema"], "oxid-unit4-current-parser-session-v1", "current session schema")
    same(session["authority_sha256"], AUTHORITY_SHA, "current transition authority")
    for key, filename, expected_sha in (("transition_authority", "current-authority.json", AUTHORITY_SHA),
            ("historical_authority", "historical-authority.json", HISTORICAL_AUTHORITY_SHA),
            ("current_source_manifest", "current-source-manifest.json", a["current"]["current_source_manifest"]["sha256"])):
        same(session[key]["path"], str(root / filename), "transition artifact path")
        same(sha(resolve(session[key])), expected_sha, "transition artifact identity")
    same(load(resolve(session["current_source_manifest"])), a["current_source"], "complete current manifest copy")
    same(len(session["transitions"]), 2, "both source transitions required")
    for transition, control in zip(session["transitions"], (False, True)):
        same(set(transition), {"control", "historical_candidate", "historical_overlay", "current_candidate", "current_overlay", "changes"}, "transition fields")
        same(transition["control"], control, "transition role/order")
        same(transition["changes"], a["current"]["source_delta"], "transition exact 117 changes")
        source = root / ("control-source" if control else "source")
        invocation = root / ("prepare-control" if control else "prepare")
        for key, filename in (("historical_candidate", "historical-candidate-source-manifest.json"), ("historical_overlay", "historical-overlay-manifest.json")):
            same(transition[key]["path"], str(invocation / filename), "historical metadata snapshot path")
        raw = resolve(transition["historical_candidate"])
        same(sha(raw), a["candidate_source_manifest_sha256"], "original historical candidate manifest retained")
        same(load(raw), {"schema": "oxid-unit4-candidate-source-manifest-v1", "commit": a["base_commit"], "files": a["original_files"]}, "original ordered283 manifest")
        expected_old = {"schema": "oxid-unit4-observer-overlay-v1", "base_commit": a["base_commit"], "control": control,
                "source": str(source), "candidate_source_manifest_sha256": a["candidate_source_manifest_sha256"],
                "observer_source_sha256": a["helper_manifest_sha256"], "observer_files": a["helper_files"],
                "instrumentation": a["control_instrumentation" if control else "instrumentation"],
                "files": a["control_derived_files" if control else "derived_files"]}
        same(load(resolve(transition["historical_overlay"])), expected_old, "exact historical overlay snapshot")
        same(transition["current_overlay"], session["control_overlay" if control else "overlay"], "current overlay transition/session binding")
        same(load(resolve(transition["current_overlay"])), current_overlay(source, a, control), "exact current overlay snapshot")
        candidate = next(row for row in a["current"]["current_control_derived_files" if control else "current_derived_files"] if row["path"] == "candidate-source-manifest.json")
        same(transition["current_candidate"], {**candidate, "path": str(source / candidate["path"])}, "current candidate transition binding")

def verify_historical_overlay(root, a, control=False):
    source = Path(root) / ("control-source" if control else "source")
    files = a["control_derived_files"] if control else a["derived_files"]
    instrumentation = a["control_instrumentation"] if control else a["instrumentation"]
    verify_map(source, files, exact=True, extras=("observer-source-manifest.json", "overlay-manifest.json"))
    overlay = read(source / "overlay-manifest.json")
    expected = {"schema": "oxid-unit4-observer-overlay-v1", "base_commit": a["base_commit"], "control": control,
                "source": str(source), "candidate_source_manifest_sha256": a["candidate_source_manifest_sha256"],
                "observer_source_sha256": a["helper_manifest_sha256"], "observer_files": a["helper_files"],
                "instrumentation": instrumentation, "files": files}
    same(overlay, expected, "derived overlay must match reviewed recipe exactly")
    same(read(source / "candidate-source-manifest.json"), {"schema": "oxid-unit4-candidate-source-manifest-v1",
          "commit": a["base_commit"], "files": a["original_files"]}, "original ordered283 map")
    helper_raw = (source / "observer-source-manifest.json").read_bytes()
    same(sha(helper_raw), a["helper_manifest_sha256"], "helper manifest bytes")
    verify_map(Path(root) / "helpers", a["helper_files"], exact=True)
    no_cargo_configs(source)
    return overlay


def session_at(path, a):
    path = Path(path).absolute()
    regular(path)
    session = read(path)
    same(session["schema"], "oxid-unit4-current-parser-session-v1", "current session schema")
    verify_transition_records(session, a)
    same(session["authority_sha256"], AUTHORITY_SHA, "session authority")
    require(re.fullmatch("[0-9a-f]{32}", session["run_id"]) is not None, "session nonce")
    root = Path(session["root"])
    same(path, root / "session.json", "session bound root")
    artifact(session["adapter"], Path(__file__))
    artifact(session["authority"], HERE / "authority.json")
    artifact(session["overlay"], root / "source/overlay-manifest.json")
    artifact(session["control_overlay"], root / "control-source/overlay-manifest.json")
    prep = load(artifact(session["prepare_invocation"], root / "prepare/invocation.json"))
    same(prep["argv"], [sys.executable, "-B", str(root / "helpers/prepare.py"), "--repo", session["historical_repo"],
                       "--output", str(root / "source")], "prepare actual argv")
    same(prep["environment"], minimal_env(root / "home"), "prepare clean environment")
    same(prep["cwd"], str(root), "prepare cwd")
    same(prep["exit_code"], 0, "prepare exit")
    same(prep["host"], session["host"], "preparation/session host snapshot")
    verify_process_tools(prep)
    require(prep["started_ns"] <= prep["finished_ns"] <= session["prepared_ns"], "prepare/session ordering")
    artifact(prep["stdout"], root / "prepare/driver.stdout"); artifact(prep["stderr"], root / "prepare/driver.stderr")
    control = load(artifact(session["control_prepare_invocation"], root / "prepare-control/invocation.json"))
    same(control["argv"], [sys.executable, "-B", str(root / "helpers/prepare.py"), "--repo", session["historical_repo"],
                          "--output", str(root / "control-source"), "--control"], "control prepare actual argv")
    same(control["environment"], minimal_env(root / "home"), "control prepare clean environment")
    same(control["cwd"], str(root), "control prepare cwd")
    same(control["exit_code"], 0, "control prepare exit")
    same(control["host"], session["host"], "control preparation/session host snapshot")
    verify_process_tools(control)
    require(prep["finished_ns"] <= control["started_ns"] <= control["finished_ns"] <= session["prepared_ns"], "control preparation ordering")
    artifact(control["stdout"], root / "prepare-control/driver.stdout"); artifact(control["stderr"], root / "prepare-control/driver.stderr")
    same(session["checkout"], verify_checkout(session["checkout"]["path"], a), "current compiler source binding")
    check_host(session["host"], a)
    verify_overlay(root, a)
    verify_overlay(root, a, control=True)
    return session, root


def cargo_cache(source, dest, a):
    source, dest = Path(source).absolute(), Path(dest).absolute()
    verify_map(source, a["dependency_files"])
    dest.mkdir()
    for row in a["dependency_files"]:
        target = dest / row["path"]; target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(source / row["path"], target)
    index = source / "registry/index"
    require(index.is_dir(), "offline Cargo index required")
    for p in index.rglob("*"):
        require(not p.is_symlink() and (p.is_file() or p.is_dir()), "invalid Cargo index member")
    shutil.copytree(index, dest / "registry/index")


def selected_compiler_path(name):
    return (name in ("bin/cargo", "bin/rustc", "lib/rustlib/x86_64-unknown-linux-gnu/bin/rust-lld",
                     "lib/rustlib/x86_64-unknown-linux-gnu/bin/gcc-ld/ld.lld")
            or name.startswith(("lib/libLLVM", "lib/librustc_driver"))
            or (name.startswith("lib/rustlib/x86_64-unknown-linux-gnu/lib/") and name.endswith((".rlib", ".rmeta", ".so"))))


def verify_toolchain(root, a):
    root = Path(root)
    verify_map(root, a["compiler_files"])
    actual = []
    for path in root.rglob("*"):
        name = path.relative_to(root).as_posix()
        if selected_compiler_path(name):
            regular(path); actual.append(name)
    same(sorted(actual), sorted(x["path"] for x in a["compiler_files"]), "selected toolchain inventory changed")


def verify_executable_view(view, toolchain):
    view, toolchain = Path(view).absolute(), Path(toolchain).absolute()
    require(view.is_dir() and not any(p.is_symlink() for p in [view, *view.parents]), "unsafe executable view directory")
    same(sorted(p.name for p in view.iterdir()), ["cargo", "rustc"], "closed Rust executable view inventory")
    for name in ("cargo", "rustc"):
        path = view / name; target = toolchain / "bin" / name
        require(path.is_symlink(), "Rust executable view requires explicit verified link")
        same(os.readlink(path), str(target), "Rust executable link target")
        regular(target)


def create_executable_view(view, toolchain, a):
    toolchain = Path(toolchain).absolute()
    verify_toolchain(toolchain, a)
    view = fresh(view)
    for name in ("cargo", "rustc"):
        (view / name).symlink_to(toolchain / "bin" / name)
    verify_executable_view(view, toolchain)
    return view


def verify_sysroot(view, toolchain, env):
    actual = subprocess.check_output([str(Path(view) / "rustc"), "--print", "sysroot"], env=env, text=True, timeout=15).strip()
    same(Path(actual), Path(toolchain), "executable view preserves original reviewed Rust sysroot")
    return actual


def build(session_path, profile, toolchain, cache, control=False):
    a = authority(); session, root = session_at(session_path, a)
    require(profile in a["recipe"]["profiles"], "unsupported profile")
    toolchain = Path(toolchain).absolute()
    verify_toolchain(toolchain, a)
    version = subprocess.check_output([str(toolchain / "bin/rustc"), "--version", "--verbose"], env=minimal_env(root / "home"), text=True, timeout=15)
    same(version, a["recipe"]["rustc_verbose"], "exact Rust compiler version")
    source = root / ("control-source" if control else "source")
    out = fresh(root / (("build-control-" if control else "build-") + profile))
    home = out / "home"; home.mkdir()
    cargo_cache(cache, out / "cargo-home", a)
    host = measured_host(); check_host(host, a)
    view = create_executable_view(out / "rust-bin", toolchain, a)
    env = minimal_env(home, out / "cargo-home", toolchain, view)
    sysroot = verify_sysroot(view, toolchain, env)
    same(host, session["host"], "build/session host snapshot")
    argv = [sys.executable, "-B", str(root / "helpers/build.py"), "--overlay", str(source / "overlay-manifest.json"),
            "--profile", profile, "--output", str(out / "result")]
    invocation = invoke(argv, root, env, out, host)
    envelope = {"schema": "oxid-unit4-portable-parser-build-v1", "session": identity(session_path), "profile": profile,
                "toolchain": str(toolchain), "invocation": identity(out / "invocation.json"),
                "receipt": identity(out / "result/build-receipt.json"), "executable_view": str(view), "rust_sysroot": sysroot}
    envelope["control"] = control
    write(out / "portable-build.json", envelope)
    verify_build(root, session_path, profile, a, control=control)
    require(invocation["started_ns"] >= session["prepared_ns"], "build predates session")
    return identity(out / "portable-build.json")


def verify_cargo(build, root, out, profile, toolchain, invocation, a, control=False):
    source = root / ("control-source" if control else "source")
    same(build["schema"], "oxid-unit4-parser-build-v1", "build schema")
    same(build["status"], "built", "successful build required")
    same(build["exit_code"], 0, "Cargo exit")
    same(build["control"], control, "exact observer/control selection")
    same(build["profile"], profile, "build profile")
    same(build["target"], a["recipe"]["target"], "compiler target")
    same(build["argv"], [str(out / "rust-bin/cargo"), *a["recipe"]["cargo_tail"], *(["--release"] if profile == "release" else [])], "exact Cargo command")
    same(build["environment"], a["recipe"]["build_environment"], "bounded Cargo environment")
    same(build["cwd"], str(source), "actual source cwd")
    same(build["rustc_version"], a["recipe"]["rustc_verbose"], "actual Rust version")
    artifact(build["rustc"], toolchain / "bin/rustc")
    artifact(build["overlay_manifest"], source / "overlay-manifest.json")
    same(build["candidate_source_manifest_sha256"], a["current"]["current_candidate_source_manifest_sha256"], "current manifest binding")
    same(build["observer_source_sha256"], a["helper_manifest_sha256"], "fresh helper identity")
    rows = [load(line) for line in artifact(build["stdout"], out / "result/stdout.jsonl").splitlines()]
    artifact(build["stderr"], out / "result/stderr.txt")
    observed = [x for x in rows if x.get("reason") == "compiler-artifact" and x.get("executable")]
    same(len(observed), 1, "exactly one test executable")
    emitted = observed[0]
    same(emitted["package_id"], "path+" + source.as_uri() + "#oxid@0.9.0", "actual Cargo package")
    same(emitted["manifest_path"], str(source / "Cargo.toml"), "actual Cargo manifest")
    same(emitted["target"], {"kind": ["bin"], "crate_types": ["bin"], "name": "oxid", "src_path": str(source / "src/cli.rs"),
                           "edition": "2021", "doc": True, "doctest": False, "test": True}, "actual Cargo target/body")
    same(emitted["profile"], {"opt_level": "0" if profile == "debug" else "3", "debuginfo": 0,
                            "debug_assertions": profile == "debug", "overflow_checks": profile == "debug", "test": True}, "actual Cargo profile")
    same(emitted["features"], [], "unexpected Cargo features")
    same(emitted["fresh"], False, "cached/stale observer build forbidden")
    same(emitted["executable"], build["binary"]["path"], "actual emitted binary")
    same(emitted["filenames"], [build["binary"]["path"]], "actual emitted binary roster")
    finished = [x for x in rows if x.get("reason") == "build-finished"]
    same(finished, [{"reason": "build-finished", "success": True}], "actual Cargo build finished")
    binary = Path(build["binary"]["path"])
    same(binary.parent, out / "result/target" / a["recipe"]["target"] / profile / "deps", "binary belongs to fresh explicit target/profile")
    require(re.fullmatch(r"oxid-[0-9a-f]+", binary.name) is not None, "Cargo binary filename")
    raw = artifact(build["binary"])
    require(raw[:6] == b"\x7fELF\x02\x01" and int.from_bytes(raw[18:20], "little") == 62, "native executable ABI must be ELF64 x86_64")
    require(invocation["started_ns"] <= binary.stat().st_mtime_ns <= invocation["finished_ns"], "executable timestamp predates fresh build")
    return build


def verify_build(root, session_path, profile, a, control=False):
    root = Path(root); out = root / (("build-control-" if control else "build-") + profile)
    source = root / ("control-source" if control else "source")
    envelope = read(out / "portable-build.json")
    same(envelope["schema"], "oxid-unit4-portable-parser-build-v1", "portable build schema")
    same(envelope["profile"], profile, "envelope profile")
    same(envelope["control"], control, "envelope observer/control role")
    session = load(artifact(envelope["session"], session_path))
    toolchain = Path(envelope["toolchain"])
    verify_toolchain(toolchain, a)
    same(envelope["executable_view"], str(out / "rust-bin"), "bound executable view")
    verify_executable_view(out / "rust-bin", toolchain)
    same(envelope["rust_sysroot"], str(toolchain), "recorded reviewed Rust sysroot")
    verify_map(out / "cargo-home", a["dependency_files"])
    for name in ("config", "config.toml"):
        require(not (out / "cargo-home" / name).exists(), "private Cargo config forbidden")
    invocation = load(artifact(envelope["invocation"], out / "invocation.json"))
    same(invocation["argv"], [sys.executable, "-B", str(root / "helpers/build.py"), "--overlay", str(source / "overlay-manifest.json"),
                            "--profile", profile, "--output", str(out / "result")], "actual helper build command")
    same(invocation["environment"], minimal_env(out / "home", out / "cargo-home", toolchain, out / "rust-bin"), "actual clean build environment")
    same(invocation["cwd"], str(root), "actual helper cwd")
    same(invocation["exit_code"], 0, "actual helper exit")
    verify_process_tools(invocation)
    require(session["prepared_ns"] <= invocation["started_ns"] <= invocation["finished_ns"], "fresh build ordering")
    check_host(invocation["host"], a)
    same(invocation["host"], session["host"], "build/session host snapshot")
    same(verify_sysroot(out / "rust-bin", toolchain, invocation["environment"]), envelope["rust_sysroot"], "actual reviewed Rust sysroot")
    artifact(invocation["stdout"], out / "driver.stdout"); artifact(invocation["stderr"], out / "driver.stderr")
    receipt = load(artifact(envelope["receipt"], out / "result/build-receipt.json"))
    return verify_cargo(receipt, root, out, profile, toolchain, invocation, a, control=control), envelope


def collect(session_path, profile, contract_dir):
    a = authority(); session, root = session_at(session_path, a)
    require(profile in a["recipe"]["profiles"], "unsupported profile")
    verify_build(root, session_path, profile, a)
    comparator()[0].load_contract(contract_dir)
    out = fresh(root / ("collect-" + profile))
    host = measured_host(); check_host(host, a)
    same(host, session["host"], "collection/session host snapshot")
    argv = [sys.executable, "-B", str(root / "helpers/run.py"), "--build-receipt", str(root / ("build-" + profile) / "result/build-receipt.json"),
            "--contract-dir", str(Path(contract_dir).absolute()), "--checkpoint", str(Path(session_path).absolute()), "--output", str(out / "result")]
    invoke(argv, root, minimal_env(root / "home"), out, host)
    envelope = {"schema": "oxid-unit4-portable-parser-collection-v1", "session": identity(session_path), "profile": profile,
                "contract_dir": str(Path(contract_dir).absolute()), "invocation": identity(out / "invocation.json"),
                "manifest": identity(out / "result/execution-manifest.json")}
    write(out / "portable-collection.json", envelope)
    return identity(out / "portable-collection.json")


def passivity(session_path, profile):
    a = authority(); session, root = session_at(session_path, a)
    require(profile in a["recipe"]["profiles"], "unsupported passivity profile")
    for control in (False, True):
        verify_build(root, session_path, profile, a, control=control)
    out = fresh(root / ("passivity-" + profile))
    host = measured_host(); same(host, session["host"], "passivity/session host snapshot")
    argv = [sys.executable, "-B", str(root / "helpers/passivity.py"),
            "--instrumented", str(root / ("build-" + profile) / "result/build-receipt.json"),
            "--control", str(root / ("build-control-" + profile) / "result/build-receipt.json"),
            "--checkpoint", str(Path(session_path).absolute()), "--output", str(out / "result")]
    invoke(argv, root, minimal_env(root / "home"), out, host)
    envelope = {"schema": "oxid-unit4-portable-parser-passivity-v1", "session": identity(session_path), "profile": profile,
                "invocation": identity(out / "invocation.json"), "report": identity(out / "result/report.json")}
    write(out / "portable-passivity.json", envelope)
    verify_passivity(root, session_path, profile, a, set())
    return identity(out / "portable-passivity.json")


def verify_passivity(root, session_path, profile, a, nonces):
    root, session_path = Path(root).absolute(), Path(session_path).absolute()
    regular(session_path)
    out = root / ("passivity-" + profile); result = out / "result"
    envelope = read(out / "portable-passivity.json")
    same(envelope["schema"], "oxid-unit4-portable-parser-passivity-v1", "passivity envelope schema")
    same(envelope["profile"], profile, "passivity envelope profile")
    session = load(artifact(envelope["session"], session_path))
    invocation = load(artifact(envelope["invocation"], out / "invocation.json"))
    same(invocation["argv"], [sys.executable, "-B", str(root / "helpers/passivity.py"),
                             "--instrumented", str(root / ("build-" + profile) / "result/build-receipt.json"),
                             "--control", str(root / ("build-control-" + profile) / "result/build-receipt.json"),
                             "--checkpoint", str(session_path), "--output", str(result)], "actual passivity command")
    same(invocation["environment"], minimal_env(root / "home"), "actual clean passivity environment")
    same(invocation["cwd"], str(root), "actual passivity cwd")
    same(invocation["exit_code"], 0, "actual passivity exit")
    same(invocation["host"], session["host"], "passivity/session host snapshot")
    verify_process_tools(invocation)
    artifact(invocation["stdout"], out / "driver.stdout"); artifact(invocation["stderr"], out / "driver.stderr")
    builds = []
    for control in (False, True):
        build, bound = verify_build(root, session_path, profile, a, control=control)
        builds.append((build, bound))
        built = load(artifact(bound["invocation"]))
        require(built["finished_ns"] <= invocation["started_ns"] <= invocation["finished_ns"], "passivity predates fresh build")
    require(builds[0][0]["binary"]["sha256"] != builds[1][0]["binary"]["sha256"], "passivity reuses same observer/control binary")
    report = load(artifact(envelope["report"], result / "report.json"))
    same(report["schema"], "oxid-unit4-parser-passivity-v1", "passivity report schema")
    same(report["status"], "pass", "ordinary passivity report failed")
    same(report["scope"], "three hand-prescribed ordinary controls in both modes; no seam equivalence or full-corpus assertion", "passivity bounded scope")
    same(report["instrumented"], builds[0][1]["receipt"], "passivity instrumented build")
    same(report["control"], builds[1][1]["receipt"], "passivity control build")
    artifact(report["authority_checkpoint"], session_path)
    prescribed = a["recipe"]["passivity"]
    same([r["case"] for r in report["results"]], [name for name, _ in prescribed["cases"]], "exact ordinary passivity roster")
    inventory = {"report.json"}; pairs = 0
    for row, (name, source) in zip(report["results"], prescribed["cases"]):
        same(artifact(row["source"], result / (name + ".ox")), source.encode(), "prescribed ordinary passivity source")
        inventory.add(name + ".ox")
        same(row["pairs"], 2, "passivity mode pair count")
        same(row["equal_fields"], prescribed["compare_fields"], "passivity compared-field roster")
        same(len(row["receipts"]), 2, "instrumented/control execution receipt pair")
        observations = []
        for index, (receipt, (build, _)) in enumerate(zip(row["receipts"], builds)):
            work = result / (name + ("-control" if index else "-instrumented"))
            same(receipt["argv"], [build["binary"]["path"], a["recipe"]["entrypoint"], "--exact", "--ignored", "--nocapture", "--test-threads=1"], "passivity actual observer command")
            same(receipt["exit_code"], 0, "passivity execution exit")
            stdout = artifact(receipt["stdout"], work / "stdout.txt").decode()
            artifact(receipt["stderr"], work / "stderr.txt")
            raw = load(artifact(receipt["raw"], work / "raw.json"))
            same(raw["schema"], "oxid-unit4-parser-raw-v1", "passivity raw schema")
            same(raw["case_id"], name, "passivity raw case")
            same(raw["source_utf8"], source, "passivity actual source bytes")
            same(raw["display_path"], name + ".ox", "passivity actual source display path")
            nonce = raw["nonce"]
            require(isinstance(nonce, str) and re.fullmatch("[0-9a-f]{32}", nonce) is not None and nonce not in nonces, "reused passivity execution nonce")
            nonces.add(nonce)
            require(stdout.count("UNIT4_EXECUTED " + nonce) == 1 and re.search(r"test result: ok\. 1 passed; 0 failed; 0 ignored;", stdout), "zero-execution passivity witness")
            same([x["mode"] for x in raw["observations"]], prescribed["modes"], "passivity exact executed mode roster")
            for mode_index, observed in enumerate(raw["observations"]):
                same(observed["executed"], True, "passivity actual execution")
                same(observed["mode_execution_index"], mode_index, "passivity mode execution index")
                same(observed["runtime_os"], session["host"]["os"], "passivity Rust runtime OS")
                same(observed["runtime_architecture"], session["host"]["architecture"], "passivity Rust runtime architecture")
                same(observed["pointer_width"], session["host"]["python_pointer_width"], "passivity Rust runtime ABI")
            observations.append(raw["observations"])
            inventory |= {work.name + "/" + filename for filename in ("stdout.txt", "stderr.txt", "raw.json")}
        for left, right in zip(*observations):
            for field in prescribed["compare_fields"]:
                same(left[field], right[field], "ordinary passivity differs: " + name + "." + left["mode"] + "." + field)
            if left["result"] == "ok":
                same(left["nodes_admitted"], right["nodes_admitted"], "successful passivity node ledger")
            pairs += 1
    same(pairs, prescribed["pairs_per_profile"], "all six ordinary passivity pairs")
    actual = []
    for path in result.rglob("*"):
        require(not path.is_symlink() and (path.is_file() or path.is_dir()), "unsafe passivity evidence")
        if path.is_file(): actual.append(path.relative_to(result).as_posix())
    same(sorted(actual), sorted(inventory), "complete passivity evidence inventory")
    return {"profile": profile, "pairs": pairs, "report": envelope["report"], "scope": prescribed["scope"]}


def admit_execution(path, contract, approval):
    a = approval["authority"]; session_path = approval["session_path"]
    session, root = session_at(session_path, a)
    manifest = read(path); profile = manifest["profile"]
    require(profile in a["recipe"]["profiles"], "execution profile")
    out = root / ("collect-" + profile)
    envelope = read(out / "portable-collection.json")
    same(envelope["schema"], "oxid-unit4-portable-parser-collection-v1", "collection envelope schema")
    same(envelope["profile"], profile, "collection envelope profile")
    artifact(envelope["session"], session_path)
    artifact(envelope["manifest"], path)
    same(Path(path), out / "result/execution-manifest.json", "fresh collection evidence path")
    invocation = load(artifact(envelope["invocation"], out / "invocation.json"))
    same(invocation["argv"], [sys.executable, "-B", str(root / "helpers/run.py"), "--build-receipt", str(root / ("build-" + profile) / "result/build-receipt.json"),
                            "--contract-dir", envelope["contract_dir"], "--checkpoint", str(session_path), "--output", str(out / "result")], "actual collection command")
    same(invocation["environment"], minimal_env(root / "home"), "actual clean collection environment")
    same(invocation["cwd"], str(root), "actual collection cwd")
    same(invocation["exit_code"], 0, "actual collection exit")
    verify_process_tools(invocation)
    artifact(invocation["stdout"], out / "driver.stdout"); artifact(invocation["stderr"], out / "driver.stderr")
    build, envelope_build = verify_build(root, session_path, profile, a)
    build_invocation = load(artifact(envelope_build["invocation"]))
    require(build_invocation["finished_ns"] <= invocation["started_ns"] <= invocation["finished_ns"], "collection predates build")
    same(manifest["schema"], "oxid-unit4-parser-execution-v1", "execution schema")
    same(manifest["status"], "collected", "complete collection required")
    same(manifest["contract_decoded_sha256"], a["contract_sha256"], "execution contract")
    same(manifest["package_freeze_sha256"], a["freeze_sha256"], "execution freeze")
    artifact(manifest["authority_checkpoint"], session_path)
    same(manifest["build_receipt"], envelope_build["receipt"], "collection build binding")
    host = manifest["host_runtime"]; check_host(host, a)
    same(host, invocation["host"], "separately measured collection host")
    same(host, session["host"], "collection/session host snapshot")
    same(host, build_invocation["host"], "collection/build host snapshot")
    for key, filename in (("driver", "run.py"), ("normalizer", "parse_debug.py")):
        artifact(manifest[key], root / "helpers" / filename)
    expected_files = {"execution-manifest.json", "observations.jsonl", "test-roster.stdout", "test-roster.stderr"}
    nonces = approval["nonces"]
    for row in manifest["case_receipts"]:
        case_dir = out / "result" / safe_relative(row["case_id"])
        require(row["execution_id"] not in nonces, "cross-profile reused execution nonce")
        nonces.add(row["execution_id"])
        for key, filename in (("request", "request.json"), ("raw", "raw.json"), ("stdout", "stdout.txt"), ("stderr", "stderr.txt")):
            artifact(row[key], case_dir / filename)
        request = load(artifact(row["request"]))
        artifact(request["source"], case_dir / "source.ox")
        same(read(case_dir / "receipt.json"), row, "on-disk case receipt matches manifest")
        expected_files |= {row["case_id"] + "/" + x for x in ("request.json", "raw.json", "stdout.txt", "stderr.txt", "source.ox", "receipt.json")}
    files = set()
    for p in (out / "result").rglob("*"):
        require(not p.is_symlink() and (p.is_dir() or p.is_file()), "unsafe evidence member")
        if p.is_file():
            files.add(p.relative_to(out / "result").as_posix())
    same(files, expected_files, "missing/extra complete execution inventory")
    same(manifest["observations"]["path"], str(out / "result/observations.jsonl"), "normalized rows evidence path")
    return manifest, host, build, profile


def comparator():
    a = authority()
    path = FROZEN / "frozen/comparator/comparator.py"
    raw = path.read_bytes(); same(sha(raw), COMPARATOR_SHA, "reviewed comparator source")
    source = raw.decode()
    function_start = source.index("def execution_manifest(path, contract, approval):\n")
    start = source.index(PREFIX_START, function_start)
    end = source.index(PREFIX_END, start)
    old = source[start:end]
    replacement = "    manifest, host, build, profile = portable_admission(path, contract, approval)\n"
    derived = source[:start] + replacement + source[end:]
    proof = {"schema": "oxid-unit4-portable-comparator-derivation-v1", "original_sha256": COMPARATOR_SHA,
             "derived_sha256": sha(derived.encode()), "replaced_region_sha256": sha(old.encode()),
             "replacement_sha256": sha(replacement.encode()), "unchanged_prefix_sha256": sha(source[:start].encode()),
             "unchanged_suffix_sha256": sha(source[end:].encode()), "semantic_predicate_handlers_unchanged": 22}
    decoder = types.ModuleType("debug_decoder")
    decoder.__file__ = str(path.parent / "debug_decoder.py")
    exec(compile(Path(decoder.__file__).read_bytes(), decoder.__file__, "exec"), decoder.__dict__)
    sys.modules["debug_decoder"] = decoder
    result = types.ModuleType("portable_derived_comparator")
    result.__file__ = str(path)
    result.portable_admission = admit_execution
    exec(compile(derived, str(path), "exec"), result.__dict__)
    return result, proof


def effective_authority(a):
    """Bind packaged amendment artifacts without changing execution identities."""
    expected = a["effective_contract_authority"]
    require(set(expected) == {"checkpoint", "descriptor", "amendment", "review"}, "complete effective comparison authority")
    resolved = {}
    for key, record in expected.items():
        safe_relative(record["path"])
        resolved[key] = {**record, "path": str(FROZEN / record["path"])}
        artifact(resolved[key])
    return resolved


def compare(session_path, contract_dir):
    a = authority(); session, root = session_at(session_path, a)
    c, proof = comparator(); contract = c.load_contract(contract_dir)
    effective, effective_receipt = c.admit_contract_amendment(contract, contract_dir, effective_authority(a))
    same(effective_receipt["effective_contract_identity"], a["effective_contract_identity"], "portable effective comparison identity")
    same(effective_receipt["effective_parser_document_canonical_sha256"], a["effective_document_canonical_sha256"], "portable effective comparison document")
    rows, identities = [], {}
    approval = {"authority": a, "session_path": Path(session_path).absolute(), "nonces": set()}
    ordinary_passivity = [verify_passivity(root, session_path, profile, a, approval["nonces"]) for profile in a["recipe"]["profiles"]]
    for profile in a["recipe"]["profiles"]:
        actual, observed, bound = c.execution_manifest(root / ("collect-" + profile) / "result/execution-manifest.json", contract, approval)
        same(actual, profile, "exact profile order")
        rows.extend(observed); identities[profile] = bound
    require(identities["debug"]["binary_sha256"] != identities["release"]["binary_sha256"], "same binary reused across profiles")
    result = c.compare_effective_rows(effective, effective_receipt, rows, identities)
    result.update(portable_authority_sha256=AUTHORITY_SHA, session=identity(session_path), derivation=proof,
                  ordinary_passivity=ordinary_passivity, evidence_scope="fresh execution at recorded paths; hosting is not inferred")
    write(root / "comparison.json", result)
    return result


def main():
    require(not sys.flags.optimize and __debug__, "OPTIMIZED_PYTHON_UNSUPPORTED: run portable admission with assertions enabled")
    p = argparse.ArgumentParser(description=__doc__); commands = p.add_subparsers(dest="command", required=True)
    prepare_p = commands.add_parser("prepare")
    for name in ("repo", "checkout", "output"): prepare_p.add_argument("--" + name, type=Path, required=True)
    build_p = commands.add_parser("build")
    for name in ("session", "toolchain", "cargo-cache"): build_p.add_argument("--" + name, type=Path, required=True)
    build_p.add_argument("--profile", choices=("debug", "release"), required=True)
    build_p.add_argument("--control", action="store_true")
    passive_p = commands.add_parser("passivity")
    passive_p.add_argument("--session", type=Path, required=True)
    passive_p.add_argument("--profile", choices=("debug", "release"), required=True)
    collect_p = commands.add_parser("collect"); compare_p = commands.add_parser("compare")
    for sub in (collect_p, compare_p):
        sub.add_argument("--session", type=Path, required=True); sub.add_argument("--contract-dir", type=Path, required=True)
    collect_p.add_argument("--profile", choices=("debug", "release"), required=True)
    commands.add_parser("inspect")
    args = p.parse_args()
    if args.command == "prepare": result = prepare(args.repo, args.checkout, args.output)
    elif args.command == "build": result = build(args.session, args.profile, args.toolchain, args.cargo_cache, args.control)
    elif args.command == "passivity": result = passivity(args.session, args.profile)
    elif args.command == "collect": result = collect(args.session, args.profile, args.contract_dir)
    elif args.command == "compare": result = compare(args.session, args.contract_dir)
    else:
        a = authority(); _, proof = comparator()
        result = {"authority_sha256": AUTHORITY_SHA, "derivation": proof,
                  "historical_files": len(a["original_files"]), "current_base_files": len(a["current"]["current_base_files"]),
                  "current_derived_files": len(a["current"]["current_derived_files"]), "candidate_executions": 0}
    print(json.dumps(result, sort_keys=True, indent=2))
    return 0 if result.get("status", "pass") == "pass" else 1


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (Rejected, OSError, ValueError, KeyError, TypeError, IndexError) as error:
        print(json.dumps({"status": "fail", "error": str(error)}), file=sys.stderr)
        raise SystemExit(1)
