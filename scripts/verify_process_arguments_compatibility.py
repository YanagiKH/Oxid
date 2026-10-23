#!/usr/bin/env python3
"""Replay retained bounded semantics and resource assertions after program arguments.

The complete File predecessor and observer stay authenticated and unchanged.
Only the exact reviewed argument/compiler delta may differ. Argument semantics
are established separately through public CLI and source-free native execution.
"""
import argparse
import copy
import datetime
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import stat
import subprocess
import sys
import types

PREDECESSOR = '3dd343e0c3aa0bb77c392c8985479facf08ead17'
PREDECESSOR_POLICY = 'scripts/verify_native_file_compatibility.py'
PREDECESSOR_POLICY_SHA = 'a3217cd056aeba40953b42f505227e46d14f8cd745b9f28236c880b9cd636ea1'
PREDECESSOR_POLICY_BYTES = 30148
ARGUMENT_CHANGED = frozenset(('src/cli.rs', 'src/frontend/builtin_catalog.rs', 'src/frontend/declaration_index/experimental_tests.rs', 'src/frontend/declaration_index/sealed.rs', 'src/frontend/experimental_builtins.rs', 'src/frontend/native.rs', 'src/frontend/oir/owned/execute.rs', 'src/frontend/oir/owned/execute_experimental.rs', 'src/frontend/oir/owned/execute_io_delivery.rs', 'src/frontend/oir/owned/native.rs', 'src/frontend/oir/owned/native_experimental.rs', 'src/frontend/oir/owned/plan.rs', 'src/frontend/oir/owned/source/experimental_source_tests.rs'))
ARGUMENT_ADDED = frozenset(('native/experimental_arguments.c', 'src/frontend/oir/owned/source/process_arguments_tests.rs', 'src/program_arguments.rs'))
ARGUMENT_INPUTS = {'native/experimental_arguments.c': (2671,
                                     '5d8268c9de310a088b466810641341265eeca1b27dfd09f760e4fc4459c03498'),
 'src/cli.rs': (1352, '13fd88ed2bb75d25b45f83033ff3c843c0228c4bcaf376741d53bd71a4bd33ae'),
 'src/frontend/builtin_catalog.rs': (14035,
                                     'e3dcf4d97277d6dc9b97b2a0f555161edd9c6c9c34682fda7441866e746f1242'),
 'src/frontend/declaration_index/experimental_tests.rs': (3492,
                                                          'eb48d1d19efca1ba49d7c965527c269f3f3f1da19cd5d251bb3faff16c2f92ce'),
 'src/frontend/declaration_index/sealed.rs': (82662,
                                              '10e9fc8e2d79c70ff8d0ab939b6f2748a93c02ead54cf221d36531af01afc2f5'),
 'src/frontend/experimental_builtins.rs': (17791,
                                           '2f19fdefe19a434abb128a5225ab82ff3040c598c5ba1c41c1f38acbf41db906'),
 'src/frontend/native.rs': (17703, 'e2069b4fdfefbce9033d3df099013cb92f63dd14bc3983baa963f45d9e94378e'),
 'src/frontend/oir/owned/execute.rs': (178542,
                                       '9821c344fa2145c6b78d2fb732f0cee014558bce93926e673fbcc82dfafe87fe'),
 'src/frontend/oir/owned/execute_experimental.rs': (24585,
                                                    '1037d60c876a9cd42065fa597c100946b641c6fcfa7c0f24ec7dca1e50d491c6'),
 'src/frontend/oir/owned/execute_io_delivery.rs': (12314,
                                                   '8e422571a50b0dbebb77ad3c345cd18d61195b4f54b145da8db4cebfa164f71d'),
 'src/frontend/oir/owned/native.rs': (213632,
                                      'a209825db1de94cc7de37e913402e45a34b0bfa68fc3e785bfb3a7a733e87547'),
 'src/frontend/oir/owned/native_experimental.rs': (16065,
                                                   'ececcbd97e09d207a3d2680ea0e5789b40b1efea7c8fb618c218bc25854cfe3b'),
 'src/frontend/oir/owned/plan.rs': (25940,
                                    '7b4a73a93ea5c23cba25fd6a00fe1824e66b15f464162c92ebe5896f85527f03'),
 'src/frontend/oir/owned/source/experimental_source_tests.rs': (24708,
                                                                '20a6af7ee55e42a89bb9ea7e0339b9ac2837618a8c54e622d999e60bcf493668'),
 'src/frontend/oir/owned/source/process_arguments_tests.rs': (4901,
                                                              '502fcb157c51b01e8aa6296431031515fc84b665c3eaee50e0052050c7176270'),
 'src/program_arguments.rs': (14053, 'dbee1a96d8d269665e3c2844fad0a059b28c17e77f3cf8d5a1255c5df934b39d')}
