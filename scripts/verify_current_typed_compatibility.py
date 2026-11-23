#!/usr/bin/env python3
"""Replay the retained checks against the integrated typed compiler."""
import argparse
from contextlib import contextmanager
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import stat
import sys
import types


ROOT = Path(__file__).resolve().parents[1]
PREDECESSOR = "197ab7cad557038c40b9bac3b3019f9cdffcbab4"
PREDECESSOR_POLICY = "scripts/verify_windows_local_package_compatibility.py"
PREDECESSOR_POLICY_PIN = (82644, "91c0ffa8cf3727dbcd97984defb6f2ab09524366978446fd24d3fb502257a62c")
REVIEWED_SOURCE = "08ced234ec51fdd976aad0b946135818bd36cf12"
REVIEWED_PARENTS = ("a44c9c89331028775254be181773548f23dc0dbd",)
REVIEWED_TREE = "e35b6466d4af7bcd3bf806957c77625d29c86c2c"
PUBLIC_TESTS = {
    "tests/file_open_append.rs",
    "tests/termination_runtime.c", "tests/typed_process_termination.rs",
    "tests/typed_owned_record_element_loans.rs", "tests/typed_registry_ranges.rs",
    "tests/windows_file_io.rs", "tests/windows_file_runtime.c",
}
PUBLIC_TEST_UPDATES = {"tests/typed_language_server.rs"}
PUBLIC_VECTOR_UPDATES = {
    "examples/owned-vector-results/src/main.ox",
    "packages/owned_vec/src/lib.ox",
}
SOURCE_ROOTS = ("src", "native")
RESOURCE_ADAPTER = "scripts/process_termination_resource_adapter.py"
RESOURCE_ADAPTER_PIN = (3196, "d6b3bcd1bd0c958e4a2164d66425a231c086ba1f767657ea9c6ac360473aad0d")


def require(condition, message):
    if not condition:
        raise ValueError(message)


def load_predecessor():
    path = ROOT / PREDECESSOR_POLICY
    require(not path.is_symlink() and not any(parent.is_symlink() for parent in path.parents),
            "symlink retained compiler policy")
    before = path.stat()
    require(stat.S_ISREG(before.st_mode) and before.st_nlink == 1 and not before.st_mode & 0o111
            and before.st_size == PREDECESSOR_POLICY_PIN[0], "unsafe retained compiler policy")
    body = path.read_bytes()
    after = path.stat()
    fields = ("st_dev", "st_ino", "st_mode", "st_nlink", "st_size", "st_mtime_ns", "st_ctime_ns")
    require(all(getattr(before, field) == getattr(after, field) for field in fields)
            and (len(body), hashlib.sha256(body).hexdigest()) == PREDECESSOR_POLICY_PIN,
            "changed retained compiler policy")
    module = types.ModuleType("_integrated_typed_retained")
    module.__file__ = str(path)
    exec(compile(body, str(path), "exec"), module.__dict__)
    return module


predecessor = load_predecessor()
library = predecessor.library
RETAINED_RUN = predecessor.RETAINED_RUN
require = predecessor.require
regular = predecessor.regular
git = predecessor.git
identity = predecessor.identity
encoded = predecessor.encoded


def reviewed_delta(repo):
    require(tuple(git(repo, "show", "-s", "--format=%P", REVIEWED_SOURCE).decode().split()) == REVIEWED_PARENTS
            and git(repo, "rev-parse", REVIEWED_SOURCE + "^{tree}").decode().strip() == REVIEWED_TREE,
            "wrong reviewed integrated typed source")
    raw = git(repo, "diff", "--name-status", "--no-renames", "-z", PREDECESSOR, REVIEWED_SOURCE)
    words = raw.decode("utf-8").split("\0")
    require(words[-1] == "" and len(words) == 297, "invalid reviewed compiler delta")
    rows = list(zip(words[:-1:2], words[1:-1:2]))
    added = {name for status, name in rows if status == "A"}
    modified = {name for status, name in rows if status == "M"}
    require(len(added) == 28 and len(modified) == 120 and not added & modified,
            "wrong reviewed compiler delta membership")
    require(PUBLIC_TESTS <= added and PUBLIC_TEST_UPDATES | PUBLIC_VECTOR_UPDATES <= modified
            and all(name.startswith(("src/", "native/"))
                    for name in (added | modified) - PUBLIC_TESTS - PUBLIC_TEST_UPDATES - PUBLIC_VECTOR_UPDATES),
            "unexpected reviewed compiler path")
    return added, modified


def capture_sources(repo, baseline, parent):
    added, modified = reviewed_delta(repo)
    require(set(parent) == set(baseline) | predecessor.PARENT_ADDED and len(parent) == 603,
            "wrong retained compiler membership")
    require(modified <= parent.keys() and not added & parent.keys(),
            "reviewed delta does not apply to retained compiler inputs")
    expected = set(parent) | added
    compiler = {name for name in expected if name.startswith(("src/", "native/"))}
    expected_dirs = {str(directory) for name in compiler for directory in PurePosixPath(name).parents
                     if str(directory) not in (".", *SOURCE_ROOTS)}
    actual, directories = set(), set()
    for part in SOURCE_ROOTS:
        require((repo / part).is_dir() and not (repo / part).is_symlink(), "missing compiler root")
        for directory, dirs, files in os.walk(repo / part, followlinks=False):
            for name in dirs + files:
                path = Path(directory) / name
                require(not path.is_symlink(), "symlink compiler input")
                (directories if name in dirs else actual).add(path.relative_to(repo).as_posix())
    require(actual == compiler and directories == expected_dirs and len(compiler) == 521,
            "wrong compiler source membership")
    inputs = {name: regular(repo, name) for name in sorted(expected)}
    for name, body in inputs.items():
        target = git(repo, "show", REVIEWED_SOURCE + ":" + name) if name in added | modified else parent[name]
        require(body == target, "compiler input differs from reviewed source: " + name)
    return inputs


