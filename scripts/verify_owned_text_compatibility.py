#!/usr/bin/env python3
"""Bind owned-text source tests and their exact asset to the full retained replay.

All inherited execution, observer, semantic and resource predicates remain the
existing functions. Provisional pins allow preparation only, not qualification.
"""
import argparse
from contextlib import contextmanager
import datetime
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import stat
import sys
import types


ROOT = Path(__file__).resolve().parents[1]
PREDECESSOR = "0e8f23f7481c7ec4e24cd416ed30607f8eb75daf"
PREDECESSOR_POLICY = "scripts/verify_owned_record_vector_compatibility.py"
PREDECESSOR_POLICY_PIN = (110006, "16c7affd778deda3d61f9a4e4aebd66c9e01d842c6afe3e9c1a20de57caab6e0")
REVIEWED_SOURCE_HEAD = "798cb875e1cd45592e11f9658aa0fab54dc3331d"
PINS_PROVISIONAL = False
PROGRAM = "src/frontend/oir/owned/source/program.rs"
TEST = "src/frontend/oir/owned/source/owned_text_source_resource_tests.rs"
ASSET = "packages/utf8_text/src/lib.ox"
RELATIVE_ASSET = "../../../../../packages/utf8_text/src/lib.ox"
ASSET_INCLUDE = b'include_str!("../../../../../packages/utf8_text/src/lib.ox")'
OLD_HOOK = (b'#[cfg(test)]\n#[path = "owned_vector_source_execution_tests.rs"]\n'
            b'mod owned_vector_source_execution_tests;\n')
NEW_HOOK = (b'#[cfg(test)]\n#[path = "owned_text_source_resource_tests.rs"]\n'
            b'mod owned_text_source_resource_tests;\n')
TEXT_CHANGED = frozenset((PROGRAM,))
TEXT_ADDED = frozenset((TEST,))
REVIEWED_INPUTS = {
    PROGRAM: (51207, "2522e4d1b961b3fa9bc6238f4e56de46aee463766943760bc48f86c0ff211eda"),
    TEST: (8909, "7be9540165fab3d097f2ef953f532ccd5b07f2937f42e412274cc96da323ec05"),
}
TEXT_SOURCE_ASSETS = {ASSET: (11338, "5e43415b9c89dabefc7ec07db0287ff477f1f712bc25fcae215793c7b2f63f7b")}


def require(condition, message):
    if not condition:
        raise ValueError(message)


def load_predecessor(repo):
    path = repo / PREDECESSOR_POLICY
    require(not path.is_symlink() and not any(parent.is_symlink() for parent in path.parents),
            "symlink pinned owned-vector controller")
    before = path.stat()
    require(stat.S_ISREG(before.st_mode) and before.st_nlink == 1 and not before.st_mode & 0o111
            and before.st_size == PREDECESSOR_POLICY_PIN[0],
            "unsafe pinned owned-vector controller")
    body = path.read_bytes()
    fields = ("st_dev", "st_ino", "st_mode", "st_nlink", "st_size", "st_mtime_ns", "st_ctime_ns")
    require(tuple(getattr(before, name) for name in fields) == tuple(getattr(path.stat(), name) for name in fields)
            and (len(body), hashlib.sha256(body).hexdigest()) == PREDECESSOR_POLICY_PIN,
            "changed pinned owned-vector controller")
    module = types.ModuleType("_owned_text_retained_owned_vector")
    module.__file__ = str(path)
    exec(compile(body, str(path), "exec"), module.__dict__)
    return module


predecessor = load_predecessor(ROOT)
library = predecessor.library
RETAINED_RUN = predecessor.RETAINED_RUN
PREDECESSOR_ADDED = predecessor.PARENT_ADDED
PARENT_ADDED = PREDECESSOR_ADDED | TEXT_ADDED | frozenset(TEXT_SOURCE_ASSETS)
PREDECESSOR_POLICIES = {**predecessor.PREDECESSOR_POLICIES,
                        PREDECESSOR_POLICY: PREDECESSOR_POLICY_PIN[1]}