REVIEWED_DIRECTIVES = {'native/experimental_arguments.c': ([], []),
 'src/frontend/native.rs': ([b'include_str!("../../native/typed_preview.c")',
                             b'include_str!("../../native/typed_bytebuf.c")',
                             b'include_str!("../../native/experimental_bytebuf.c")',
                             b'include_str!("../../native/experimental_files.c")',
                             b'include_str!("../../native/experimental_arguments.c")',
                             b'include_str!("../../native/experimental_bytebuf.h")'],
                            []),
 'src/frontend/oir/owned/source/experimental_source_tests.rs': ([],
                                                                [b'#[path = "io_delivery_source_tests.rs"]',
                                                                 b'#[path = "process_arguments_tests.rs"]']),
 'src/frontend/oir/owned/source/process_arguments_tests.rs': ([], []),
 'src/program_arguments.rs': ([], [])}

def require(condition, message):
    if not condition:
        raise ValueError(message)


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


def load_predecessor(repo):
    body = regular(repo, PREDECESSOR_POLICY)
    require(len(body) == PREDECESSOR_POLICY_BYTES and hashlib.sha256(body).hexdigest() == PREDECESSOR_POLICY_SHA,
            "changed pinned native-file source policy")
    module = types.ModuleType("_program_arguments_compatibility_parent")
    module.__file__ = str(repo / PREDECESSOR_POLICY)
    exec(compile(body, module.__file__, "exec"), module.__dict__)
    return module


library = load_predecessor(Path(__file__).resolve().parents[1])
for _name in ("BASELINE", "BASELINE_SOURCE_SHA", "RETAINED_RUNNER_SHA", "BINDING", "RETAINED", "OBSERVER_ADAPTER",
              "sha", "encoded", "identity", "load", "git", "checkout_identity", "extract_baseline",
              "authenticate_observer_adapter", "prepare_current_observer", "verify_current_observer",
              "verify_materialized", "run_byte_buffer_observer_controls"):
    globals()[_name] = getattr(library, _name)
PREDECESSOR_ADDED = library.PARENT_ADDED
PARENT_ADDED = PREDECESSOR_ADDED | ARGUMENT_ADDED
directives = library.directives
PREDECESSOR_POLICIES = {**library.PREDECESSOR_POLICIES, PREDECESSOR_POLICY: PREDECESSOR_POLICY_SHA}


def authenticate_policies(repo, parent):
    for name, expected in PREDECESSOR_POLICIES.items():
        before = regular(parent, name)
        require(sha(before) == expected, "wrong pinned predecessor source policy: " + name)
        require(regular(repo, name) == before, "predecessor source policy changed: " + name)


OBSERVER_ADAPTER = library.OBSERVER_ADAPTER
authenticate_observer_adapter = library.authenticate_observer_adapter
run_file_observer_controls = library.run_file_observer_controls


