#!/usr/bin/env python3
"""Replay retained bounded assertions after integer range iteration.

The Linux-native predecessor and twenty-eight preceding policies remain
byte-authenticated. Range-loop semantics have separate public/native checks.
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

PREDECESSOR = 'f262780d0cfb7cbe5207e51494eca5e7fdd43903'
PREDECESSOR_POLICY = 'scripts/verify_linux_native_compatibility.py'
PREDECESSOR_POLICY_SHA = '12f07665dc7984e34aeb8e4de4b118fab1b0888aa6164446d32c4996fea6fe23'
PREDECESSOR_POLICY_BYTES = 40199
RANGE_FOR_CHANGED = frozenset(('src/frontend/ast.rs', 'src/frontend/declaration_index/generics.rs', 'src/frontend/declaration_index/generics/flow.rs', 'src/frontend/declaration_index/sealed.rs', 'src/frontend/format/ast_tests.rs', 'src/frontend/hir.rs', 'src/frontend/oir/owned/source/array_pipeline_rows.rs', 'src/frontend/oir/owned/source/hir.rs', 'src/frontend/oir/owned/source/hir_budget.rs', 'src/frontend/oir/owned/source/lower.rs', 'src/frontend/oir/owned/source/mod.rs', 'src/frontend/oir/owned/source/record_pattern_tests.rs', 'src/frontend/oir/owned/source/resolve.rs', 'src/frontend/oir/owned/source/resolver_storage.rs', 'src/frontend/oir/owned/source/type_storage.rs', 'src/frontend/oir/owned/source/typeck.rs', 'src/frontend/oir/source/hir_import.rs', 'src/frontend/oir/source/hir_import/candidate.rs', 'src/frontend/oir/source/sealed.rs', 'src/frontend/parser.rs'))
RANGE_FOR_ADDED = frozenset(('src/frontend/oir/owned/source/range_for_tests.rs',))
RANGE_FOR_INPUTS = {'src/frontend/ast.rs': (58805, 'c8fbca9e7f1714dceb5c8e4d10ccdfddf6e6e0e15c1ba771f9c821bb61c83ea4'),
 'src/frontend/declaration_index/generics.rs': (107552,
                                                'ba5d7f9a92ef94b2f1ff3b24b267c4478d62bcf1ae78cc1b8962ad88b17e21a7'),
 'src/frontend/declaration_index/generics/flow.rs': (3269,
                                                     '489f8a649576ad9cdec3a0a770c6258ce069b0c5ac45ffaaa93b2b9cf1bc8ea1'),
 'src/frontend/declaration_index/sealed.rs': (87731,
                                              '8bc30d262e5b63ea697f1f3f78156e38ea506aa8398e9534032aa969c54b959d'),
 'src/frontend/format/ast_tests.rs': (75757,
                                      'ad79d9ab2fe6f59e8a680f3f85c0bc4ea9af3507d8ca3d2683b20ef5d58617e9'),
 'src/frontend/hir.rs': (32946, 'a194d05c9da239c1a5ffa064faa0337aeebc1e69d9194de9b153184010e59b0a'),
 'src/frontend/oir/owned/source/array_pipeline_rows.rs': (56297,
                                                          '9f3a38929881da8e750af643fd20a322a8138883baeff4db14072e38a16a33ca'),
 'src/frontend/oir/owned/source/hir.rs': (7272,
                                          '1f98409fe3c3b7b7bc6a6c93f85f63f5a0cd868aafad56f900da66f5fc14e6e0'),
 'src/frontend/oir/owned/source/hir_budget.rs': (58023,
                                                 'd0d5ea18fba87b8abfa575e67aa678057f48e3344e4c71e4ed0502f3590cad01'),
 'src/frontend/oir/owned/source/lower.rs': (128386,
                                            'a659b91a36395c9db539c88b8230d7b9f4c1b084934087d1cda4207078d717fb'),
 'src/frontend/oir/owned/source/mod.rs': (2291,
                                          'ca63ce1e979eba379166b205549ab6b58fcfad16b00e3bbd24b72b3d1aeedbd5'),
 'src/frontend/oir/owned/source/range_for_tests.rs': (5966,
                                                      '1f676497a618d5337e88d38c154e28fc2ff67cfce22f6f10cc44dde941e040ae'),
 'src/frontend/oir/owned/source/record_pattern_tests.rs': (11832,
                                                           'dd193db6c57ba1e3ffc6c82aeb1f897b0565166ec5bb8810571cd0b19e2ac648'),
 'src/frontend/oir/owned/source/resolve.rs': (161652,
                                              '608b8940142dd5e2e80a277bf3e32736d609ab2b4f510a06e2b1cf00b16cd5b3'),
 'src/frontend/oir/owned/source/resolver_storage.rs': (30530,
                                                       '9ce4616f4815f6fb703d68b3d5d62959980beb2f3b7e9b16521c3a20647d9f75'),
 'src/frontend/oir/owned/source/type_storage.rs': (68397,
                                                   '1c03435cccbc1116fb41f1f1aadf73570940f9de1f1ccb7d3569e680087c21e6'),
 'src/frontend/oir/owned/source/typeck.rs': (219468,
                                             '6d8867f2579f049845be919756ef0ed4d075c818ece3412181c2d032b57dadf7'),
 'src/frontend/oir/source/hir_import.rs': (17496,
                                           '02b7cd05f2f2e01769f98bdc54594919c8220e29076c82bd7a318a34d6f7524b'),
 'src/frontend/oir/source/hir_import/candidate.rs': (52989,
                                                     '108849d469f71e6550bc628fdb4f9d368742bf44895a0edfd11d26f17d32e086'),
 'src/frontend/oir/source/sealed.rs': (23696,
                                       'b262584cfae959ab60a7677ae64022cef86dba5809b049fb059e8b3170cf7b31'),
 'src/frontend/parser.rs': (103843, '9ae022f5de81483f4e3a163db4e1bb18ee2f90beb17713ce1a6945660fdc8d0d')}
REVIEWED_DIRECTIVES = {'src/frontend/ast.rs': ([], []),
 'src/frontend/declaration_index/generics.rs': ([], []),
 'src/frontend/declaration_index/generics/flow.rs': ([], []),
 'src/frontend/declaration_index/sealed.rs': ([], []),
 'src/frontend/format/ast_tests.rs': ([], []),
 'src/frontend/hir.rs': ([], []),
 'src/frontend/oir/owned/source/array_pipeline_rows.rs': ([], []),
 'src/frontend/oir/owned/source/hir.rs': ([], []),
 'src/frontend/oir/owned/source/hir_budget.rs': ([],
                                                 [b'#[path = "byte_storage_resources.rs"]',
                                                  b'#[path = "hir_budget_tests.rs"]']),
 'src/frontend/oir/owned/source/lower.rs': ([], []),
 'src/frontend/oir/owned/source/mod.rs': ([], []),
 'src/frontend/oir/owned/source/range_for_tests.rs': ([], []),
 'src/frontend/oir/owned/source/record_pattern_tests.rs': ([], []),
 'src/frontend/oir/owned/source/resolve.rs': ([],
                                              [b'#[path = "resolver_paid_tests.rs"]',
                                               b'#[path = "resolver_enum_tests.rs"]',
                                               b'#[path = "enum_storage_failure_tests.rs"]',
                                               b'#[path = "builtin_signature_tests.rs"]',
                                               b'#[path = "output_typing_tests.rs"]',
                                               b'#[path = "byte_storage_type_tests.rs"]']),
 'src/frontend/oir/owned/source/resolver_storage.rs': ([],
                                                       [b'#[path = "resolver_storage_tests.rs"]',
                                                        b'#[path = "resolver_inventory_tests.rs"]']),
 'src/frontend/oir/owned/source/type_storage.rs': ([], [b'#[path = "type_storage_tests.rs"]']),
 'src/frontend/oir/owned/source/typeck.rs': ([], [b'#[path = "enum_type_tests.rs"]']),
 'src/frontend/oir/source/hir_import.rs': ([], []),
 'src/frontend/oir/source/hir_import/candidate.rs': ([], []),
 'src/frontend/oir/source/sealed.rs': ([], []),
 'src/frontend/parser.rs': ([],
                            [b'#[path = "parser/arrays.rs"]',
                             b'#[path = "parser/characters.rs"]',
                             b'#[path = "parser/conversions.rs"]',
                             b'#[path = "parser/enums.rs"]',
                             b'#[path = "parser/u8_syntax_tests.rs"]',
                             b'#[path = "parser/builtin_tests.rs"]',
                             b'#[path = "parser/array_syntax_tests.rs"]',
                             b'#[path = "parser/enum_syntax_tests.rs"]',
                             b'#[path = "parser/project_tests.rs"]',
                             b'#[path = "parser/activation_tests.rs"]'])}


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
            "changed pinned Linux-native source policy")
    module = types.ModuleType("_git_source_bundle_compatibility_parent")
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
PARENT_ADDED = PREDECESSOR_ADDED | RANGE_FOR_ADDED
directives = library.directives
PREDECESSOR_POLICIES = {**library.PREDECESSOR_POLICIES, PREDECESSOR_POLICY: PREDECESSOR_POLICY_SHA}


def authenticate_policies(repo, parent):
    for name, expected in PREDECESSOR_POLICIES.items():
        before = regular(parent, name)
        require(sha(before) == expected, "wrong pinned predecessor source policy: " + name)
        require(regular(repo, name) == before, "predecessor source policy changed: " + name)


PARSER_RESOURCE = "archive/resource/parser-resource-review-tests.rs"
PARSER_RESOURCE_INPUT = (2114, "e3081188e6dfb0bd171e806992091f3a58b9b463d38a194b481341ff3064a2b5")
PARSER_RESOURCE_OUTPUT = (2185, 'ac2d1933c7aa16a4a0b7db1f4af6d96b4d8801da7a646da29f5e9050980c72c1')
PARSER_RESOURCE_BEFORE = b"storage:enums::SyntaxStorage::default(),tokens,"
PARSER_RESOURCE_AFTER = b"storage:enums::SyntaxStorage::default(),generic_headers:Vec::new(),generic_calls:Vec::new(),pending_equal:None,tokens,"


def adapt_parser_resource(body):
    require(type(body) is bytes and (len(body), sha(body)) == PARSER_RESOURCE_INPUT,
            "wrong parser resource predecessor")
    require(body.count(PARSER_RESOURCE_BEFORE) == 1 and PARSER_RESOURCE_AFTER not in body,
            "parser resource initializer drift")
    result = body.replace(PARSER_RESOURCE_BEFORE, PARSER_RESOURCE_AFTER, 1)
    require((len(result), sha(result)) == PARSER_RESOURCE_OUTPUT
            and result.replace(PARSER_RESOURCE_AFTER, PARSER_RESOURCE_BEFORE, 1) == body,
            "wrong parser resource successor")
    return result


def prepare_with_parser_resource(original, output, captured, adapter, resource):
    return library.prepare_with_parser_resource(original, output, captured, adapter, resource)


def prepare_current_observer(original, output, captured, adapter):
    return prepare_with_parser_resource(original, output, captured, adapter,
                                        adapt_parser_resource(captured["resource"]))


run_file_observer_controls = library.run_file_observer_controls
run_i64_observer_controls = library.run_i64_observer_controls
run_integer_width_observer_controls = library.run_integer_width_observer_controls
run_integer128_observer_controls = library.run_integer128_observer_controls
run_float_observer_controls = library.run_float_observer_controls
run_character_observer_controls = library.run_character_observer_controls


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
    expected = expected_parent | RANGE_FOR_ADDED
    require(set(parent) == expected_parent, "wrong predecessor compiler input membership")
    require(set(RANGE_FOR_INPUTS) == RANGE_FOR_CHANGED | RANGE_FOR_ADDED
            and RANGE_FOR_CHANGED <= expected_parent and not RANGE_FOR_ADDED & expected_parent,
            "wrong reviewed range-for input membership")
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
        if name not in RANGE_FOR_CHANGED:
            require(inputs[name] == before, "unreviewed change outside range-for boundary: " + name)
    for name, (length, digest) in RANGE_FOR_INPUTS.items():
        require(len(inputs[name]) == length and sha(inputs[name]) == digest,
                "range-for source does not match reviewed input: " + name)
        require(directives(inputs[name]) == REVIEWED_DIRECTIVES.get(name, directives(parent[name]) if name in parent else ([], [])),
                "range-for compile-time input directive changed: " + name)
    return inputs


def current_manifest(inputs, baseline, checkout, parent):
    changes = [{"path": name, "before": identity(name, baseline[name]), "after": identity(name, body)}
               for name, body in sorted(inputs.items()) if name in baseline and body != baseline[name]]
    result = {"schema": "oxid-project-workflow-current-source-v1", "baseline_commit": BASELINE,
              "checkout": checkout, "changes": changes, "additions": sorted(PARENT_ADDED),
              "assertion_scope": "retained bounded scalar/record semantics and resources",
              "source_policy_parent": PREDECESSOR, "source_policy_inputs": PREDECESSOR_POLICIES,
              "range_for_reviewed_inputs": [{"path": name, "bytes": length, "sha256": digest}
                                          for name, (length, digest) in sorted(RANGE_FOR_INPUTS.items())],
              "range_for_changes": [{"path": name, "before": identity(name, parent[name]),
                                  "after": identity(name, inputs[name])} for name in sorted(RANGE_FOR_CHANGED)],
              "unchanged_baseline_inputs": len(baseline) - len(changes),
              "files": [identity(name, body) for name, body in sorted(inputs.items())]}
    for name in ("package", "workspace", "native_reproducibility", "native_optimization", "owned_enum",
                 "byte_field", "byte_buffer", "binary_io", "native_heap_safety", "hosted_execution",
                 "owned_buffer_transfer", "byte_string", "native_file", "program_arguments", "i64", "integer_widths", "byte_integer", "integer128", "floats", "characters", "project_formatting", "record_field_borrowing", "hosted_native_recursion", "git_dependencies", "git_source_bundles", "inferred_generic_functions", "explicit_generic_arguments", "nominal_generics", "record_patterns", "record_enum_fields", "constructor_inference", "linux_native", "range_for"):
        key = name + ("_qualified_by_this_gate" if name.startswith("native_") and name != "native_file" else "_semantics_qualified_by_this_gate")
        result[key] = False
    return result


def run(args):
    require(__debug__ and not sys.flags.optimize, "qualification refuses optimized Python")
    repo, output = args.repo.resolve(strict=True), args.output.resolve()
    require(Path(__file__).resolve() == repo / "scripts/verify_range_for_compatibility.py",
            "run the controller from the selected checkout")
    require(output != repo and repo not in output.parents, "output must be outside repository")
    require(not output.exists(), "output must be new")
    output.mkdir(parents=True)
    report = {"schema": "oxid-project-workflow-current-result-v1", "status": "running",
              "scope": "retained bounded scalar/record semantics and resources after Linux ARM64 native support; new target behavior qualified separately",
              "semantic_pass": False, "compiler_executions": 0, "baseline_commit": BASELINE,
              "started_at_utc": datetime.datetime.now(datetime.timezone.utc).isoformat(),
              "controller": identity("scripts/verify_range_for_compatibility.py", Path(__file__).read_bytes())}
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
            verified.update(run_float_observer_controls(repo, output, selected, seam, original, observer_adapter))
            verified.update(run_character_observer_controls(repo, output, selected, seam, original, observer_adapter))
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