for _name in ("BASELINE", "BASELINE_SOURCE_SHA", "BINDING", "RETAINED", "RETAINED_RUNNER_SHA",
              "sha", "encoded", "identity", "regular", "git", "source_directives", "directive_manifest"):
    globals()[_name] = getattr(predecessor, _name)


def approved_program(before):
    require(before.count(OLD_HOOK) == 1 and NEW_HOOK not in before, "wrong predecessor source test hook")
    return before.replace(OLD_HOOK, NEW_HOOK + OLD_HOOK, 1)


def program_directives():
    old = predecessor.REVIEWED_DIRECTIVES[PROGRAM]
    previous = b'#[path = "owned_vector_source_execution_tests.rs"]'
    added = b'#[path = "owned_text_source_resource_tests.rs"]'
    paths = list(old["legacy"][1])
    paths.insert(paths.index(previous), added)
    attributes = list(old["attributes"])
    position = attributes.index((previous, 1))
    require(attributes[position - 1] == (b"#[cfg(test)]", 1), "wrong predecessor test-only attribute")
    attributes[position:position] = [(added, 1), (b"#[cfg(test)]", 1)]
    return {**old, "legacy": (list(old["legacy"][0]), paths), "attributes": tuple(attributes)}


REVIEWED_DIRECTIVES = {
    PROGRAM: program_directives(),
    TEST: {"legacy": ([ASSET_INCLUDE], []), "attributes": ((b"#[test]", 3),),
           "macros": ((ASSET_INCLUDE, 1),), "c_preprocessor": (), "build_script": None},
}


def validate_source_assets(inputs):
    require(set(TEXT_SOURCE_ASSETS) == {ASSET}, "wrong finite owned-text asset membership")
    owner = PurePosixPath(TEST).parent
    parts = list(owner.parts)
    for part in PurePosixPath(RELATIVE_ASSET).parts:
        if part == "..":
            require(parts, "owned-text asset include escapes repository")
            parts.pop()
        else:
            parts.append(part)
    require(PurePosixPath(*parts).as_posix() == ASSET
            and ASSET_INCLUDE == ('include_str!("' + RELATIVE_ASSET + '")').encode(),
            "owned-text asset path and literal differ")
    require(inputs[TEST].count(ASSET_INCLUDE) == 1
            and (len(inputs[ASSET]), sha(inputs[ASSET])) == TEXT_SOURCE_ASSETS[ASSET],
            "owned-text compiled asset or its exact include changed")


def capture_sources(repo, baseline, parent):
    require(set(TEXT_SOURCE_ASSETS) == {ASSET}, "wrong finite owned-text asset membership")
    expected_parent = set(baseline) | PREDECESSOR_ADDED
    expected = expected_parent | TEXT_ADDED | set(TEXT_SOURCE_ASSETS)
    require(set(parent) == expected_parent and len(parent) == 572, "wrong pinned owned-vector parent membership")
    require(TEXT_CHANGED == {PROGRAM} and TEXT_ADDED == {TEST}
            and TEXT_CHANGED <= expected_parent and not TEXT_ADDED & expected_parent
            and not set(TEXT_SOURCE_ASSETS) & expected_parent and len(expected) == 574,
            "wrong finite owned-text source membership")
    require(set(REVIEWED_INPUTS) == TEXT_CHANGED | TEXT_ADDED
            and set(REVIEWED_DIRECTIVES) == set(REVIEWED_INPUTS), "wrong owned-text pin/directive membership")
    actual, directories = set(), set()
    for part in ("src", "native"):
        require((repo / part).is_dir() and not (repo / part).is_symlink(), "missing compiler source root")
        for directory, dirs, files in os.walk(repo / part, followlinks=False):
            for name in dirs + files:
                path = Path(directory) / name
                require(not path.is_symlink(), "symlink compiler source member")
                (directories if name in dirs else actual).add(path.relative_to(repo).as_posix())
    compiler = {name for name in expected if name.startswith(("src/", "native/"))}
    require(actual == compiler, "unexpected compiler source membership")
    expected_dirs = {str(path) for name in compiler for path in PurePosixPath(name).parents
                     if str(path) not in (".", "src", "native")}
    require(directories == expected_dirs, "unexpected compiler source directory")
    try:
        inputs = {name: regular(repo, name) for name in sorted(expected)}
    except OSError as error:
        raise ValueError("unreadable owned-text compiler input: " + str(error)) from error
    for name, before in parent.items():
        if name != PROGRAM:
            require(inputs[name] == before, "unreviewed change outside owned-text boundary: " + name)
    require(inputs[PROGRAM] == approved_program(parent[PROGRAM]), "source program change exceeds exact test hook")
    for name, (length, digest) in REVIEWED_INPUTS.items():
        require((len(inputs[name]), sha(inputs[name])) == (length, digest), "owned-text reviewed input changed: " + name)
        require(source_directives(name, inputs[name]) == REVIEWED_DIRECTIVES[name],
                "owned-text compile-time directive changed: " + name)
    validate_source_assets(inputs)
    return inputs