def capture_parent(repo, parent, names):
    objects = {}
    for row in git(repo, "ls-tree", "-rzl", "--full-tree", PREDECESSOR).split(b"\0"):
        if not row:
            continue
        metadata, name = row.split(b"\t", 1)
        mode, kind, object_id, length = metadata.split()
        require(kind == b"blob" and name not in objects, "invalid parent blob roster")
        objects[name] = (mode, object_id, length)
    result = {}
    for name in sorted(names):
        body = regular(parent, name)
        mode, object_id, length = objects[name.encode()]
        blob = b"blob " + str(len(body)).encode() + b"\0" + body
        require(mode == b"100644" and int(length) == len(body)
                and hashlib.sha1(blob, usedforsecurity=False).hexdigest().encode() == object_id,
                "parent compiler input differs from pinned Git blob: " + name)
        result[name] = body
    return result


def capture_sources(repo, baseline, parent):
    expected_parent = set(baseline) | PREDECESSOR_ADDED
    expected = expected_parent | ARGUMENT_ADDED
    require(set(parent) == expected_parent, "wrong predecessor compiler input membership")
    require(set(ARGUMENT_INPUTS) == ARGUMENT_CHANGED | ARGUMENT_ADDED
            and ARGUMENT_CHANGED <= expected_parent and not ARGUMENT_ADDED & expected_parent,
            "wrong reviewed program-argument input membership")
    actual, directories = set(), set()
    for part in ("src", "native"):
        require((repo / part).is_dir() and not (repo / part).is_symlink(), "missing source root")
        for directory, dirs, files in os.walk(repo / part, followlinks=False):
            for name in dirs + files:
                path = Path(directory) / name
                require(not path.is_symlink(), "symlink source member")
                (directories if name in dirs else actual).add(path.relative_to(repo).as_posix())
    compiler = {name for name in expected if name.startswith(("src/", "native/"))}
    require(actual == compiler, "unexpected compiler source membership")
    expected_dirs = {str(path) for name in compiler for path in PurePosixPath(name).parents
                     if str(path) not in (".", "src", "native")}
    require(directories == expected_dirs, "unexpected compiler source directory")
    inputs = {name: regular(repo, name) for name in sorted(expected)}
    for name, before in parent.items():
        if name not in ARGUMENT_CHANGED:
            require(inputs[name] == before, "unreviewed change outside program-argument boundary: " + name)
    for name, (length, digest) in ARGUMENT_INPUTS.items():
        require(len(inputs[name]) == length and sha(inputs[name]) == digest,
                "program-argument source does not match reviewed input: " + name)
        require(directives(inputs[name]) == REVIEWED_DIRECTIVES.get(name, directives(parent[name]) if name in parent else ([], [])),
                "program-argument compile-time input directive changed: " + name)
    return inputs


def current_manifest(inputs, baseline, checkout, parent):
    changes = [{"path": name, "before": identity(name, baseline[name]), "after": identity(name, body)}
               for name, body in sorted(inputs.items()) if name in baseline and body != baseline[name]]
    result = {"schema": "oxid-project-workflow-current-source-v1", "baseline_commit": BASELINE,
              "checkout": checkout, "changes": changes, "additions": sorted(PARENT_ADDED),
              "assertion_scope": "retained bounded scalar/record semantics and resources",
              "source_policy_parent": PREDECESSOR, "source_policy_inputs": PREDECESSOR_POLICIES,
              "program_arguments_reviewed_inputs": [{"path": name, "bytes": length, "sha256": digest}
                                          for name, (length, digest) in sorted(ARGUMENT_INPUTS.items())],
              "program_arguments_changes": [{"path": name, "before": identity(name, parent[name]),
                                  "after": identity(name, inputs[name])} for name in sorted(ARGUMENT_CHANGED)],
              "unchanged_baseline_inputs": len(baseline) - len(changes),
              "files": [identity(name, body) for name, body in sorted(inputs.items())]}
    for name in ("package", "workspace", "native_reproducibility", "native_optimization", "owned_enum",
                 "byte_field", "byte_buffer", "binary_io", "native_heap_safety", "hosted_execution",
                 "owned_buffer_transfer", "byte_string", "native_file", "program_arguments"):
        key = name + ("_qualified_by_this_gate" if name.startswith("native_") and name != "native_file" else "_semantics_qualified_by_this_gate")
        result[key] = False
    return result


