#!/usr/bin/env python3
"""Replay retained bounded assertions after local reference lifetime verification.

The reviewed delta preserves all other compiler inputs and preceding policies.
Raw local-reference lifetimes and runtime checks are qualified separately.
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

PREDECESSOR = '4afb0ec06d66ddf0e8eed43e285d0fc42d18672b'
PREDECESSOR_POLICY = 'scripts/verify_disjoint_fields_compatibility.py'
PREDECESSOR_POLICY_SHA = 'b42714328ed65eeaae7c7a6598e0b4936c5f0e62d7328ead99c1ec2b8e68d98e'
PREDECESSOR_POLICY_BYTES = 21042
LOCAL_REFERENCES_CHANGED = frozenset({'src/frontend/oir/owned/builtin_output_fixtures.rs', 'src/frontend/oir/owned/array_native_tests.rs', 'src/frontend/oir/owned/builtin_input_fixtures.rs', 'src/frontend/oir/owned/native_heldout_review.rs', 'src/frontend/oir/owned/mod.rs', 'src/frontend/oir/owned/budget.rs', 'src/frontend/oir/owned/denial_tests.rs', 'src/frontend/oir/owned/source/candidate_native.rs', 'src/frontend/oir/owned/source/diagnostic.rs', 'src/frontend/oir/owned/tests.rs', 'src/frontend/oir/owned/oracle_tests.rs', 'src/frontend/oir/owned/reviewer_array_reference_tests.rs', 'src/frontend/oir/owned/shape.rs', 'src/frontend/oir/owned/reviewer_reference_tests.rs', 'src/frontend/oir/owned/composition_reference_tests.rs', 'src/frontend/oir/owned/flow.rs', 'src/frontend/oir/owned/verified.rs', 'src/frontend/oir/owned/plan.rs', 'src/frontend/oir/owned/builtin_descriptor_tests.rs', 'src/frontend/oir/owned/array_reference_tests.rs', 'src/frontend/oir/owned/native.rs', 'src/frontend/oir/owned/native_tests.rs', 'src/frontend/oir/owned/consumer_fixtures.rs', 'src/frontend/oir/owned/reviewer_heldout.rs', 'src/frontend/oir/owned/source/array_pipeline_rows.rs', 'src/frontend/oir/owned/source/association.rs', 'src/frontend/oir/owned/source/candidate_adapter.rs', 'src/frontend/oir/owned/consumer_pilot.rs', 'src/frontend/oir/owned/native_experimental.rs', 'src/frontend/oir/owned/execute_tests.rs', 'src/frontend/oir/owned/source/lower.rs', 'src/frontend/oir/owned/execute.rs'})
LOCAL_REFERENCES_ADDED = frozenset()
LOCAL_REFERENCES_INPUTS = {'src/frontend/oir/owned/array_native_tests.rs': (53710,
                                                  '1af9d2446da9666239398d7caf2c1ea59a2bdfd573edc364e89a10064bbb1669'),
 'src/frontend/oir/owned/array_reference_tests.rs': (46017,
                                                     '9b052c65add8a38345b447d2747ae3ff8fa1c35c46148cc66189681afca40c6f'),
 'src/frontend/oir/owned/budget.rs': (20448,
                                      'b2efb520e04e5a457ac69ab7eadb7e93e6aad2c3551af45a2156c4419b047228'),
 'src/frontend/oir/owned/builtin_descriptor_tests.rs': (23886,
                                                        'db7e2bb2c2a34320748d762f34bbb08d44e571ed616aae54a97301136b2c8828'),
 'src/frontend/oir/owned/builtin_input_fixtures.rs': (20462,
                                                      '6074c13c9dd7ebd549b42a1571b2311f2d428462ee828f8616d66e363a0ec297'),
 'src/frontend/oir/owned/builtin_output_fixtures.rs': (9930,
                                                       'af90ca4bf2a1d8b4c5354668712343d11b0e48e3787b0f108ad05428227026d9'),
 'src/frontend/oir/owned/composition_reference_tests.rs': (29039,
                                                           'a91f4c76a9dcab80b43fc5d904e39b9a04ec932c270086310138d9ec98fccbdb'),
 'src/frontend/oir/owned/consumer_fixtures.rs': (45920,
                                                 '2ca0b966e89ce417d7a73e9e8fcb675c190296299675f3da59186fb3a517eb9d'),
 'src/frontend/oir/owned/consumer_pilot.rs': (35242,
                                              'dc30214b088ff45cbccb530ddb96ccd1e5d9704756097dff550ef1954a8a9163'),
 'src/frontend/oir/owned/denial_tests.rs': (27237,
                                            '660315a065927d8587c07decec7e43245e39a79460b3fc63c285c9898db71526'),
 'src/frontend/oir/owned/execute.rs': (209673,
                                       'a64bb4fda3260c46d558f704de5b77e99a111659a19908871c74aba9e830183a'),
 'src/frontend/oir/owned/execute_tests.rs': (48977,
                                             '61b3b02492442bfb6821d3bd0a1e7b936d1469899e3c532e1fc05a54c00b49ff'),
 'src/frontend/oir/owned/flow.rs': (53263,
                                    '9f24551a19a959c6253776f7ca4609759d6305efdec46295c9c49ce0a70e497b'),
 'src/frontend/oir/owned/mod.rs': (15874,
                                   '5306789b80c71830ff38f72700a733704f7e9cbbb5caba037bc6005afa3d457e'),
 'src/frontend/oir/owned/native.rs': (259756,
                                      'a474ced621400e37cc99881a117f2b97ea5c56922ebfc71e75f9882de0e64d4d'),
 'src/frontend/oir/owned/native_experimental.rs': (16967,
                                                   'fd33961fd411cb7e93eea35396ad61563be412c5eb0766b8af5d5a2e84c1d8b8'),
 'src/frontend/oir/owned/native_heldout_review.rs': (25828,
                                                     'ae330c8fdcc480cc536b3d6cfbe2172b8d15ec04c1d58adef704e687a4ac7e2c'),
 'src/frontend/oir/owned/native_tests.rs': (90881,
                                            '4a99138fc8cdc57c914f648a29dd309faa17e1fdbf54799cf85f82006a7d6ad6'),
 'src/frontend/oir/owned/oracle_tests.rs': (114848,
                                            '83e3cf962ac4c988daaca315f6a6a6e8116e6842f3eea20c0720d971eb0af8e9'),
 'src/frontend/oir/owned/plan.rs': (31851,
                                    'd72df158a25d5676a3090ae82df3fafd40acaf686961992693239cc7bdb65c6b'),
 'src/frontend/oir/owned/reviewer_array_reference_tests.rs': (66307,
                                                              '2d7d7075e5f4097668bb3c5833230c2397a867025c59d7ce45a6e059ca2100fb'),
 'src/frontend/oir/owned/reviewer_heldout.rs': (58080,
                                                'f360c17efee550a2dbe4bbde3daec3df4455d8a38ea5daaab2872b5972a203fa'),
 'src/frontend/oir/owned/reviewer_reference_tests.rs': (71982,
                                                        'd9349b5faac5e70ab4b81af8340d8b93dc1967d0a2ca5a315a1986a1d5c37b28'),
 'src/frontend/oir/owned/shape.rs': (59961,
                                     'b304ecfe7a079c5ea2e202ea4c01d0180b9ae09e84f3da27291ea5976f7645a6'),
 'src/frontend/oir/owned/source/array_pipeline_rows.rs': (57064,
                                                          'b23aa16c7d96c541d0c365285d52edd1720837c8343a08f360daa6e6b8144e27'),
 'src/frontend/oir/owned/source/association.rs': (82043,
                                                  '63b25d9a92b99bcaab676d03f37c76b13f965b3dae0365bcda5448562ed19dc2'),
 'src/frontend/oir/owned/source/candidate_adapter.rs': (44331,
                                                        '756117af462389a44e2fba5c7927f5a91fd1da82afa94d53e219c9cd590c9f1c'),
 'src/frontend/oir/owned/source/candidate_native.rs': (32666,
                                                       '695bf5b5a2260c9cc4f0587e26c46ca9447b4bc6e2100a96d84ec86171c7231a'),
 'src/frontend/oir/owned/source/diagnostic.rs': (8158,
                                                 'bf76ad0fc8aaed1a51ddada220abacdbab6b2a11f870db59f886f5e5dbb8fe84'),
 'src/frontend/oir/owned/source/lower.rs': (128433,
                                            '7998540c7e70c22f9c05112af9028ee7fb2b561240647dae51e4a76df05912bc'),
 'src/frontend/oir/owned/tests.rs': (78804,
                                     'a21f6d9492e1b1a95ac6b7ecfb0cbd15126beebc126ae670a29551edb000cb4c'),
 'src/frontend/oir/owned/verified.rs': (28914,
                                        '3ef2f3088bb1f5c84440ec35c24c58517ec47220caf5795fbbd59aeb7525ca45')}
REVIEWED_DIRECTIVES = {'src/frontend/oir/owned/array_native_tests.rs': ([], []),
 'src/frontend/oir/owned/array_reference_tests.rs': ([],
                                                     [b'#[path = "array_refe'
                                                      b'rence_boundary_tests'
                                                      b'.rs"]']),
 'src/frontend/oir/owned/budget.rs': ([], []),
 'src/frontend/oir/owned/builtin_descriptor_tests.rs': ([], []),
 'src/frontend/oir/owned/builtin_input_fixtures.rs': ([], []),
 'src/frontend/oir/owned/builtin_output_fixtures.rs': ([], []),
 'src/frontend/oir/owned/composition_reference_tests.rs': ([], []),
 'src/frontend/oir/owned/consumer_fixtures.rs': ([], []),
 'src/frontend/oir/owned/consumer_pilot.rs': ([], []),
 'src/frontend/oir/owned/denial_tests.rs': ([], []),
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
                                        b'#[path = "builtin_output_reference_t'
                                        b'ests.rs"]',
                                        b'#[path = "byte_storage_codec_tests.r'
                                        b's"]']),
 'src/frontend/oir/owned/execute_tests.rs': ([b'include!("reviewer_reference'
                                              b'_tests.rs")'],
                                             [b'#[path = "source/runtime_res'
                                              b'ource_tests.rs"]',
                                              b'#[path = "source/reviewer_re'
                                              b'source_runtime.rs"]']),
 'src/frontend/oir/owned/flow.rs': ([], []),
 'src/frontend/oir/owned/mod.rs': ([], []),
 'src/frontend/oir/owned/native.rs': ([],
                                      [b'#[path = "native_bitwise.rs"]',
                                       b'#[path = "native_composition.rs"]',
                                       b'#[path = "native_experimental.rs"]',
                                       b'#[path = "native_floats.rs"]',
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
                                       b'#[path = "hosted_capacity_tests.rs"]',
                                       b'#[path = "byte_storage_native_tests.'
                                       b'rs"]']),
 'src/frontend/oir/owned/native_experimental.rs': ([], []),
 'src/frontend/oir/owned/native_heldout_review.rs': ([], []),
 'src/frontend/oir/owned/native_tests.rs': ([],
                                            [b'#[path = "source/native_resource'
                                             b'_tests.rs"]',
                                             b'#[path = "native_heldout_review.'
                                             b'rs"]',
                                             b'#[path = "source/reviewer_resour'
                                             b'ce_native.rs"]',
                                             b'#[path = "array_native_resource_'
                                             b'tests.rs"]',
                                             b'#[path = "array_native_tests.rs"'
                                             b']',
                                             b'#[path = "slice_native_tests.rs"'
                                             b']',
                                             b'#[path = "composition_native_tes'
                                             b'ts.rs"]',
                                             b'#[path = "projected_slice_native'
                                             b'_tests.rs"]',
                                             b'#[path = "enum_native_tests.rs"]']),
 'src/frontend/oir/owned/oracle_tests.rs': ([], []),
 'src/frontend/oir/owned/plan.rs': ([], [b'#[path = "native_storage.rs"]']),
 'src/frontend/oir/owned/reviewer_array_reference_tests.rs': ([], []),
 'src/frontend/oir/owned/reviewer_heldout.rs': ([], []),
 'src/frontend/oir/owned/reviewer_reference_tests.rs': ([], []),
 'src/frontend/oir/owned/shape.rs': ([], []),
 'src/frontend/oir/owned/source/array_pipeline_rows.rs': ([], []),
 'src/frontend/oir/owned/source/association.rs': ([],
                                                  [b'#[path = "output_lower_t'
                                                   b'ests.rs"]',
                                                   b'#[path = "byte_storage_a'
                                                   b'ssociation_tests.rs"]']),
 'src/frontend/oir/owned/source/candidate_adapter.rs': ([],
                                                        [b'#[path = "candidate_'
                                                         b'mutations.rs"]',
                                                         b'#[path = "candidate_'
                                                         b'native.rs"]']),
 'src/frontend/oir/owned/source/candidate_native.rs': ([],
                                                       [b'#[ignore = "bounded '
                                                        b'native source collec'
                                                        b'tor; requires frozen'
                                                        b' original source, bu'
                                                        b'dget controls, pinne'
                                                        b'd LLVM and a fresh o'
                                                        b'utput path"]']),
 'src/frontend/oir/owned/source/diagnostic.rs': ([], []),
 'src/frontend/oir/owned/source/lower.rs': ([], []),
 'src/frontend/oir/owned/tests.rs': ([],
                                     [b'#[path = "reviewer_heldout.rs"]',
                                      b'#[path = "array_tests.rs"]']),
 'src/frontend/oir/owned/verified.rs': ([], [])}


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
            "changed pinned field-borrowing source policy")
    module = types.ModuleType("_local_references_compatibility_parent")
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
PARENT_ADDED = PREDECESSOR_ADDED | LOCAL_REFERENCES_ADDED
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
    expected = expected_parent | LOCAL_REFERENCES_ADDED
    require(set(parent) == expected_parent, "wrong predecessor compiler input membership")
    require(set(LOCAL_REFERENCES_INPUTS) == LOCAL_REFERENCES_CHANGED | LOCAL_REFERENCES_ADDED
            and LOCAL_REFERENCES_CHANGED <= expected_parent and not LOCAL_REFERENCES_ADDED & expected_parent,
            "wrong reviewed local-references input membership")
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
        if name not in LOCAL_REFERENCES_CHANGED:
            require(inputs[name] == before, "unreviewed change outside local-references boundary: " + name)
    for name, (length, digest) in LOCAL_REFERENCES_INPUTS.items():
        require(len(inputs[name]) == length and sha(inputs[name]) == digest,
                "local-references source does not match reviewed input: " + name)
        require(directives(inputs[name]) == REVIEWED_DIRECTIVES.get(name, directives(parent[name]) if name in parent else ([], [])),
                "local-references compile-time input directive changed: " + name)
    return inputs


def current_manifest(inputs, baseline, checkout, parent):
    changes = [{"path": name, "before": identity(name, baseline[name]), "after": identity(name, body)}
               for name, body in sorted(inputs.items()) if name in baseline and body != baseline[name]]
    result = {"schema": "oxid-project-workflow-current-source-v1", "baseline_commit": BASELINE,
              "checkout": checkout, "changes": changes, "additions": sorted(PARENT_ADDED),
              "assertion_scope": "retained bounded scalar/record semantics and resources",
              "source_policy_parent": PREDECESSOR, "source_policy_inputs": PREDECESSOR_POLICIES,
              "local_references_reviewed_inputs": [{"path": name, "bytes": length, "sha256": digest}
                                          for name, (length, digest) in sorted(LOCAL_REFERENCES_INPUTS.items())],
              "local_references_changes": [{"path": name, "before": identity(name, parent[name]),
                                  "after": identity(name, inputs[name])} for name in sorted(LOCAL_REFERENCES_CHANGED)],
              "unchanged_baseline_inputs": len(baseline) - len(changes),
              "files": [identity(name, body) for name, body in sorted(inputs.items())]}
    for name in ("package", "workspace", "native_reproducibility", "native_optimization", "owned_enum",
                 "byte_field", "byte_buffer", "binary_io", "native_heap_safety", "hosted_execution",
                 "owned_buffer_transfer", "byte_string", "native_file", "program_arguments", "i64", "integer_widths", "byte_integer", "integer128", "floats", "characters", "project_formatting", "record_field_borrowing", "hosted_native_recursion", "git_dependencies", "git_source_bundles", "inferred_generic_functions", "explicit_generic_arguments", "nominal_generics", "record_patterns", "record_enum_fields", "constructor_inference", "linux_native", "range_for", "hosted_heap", "documentation", "native_debug_variables", "language_server", "editor_navigation", "disjoint_fields", "local_references"):
        key = name + ("_qualified_by_this_gate" if name.startswith("native_") and name != "native_file" else "_semantics_qualified_by_this_gate")
        result[key] = False
    return result


def run(args):
    require(__debug__ and not sys.flags.optimize, "qualification refuses optimized Python")
    repo, output = args.repo.resolve(strict=True), args.output.resolve()
    require(Path(__file__).resolve() == repo / "scripts/verify_local_references_compatibility.py",
            "run the controller from the selected checkout")
    require(output != repo and repo not in output.parents, "output must be outside repository")
    require(not output.exists(), "output must be new")
    output.mkdir(parents=True)
    report = {"schema": "oxid-project-workflow-current-result-v1", "status": "running",
              "scope": "retained bounded scalar/record semantics and resources; local reference lifetimes qualified separately through raw verifier, reference and native tests",
              "semantic_pass": False, "compiler_executions": 0, "baseline_commit": BASELINE,
              "started_at_utc": datetime.datetime.now(datetime.timezone.utc).isoformat(),
              "controller": identity("scripts/verify_local_references_compatibility.py", Path(__file__).read_bytes())}
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
        env = dict(os.environ, CARGO_INCREMENTAL="0", CARGO_BUILD_JOBS="1", PYTHONDONTWRITEBYTECODE="1")
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