def current_manifest(inputs, baseline, checkout, parent):
    registry_names = set(baseline) | predecessor.PREDECESSOR_ADDED
    inherited = predecessor.current_manifest(inputs=parent, baseline=baseline, checkout=checkout,
                                            parent={name: parent[name] for name in registry_names})
    result = {**inherited,
              "source_policy_parent": PREDECESSOR, "source_policy_inputs": PREDECESSOR_POLICIES,
              "source_policy_input_members": len(PREDECESSOR_POLICIES),
              "predecessor_compiler_input_members": len(parent), "owned_text_pins_provisional": PINS_PROVISIONAL,
              "owned_text_reviewed_source_head": REVIEWED_SOURCE_HEAD,
              "owned_text_controller": identity("scripts/verify_owned_text_compatibility.py",
                                                 regular(ROOT, "scripts/verify_owned_text_compatibility.py")),
              "owned_text_changes": [{"path": PROGRAM, "before": identity(PROGRAM, parent[PROGRAM]),
                                      "after": identity(PROGRAM, inputs[PROGRAM])}],
              "owned_text_reviewed_inputs": [{"path": name, "bytes": length, "sha256": digest}
                                               for name, (length, digest) in sorted(REVIEWED_INPUTS.items())],
              "owned_text_directive_inventory": [
                  {"path": name, "before": directive_manifest(source_directives(name, parent[name])) if name in parent else None,
                   "after": directive_manifest(REVIEWED_DIRECTIVES[name])} for name in sorted(REVIEWED_INPUTS)],
              "owned_text_compiled_source_assets": [{**identity(ASSET, inputs[ASSET]), "included_by": TEST,
                                                      "relative_include": RELATIVE_ASSET, "include_occurrences": 1}],
              "changes": [{"path": name, "before": identity(name, baseline[name]), "after": identity(name, value)}
                          for name, value in sorted(inputs.items()) if name in baseline and value != baseline[name]],
              "additions": sorted(PARENT_ADDED), "files": [identity(name, value) for name, value in sorted(inputs.items())],
              "unchanged_baseline_inputs": sum(inputs[name] == value for name, value in baseline.items()),
              "owned_text_semantics_qualified_by_this_gate": False,
              "owned_text_allocation_failures_qualified_by_this_gate": False,
              "owned_text_platform_qualified_by_this_gate": False}
    require(all(value is False for name, value in result.items() if name.endswith("qualified_by_this_gate")),
            "unexpected new qualification claim")
    return result


@contextmanager
def bound_predecessor():
    bindings = {"PREDECESSOR": PREDECESSOR, "PREDECESSOR_ADDED": PREDECESSOR_ADDED,
                "PREDECESSOR_POLICIES": PREDECESSOR_POLICIES, "PINS_PROVISIONAL": PINS_PROVISIONAL,
                "capture_sources": capture_sources, "current_manifest": current_manifest}
    require(library.run is RETAINED_RUN, "retained execution function was replaced")
    with predecessor.bound_registry() as context:
        before = {name: getattr(library, name) for name in bindings}
        try:
            for name, value in bindings.items():
                setattr(library, name, value)
            yield context
            require(library.run is RETAINED_RUN, "retained execution function changed")
        finally:
            for name, value in before.items():
                setattr(library, name, value)


