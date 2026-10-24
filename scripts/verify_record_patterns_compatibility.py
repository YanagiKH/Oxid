#!/usr/bin/env python3
"""Replay retained bounded assertions after complete record patterns.

The generic nominal predecessor and twenty-four preceding source policies stay
byte-authenticated. Record patterns and native execution have separate tests.
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

PREDECESSOR = '6a4d547522f5b1a7d91af50b2f0cc7b56497e22d'
PREDECESSOR_POLICY = 'scripts/verify_nominal_generics_compatibility.py'
PREDECESSOR_POLICY_SHA = '908d106ed1268050286c7183075448a1176ec47398b385d32a3bf5224358a426'
PREDECESSOR_POLICY_BYTES = 26106
RECORD_PATTERNS_CHANGED = frozenset(('src/frontend/ast.rs', 'src/frontend/ast/generic_syntax.rs', 'src/frontend/declaration_index/generics.rs', 'src/frontend/declaration_index/sealed.rs', 'src/frontend/declaration_index/source_owner.rs', 'src/frontend/format/ast_tests.rs', 'src/frontend/hir.rs', 'src/frontend/oir/owned/budget.rs', 'src/frontend/oir/owned/execute.rs', 'src/frontend/oir/owned/execute_tests.rs', 'src/frontend/oir/owned/flow.rs', 'src/frontend/oir/owned/mod.rs', 'src/frontend/oir/owned/native.rs', 'src/frontend/oir/owned/plan.rs', 'src/frontend/oir/owned/shape.rs', 'src/frontend/oir/owned/source/array_pipeline_rows.rs', 'src/frontend/oir/owned/source/association.rs', 'src/frontend/oir/owned/source/budget.rs', 'src/frontend/oir/owned/source/candidate_adapter.rs', 'src/frontend/oir/owned/source/candidate_native.rs', 'src/frontend/oir/owned/source/hir.rs', 'src/frontend/oir/owned/source/hir_budget.rs', 'src/frontend/oir/owned/source/lower.rs', 'src/frontend/oir/owned/source/mod.rs', 'src/frontend/oir/owned/source/resolve.rs', 'src/frontend/oir/owned/source/resolver_storage.rs', 'src/frontend/oir/owned/source/typeck.rs', 'src/frontend/oir/owned_types.rs', 'src/frontend/oir/source/hir_import.rs', 'src/frontend/oir/source/hir_import/candidate.rs', 'src/frontend/parser.rs', 'src/frontend/parser/generic_syntax_tests.rs'))
RECORD_PATTERNS_ADDED = frozenset(('src/frontend/oir/owned/source/record_pattern_tests.rs',))
RECORD_PATTERNS_INPUTS = {'src/frontend/ast.rs': (56837, 'cad4a6becbdd94588fabc6bc56476dc80a8fba859915b193db6e098bc48e8d3f'),
 'src/frontend/ast/generic_syntax.rs': (8358,
                                        '175fb76f2d244a450e37f0be652f762d1dada7a7b116c734abbed861f0dda72b'),
 'src/frontend/declaration_index/generics.rs': (104352,
                                                'af43e71620aa74f994126d6f538559d9cee41468c51629de0403035a4096fb44'),
 'src/frontend/declaration_index/sealed.rs': (86834,
                                              '57a2703cfc965d18eb02b42af3fc3c6e8ce51efaf47d22be1f6c16720a2c71c7'),
 'src/frontend/declaration_index/source_owner.rs': (23475,
                                                    'eb8ba6c49bc4c27a71388cfdd37232241aeec85aaba839336b9aac0647bb5940'),
 'src/frontend/format/ast_tests.rs': (75393,
                                      'fd0d30b1cbe7c76a327951909c8d81c2453aa5078e2fd4939b509d48dd3065e7'),
 'src/frontend/hir.rs': (32617, '8f5d711f7e10969931734d774668baf17ed7be15ffc5d2287b99eca1d9a940f4'),
 'src/frontend/oir/owned/budget.rs': (19629,
                                      '1db04df827cadc92aa1410095a158008f1ff9173af95433c5ad8d081eb2b7f6e'),
 'src/frontend/oir/owned/execute.rs': (200908,
                                       '0d368a2b815bf4a9e7c805ab9e7ffa82f717bdcf943159a45176ed09b68e289c'),
 'src/frontend/oir/owned/execute_tests.rs': (45288,
                                             'd30b220d54c13678db1fef5068af206d57daf1a8be5dcaf6a2b07d138ac4d443'),
 'src/frontend/oir/owned/flow.rs': (47500,
                                    'c1919a47ce754f2a45ce552e52901af9ee8c6271fc9012f5842216350d11f412'),
 'src/frontend/oir/owned/mod.rs': (15096, 'a7e90751f8328fbb58a08dd15038281323ddfeeccd134d80375bfed256d2ec93'),
 'src/frontend/oir/owned/native.rs': (246623,
                                      'd4f8be459d904b32d7d9d269528897dd30e9c6da85047bd483c53faae847862d'),
 'src/frontend/oir/owned/plan.rs': (31482,
                                    '8c20c623311aefeadf1d2c1b6b60dcc9a94c13ecfb6acabc780d9a29de641094'),
 'src/frontend/oir/owned/shape.rs': (58111,
                                     '4c75df169e75c8af55e7a08cc18eb43885fb4a1fdb04b96563ddb82f1f815c5c'),
 'src/frontend/oir/owned/source/array_pipeline_rows.rs': (55985,
                                                          '12caab5c3c3caf4dc80276b4ed7c27d797135c6fb5ba38c21c7c9ff895fb2cde'),
 'src/frontend/oir/owned/source/association.rs': (77629,
                                                  'c8388b90e10ec9eaefe7c9f99398da241c08e2e7a2c6183ede76541226dd9c53'),
 'src/frontend/oir/owned/source/budget.rs': (16165,
                                             'bf9830c398d16ecd3c792d1b1fc62a43e39ed581fbc3376b17b479a6ce0daef0'),
 'src/frontend/oir/owned/source/candidate_adapter.rs': (43516,
                                                        '4432e067002367084bb5daa86eb9edf73e1e004358f884fd3c707d4b35f986ed'),
 'src/frontend/oir/owned/source/candidate_native.rs': (32417,
                                                       'f417cad4038e95711b75a728babe4d7b3b6e10b270157272c216952c86cd2060'),
 'src/frontend/oir/owned/source/hir.rs': (7126,
                                          '1bcf8a2bcbe1639a9c08c1732ecfa2a73aa8ca33f0697a2b34f97566f91f469d'),
 'src/frontend/oir/owned/source/hir_budget.rs': (57699,
                                                 'a44ea195fc1c6852ed2b52f0c07dba61333df3d99342f79a49b9710bc9992fd5'),
 'src/frontend/oir/owned/source/lower.rs': (121625,
                                            '1a88894fad11393639bac03dccef118d86947322a4df5ac711cfb46360903ac7'),
 'src/frontend/oir/owned/source/mod.rs': (2190,
                                          'b5f682dea2694544650720e1d9e5779cf61290cf74577202df561a2841c2de56'),
 'src/frontend/oir/owned/source/record_pattern_tests.rs': (11795,
                                                           '28057f5a60a9a6319a80c78d538bc51110b7e5c0bf6cbd0991f9a5807e5af12b'),
 'src/frontend/oir/owned/source/resolve.rs': (159018,
                                              'a605908fad98cc0750314725e99582f9cd9c09e3865431421e115523e5bc5c9c'),
 'src/frontend/oir/owned/source/resolver_storage.rs': (30607,
                                                       'cdde177846149e2fd816cadfd66ebe240ef9e5c7f38aa4dd48bc1c2518aaea4f'),
 'src/frontend/oir/owned/source/typeck.rs': (216804,
                                             '67ab342a5e5e85a58661979a36d08ea29cb449cd0c12b0e4c1b2c10b424dd9c9'),
 'src/frontend/oir/owned_types.rs': (82170,
                                     '6d56a2a8f32b027bdd9ff97db815e19ca8e2177d6aba8e727efef90ea9cb0ddd'),
 'src/frontend/oir/source/hir_import.rs': (17443,
                                           '1249c27fdc3e6718aefc899e2ef7c2c9b161b1d2a84b9734cf780664a674b606'),
 'src/frontend/oir/source/hir_import/candidate.rs': (52948,
                                                     '993da51a6f8b63789506d29202cfb9720c51aca0592510ec9d02c4d2f284c4fc'),
 'src/frontend/parser.rs': (102083, '1bc99e2f37c875bba766f64c8c3b7caa8466598f83b78972b249fbb07df9f083'),
 'src/frontend/parser/generic_syntax_tests.rs': (15871,
                                                 '2adfa4950d42c61e6ee9145901c2c9df8179bb93a1032878a0887cea7b9ec957')}
REVIEWED_DIRECTIVES = {'src/frontend/ast.rs': ([], []),
 'src/frontend/ast/generic_syntax.rs': ([], []),
 'src/frontend/declaration_index/generics.rs': ([], []),
 'src/frontend/declaration_index/sealed.rs': ([], []),
 'src/frontend/declaration_index/source_owner.rs': ([], []),
 'src/frontend/format/ast_tests.rs': ([], []),
 'src/frontend/hir.rs': ([], []),
 'src/frontend/oir/owned/budget.rs': ([], []),
 'src/frontend/oir/owned/execute.rs': ([],
                                       [b'#[path = "execute_floats.rs"]',
                                        b'#[path = "execute_values.rs"]',
                                        b'#[path = "execute_bitwise.rs"]',
                                        b'#[path = "execute_integer128.rs"]',
                                        b'#[path = "execute_integers.rs"]',
                                        b'#[path = "execute_experimental.rs"]',
                                        b'#[path = "execute_resources.rs"]',
                                        b'#[path = "array_observe.rs"]',
                                        b'#[path = "execute_tests.rs"]',
                                        b'#[path = "builtin_output_reference_tests.rs"]',
                                        b'#[path = "byte_storage_codec_tests.rs"]']),
 'src/frontend/oir/owned/execute_tests.rs': ([b'include!("reviewer_reference_tests.rs")'],
                                             [b'#[path = "source/runtime_resource_tests.rs"]',
                                              b'#[path = "source/reviewer_resource_runtime.rs"]']),
 'src/frontend/oir/owned/flow.rs': ([], []),
 'src/frontend/oir/owned/mod.rs': ([], []),
 'src/frontend/oir/owned/native.rs': ([],
                                      [b'#[path = "native_bitwise.rs"]',
                                       b'#[path = "native_experimental.rs"]',
                                       b'#[path = "native_floats.rs"]',
                                       b'#[path = "native_integer128.rs"]',
                                       b'#[path = "native_integers.rs"]',
                                       b'#[path = "native_numeric.rs"]',
                                       b'#[path = "native_resources.rs"]',
                                       b'#[path = "builtin_input_native_tests.rs"]',
                                       b'#[path = "builtin_output_native_tests.rs"]',
                                       b'#[path = "native_tests.rs"]',
                                       b'#[path = "native_inventory_admission_tests.rs"]',
                                       b'#[path = "byte_storage_native_tests.rs"]']),
 'src/frontend/oir/owned/plan.rs': ([], [b'#[path = "native_storage.rs"]']),
 'src/frontend/oir/owned/shape.rs': ([], []),
 'src/frontend/oir/owned/source/array_pipeline_rows.rs': ([], []),
 'src/frontend/oir/owned/source/association.rs': ([],
                                                  [b'#[path = "output_lower_tests.rs"]',
                                                   b'#[path = "byte_storage_association_tests.rs"]']),
 'src/frontend/oir/owned/source/budget.rs': ([], []),
 'src/frontend/oir/owned/source/candidate_adapter.rs': ([],
                                                        [b'#[path = "candidate_mutations.rs"]',
                                                         b'#[path = "candidate_native.rs"]']),
 'src/frontend/oir/owned/source/candidate_native.rs': ([],
                                                       [b'#[ignore = "bounded native source collector; req'
                                                        b'uires frozen original source, budget controls, p'
                                                        b'inned LLVM and a fresh output path"]']),
 'src/frontend/oir/owned/source/hir.rs': ([], []),
 'src/frontend/oir/owned/source/hir_budget.rs': ([],
                                                 [b'#[path = "byte_storage_resources.rs"]',
                                                  b'#[path = "hir_budget_tests.rs"]']),
 'src/frontend/oir/owned/source/lower.rs': ([], []),
 'src/frontend/oir/owned/source/mod.rs': ([], []),
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
 'src/frontend/oir/owned/source/typeck.rs': ([], [b'#[path = "enum_type_tests.rs"]']),
 'src/frontend/oir/owned_types.rs': ([], []),
 'src/frontend/oir/source/hir_import.rs': ([], []),
 'src/frontend/oir/source/hir_import/candidate.rs': ([], []),
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
 'src/frontend/parser/generic_syntax_tests.rs': ([], [])}


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
            "changed pinned generic-nominal source policy")
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
PARENT_ADDED = PREDECESSOR_ADDED | RECORD_PATTERNS_ADDED
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
    expected = expected_parent | RECORD_PATTERNS_ADDED
    require(set(parent) == expected_parent, "wrong predecessor compiler input membership")
    require(set(RECORD_PATTERNS_INPUTS) == RECORD_PATTERNS_CHANGED | RECORD_PATTERNS_ADDED
            and RECORD_PATTERNS_CHANGED <= expected_parent and not RECORD_PATTERNS_ADDED & expected_parent,
            "wrong reviewed record-patterns input membership")
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
        if name not in RECORD_PATTERNS_CHANGED:
            require(inputs[name] == before, "unreviewed change outside record-patterns boundary: " + name)
    for name, (length, digest) in RECORD_PATTERNS_INPUTS.items():
        require(len(inputs[name]) == length and sha(inputs[name]) == digest,
                "record-patterns source does not match reviewed input: " + name)
        require(directives(inputs[name]) == REVIEWED_DIRECTIVES.get(name, directives(parent[name]) if name in parent else ([], [])),
                "record-patterns compile-time input directive changed: " + name)
    return inputs


def current_manifest(inputs, baseline, checkout, parent):
    changes = [{"path": name, "before": identity(name, baseline[name]), "after": identity(name, body)}
               for name, body in sorted(inputs.items()) if name in baseline and body != baseline[name]]
    result = {"schema": "oxid-project-workflow-current-source-v1", "baseline_commit": BASELINE,
              "checkout": checkout, "changes": changes, "additions": sorted(PARENT_ADDED),
              "assertion_scope": "retained bounded scalar/record semantics and resources",
              "source_policy_parent": PREDECESSOR, "source_policy_inputs": PREDECESSOR_POLICIES,
              "record_patterns_reviewed_inputs": [{"path": name, "bytes": length, "sha256": digest}
                                          for name, (length, digest) in sorted(RECORD_PATTERNS_INPUTS.items())],
              "record_patterns_changes": [{"path": name, "before": identity(name, parent[name]),
                                  "after": identity(name, inputs[name])} for name in sorted(RECORD_PATTERNS_CHANGED)],
              "unchanged_baseline_inputs": len(baseline) - len(changes),
              "files": [identity(name, body) for name, body in sorted(inputs.items())]}
    for name in ("package", "workspace", "native_reproducibility", "native_optimization", "owned_enum",
                 "byte_field", "byte_buffer", "binary_io", "native_heap_safety", "hosted_execution",
                 "owned_buffer_transfer", "byte_string", "native_file", "program_arguments", "i64", "integer_widths", "byte_integer", "integer128", "floats", "characters", "project_formatting", "record_field_borrowing", "hosted_native_recursion", "git_dependencies", "git_source_bundles", "inferred_generic_functions", "explicit_generic_arguments", "nominal_generics", "record_patterns"):
        key = name + ("_qualified_by_this_gate" if name.startswith("native_") and name != "native_file" else "_semantics_qualified_by_this_gate")
        result[key] = False
    return result


def run(args):
    require(__debug__ and not sys.flags.optimize, "qualification refuses optimized Python")
    repo, output = args.repo.resolve(strict=True), args.output.resolve()
    require(Path(__file__).resolve() == repo / "scripts/verify_record_patterns_compatibility.py",
            "run the controller from the selected checkout")
    require(output != repo and repo not in output.parents, "output must be outside repository")
    require(not output.exists(), "output must be new")
    output.mkdir(parents=True)
    report = {"schema": "oxid-project-workflow-current-result-v1", "status": "running",
              "scope": "retained bounded scalar/record semantics and resources after complete record patterns; record pattern semantics qualified separately",
              "semantic_pass": False, "compiler_executions": 0, "baseline_commit": BASELINE,
              "started_at_utc": datetime.datetime.now(datetime.timezone.utc).isoformat(),
              "controller": identity("scripts/verify_record_patterns_compatibility.py", Path(__file__).read_bytes())}
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
