#!/usr/bin/env python3
"""Replay retained bounded assertions after inferred generic functions.

The source-bundle predecessor and twenty-one preceding source policies stay
byte-authenticated. Generic semantics and native execution have separate tests.
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

PREDECESSOR = 'c45d45370cb1c9b52d05a348bd3ecbd896b76be3'
PREDECESSOR_POLICY = 'scripts/verify_git_source_bundle_compatibility.py'
PREDECESSOR_POLICY_SHA = 'c8d96660c9f263715e839fa8c688093b0b9362a2adeddf1362a73b5c5d30d9de'
PREDECESSOR_POLICY_BYTES = 18294
GENERIC_FUNCTIONS_CHANGED = frozenset(('src/frontend/ast.rs', 'src/frontend/declaration_index.rs', 'src/frontend/declaration_index/sealed.rs', 'src/frontend/declaration_index/source_owner.rs', 'src/frontend/format.rs', 'src/frontend/format/ast_tests.rs', 'src/frontend/lexer.rs', 'src/frontend/oir/owned/source/association.rs', 'src/frontend/oir/owned/source/resolve.rs', 'src/frontend/parser.rs', 'src/frontend/parser/enum_syntax_tests.rs', 'src/frontend/parser/enums.rs', 'src/frontend/parser/project_tests.rs', 'src/frontend/project.rs', 'src/frontend/project/budget.rs'))
GENERIC_FUNCTIONS_ADDED = frozenset(('src/frontend/ast/generic_syntax.rs', 'src/frontend/declaration_index/generics.rs', 'src/frontend/declaration_index/generics/flow.rs', 'src/frontend/parser/generic_syntax_tests.rs'))
GENERIC_FUNCTIONS_INPUTS = {'src/frontend/ast.rs': (52662, 'f23a2518d0a2d072bb8b3c49801dd76f337046b9e3513a48c07fcc836c6688ce'),
 'src/frontend/ast/generic_syntax.rs': (7219,
                                        '7172dff31a7a301bde3b7736914c4a4a39e4ff855ff9df3939c21f18be162fb3'),
 'src/frontend/declaration_index.rs': (77976,
                                       '745fca3ef95ddf154a648f0b72603c5adc4492a7a5eae5bbbd99685064636608'),
 'src/frontend/declaration_index/generics.rs': (73324,
                                                'c7a2c11567bc5150e050facfccb43621cb47733d07d3004a071872a849a53fb7'),
 'src/frontend/declaration_index/generics/flow.rs': (3190,
                                                     '643285baa5d6a9c8e2351ddae1378083eda19ddb00257501c000cc8a60fd0c14'),
 'src/frontend/declaration_index/sealed.rs': (84920,
                                              '4613c56ae4fb229aaba62c516aa3f620fd69b9da8d261808f70592358d52f36a'),
 'src/frontend/declaration_index/source_owner.rs': (23300,
                                                    'cfe25cc13a2ef2304f0dfdfe748515b25a7b6b38abce36d91e50e4f56a54356b'),
 'src/frontend/format.rs': (52019, '2e2e5e1c41576917b33f9895aa02b485d61406078ae1731a4921fd09d86f444c'),
 'src/frontend/format/ast_tests.rs': (69565,
                                      'ffabfb5da43a15b659d2f207d4f6cd2ad6ff9b66bb4a6fa9a0b8ff905872434a'),
 'src/frontend/lexer.rs': (36721, '05f2fa9708d283382677d254728a6ccfef4b684ca22dfff58fbb1b8ab6c6998b'),
 'src/frontend/oir/owned/source/association.rs': (64078,
                                                  'aa82139040f1ea21d4e3e0eface6f4085441f9c743401cf458aae6f75dec4840'),
 'src/frontend/oir/owned/source/resolve.rs': (150258,
                                              'dd8d4a0d8776de79c1983b4b0d0baf737baf73a080438df9c4b099b14da12e48'),
 'src/frontend/parser.rs': (91895, '991e3ec11b86d1f7f344d786461336133d2b94f102a62624cea191490483a4f4'),
 'src/frontend/parser/enum_syntax_tests.rs': (29731,
                                              'a2c9c610291855a8666362cad1ac4634f94b8fd4fe5eb391c34ca8197fd58630'),
 'src/frontend/parser/enums.rs': (16811, 'c85a32df48dfef6883ec0a6613d540017a8acf701a13053e62496e41510c6d41'),
 'src/frontend/parser/generic_syntax_tests.rs': (6409,
                                                 'f7784bf400c8b677ede0e0d38c2b3809ed56e03d8c51fa2e23fbb49e711ebf29'),
 'src/frontend/parser/project_tests.rs': (20596,
                                          '0e05715058a80be589a38fb3732f2beae9b30cc53f97c5e76f11b45227ffd5b0'),
 'src/frontend/project.rs': (175547, 'c3316d9c8afda91574159eff6434cbea2078b098acf43303a850466bc45c09fd'),
 'src/frontend/project/budget.rs': (9967, '3f1b8fa28ec2d53c47576a6bc24d32901c1378c3dc3191b4229950de6c57287f')}
REVIEWED_DIRECTIVES = {'src/frontend/ast.rs': ([], []),
 'src/frontend/ast/generic_syntax.rs': ([], []),
 'src/frontend/declaration_index.rs': ([], []),
 'src/frontend/declaration_index/generics.rs': ([], []),
 'src/frontend/declaration_index/generics/flow.rs': ([], []),
 'src/frontend/declaration_index/sealed.rs': ([], []),
 'src/frontend/declaration_index/source_owner.rs': ([], []),
 'src/frontend/format.rs': ([],
                            [b'#[path = "format/resource_tests.rs"]',
                             b'#[path = "format/ast_tests.rs"]',
                             b'#[path = "format/enum_candidate_tests.rs"]']),
 'src/frontend/format/ast_tests.rs': ([], []),
 'src/frontend/lexer.rs': ([], []),
 'src/frontend/oir/owned/source/association.rs': ([],
                                                  [b'#[path = "output_lower_tests.rs"]',
                                                   b'#[path = "byte_storage_association_tests.rs"]']),
 'src/frontend/oir/owned/source/resolve.rs': ([],
                                              [b'#[path = "resolver_paid_tests.rs"]',
                                               b'#[path = "resolver_enum_tests.rs"]',
                                               b'#[path = "enum_storage_failure_tests.rs"]',
                                               b'#[path = "builtin_signature_tests.rs"]',
                                               b'#[path = "output_typing_tests.rs"]',
                                               b'#[path = "byte_storage_type_tests.rs"]']),
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
                             b'#[path = "parser/activation_tests.rs"]']),
 'src/frontend/parser/enum_syntax_tests.rs': ([], []),
 'src/frontend/parser/enums.rs': ([], []),
 'src/frontend/parser/generic_syntax_tests.rs': ([], []),
 'src/frontend/parser/project_tests.rs': ([], []),
 'src/frontend/project.rs': ([],
                             [b'#[path = "project/builtin_tests.rs"]',
                              b'#[path = "project/array_syntax_tests.rs"]',
                              b'#[path = "project/enum_carrier_tests.rs"]',
                              b'#[path = "project/enum_index_tests.rs"]',
                              b'#[path = "project/unit2_tests.rs"]']),
 'src/frontend/project/budget.rs': ([],
                                    [b'#[path = "budget_real_null_observer.rs"]',
                                     b'#[path = "budget_string_null_tests.rs"]'])}


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
            "changed pinned source-bundle source policy")
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
PARENT_ADDED = PREDECESSOR_ADDED | GENERIC_FUNCTIONS_ADDED
directives = library.directives
PREDECESSOR_POLICIES = {**library.PREDECESSOR_POLICIES, PREDECESSOR_POLICY: PREDECESSOR_POLICY_SHA}


def authenticate_policies(repo, parent):
    for name, expected in PREDECESSOR_POLICIES.items():
        before = regular(parent, name)
        require(sha(before) == expected, "wrong pinned predecessor source policy: " + name)
        require(regular(repo, name) == before, "predecessor source policy changed: " + name)


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
    expected = expected_parent | GENERIC_FUNCTIONS_ADDED
    require(set(parent) == expected_parent, "wrong predecessor compiler input membership")
    require(set(GENERIC_FUNCTIONS_INPUTS) == GENERIC_FUNCTIONS_CHANGED | GENERIC_FUNCTIONS_ADDED
            and GENERIC_FUNCTIONS_CHANGED <= expected_parent and not GENERIC_FUNCTIONS_ADDED & expected_parent,
            "wrong reviewed generic-functions input membership")
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
        if name not in GENERIC_FUNCTIONS_CHANGED:
            require(inputs[name] == before, "unreviewed change outside generic-functions boundary: " + name)
    for name, (length, digest) in GENERIC_FUNCTIONS_INPUTS.items():
        require(len(inputs[name]) == length and sha(inputs[name]) == digest,
                "generic-functions source does not match reviewed input: " + name)
        require(directives(inputs[name]) == REVIEWED_DIRECTIVES.get(name, directives(parent[name]) if name in parent else ([], [])),
                "generic-functions compile-time input directive changed: " + name)
    return inputs


def current_manifest(inputs, baseline, checkout, parent):
    changes = [{"path": name, "before": identity(name, baseline[name]), "after": identity(name, body)}
               for name, body in sorted(inputs.items()) if name in baseline and body != baseline[name]]
    result = {"schema": "oxid-project-workflow-current-source-v1", "baseline_commit": BASELINE,
              "checkout": checkout, "changes": changes, "additions": sorted(PARENT_ADDED),
              "assertion_scope": "retained bounded scalar/record semantics and resources",
              "source_policy_parent": PREDECESSOR, "source_policy_inputs": PREDECESSOR_POLICIES,
              "generic_functions_reviewed_inputs": [{"path": name, "bytes": length, "sha256": digest}
                                          for name, (length, digest) in sorted(GENERIC_FUNCTIONS_INPUTS.items())],
              "generic_functions_changes": [{"path": name, "before": identity(name, parent[name]),
                                  "after": identity(name, inputs[name])} for name in sorted(GENERIC_FUNCTIONS_CHANGED)],
              "unchanged_baseline_inputs": len(baseline) - len(changes),
              "files": [identity(name, body) for name, body in sorted(inputs.items())]}
    for name in ("package", "workspace", "native_reproducibility", "native_optimization", "owned_enum",
                 "byte_field", "byte_buffer", "binary_io", "native_heap_safety", "hosted_execution",
                 "owned_buffer_transfer", "byte_string", "native_file", "program_arguments", "i64", "integer_widths", "byte_integer", "integer128", "floats", "characters", "project_formatting", "record_field_borrowing", "hosted_native_recursion", "git_dependencies", "git_source_bundles", "inferred_generic_functions"):
        key = name + ("_qualified_by_this_gate" if name.startswith("native_") and name != "native_file" else "_semantics_qualified_by_this_gate")
        result[key] = False
    return result


def run(args):
    require(__debug__ and not sys.flags.optimize, "qualification refuses optimized Python")
    repo, output = args.repo.resolve(strict=True), args.output.resolve()
    require(Path(__file__).resolve() == repo / "scripts/verify_generic_functions_compatibility.py",
            "run the controller from the selected checkout")
    require(output != repo and repo not in output.parents, "output must be outside repository")
    require(not output.exists(), "output must be new")
    output.mkdir(parents=True)
    report = {"schema": "oxid-project-workflow-current-result-v1", "status": "running",
              "scope": "retained bounded scalar/record semantics and resources after inferred generic functions; generic semantics qualified separately",
              "semantic_pass": False, "compiler_executions": 0, "baseline_commit": BASELINE,
              "started_at_utc": datetime.datetime.now(datetime.timezone.utc).isoformat(),
              "controller": identity("scripts/verify_generic_functions_compatibility.py", Path(__file__).read_bytes())}
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
