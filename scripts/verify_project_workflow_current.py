#!/usr/bin/env python3
"""Run the retained Unit2 assertions against the complete R1 compiler sources.

The old source certificates remain historical. Their authenticated preparation,
observer, comparator and result predicates are reused with a separately recorded
current input manifest. This gate neither certifies v1.0 nor reissues old proofs.
"""
import argparse
import copy
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
import tarfile

BASELINE = "9ff8ef90a26f003f4616838dfc186fa3bfb6845c"
BASELINE_SOURCE_SHA = "d33ecc4e41975a062790540ba9e9a270d7c6043a31f8f4b9a8d7a044c79530b0"
RETAINED_RUNNER_SHA = "584e8ee937cb2d3ef62d59d3eb69a79bfb6afce23df86de3df305f81afae7c76"
BINDING = "tests/fixtures/typed_project_source_binding"
RETAINED = "tests/fixtures/typed_project_source_binding_byte_storage_v1/run.py"
ORCHESTRATION_CHANGED = frozenset(("src/cli.rs", "src/main.rs", "src/frontend/mod.rs",
                                  "src/frontend/driver.rs", "src/frontend/native.rs"))
PACKAGE_CHANGED = frozenset(("src/frontend/project.rs", "src/frontend/declaration_index.rs",
                            "src/frontend/declaration_index/sealed.rs",
                            "src/frontend/declaration_index/source_owner.rs",
                            "src/frontend/oir/project.rs", "src/frontend/oir/source.rs",
                            "src/frontend/oir/source/sealed.rs"))
CHANGED = ORCHESTRATION_CHANGED | PACKAGE_CHANGED
ADDED = frozenset(("src/project_cli.rs", "src/project_lock.rs", "src/frontend/project/packages.rs",
                   "src/frontend/project/packages/tests.rs"))
INSTRUMENTED = frozenset(("src/frontend/declaration_index.rs", "src/frontend/parser.rs",
                          "src/frontend/source.rs", "src/frontend/oir/mod.rs",
                          "src/frontend/project.rs"))
# This admission scanner is deliberately conservative, not a Rust evaluator.
# All existing expressions stay byte-identical, and no new include/path input is
# allowed, including multiline macros, alternate delimiters and cfg_attr paths.
GAP = rb'(?:\s|/\*.*?\*/|//[^\n]*(?:\n|$))*'
INCLUDE_START = re.compile(rb'\binclude(?:_str|_bytes)?' + GAP + rb'!', re.S)
INCLUDE = re.compile(INCLUDE_START.pattern + GAP
                     + rb'(?:\([^;]*?\)|\{[^;]*?\}|\[[^;]*?\])', re.S)
PATH_ATTRIBUTE = re.compile(rb'#' + GAP + rb'!?' + GAP + rb'\[[^\]]*\bpath\b[^\]]*\]', re.S)


def require(condition, message):
    if not condition:
        raise ValueError(message)


def sha(body):
    return hashlib.sha256(body).hexdigest()


def encoded(value):
    return (json.dumps(value, indent=2, sort_keys=True) + "\n").encode()


def identity(name, body):
    return {"path": name, "bytes": len(body), "sha256": sha(body)}


def directives(body):
    includes = INCLUDE.findall(body)
    require(len(includes) == len(INCLUDE_START.findall(body)), "incomplete compile-time include")
    return includes, PATH_ATTRIBUTE.findall(body)


def regular(root, name):
    path = root / name
    require(not path.is_symlink() and all(not p.is_symlink() for p in path.parents),
            "symlink source: " + name)
    before = path.stat()
    require(stat.S_ISREG(before.st_mode) and before.st_nlink == 1
            and not before.st_mode & 0o111 and before.st_size <= 2 * 1024 * 1024,
            "nonregular, executable, linked or oversized source: " + name)
    body = path.read_bytes()
    fields = ("st_dev", "st_ino", "st_mode", "st_nlink", "st_size", "st_mtime_ns", "st_ctime_ns")
    require(tuple(getattr(before, f) for f in fields)
            == tuple(getattr(path.stat(), f) for f in fields) and len(body) == before.st_size,
            "source changed during read: " + name)
    return body


