#!/usr/bin/env python3
"""Replay retained narrow assertions after checked byte arithmetic.

The complete 128-bit predecessor and thirteen preceding source policies
remain byte-authenticated. Bit semantics use separate public/native oracles.
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

PREDECESSOR = '249017cdf702c884fbd6b8a26196ce2664ad12ed'
PREDECESSOR_POLICY = "scripts/verify_integer128_compatibility.py"
PREDECESSOR_POLICY_SHA = '289338a3b6f7adde01c38a43e7676d9378d72b52bf61cd5868449ff6e4a98796'
PREDECESSOR_POLICY_BYTES = 33371
INTEGER_CHANGED = frozenset(('src/frontend/ast.rs', 'src/frontend/declaration_index/source_owner.rs', 'src/frontend/format.rs', 'src/frontend/format/ast_tests.rs', 'src/frontend/hir.rs', 'src/frontend/oir/execute.rs', 'src/frontend/oir/mod.rs', 'src/frontend/oir/native.rs', 'src/frontend/oir/native_emit_work.rs', 'src/frontend/oir/owned/execute.rs', 'src/frontend/oir/owned/native.rs', 'src/frontend/oir/owned/plan.rs', 'src/frontend/oir/owned/source/array_pipeline_rows.rs', 'src/frontend/oir/owned/source/candidate_adapter.rs', 'src/frontend/oir/owned/source/hir.rs', 'src/frontend/oir/owned/source/hir_budget.rs', 'src/frontend/oir/owned/source/lower.rs', 'src/frontend/oir/owned/source/mod.rs', 'src/frontend/oir/owned/source/resolve.rs', 'src/frontend/oir/owned/source/typeck.rs', 'src/frontend/oir/source/association.rs', 'src/frontend/oir/source/hir_import/candidate.rs', 'src/frontend/oir/source/sealed.rs', 'src/frontend/oir/verify.rs', 'src/frontend/parser.rs'))
INTEGER_CHANGED = INTEGER_CHANGED | frozenset(('tests/typed_frontend.rs',))

INTEGER_ADDED = frozenset(('src/frontend/oir/owned/execute_bitwise.rs', 'src/frontend/oir/owned/native_bitwise.rs', 'src/frontend/oir/owned/source/integer_bit_tests.rs'))
INTEGER_INPUTS = {'src/frontend/ast.rs': (47347,
                         'c702910de5adee6c38d76bf002f7443ae853ed36e3e1481b778c1e61332a8913'),
 'src/frontend/declaration_index/source_owner.rs': (23168,
                                                    '7d03b00ddc64cc08ef6f4968e6d87427a82159684a7c1e0e85a1c2fac2572a80'),
 'src/frontend/format.rs': (49986,
                            '22e8dcb94cf8d9be40d29bfebafd58130f892147478453502967f6c2bc9278b0'),
 'src/frontend/format/ast_tests.rs': (67090,
                                      '0b11f57e7aaf939737eabe3afc8062ea540d80cb9355217430d2399300d1d158'),
 'src/frontend/hir.rs': (30506,
                         '9fd888b1200b95e22015de5bbe0effb5a26cb6a33b3198951b9207e6ee17c07d'),
 'src/frontend/oir/execute.rs': (22462,
                                 '583bc4b5c1b01f82c9b5249947f3431cdacc7f22b781f31f946b9190602438d9'),
 'src/frontend/oir/mod.rs': (22151,
                             '05efe262bfccc64ff440128c7a88c9c8ceb3c02077cf307baa3dc85b6bd7d64c'),
 'src/frontend/oir/native.rs': (106503,
                                '9519d3b7343be4a4f795a29a4db258dcda73216201fd118dc5e6a75fcadd39ad'),
 'src/frontend/oir/native_emit_work.rs': (29127,
                                          '89f261869b0d385b693da6739f9f46f37a7e68897c4f74dde8d69f50c35cf03f'),
 'src/frontend/oir/owned/execute.rs': (187290,
                                       '62401351e4b613410fafc332967d73614ba604ae5b014d529d25e40d42fbc840'),
 'src/frontend/oir/owned/execute_bitwise.rs': (2646,
                                               '95ca88ca588a9c88ad3f5aa220bc97f8a8fbe1356a4596fb3640620f8b87e578'),
 'src/frontend/oir/owned/native.rs': (231205,
                                      '8bbd846116e77f71cf42da28f297599d68c33ab9f0d540e6f17bc98a1a6f2760'),
 'src/frontend/oir/owned/native_bitwise.rs': (2203,
                                              'a1904bd22020b7d10fc8bc268c113b4cae2708938afe935d658fad3c12b860f2'),
 'src/frontend/oir/owned/plan.rs': (30045,
                                    '696f7f1067caa60f7b13d51251c49d690c83cf25ac689b6b41afecfeac71455b'),
 'src/frontend/oir/owned/source/array_pipeline_rows.rs': (55383,
                                                          '2b146dbc6f66b37033608ab95162a4c97a86fae539ff6198d20ad4ef5c5c4e1a'),
 'src/frontend/oir/owned/source/candidate_adapter.rs': (43173,
                                                        'e400f40cd26e1c6b2635aefca214a89ef3e892f528c5359c1907414caf307c64'),
 'src/frontend/oir/owned/source/hir.rs': (6600,
                                          '995f64c4e720f4da61601eab5acd3d83c1c4918be9fe04348474338cfd0da00d'),
 'src/frontend/oir/owned/source/hir_budget.rs': (56969,
                                                 'c4283ae598835432ee90dc5b999df6fc2e0f3d0de4d3b2c0706161398231725a'),
 'src/frontend/oir/owned/source/integer_bit_tests.rs': (3318,
                                                        '9b0406dd66f32a462350d54fef9015082e7a911cc6300539a70de668832e85bc'),
 'src/frontend/oir/owned/source/lower.rs': (115764,
                                            'eb493f9bb7c4864716644b59dc82b9f69ae24a7100c76fed3a90ea5e87e4a22f'),
 'src/frontend/oir/owned/source/mod.rs': (2084,
                                          '83538c41c07cfa2dd20f06b24d89a2790db52d6e9aea9b003069486076f0c4d6'),
 'src/frontend/oir/owned/source/resolve.rs': (145842,
                                              'cd6f68215f3965efc185685d4ce970fc861dfad0135dad5b0add1c64fc9cda0b'),
 'src/frontend/oir/owned/source/typeck.rs': (212012,
                                             '0db3ff01fba6845e7bfea233c8899d0ce8e7cd6144313c1b45be7176eba4364d'),
 'src/frontend/oir/source/association.rs': (31989,
                                            '4b98234789a09c1029e528f94f78009cc1bbb4052a89672aa36c2148e87e2986'),
 'src/frontend/oir/source/hir_import/candidate.rs': (52740,
                                                     '4137e3a019be200a4763a922a6d49fd73aab15778a0d395538a01bf744c30f4f'),
 'src/frontend/oir/source/sealed.rs': (23587,
                                       'dd91ef95b2d638fc8f26fe5e3953eca03c89749ffd7f3f0e1b339bb19bed90f6'),
 'src/frontend/oir/verify.rs': (50863,
                                '257f5a4a9936e66b935b71786dc4f4f6e138a3892eaf0cf6ece03cc92e111f7c'),
 'src/frontend/parser.rs': (80916,
                            '190bcfd9ab9b1955d41763f021cb5b97f4411119f3a4470258d91f5894107bee')}
INTEGER_INPUTS['tests/typed_frontend.rs'] = (16668, '064513643ce241af11ff8f5574cd25b253a22d1f5d97fe2cdaf049d3b1361153')

REVIEWED_DIRECTIVES = {'src/frontend/oir/owned/execute.rs': ([],
                                       [b'#[path = "execute_values.rs"]',
                                        b'#[path = "execute_bitwise.rs"]',
                                        b'#[path = "execute_integer128.rs"]',
                                        b'#[path = "execute_integers.rs"]',
                                        b'#[path = "execute_experimental.rs"]',
                                        b'#[path = "execute_resources.rs"]',
                                        b'#[path = "array_observe.rs"]',
                                        b'#[path = "execute_tests.rs"]',
                                        b'#[path = "builtin_output_reference_t'
                                        b'ests.rs"]',
                                        b'#[path = "byte_storage_codec_tests.r'
                                        b's"]']),
 'src/frontend/oir/owned/execute_bitwise.rs': ([], []),
 'src/frontend/oir/owned/native.rs': ([],
                                      [b'#[path = "native_bitwise.rs"]',
                                       b'#[path = "native_experimental.rs"]',
                                       b'#[path = "native_integer128.rs"]',
                                       b'#[path = "native_integers.rs"]',
                                       b'#[path = "native_numeric.rs"]',
                                       b'#[path = "native_resources.rs"]',
                                       b'#[path = "builtin_input_native_tests'
                                       b'.rs"]',
                                       b'#[path = "builtin_output_native_test'
                                       b's.rs"]',
                                       b'#[path = "native_tests.rs"]',
                                       b'#[path = "native_inventory_admission'
                                       b'_tests.rs"]',
                                       b'#[path = "byte_storage_native_tests.'
                                       b'rs"]']),
 'src/frontend/oir/owned/native_bitwise.rs': ([], []),
 'src/frontend/oir/owned/source/integer_bit_tests.rs': ([], [])}

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
            "changed pinned integer-width source policy")
    module = types.ModuleType("_integer_width_compatibility_parent")
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
PARENT_ADDED = PREDECESSOR_ADDED | INTEGER_ADDED
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
run_i64_observer_controls = library.run_i64_observer_controls
run_integer_width_observer_controls = library.run_integer_width_observer_controls
run_integer128_observer_controls = library.run_integer128_observer_controls


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
    expected = expected_parent | INTEGER_ADDED
    require(set(parent) == expected_parent, "wrong predecessor compiler input membership")
    require(set(INTEGER_INPUTS) == INTEGER_CHANGED | INTEGER_ADDED
            and INTEGER_CHANGED <= expected_parent and not INTEGER_ADDED & expected_parent,
            "wrong reviewed integer-bit input membership")
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
        if name not in INTEGER_CHANGED:
            require(inputs[name] == before, "unreviewed change outside integer-bit boundary: " + name)
    for name, (length, digest) in INTEGER_INPUTS.items():
        require(len(inputs[name]) == length and sha(inputs[name]) == digest,
                "integer-bit source does not match reviewed input: " + name)
        require(directives(inputs[name]) == REVIEWED_DIRECTIVES.get(name, directives(parent[name]) if name in parent else ([], [])),
                "integer-bit compile-time input directive changed: " + name)
    return inputs


def current_manifest(inputs, baseline, checkout, parent):
    changes = [{"path": name, "before": identity(name, baseline[name]), "after": identity(name, body)}
               for name, body in sorted(inputs.items()) if name in baseline and body != baseline[name]]
    result = {"schema": "oxid-project-workflow-current-source-v1", "baseline_commit": BASELINE,
              "checkout": checkout, "changes": changes, "additions": sorted(PARENT_ADDED),
              "assertion_scope": "retained bounded scalar/record semantics and resources",
              "source_policy_parent": PREDECESSOR, "source_policy_inputs": PREDECESSOR_POLICIES,
              "integer_bit_reviewed_inputs": [{"path": name, "bytes": length, "sha256": digest}
                                          for name, (length, digest) in sorted(INTEGER_INPUTS.items())],
              "integer_bit_changes": [{"path": name, "before": identity(name, parent[name]),
                                  "after": identity(name, inputs[name])} for name in sorted(INTEGER_CHANGED)],
              "unchanged_baseline_inputs": len(baseline) - len(changes),
              "files": [identity(name, body) for name, body in sorted(inputs.items())]}
    for name in ("package", "workspace", "native_reproducibility", "native_optimization", "owned_enum",
                 "byte_field", "byte_buffer", "binary_io", "native_heap_safety", "hosted_execution",
                 "owned_buffer_transfer", "byte_string", "native_file", "program_arguments", "i64", "integer_widths", "byte_integer", "integer128", "integer_bits"):
        key = name + ("_qualified_by_this_gate" if name.startswith("native_") and name != "native_file" else "_semantics_qualified_by_this_gate")
        result[key] = False
    return result


def run(args):
    require(__debug__ and not sys.flags.optimize, "qualification refuses optimized Python")
    repo, output = args.repo.resolve(strict=True), args.output.resolve()
    require(Path(__file__).resolve() == repo / "scripts/verify_integer_bit_compatibility.py",
            "run the controller from the selected checkout")
    require(output != repo and repo not in output.parents, "output must be outside repository")
    require(not output.exists(), "output must be new")
    output.mkdir(parents=True)
    report = {"schema": "oxid-project-workflow-current-result-v1", "status": "running",
              "scope": "retained bounded scalar/record semantics and resources on the integer-bit successor; bit operations qualified separately",
              "semantic_pass": False, "compiler_executions": 0, "baseline_commit": BASELINE,
              "started_at_utc": datetime.datetime.now(datetime.timezone.utc).isoformat(),
              "controller": identity("scripts/verify_integer_bit_compatibility.py", Path(__file__).read_bytes())}
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
            verified.update(run_i64_observer_controls(repo, output, selected, seam, original, observer_adapter))
            verified.update(run_integer_width_observer_controls(repo, output, selected, seam, original, observer_adapter))
            verified.update(run_integer128_observer_controls(repo, output, selected, seam, original, observer_adapter))
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