def current_manifest(inputs, baseline, checkout, parent):
    older = {name: parent[name] for name in set(baseline) | predecessor.PREDECESSOR_ADDED}
    inherited = predecessor.current_manifest(parent, baseline, checkout, older)
    result = {**inherited, "source_policy_parent": PREDECESSOR,
              "source_policy_inputs": POLICIES, "source_policy_input_members": len(POLICIES),
              "integrated_reviewed_source": REVIEWED_SOURCE,
              "integrated_reviewed_tree": REVIEWED_TREE, "predecessor_compiler_input_members": len(parent),
              "termination_resource_adapter": identity(RESOURCE_ADAPTER, read_resource_adapter(ROOT)),
              "changes": [{"path": name, "before": identity(name, baseline[name]), "after": identity(name, body)}
                          for name, body in sorted(inputs.items()) if name in baseline and body != baseline[name]],
              "additions": sorted(inputs.keys() - baseline.keys()),
              "files": [identity(name, body) for name, body in sorted(inputs.items())],
              "unchanged_baseline_inputs": sum(inputs[name] == body for name, body in baseline.items())}
    return result


POLICIES = {**predecessor.PREDECESSOR_POLICIES, PREDECESSOR_POLICY: PREDECESSOR_POLICY_PIN[1]}


def read_resource_adapter(repo):
    body = regular(repo, RESOURCE_ADAPTER)
    require((len(body), hashlib.sha256(body).hexdigest()) == RESOURCE_ADAPTER_PIN,
            "changed termination index resource adapter")
    return body


@contextmanager
def bind_sources(repo):
    require(library.run is RETAINED_RUN, "retained compiler runner changed")
    bindings = {"PREDECESSOR": PREDECESSOR, "PREDECESSOR_ADDED": predecessor.PARENT_ADDED,
                "PREDECESSOR_POLICIES": POLICIES, "PINS_PROVISIONAL": False,
                "capture_sources": capture_sources, "current_manifest": current_manifest}
    reviewed_delta(repo)
    with predecessor.bound_predecessor() as context:
        authenticate_prior = library.authenticate_index_resource_adapter
        module = types.ModuleType("_termination_index_resource_adapter")
        module.__file__ = str(repo / RESOURCE_ADAPTER)
        body = read_resource_adapter(repo)
        exec(compile(body, module.__file__, "exec"), module.__dict__)

        def authenticate_index_resource(actual_repo):
            require(read_resource_adapter(actual_repo) == body, "termination index resource adapter changed")
            prior, prior_body = authenticate_prior(actual_repo)
            return module.bind(prior), prior_body

        bindings["authenticate_index_resource_adapter"] = authenticate_index_resource
        before = {name: getattr(library, name) for name in bindings}
        try:
            for name, value in bindings.items():
                setattr(library, name, value)
            yield context
            require(library.run is RETAINED_RUN, "retained compiler runner changed")
        finally:
            for name, value in before.items():
                setattr(library, name, value)


def run(args):
    require(__debug__ and not sys.flags.optimize, "qualification refuses optimized Python")
    repo, output = args.repo.resolve(strict=True), args.output.resolve()
    require(repo == ROOT and repo not in output.parents and output != repo and not output.exists(),
            "qualification requires a new output outside the selected checkout")
    output.mkdir(parents=True)
    report = {"status": "failed", "semantic_pass": False, "new_feature_qualification": False,
              "platform_qualification": False, "reviewed_source": REVIEWED_SOURCE}
    try:
        delegated = argparse.Namespace(**vars(args))
        delegated.output = output / "retained"
        with bind_sources(repo) as context:
            status = RETAINED_RUN(delegated)
            require(status == 0, "retained compiler checks failed")
            artifact = "prepared.json" if args.prepare_only else "result.json"
            body = regular(delegated.output, artifact)
            retained = json.loads(body)
            require(retained.get("status") == ("prepared" if args.prepare_only else "passed")
                    and retained.get("semantic_pass") is (not args.prepare_only)
                    and retained.get("current_input_members") == 631
                    and retained.get("historical_results_apply_to_current") is False
                    and context.get("output") == delegated.output, "incomplete retained compiler results")
            if not args.prepare_only:
                report.update(predecessor.owned_policy.run_owned_vector_observer_controls(
                    repo, delegated.output, context["selected"], context["seam"], context["original"],
                    context["adapter"], retained["test_function_executions"]))
                report.update(predecessor.enum_policy.run_enum_observer_controls(
                    repo, delegated.output, context["selected"], context["seam"], context["original"],
                    context["adapter"], report["test_function_executions"]))
        report.update(status=retained["status"], semantic_pass=retained["semantic_pass"],
                      current_input_members=retained["current_input_members"],
                      retained_artifact=identity("retained/" + artifact, body))
        print(json.dumps(report, sort_keys=True))
        return 0
    except (OSError, ValueError, RuntimeError) as error:
        report["error"] = f"{type(error).__name__}: {error}"
        print(report["error"], file=sys.stderr)
        return 1
    finally:
        (output / "compatibility.json").write_bytes(encoded(report))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--expected-head", required=True)
    parser.add_argument("--cargo", default="cargo")
    parser.add_argument("--rustc", default="rustc")
    parser.add_argument("--fetch-baseline", action="store_true")
    parser.add_argument("--prepare-only", action="store_true")
    args = parser.parse_args()
    args.allow_dirty = False
    return run(args)


if __name__ == "__main__":
    raise SystemExit(main())