def capture_sources(repo, baseline):
    """Record the reviewed project/package boundary; reject other input changes.

    Package namespace and lock orchestration changes are admitted explicitly. The retained assertions
    below still qualify only the old single-package semantics/resource cases;
    package graph behavior has separate compiler, CLI and real-application tests.
    Parser, ownership, OIR verification, lowering and runtime remain unchanged.
    """
    actual, directories = set(), set()
    for part in ("src", "native"):
        require((repo / part).is_dir() and not (repo / part).is_symlink(), "missing source root")
        for parent, dirs, files in os.walk(repo / part, followlinks=False):
            for name in dirs + files:
                path = Path(parent) / name
                require(not path.is_symlink(), "symlink source member")
                relative = path.relative_to(repo).as_posix()
                (directories if name in dirs else actual).add(relative)
    expected = {n for n in baseline if n.startswith(("src/", "native/"))} | ADDED
    require(actual == expected, "unexpected compiler source membership")
    expected_dirs = {str(p) for n in expected for p in PurePosixPath(n).parents
                     if str(p) not in (".", "src", "native")}
    require(directories == expected_dirs, "unexpected compiler source directory")
    inputs = {name: regular(repo, name) for name in sorted(set(baseline) | ADDED)}
    for name, before in baseline.items():
        if name not in CHANGED:
            require(inputs[name] == before, "unchanged semantic/resource input changed: " + name)
    # These files need no new compile-time inputs. Retain their exact include
    # and #[path] expressions; existing macro/fixture closure remains unchanged.
    for name in CHANGED | ADDED:
        require(directives(inputs[name]) == directives(baseline.get(name, b"")),
                "R1 compile-time input directive changed: " + name)
    return inputs


def git(repo, *args):
    result = subprocess.run(["git", "-C", str(repo), *args], capture_output=True, timeout=120)
    require(result.returncode == 0, "git command failed: " + " ".join(args)
            + ": " + result.stderr.decode(errors="replace")[-2000:])
    return result.stdout


def checkout_identity(repo, expected_head, allow_dirty):
    require(re.fullmatch("[0-9a-f]{40}", expected_head) is not None, "invalid expected head")
    head = git(repo, "rev-parse", "HEAD").decode().strip()
    require(head == expected_head, "checkout does not match expected head")
    status = git(repo, "status", "--porcelain=v1", "--untracked-files=all")
    require(allow_dirty or not status, "committed qualification requires a clean checkout")
    return {"head": head, "tree": git(repo, "rev-parse", "HEAD^{tree}").decode().strip(),
            "source_kind": "worktree" if status else "committed",
            "worktree_status_sha256": sha(status)}


def extract_baseline(repo, destination, fetch=False):
    exists = subprocess.run(["git", "-C", str(repo), "cat-file", "-e", BASELINE + "^{commit}"],
                            capture_output=True).returncode == 0
    if not exists and fetch:
        git(repo, "fetch", "--no-tags", "--depth=1", "https://github.com/YanagiKH/Oxid.git", BASELINE)
    require(git(repo, "rev-parse", BASELINE + "^{commit}").decode().strip() == BASELINE,
            "missing pinned baseline; use --fetch-baseline to fetch its exact public commit")
    destination.mkdir()
    archive = destination.with_suffix(".tar")
    with archive.open("xb") as stream:
        result = subprocess.run(["git", "-C", str(repo), "archive", "--format=tar", BASELINE],
                                stdout=stream, stderr=subprocess.PIPE, timeout=120)
    require(result.returncode == 0, "cannot archive pinned baseline")
    with tarfile.open(archive) as source:
        for member in source:
            name = PurePosixPath(member.name)
            require(not name.is_absolute() and ".." not in name.parts, "unsafe baseline archive path")
            path = destination.joinpath(*name.parts)
            if member.isdir():
                path.mkdir(parents=True, exist_ok=True)
            else:
                require(member.isfile(), "nonregular baseline archive member")
                path.parent.mkdir(parents=True, exist_ok=True)
                with source.extractfile(member) as incoming, path.open("xb") as target:
                    target.write(incoming.read())
                path.chmod(0o755 if member.mode & 0o111 else 0o644)
    archive.unlink()