def run(args):
    require(__debug__ and not sys.flags.optimize, "qualification refuses optimized Python")
    repo, output = args.repo.resolve(strict=True), args.output.resolve()
    require(Path(__file__).resolve() == repo / "scripts/verify_process_arguments_compatibility.py",
            "run the controller from the selected checkout")
    require(output != repo and repo not in output.parents, "output must be outside repository")
    require(not output.exists(), "output must be new")
    output.mkdir(parents=True)
    report = {"schema": "oxid-project-workflow-current-result-v1", "status": "running",
              "scope": "retained bounded scalar/record semantics and resources on the program-argument successor; program arguments qualified separately",
              "semantic_pass": False, "compiler_executions": 0, "baseline_commit": BASELINE,
              "started_at_utc": datetime.datetime.now(datetime.timezone.utc).isoformat(),
              "controller": identity("scripts/verify_process_arguments_compatibility.py", Path(__file__).read_bytes())}
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
        predecessor_root = output / "predecessor"
        extract_baseline(repo, predecessor_root, args.fetch_baseline, PREDECESSOR)
        authenticate_policies(repo, predecessor_root)
        observer_adapter, observer_adapter_body = authenticate_observer_adapter(repo)
        predecessor_inputs = capture_parent(repo, predecessor_root, set(captured["inputs"]) | PREDECESSOR_ADDED)
        inputs = capture_sources(repo, captured["inputs"], predecessor_inputs)
        manifest = current_manifest(inputs, captured["inputs"], checkout, predecessor_inputs)
        manifest_raw = encoded(manifest)
        manifest_path = output / "current-source.json"
        manifest_path.write_bytes(manifest_raw)
        report.update(checkout=checkout, current_source=identity("current-source.json", manifest_raw),
                      historical_source=identity(BINDING + "/current-source.json", old_source),
                      retained_assertions=identity(RETAINED, original_body),
                      current_input_members=len(inputs), historical_results_apply_to_current=False)
        selected, seam = prepare_current_observer(original, output, captured, observer_adapter)
        report["byte_buffer_observer_adapter"] = observer_adapter.descriptor()
        report["byte_buffer_observer_adapter_controller"] = identity(OBSERVER_ADAPTER, observer_adapter_body)
        (output / "resource-seam.json").write_bytes(encoded(seam))
        # The separate source manifest is the existing compatibility seam. Other
        # historical captures, identities, comparators and expectations stay exact.
        selected["package_bytes"]["current-source.json"] = manifest_raw

        def unchanged(actual_repo, _capture):
            require(checkout_identity(actual_repo, args.expected_head, args.allow_dirty) == checkout,
                    "checkout changed during qualification")
            authenticate_policies(actual_repo, predecessor_root)
            require(capture_parent(actual_repo, predecessor_root, predecessor_inputs) == predecessor_inputs,
                    "pinned parent compiler materialization changed")
            require(authenticate_observer_adapter(actual_repo)[1] == observer_adapter_body,
                    "ByteBuf observer adapter changed during qualification")
            require(capture_sources(actual_repo, captured["inputs"], predecessor_inputs) == inputs,
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
        verify_current_observer(output, observer_adapter, selected["observer"])
        if not args.prepare_only:
            verified.update(original.run_unit2_observer_controls(repo, output, selected, seam))
            verified.update(run_byte_buffer_observer_controls(repo, output, selected, seam, original, observer_adapter))
            verified.update(run_file_observer_controls(repo, output, selected, seam, original, observer_adapter))
            original.verify_unit2_result(output, selected, seam, False)
            verify_materialized(output, inputs)
            verify_current_observer(output, observer_adapter, selected["observer"])
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
