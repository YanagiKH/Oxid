#!/usr/bin/env python3
"""Replay retained bounded assertions after record fields gain finite enums.

The record-pattern predecessor and twenty-five preceding source policies stay
byte-authenticated. New field semantics have separate Rust and native tests.
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

PREDECESSOR = 'b4014ae23bacc6185aa60dabe9eea9bfe60e859f'
PREDECESSOR_POLICY = 'scripts/verify_record_patterns_compatibility.py'
PREDECESSOR_POLICY_SHA = 'f1a352493f232337acccbf442968ea1f776a7cf38a613ce67aaf21e95c291981'
PREDECESSOR_POLICY_BYTES = 31834
RECORD_ENUM_FIELDS_CHANGED = frozenset(('src/frontend/declaration_index.rs', 'src/frontend/declaration_index/generics.rs', 'src/frontend/declaration_index/generics/nominal.rs', 'src/frontend/enum_public_tests.rs', 'src/frontend/oir/owned/execute.rs', 'src/frontend/oir/owned/execute_tests.rs', 'src/frontend/oir/owned/native.rs', 'src/frontend/oir/owned/native_resources.rs', 'src/frontend/oir/owned/plan.rs', 'src/frontend/oir/owned/source/association.rs', 'src/frontend/oir/owned/source/budget.rs', 'src/frontend/oir/owned/source/byte_storage_type_tests.rs', 'src/frontend/oir/owned/source/mod.rs', 'src/frontend/oir/owned/source/record_pattern_tests.rs', 'src/frontend/oir/owned/source/resolve.rs', 'src/frontend/oir/owned/source/resolver_enum_tests.rs', 'src/frontend/oir/owned/source/resolver_inventory_tests.rs', 'src/frontend/oir/owned/source/resolver_paid_tests.rs', 'src/frontend/oir/owned/source/resolver_storage.rs', 'src/frontend/oir/owned/source/type_storage.rs', 'src/frontend/oir/owned/source/type_storage_tests.rs', 'src/frontend/oir/owned_types.rs', 'src/frontend/oir/owned_types/enum_integration_tests.rs', 'src/frontend/oir/owned_types/enums.rs', 'src/frontend/oir/owned_types/resources.rs'))
RECORD_ENUM_FIELDS_ADDED = frozenset(('src/frontend/oir/owned/execute_composition.rs', 'src/frontend/oir/owned/native_composition.rs', 'src/frontend/oir/owned/source/record_enum_field_tests.rs', 'src/frontend/oir/owned/source/record_enum_layouts.rs', 'src/frontend/oir/owned_types/composition.rs'))
RECORD_ENUM_FIELDS_INPUTS = {'src/frontend/declaration_index.rs': (80462,
                                       '29f5b607b3f56a0f1fef8263bb4c778364a16d963fab3385fb7a45ac3f02262a'),
 'src/frontend/declaration_index/generics.rs': (105594,
                                                '88cfdabd7090b34ff252ff6491450505f6f7ed3a09cd5017a6ee58a6ff6ccf36'),
 'src/frontend/declaration_index/generics/nominal.rs': (22789,
                                                        '5a863827b6916747630eaa2fdbd3e6d663ff12da0f5d262f58f2aa8c306822df'),
 'src/frontend/enum_public_tests.rs': (24330,
                                       '93329214a2381058bbb0c78db6e9a31063447b21f09095dccf3468830aeeb89b'),
 'src/frontend/oir/owned/execute.rs': (202438,
                                       'e167a8edcafe5478def00c1d495d68127e2069b20edfbf4ef471b39cbcf8ea3f'),
 'src/frontend/oir/owned/execute_composition.rs': (11132,
                                                   '621871ca1fa1b25d60d3c635daaa33a8ff0423cf9fd815cfec097ae3c46262e4'),
 'src/frontend/oir/owned/execute_tests.rs': (45823,
                                             '3c98f0bfebf0ad16a29ec0e9f2048120ef417113ce03fd193b1f37875f23770a'),
 'src/frontend/oir/owned/native.rs': (253272,
                                      'd40337d13c305c86e0a079e0c26fe31120b95724ee9c8ebedb2c5cf6e9a1d7ce'),
 'src/frontend/oir/owned/native_composition.rs': (25872,
                                                  '7f4f76c76b7498ce9922dc5d8bb999278a47124fcf462e804fa70e1963ad0cc8'),
 'src/frontend/oir/owned/native_resources.rs': (6209,
                                                '36a2109f368d59f8f2eab9f04725861ed2bd346255db36b751f51ba0d4ce6122'),
 'src/frontend/oir/owned/plan.rs': (31850,
                                    '7837fdcbbd5ab38ae303605e2c96ef09e780fb5a6973386efbfde28c3cd2c624'),
 'src/frontend/oir/owned/source/association.rs': (78002,
                                                  '6be306b994a689bda3394c7e7b42d951013be3c89806127712267d005b097a01'),
 'src/frontend/oir/owned/source/budget.rs': (16954,
                                             'ec98fe04feb2607f74da7a4c4cde3d986dfeade34bb8cbe0562edd6874ba292a'),
 'src/frontend/oir/owned/source/byte_storage_type_tests.rs': (11504,
                                                              'a1803c58426e876731f54c7a9a372546f0c8fc6d1f753012fedb289b4f9148ac'),
 'src/frontend/oir/owned/source/mod.rs': (2257,
                                          '1f202e15103b0bb5d6018cdbf732b2bc8fd35803e61a6b80657971b9411508ef'),
 'src/frontend/oir/owned/source/record_enum_field_tests.rs': (7422,
                                                              '4726866dd6dd73f95c7197f35cd2cf3ec48dbe9fcda921d62da3687f41ea8a71'),
 'src/frontend/oir/owned/source/record_enum_layouts.rs': (13277,
                                                          'abe52d9817934d5c491d0795bc5d35d143464de98bb53ce4dae5e7ddc21999a9'),
 'src/frontend/oir/owned/source/record_pattern_tests.rs': (11817,
                                                           '6e5e1398cd1285de0a04de980968cd7c34eaac69ecbf93bca37568f2495f161c'),
 'src/frontend/oir/owned/source/resolve.rs': (159563,
                                              '2283bc7ca4343c8a96ae7b48ad4f83e5d26708b9914a5810195e6bfe383f40f6'),
 'src/frontend/oir/owned/source/resolver_enum_tests.rs': (24081,
                                                          '0c35df59c099fa6605207818f853192a262a54fd4b7795995d661fbc43e13506'),
 'src/frontend/oir/owned/source/resolver_inventory_tests.rs': (18297,
                                                               'fdd7ddd812e50dea0b426033889840808a97f2cbd886e12b23bca0233f8e2782'),
 'src/frontend/oir/owned/source/resolver_paid_tests.rs': (113732,
                                                          'a5c3fd7e284a4c6e38b7c3ecf348a0222ae52e7cc4d145c033945e6798f2ebeb'),
 'src/frontend/oir/owned/source/resolver_storage.rs': (30491,
                                                       '9860be5ab4050f903fac781883f25cb371a59fedd2d21a29d8665e55b26a14e7'),
 'src/frontend/oir/owned/source/type_storage.rs': (68332,
                                                   '91b0b09fdac4efb8632628457453acf4a3688ed2e38723c17a372230b05c1050'),
 'src/frontend/oir/owned/source/type_storage_tests.rs': (117364,
                                                         'dca0a580b9e4abc7e24a79d8ded9e64404c5fb4c800ba0dee7a13d2631db4f24'),
 'src/frontend/oir/owned_types.rs': (89339,
                                     '9e66b7a0c1b5a892804628f955e4e1ef28d45d1de2dbe26c8d26f584bba9199c'),
 'src/frontend/oir/owned_types/composition.rs': (19335,
                                                 '8be3ff13f3b13c4fc2d49b48a1bcb067dab354ace98b7c2aa379f43222b7a473'),
 'src/frontend/oir/owned_types/enum_integration_tests.rs': (12051,
                                                            'c511a36efafa7b562444bf53e6c4ffe5ea35958ddf20bcc055960bdc6b1254e9'),
 'src/frontend/oir/owned_types/enums.rs': (58912,
                                           'da3e24090cea7393065a0b901c50cf829917a65949aee7e364a480fd586a38e9'),
 'src/frontend/oir/owned_types/resources.rs': (25020,
                                               '490f55523212e1412eba3dc04fa156105cd2ccc26552d5e3ee18bc17750927a2')}
REVIEWED_DIRECTIVES = {'src/frontend/declaration_index.rs': ([], []),
 'src/frontend/declaration_index/generics.rs': ([], []),
 'src/frontend/declaration_index/generics/nominal.rs': ([], []),
 'src/frontend/enum_public_tests.rs': ([b'include_str!("../../tests/fixtures/bounded_enum_scanner/main.ox")',
                                        b'include_str!("../../tests/fixtures/bounded_enum_scanner/scanner.'
                                        b'ox")'],
                                       []),
 'src/frontend/oir/owned/execute.rs': ([],
                                       [b'#[path = "execute_floats.rs"]',
                                        b'#[path = "execute_values.rs"]',
                                        b'#[path = "execute_bitwise.rs"]',
                                        b'#[path = "execute_integer128.rs"]',
                                        b'#[path = "execute_integers.rs"]',
                                        b'#[path = "execute_experimental.rs"]',
                                        b'#[path = "execute_composition.rs"]',
                                        b'#[path = "execute_resources.rs"]',
                                        b'#[path = "array_observe.rs"]',
                                        b'#[path = "execute_tests.rs"]',
                                        b'#[path = "builtin_output_reference_tests.rs"]',
                                        b'#[path = "byte_storage_codec_tests.rs"]']),
 'src/frontend/oir/owned/execute_composition.rs': ([], []),
 'src/frontend/oir/owned/execute_tests.rs': ([b'include!("reviewer_reference_tests.rs")'],
                                             [b'#[path = "source/runtime_resource_tests.rs"]',
                                              b'#[path = "source/reviewer_resource_runtime.rs"]']),
 'src/frontend/oir/owned/native.rs': ([],
                                      [b'#[path = "native_bitwise.rs"]',
                                       b'#[path = "native_composition.rs"]',
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
 'src/frontend/oir/owned/native_composition.rs': ([], []),
 'src/frontend/oir/owned/native_resources.rs': ([], []),
 'src/frontend/oir/owned/plan.rs': ([], [b'#[path = "native_storage.rs"]']),
 'src/frontend/oir/owned/source/association.rs': ([],
                                                  [b'#[path = "output_lower_tests.rs"]',
                                                   b'#[path = "byte_storage_association_tests.rs"]']),
 'src/frontend/oir/owned/source/budget.rs': ([], []),
 'src/frontend/oir/owned/source/byte_storage_type_tests.rs': ([], []),
 'src/frontend/oir/owned/source/mod.rs': ([], []),
 'src/frontend/oir/owned/source/record_enum_field_tests.rs': ([], []),
 'src/frontend/oir/owned/source/record_enum_layouts.rs': ([], []),
 'src/frontend/oir/owned/source/record_pattern_tests.rs': ([], []),
 'src/frontend/oir/owned/source/resolve.rs': ([],
                                              [b'#[path = "resolver_paid_tests.rs"]',
                                               b'#[path = "resolver_enum_tests.rs"]',
                                               b'#[path = "enum_storage_failure_tests.rs"]',
                                               b'#[path = "builtin_signature_tests.rs"]',
                                               b'#[path = "output_typing_tests.rs"]',
                                               b'#[path = "byte_storage_type_tests.rs"]']),
 'src/frontend/oir/owned/source/resolver_enum_tests.rs': ([], []),
 'src/frontend/oir/owned/source/resolver_inventory_tests.rs': ([], []),
 'src/frontend/oir/owned/source/resolver_paid_tests.rs': ([], []),
 'src/frontend/oir/owned/source/resolver_storage.rs': ([],
                                                       [b'#[path = "resolver_storage_tests.rs"]',
                                                        b'#[path = "resolver_inventory_tests.rs"]']),
 'src/frontend/oir/owned/source/type_storage.rs': ([], [b'#[path = "type_storage_tests.rs"]']),
 'src/frontend/oir/owned/source/type_storage_tests.rs': ([], []),
 'src/frontend/oir/owned_types.rs': ([], []),
 'src/frontend/oir/owned_types/composition.rs': ([], []),
 'src/frontend/oir/owned_types/enum_integration_tests.rs': ([], []),
 'src/frontend/oir/owned_types/enums.rs': ([], []),
 'src/frontend/oir/owned_types/resources.rs': ([], [])}


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
            "changed pinned record-pattern source policy")
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
PARENT_ADDED = PREDECESSOR_ADDED | RECORD_ENUM_FIELDS_ADDED
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
    expected = expected_parent | RECORD_ENUM_FIELDS_ADDED
    require(set(parent) == expected_parent, "wrong predecessor compiler input membership")
    require(set(RECORD_ENUM_FIELDS_INPUTS) == RECORD_ENUM_FIELDS_CHANGED | RECORD_ENUM_FIELDS_ADDED
            and RECORD_ENUM_FIELDS_CHANGED <= expected_parent and not RECORD_ENUM_FIELDS_ADDED & expected_parent,
            "wrong reviewed record-enum-fields input membership")
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
        if name not in RECORD_ENUM_FIELDS_CHANGED:
            require(inputs[name] == before, "unreviewed change outside record-enum-fields boundary: " + name)
    for name, (length, digest) in RECORD_ENUM_FIELDS_INPUTS.items():
        require(len(inputs[name]) == length and sha(inputs[name]) == digest,
                "record-enum-fields source does not match reviewed input: " + name)
        require(directives(inputs[name]) == REVIEWED_DIRECTIVES.get(name, directives(parent[name]) if name in parent else ([], [])),
                "record-enum-fields compile-time input directive changed: " + name)
    return inputs


def current_manifest(inputs, baseline, checkout, parent):
    changes = [{"path": name, "before": identity(name, baseline[name]), "after": identity(name, body)}
               for name, body in sorted(inputs.items()) if name in baseline and body != baseline[name]]
    result = {"schema": "oxid-project-workflow-current-source-v1", "baseline_commit": BASELINE,
              "checkout": checkout, "changes": changes, "additions": sorted(PARENT_ADDED),
              "assertion_scope": "retained bounded scalar/record semantics and resources",
              "source_policy_parent": PREDECESSOR, "source_policy_inputs": PREDECESSOR_POLICIES,
              "record_enum_fields_reviewed_inputs": [{"path": name, "bytes": length, "sha256": digest}
                                          for name, (length, digest) in sorted(RECORD_ENUM_FIELDS_INPUTS.items())],
              "record_enum_fields_changes": [{"path": name, "before": identity(name, parent[name]),
                                  "after": identity(name, inputs[name])} for name in sorted(RECORD_ENUM_FIELDS_CHANGED)],
              "unchanged_baseline_inputs": len(baseline) - len(changes),
              "files": [identity(name, body) for name, body in sorted(inputs.items())]}
    for name in ("package", "workspace", "native_reproducibility", "native_optimization", "owned_enum",
                 "byte_field", "byte_buffer", "binary_io", "native_heap_safety", "hosted_execution",
                 "owned_buffer_transfer", "byte_string", "native_file", "program_arguments", "i64", "integer_widths", "byte_integer", "integer128", "floats", "characters", "project_formatting", "record_field_borrowing", "hosted_native_recursion", "git_dependencies", "git_source_bundles", "inferred_generic_functions", "explicit_generic_arguments", "nominal_generics", "record_patterns", "record_enum_fields"):
        key = name + ("_qualified_by_this_gate" if name.startswith("native_") and name != "native_file" else "_semantics_qualified_by_this_gate")
        result[key] = False
    return result


def run(args):
    require(__debug__ and not sys.flags.optimize, "qualification refuses optimized Python")
    repo, output = args.repo.resolve(strict=True), args.output.resolve()
    require(Path(__file__).resolve() == repo / "scripts/verify_record_enum_fields_compatibility.py",
            "run the controller from the selected checkout")
    require(output != repo and repo not in output.parents, "output must be outside repository")
    require(not output.exists(), "output must be new")
    output.mkdir(parents=True)
    report = {"schema": "oxid-project-workflow-current-result-v1", "status": "running",
              "scope": "retained bounded scalar/record semantics and resources after record enum fields; new field semantics qualified separately",
              "semantic_pass": False, "compiler_executions": 0, "baseline_commit": BASELINE,
              "started_at_utc": datetime.datetime.now(datetime.timezone.utc).isoformat(),
              "controller": identity("scripts/verify_record_enum_fields_compatibility.py", Path(__file__).read_bytes())}
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