def load(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def current_manifest(inputs, baseline, checkout):
    changes = [{"path": n, "before": identity(n, baseline[n]), "after": identity(n, b)}
               for n, b in sorted(inputs.items()) if n in baseline and b != baseline[n]]
    return {"schema": "oxid-project-workflow-current-source-v1", "baseline_commit": BASELINE,
            "checkout": checkout, "changes": changes, "additions": sorted(ADDED),
            "assertion_scope": "retained single-package Unit2 semantics and resources",
            "package_boundary_inputs": sorted(PACKAGE_CHANGED),
            "package_semantics_qualified_by_this_gate": False,
            "unchanged_baseline_inputs": len(baseline) - len(changes),
            "files": [identity(n, b) for n, b in sorted(inputs.items())]}


def verify_materialized(output, inputs):
    source = output / "unit2/run/source"
    assembly = json.loads((output / "unit2/run/assembly.json").read_bytes())
    changes = assembly["instrumentation"]
    require(len(changes) == len(INSTRUMENTED) and {r["path"] for r in changes} == INSTRUMENTED,
            "unexpected Unit2 instrumentation scope")
    for row in changes:
        require(row["before_sha256"] == sha(inputs[row["path"]]), "wrong instrumentation input")
        require(row["after_sha256"] == sha(regular(source, row["path"])), "changed instrumentation output")
    for name, body in inputs.items():
        if name not in INSTRUMENTED:
            require(regular(source, name) == body, "materialized current input changed: " + name)


def run(args):
    require(__debug__ and not sys.flags.optimize, "qualification refuses optimized Python")
    repo, output = args.repo.resolve(strict=True), args.output.resolve()
    require(output != repo and repo not in output.parents, "output must be outside repository")
    require(not output.exists(), "output must be new")
    output.mkdir(parents=True)
    report = {"schema": "oxid-project-workflow-current-result-v1", "status": "running",
              "scope": "retained single-package Unit2 semantics and resources; not package graph qualification",
              "semantic_pass": False, "compiler_executions": 0, "baseline_commit": BASELINE,
              "started_at_utc": datetime.datetime.now(datetime.timezone.utc).isoformat(),
              "controller": identity("scripts/verify_project_workflow_current.py", Path(__file__).read_bytes())}
    aliases = dict(sys.modules)
    try:
        checkout = checkout_identity(repo, args.expected_head, args.allow_dirty)
        baseline_root = output / "baseline"
        extract_baseline(repo, baseline_root, args.fetch_baseline)
        old_source = regular(baseline_root, BINDING + "/current-source.json")
        require(sha(old_source) == BASELINE_SOURCE_SHA, "wrong historical source manifest")
        original_body = regular(baseline_root, RETAINED)
        require(sha(original_body) == RETAINED_RUNNER_SHA, "wrong retained Unit2 predicates")
        historical = load("r1_historical_source_binding", baseline_root / BINDING / "run.py")
        captured = historical.preflight(baseline_root)
        original = load("r1_retained_unit2_assertions", baseline_root / RETAINED)
        inputs = capture_sources(repo, captured["inputs"])
        manifest = current_manifest(inputs, captured["inputs"], checkout)
        manifest_raw = encoded(manifest)
        manifest_path = output / "current-source.json"
        manifest_path.write_bytes(manifest_raw)
        report.update(checkout=checkout, current_source=identity("current-source.json", manifest_raw),
                      historical_source=identity(BINDING + "/current-source.json", old_source),
                      retained_assertions=identity(RETAINED, original_body),
                      current_input_members=len(inputs), historical_results_apply_to_current=False)
        seam = original.prepare_unit2(output, captured)
        (output / "resource-seam.json").write_bytes(encoded(seam))
        selected = copy.deepcopy(captured)
        # The separate source manifest is the existing compatibility seam. Other
        # historical captures, identities, comparators and expectations stay exact.
        selected["package_bytes"]["current-source.json"] = manifest_raw

        def unchanged(actual_repo, _capture):
            require(checkout_identity(actual_repo, args.expected_head, args.allow_dirty) == checkout,
                    "checkout changed during qualification")
            require(capture_sources(actual_repo, captured["inputs"]) == inputs,
                    "current source changed during qualification")
            require(manifest_path.read_bytes() == manifest_raw, "current manifest changed")

        original.assert_unchanged = unchanged
        original.CURRENT_SOURCE_SHA = sha(manifest_raw)
        unchanged(repo, selected)
        command = [sys.executable, "-B", seam["compatibility_runner"], "--repo", str(repo),
                   "--source-manifest", str(manifest_path), "--output", str(output / "unit2"),
                   "--cargo", args.cargo, "--rustc", args.rustc]
        if args.prepare_only:
            command.append("--prepare-only")
        env = dict(os.environ, CARGO_INCREMENTAL="0", CARGO_BUILD_JOBS="2", PYTHONDONTWRITEBYTECODE="1")
        env.pop("PYTHONOPTIMIZE", None)
        report["compiler_executions"] = 0 if args.prepare_only else "see retained Cargo command evidence"
        with (output / "runner.stdout").open("xb") as stdout, (output / "runner.stderr").open("xb") as stderr:
            completed = subprocess.run(command, cwd=repo, env=env, stdout=stdout, stderr=stderr, timeout=5400)
        (output / "command.json").write_bytes(encoded({"argv": command, "exit_status": completed.returncode,
            "stdout": identity("runner.stdout", (output / "runner.stdout").read_bytes()),
            "stderr": identity("runner.stderr", (output / "runner.stderr").read_bytes())}))
        require(completed.returncode == 0, "retained Unit2 gate failed; see runner.stderr and unit2 evidence")
        verified = original.verify_unit2_result(output, selected, seam, args.prepare_only)
        verify_materialized(output, inputs)
        if not args.prepare_only:
            verified.update(original.run_unit2_observer_controls(repo, output, selected, seam))
            original.verify_unit2_result(output, selected, seam, False)
            verify_materialized(output, inputs)
        unchanged(repo, selected)
        report.update(verified)
        report["status"] = "prepared" if args.prepare_only else "passed"
        report["finished_at_utc"] = datetime.datetime.now(datetime.timezone.utc).isoformat()
        artifact = "prepared.json" if args.prepare_only else "result.json"
        (output / artifact).write_bytes(encoded(report))
        print(json.dumps({"status": report["status"], "semantic_pass": report["semantic_pass"],
                          "source_kind": checkout["source_kind"], "output": str(output)}))
        return 0
    except (Exception, KeyboardInterrupt) as error:
        report.update(status="failed", semantic_pass=False, error=f"{type(error).__name__}: {error}")
        (output / "failure.json").write_bytes(encoded(report))
        print(report["error"], file=sys.stderr)
        return 1
    finally:
        for name in set(sys.modules) - set(aliases):
            sys.modules.pop(name, None)
        sys.modules.update(aliases)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--expected-head", required=True)
    parser.add_argument("--cargo", default="cargo")
    parser.add_argument("--rustc", default="rustc")
    parser.add_argument("--fetch-baseline", action="store_true", help="fetch only the fixed public baseline commit if missing")
    parser.add_argument("--allow-dirty", action="store_true", help="record local worktree evidence; never label it committed")
    parser.add_argument("--prepare-only", action="store_true", help="exercise admission/assembly with zero compiler executions")
    return run(parser.parse_args())


if __name__ == "__main__":
    raise SystemExit(main())