def run(args):
    require(__debug__ and not sys.flags.optimize, "qualification refuses optimized Python")
    repo, output = args.repo.resolve(strict=True), args.output.resolve()
    require(repo == ROOT and Path(__file__).resolve() == repo / "scripts/verify_owned_text_compatibility.py",
            "run the owned-text controller from the selected checkout")
    require(output != repo and repo not in output.parents and not output.exists(), "output must be new outside repository")
    output.mkdir(parents=True)
    controller = regular(repo, "scripts/verify_owned_text_compatibility.py")
    report = {"schema": "oxid-owned-text-retained-wrapper-v1", "status": "running", "semantic_pass": False,
              "compiler_executions": 0, "controller": identity("scripts/verify_owned_text_compatibility.py", controller),
              "source_policy_parent": PREDECESSOR, "inherited_input_members": 572, "expected_current_input_members": 574,
              "pins_provisional": PINS_PROVISIONAL, "reviewed_source_head": REVIEWED_SOURCE_HEAD,
              "new_feature_qualification": False, "platform_qualification": False,
              "started_at_utc": datetime.datetime.now(datetime.timezone.utc).isoformat()}
    try:
        require(args.prepare_only or not PINS_PROVISIONAL, "provisional owned-text pins permit prepare-only admission, not qualification")
        require(args.prepare_only or not predecessor.PINS_PROVISIONAL, "provisional predecessor source pins")
        require(args.prepare_only or not predecessor.OWNED_VECTOR_OBSERVER_PROVISIONAL, "provisional predecessor observer pins")
        delegated = argparse.Namespace(**vars(args))
        delegated.output = output / "retained"
        with bound_predecessor() as context:
            status = RETAINED_RUN(delegated)
            require(status == 0, "unchanged retained replay refused; inspect retained raw evidence")
            artifact = "prepared.json" if args.prepare_only else "result.json"
            raw = regular(delegated.output, artifact)
            retained = json.loads(raw)
            require(retained.get("status") == ("prepared" if args.prepare_only else "passed")
                    and retained.get("semantic_pass") is (not args.prepare_only)
                    and retained.get("current_input_members") == 574
                    and retained.get("historical_results_apply_to_current") is False
                    and retained.get("controller") == identity(predecessor.PREDECESSOR_POLICY,
                                                               regular(repo, predecessor.PREDECESSOR_POLICY)),
                    "retained replay lacks the exact owned-text source/materialization binding")
            require(context and context["output"] == delegated.output, "missing unchanged observer preparation context")
            if not args.prepare_only:
                report.update(predecessor.run_owned_vector_observer_controls(
                    repo, delegated.output, context["selected"], context["seam"], context["original"],
                    context["adapter"], retained["test_function_executions"]))
        require(regular(repo, "scripts/verify_owned_text_compatibility.py") == controller, "owned-text controller changed")
        report.update(status="prepared" if args.prepare_only else "passed", semantic_pass=retained["semantic_pass"],
                      compiler_executions=retained["compiler_executions"], current_input_members=retained["current_input_members"],
                      retained_artifact=identity("retained/" + artifact, raw),
                      retained_source=retained["current_source"])
        report["finished_at_utc"] = datetime.datetime.now(datetime.timezone.utc).isoformat()
        (output / artifact).write_bytes(encoded(report))
        print(json.dumps({"status": report["status"], "semantic_pass": report["semantic_pass"], "output": str(output)}))
        return 0
    except (Exception, KeyboardInterrupt) as error:
        report.update(status="failed", semantic_pass=False, error=f"{type(error).__name__}: {error}")
        (output / "failure.json").write_bytes(encoded(report))
        print(report["error"], file=sys.stderr)
        return 1
    finally:
        report.setdefault("finished_at_utc", datetime.datetime.now(datetime.timezone.utc).isoformat())
        (output / "wrapper.json").write_bytes(encoded(report))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--expected-head", required=True)
    parser.add_argument("--cargo", default="cargo")
    parser.add_argument("--rustc", default="rustc")
    parser.add_argument("--fetch-baseline", action="store_true")
    parser.add_argument("--allow-dirty", action="store_true")
    parser.add_argument("--prepare-only", action="store_true")
    return run(parser.parse_args())


if __name__ == "__main__":
    raise SystemExit(main())
