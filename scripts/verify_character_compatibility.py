#!/usr/bin/env python3
"""Replay retained narrow assertions after character support.

The complete floating-point predecessor and fifteen preceding source policies
remain byte-authenticated. Floating-point semantics use separate public/native oracles.
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

PREDECESSOR = '3f29097f838845da758e570ac3ec022666815504'
PREDECESSOR_POLICY = "scripts/verify_float_compatibility.py"
PREDECESSOR_POLICY_SHA = '6d289ee3d0889ca70a06594362e862bb9774134d1fbc627a63d35328fec38097'
PREDECESSOR_POLICY_BYTES = 34905
CHARACTER_CHANGED = frozenset(('src/frontend/ast.rs', 'src/frontend/declaration_index.rs', 'src/frontend/declaration_index/enum_views.rs', 'src/frontend/declaration_index/source_owner.rs', 'src/frontend/declaration_index/u8_reservation.rs', 'src/frontend/format.rs', 'src/frontend/format/ast_tests.rs', 'src/frontend/hir.rs', 'src/frontend/lexer.rs', 'src/frontend/native.rs', 'src/frontend/oir/comparison_tests.rs', 'src/frontend/oir/execute.rs', 'src/frontend/oir/lower.rs', 'src/frontend/oir/mod.rs', 'src/frontend/oir/native.rs', 'src/frontend/oir/native_emit_work.rs', 'src/frontend/oir/owned/array_native_tests.rs', 'src/frontend/oir/owned/array_reference_boundary_tests.rs', 'src/frontend/oir/owned/array_reference_tests.rs', 'src/frontend/oir/owned/array_tests.rs', 'src/frontend/oir/owned/enum_consumer_fixtures.rs', 'src/frontend/oir/owned/enum_match_tests.rs', 'src/frontend/oir/owned/enum_native_tests.rs', 'src/frontend/oir/owned/enum_reference_tests.rs', 'src/frontend/oir/owned/execute.rs', 'src/frontend/oir/owned/execute_values.rs', 'src/frontend/oir/owned/native.rs', 'src/frontend/oir/owned/plan.rs', 'src/frontend/oir/owned/reviewer_array_reference_tests.rs', 'src/frontend/oir/owned/shape.rs', 'src/frontend/oir/owned/source/array_pipeline_rows.rs', 'src/frontend/oir/owned/source/candidate_adapter.rs', 'src/frontend/oir/owned/source/hir.rs', 'src/frontend/oir/owned/source/lower.rs', 'src/frontend/oir/owned/source/mod.rs', 'src/frontend/oir/owned/source/resolve.rs', 'src/frontend/oir/owned/source/typeck.rs', 'src/frontend/oir/owned_types.rs', 'src/frontend/oir/owned_types/enums.rs', 'src/frontend/oir/source/association.rs', 'src/frontend/oir/source/hir_import/candidate.rs', 'src/frontend/oir/source/hir_import/candidate/typed_compare.rs', 'src/frontend/oir/source/sealed.rs', 'src/frontend/oir/verify.rs', 'src/frontend/parser.rs', 'src/frontend/parser/arrays.rs', 'src/frontend/parser/conversions.rs', 'src/frontend/parser/enums.rs', 'src/frontend/project.rs', 'src/frontend/typeck.rs'))
CHARACTER_ADDED = frozenset(('native/typed_character.c', 'src/frontend/oir/owned/source/character_tests.rs', 'src/frontend/parser/characters.rs'))
CHARACTER_INPUTS = {'native/typed_character.c': (1154,
                              'db1ad3447c090c983918a07c54892ef90802f085ed960804a5e65bce7a91d405'),
 'src/frontend/ast.rs': (49519, '7ab7ee23f4cbe292e27c3eae84f774f91dd9ce41883af1ea86229e3b626472cf'),
 'src/frontend/declaration_index.rs': (77962,
                                       'a516a9b54edddb6f431cf689cd8650f88a77bca6249767c8a97f0e7f309b87ad'),
 'src/frontend/declaration_index/enum_views.rs': (11385,
                                                  '1e7abc3bdb0d7c67fed31afd7826e7eb8b2be6954762d3109767c6d3a32ee555'),
 'src/frontend/declaration_index/source_owner.rs': (23274,
                                                    '9b5ea12274a96c94077030b4763094e3a34188b0d0b9a613dcc063f4655b6c64'),
 'src/frontend/declaration_index/u8_reservation.rs': (32281,
                                                      'd3f9b293c5b636f15b2398ee4536d7503ce26f08397cfcd64f818a167a226483'),
 'src/frontend/format.rs': (51011,
                            '87450b3303b00aa5d8e5282f7d4b2d235d8c54214d6141914722705f3b8fc26c'),
 'src/frontend/format/ast_tests.rs': (68887,
                                      '7b614edd890371aca51cf821882543abd6b8e7a6a52ec12d8884783a0af48627'),
 'src/frontend/hir.rs': (32211, '86f6a43f5c4c45fc400168488e95b6e6264d05c47df6c00565ad561a6dc0c3f8'),
 'src/frontend/lexer.rs': (36706,
                           'be4330aaa918ab1b205656963282d3da089fb0e3beb79c6c1f0ebaf0d8c286ab'),
 'src/frontend/native.rs': (19665,
                            'cad832d37da21310938b045e2c39f43aa89b5b82140a9ea100d418cdd032fed5'),
 'src/frontend/oir/comparison_tests.rs': (9866,
                                          '97f5f3716a7ecc6cc4503e3df5091d8e13c3bea6558f9733a33e45a6a4be6ba4'),
 'src/frontend/oir/execute.rs': (22730,
                                 'c819628154a44cc3a0d59c0cca1e9a9b490eae286af7ae7a75809c0e16dab000'),
 'src/frontend/oir/lower.rs': (42504,
                               'a18fb49e9e39954e70c896390e08e69c293b3651718fd1f98cadce8b01e8181c'),
 'src/frontend/oir/mod.rs': (24400,
                             'c9c63db99ac0be8311e6ec4dd5055082915f50dcc3996ada7a862e4c8d6b8538'),
 'src/frontend/oir/native.rs': (106862,
                                '740d1432b1710baec86c1787e8a635d074436515ee9383c7f087b00049df597d'),
 'src/frontend/oir/native_emit_work.rs': (29371,
                                          '63672604bf95502f292ffcccaee37a73c7e59707cb324766712327267d7f7be1'),
 'src/frontend/oir/owned/array_native_tests.rs': (52618,
                                                  '7d742594e079e597dd95c19c3bc4134ea31810c9e929875e0edf14e077bfd2af'),
 'src/frontend/oir/owned/array_reference_boundary_tests.rs': (34066,
                                                              '098a64f7ce15f55c6bbb49ae8bb094593aa91b0029b98bf793d1b60b7bd0ad28'),
 'src/frontend/oir/owned/array_reference_tests.rs': (45963,
                                                     'f89952cfe73c61ec6a78b00c17e48aa3d68b4d2bb190ee5282c88150e0dfa3bf'),
 'src/frontend/oir/owned/array_tests.rs': (34067,
                                           'bd5502ea4e5c4b20bea01215089220794cc18462e5b927ed4e7b46b296eb9c84'),
 'src/frontend/oir/owned/enum_consumer_fixtures.rs': (31748,
                                                      '7a88c3019bef9bdcae0c6057a77e23e6ae51ce2ce729e601a24079a9cd8f8583'),
 'src/frontend/oir/owned/enum_match_tests.rs': (50104,
                                                '136469d9a1e23ff5f59611cf660243c9aee9cae288976f6797eb7d2466693e43'),
 'src/frontend/oir/owned/enum_native_tests.rs': (62679,
                                                 '896a7fb784a43194d6280b46fda9e8aaa8e7be215125249a8950829e57a43666'),
 'src/frontend/oir/owned/enum_reference_tests.rs': (43706,
                                                    '6a22b532b7a2c8c456824dd2f3ac97a48cd9852589ac4806354bec27544e1ed9'),
 'src/frontend/oir/owned/execute.rs': (189188,
                                       '3bb0d72939294d70a0723c687b61f6d93fa719117cb8403e62ef00b0dcca275f'),
 'src/frontend/oir/owned/execute_values.rs': (17026,
                                              'a8993965d2af43ff3df4cbc6900650e3dd9adb3bf5598817e5b02c94407e3139'),
 'src/frontend/oir/owned/native.rs': (237980,
                                      'dea733ebc11c0b022a13cdfc94d5e9fccf2c500999de9afdb1a6f767a07416f3'),
 'src/frontend/oir/owned/plan.rs': (30951,
                                    '20c48c27ebaa5a4b6f650614419f90d56be56c2281c5aef2015523a65a7556de'),
 'src/frontend/oir/owned/reviewer_array_reference_tests.rs': (66191,
                                                              'c96b8b3f91eb9248c6b99518d73206c4cc9bfb1f50b71a7b15566f0925bf5926'),
 'src/frontend/oir/owned/shape.rs': (56394,
                                     '89d73e56516a5a759d8b9ffb061e05dad901ff506244697906f0e6191b0cad6b'),
 'src/frontend/oir/owned/source/array_pipeline_rows.rs': (55807,
                                                          '0037c69b39abaf9533426d8f82ca841b5928bd49aaa65cecdefd4dbc54f1b081'),
 'src/frontend/oir/owned/source/candidate_adapter.rs': (43463,
                                                        'b377e42b2e4094c7e7d2e0e56b74559c9878d940069f0f2c4b6a8e0154f9accf'),
 'src/frontend/oir/owned/source/character_tests.rs': (3218,
                                                      'f8fb06346a39727c2dc8bef32a905750c5701000e24db8e412b99237f6dce268'),
 'src/frontend/oir/owned/source/hir.rs': (6675,
                                          '8d3770436822db25a844bda0437b9d45325e445ba1ac8b01d61cde1ca1075f52'),
 'src/frontend/oir/owned/source/lower.rs': (117220,
                                            'd90141fd274b9aca889cb08d2e1e900de85e45d0cb0596df178769d5e96ae6a4'),
 'src/frontend/oir/owned/source/mod.rs': (2150,
                                          '76035ce75adb9e6645f68b4029a173949303d5532e932e60d13f67dd7c12a31a'),
 'src/frontend/oir/owned/source/resolve.rs': (147904,
                                              'a0d3aaffcf4a54b2fd6ac7ef730089616bdcd7308221fcbac0282abc56c01707'),
 'src/frontend/oir/owned/source/typeck.rs': (213685,
                                             '0099d54669b6c7138b9ecd9163f688c839d7a3f966c43fa809e17eef881ba7cb'),
 'src/frontend/oir/owned_types.rs': (80125,
                                     '0b88d5be2ce9a4b78074f8999dbabe597161cd07e024c93e978192072f730bd0'),
 'src/frontend/oir/owned_types/enums.rs': (57668,
                                           '8fb98eff7263c9ed0250545a866a1275307e031d377f356316d3a532893ece0b'),
 'src/frontend/oir/source/association.rs': (33587,
                                            '94d25f1d80845c151cc50465e019681811c28016a204d42f3c9517df1f6e0bc3'),
 'src/frontend/oir/source/hir_import/candidate.rs': (52822,
                                                     'f345e6aacaa006d25125db2b6c134365ac380903a8ef23487e015b2e11be963a'),
 'src/frontend/oir/source/hir_import/candidate/typed_compare.rs': (17614,
                                                                   '52d840459eab2ca0da67d72a49fd6c78ddae23ba8a889d78c32913a5d015fb12'),
 'src/frontend/oir/source/sealed.rs': (23673,
                                       '2ebee4addf286bbe63855c3b01e743b129005482122646f7015ca38d1c519397'),
 'src/frontend/oir/verify.rs': (53166,
                                'bfe96277a9f41839a23633db55be2701af262d9f63b2bb595829e39cd4a1f5f7'),
 'src/frontend/parser.rs': (87348,
                            'c2e25738c227a0fdb2d5933b6f5c397c788ea08347dd63be53d849bb7473de09'),
 'src/frontend/parser/arrays.rs': (10860,
                                   'a56b0d30f89650bdda0c80ee030e6cee386b24d2ea156afdbe2c1e93cdff702a'),
 'src/frontend/parser/characters.rs': (3383,
                                       '5c593d43b8f907a2764f1a9c53791dfebc2a8a46d260d7e0e33a330a6c5afee2'),
 'src/frontend/parser/conversions.rs': (3663,
                                        '06f3ab07efa6cc01e84cd9495a5cc13f8fea484e06367b57221c457b2ad039fa'),
 'src/frontend/parser/enums.rs': (16771,
                                  '08826ce1e3c795fc4842c31470e4848a68e2a7b462618883054ee07c37661fbc'),
 'src/frontend/project.rs': (174422,
                             '76a3817e539f61d1bf748ad6de75398daa3e8c516292b4c431e6cb2abb40c02e'),
 'src/frontend/typeck.rs': (36499,
                            'b57b2518cf9a6d47f0f19d2acf1858e79ff31b640943acf4a46737cc123b311c')}
REVIEWED_DIRECTIVES = {'native/typed_character.c': ([], []),
 'src/frontend/native.rs': ([b'include_str!("../../native/typed_preview.c")',
                             b'include_str!("../../native/typed_character.c")',
                             b'include_str!("../../native/typed_float.c")',
                             b'include_bytes!(concat!(env!("OUT_DIR")',
                             b'include_str!("../../native/typed_integer128.c")',
                             b'include_str!("../../native/typed_bytebuf.c")',
                             b'include_str!("../../native/experimental_bytebuf.c")',
                             b'include_str!("../../native/experimental_files.c")',
                             b'include_str!("../../native/experimental_arguments.c")',
                             b'include_str!("../../native/experimental_bytebuf.h")'],
                            []),
 'src/frontend/oir/owned/source/character_tests.rs': ([], []),
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
 'src/frontend/parser/characters.rs': ([], [])}

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
PARENT_ADDED = PREDECESSOR_ADDED | CHARACTER_ADDED
directives = library.directives
PREDECESSOR_POLICIES = {**library.PREDECESSOR_POLICIES, PREDECESSOR_POLICY: PREDECESSOR_POLICY_SHA}


def authenticate_policies(repo, parent):
    for name, expected in PREDECESSOR_POLICIES.items():
        before = regular(parent, name)
        require(sha(before) == expected, "wrong pinned predecessor source policy: " + name)
        require(regular(repo, name) == before, "predecessor source policy changed: " + name)


OBSERVER_ADAPTER = "scripts/character_compatibility_observer.py"
OBSERVER_ADAPTER_BYTES = 3548
OBSERVER_ADAPTER_SHA = '6acae0c103ec7cdd2df4556e00db2497bb0197ed0bda197d3b4ee45df9b812b2'

def authenticate_observer_adapter(repo):
    body = regular(repo, OBSERVER_ADAPTER)
    require(len(body) == OBSERVER_ADAPTER_BYTES and sha(body) == OBSERVER_ADAPTER_SHA,
            "changed pinned character observer adapter")
    adapter = types.ModuleType("_character_observer_successor")
    adapter.__file__ = str(repo / OBSERVER_ADAPTER)
    exec(compile(body, adapter.__file__, "exec"), adapter.__dict__)
    return adapter, body
run_file_observer_controls = library.run_file_observer_controls
run_i64_observer_controls = library.run_i64_observer_controls
run_integer_width_observer_controls = library.run_integer_width_observer_controls
run_integer128_observer_controls = library.run_integer128_observer_controls
run_float_observer_controls = library.run_float_observer_controls


def run_character_observer_controls(repo, output, captured, seam, original, adapter):
    """Run character exclusions in addition to all unchanged prior observer controls."""
    run = output / "unit2/run"
    controls = output / "character-observer-controls"
    controls.mkdir()
    protocol_body = regular(Path(seam["resource_package_root"]), "protocol.py")
    require(protocol_body == captured["historical_bytes"]["protocol.py"], "changed control protocol")
    protocol = types.ModuleType("_character_observer_protocol")
    exec(compile(protocol_body, "protocol.py", "exec"), protocol.__dict__)
    receipts = []
    for profile in ("debug", "release"):
        original.assert_unchanged(repo, captured)
        receipt_body = (run / (profile + "-receipt.json")).read_bytes()
        receipt = json.loads(receipt_body)
        binary = Path(receipt["binary"])
        require(binary.is_absolute() and (run / "target") in binary.parents and not binary.is_symlink(),
                "float control binary outside isolated build")
        require(sha(binary.read_bytes()) == receipt["binary_sha256"], "float control binary changed")
        assembly_body = (run / "assembly.json").read_bytes()
        original.check_entries(run / "source", json.loads(assembly_body)["files"], exact=True)
        verify_current_observer(output, adapter, captured["observer"])
        commands, streams = [], {}
        for label, filters in (("list", ["--list", adapter.CHARACTER_CONTROL_FILTER]),
                               ("run", [adapter.CHARACTER_CONTROL_FILTER, "--test-threads=1", "--color", "never"])):
            argv = [str(binary), *filters]
            out, err = controls / (profile + "-" + label + ".stdout"), controls / (profile + "-" + label + ".stderr")
            with out.open("xb") as stdout, err.open("xb") as stderr:
                completed = subprocess.run(argv, cwd=run / "source", stdout=stdout, stderr=stderr, timeout=60)
            commands.append({"argv": argv, "exit_status": completed.returncode,
                             "stdout": identity(out.name, out.read_bytes()),
                             "stderr": identity(err.name, err.read_bytes())})
            (controls / (profile + "-commands.json")).write_bytes(encoded(commands))
            require(completed.returncode == 0, "File observer controls failed; see retained streams")
            streams[label] = out.read_text(encoding="utf-8")
        names = list(adapter.CHARACTER_CONTROL_NAMES)
        protocol.rust_listing(streams["list"], names)
        result = protocol.rust_success(streams["run"], [name + " - should panic" for name in names])
        require(sha(binary.read_bytes()) == receipt["binary_sha256"], "float control binary changed")
        require((run / "assembly.json").read_bytes() == assembly_body
                and (run / (profile + "-receipt.json")).read_bytes() == receipt_body,
                "float observer source binding changed")
        original.check_entries(run / "source", json.loads(assembly_body)["files"], exact=True)
        verify_current_observer(output, adapter, captured["observer"])
        original.assert_unchanged(repo, captured)
        record = {"status": "passed", "profile": profile, "tests": names, "result": result,
                  "binary": identity(str(binary), binary.read_bytes()),
                  "source_inputs_sha256": original.CURRENT_SOURCE_SHA,
                  "commands": identity(profile + "-commands.json", (controls / (profile + "-commands.json")).read_bytes())}
        path = controls / (profile + "-receipt.json")
        path.write_bytes(encoded(record))
        receipts.append(identity(str(path.relative_to(output)), path.read_bytes()))
    return {"character_observer_control_tests_per_profile": 2,
            "character_observer_control_receipts": receipts,
            "test_function_executions": 2 * (1 + 21 + 6 + 3 + 3 + 2 + 14 + 4 + 4 + 2)}


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
    expected = expected_parent | CHARACTER_ADDED
    require(set(parent) == expected_parent, "wrong predecessor compiler input membership")
    require(set(CHARACTER_INPUTS) == CHARACTER_CHANGED | CHARACTER_ADDED
            and CHARACTER_CHANGED <= expected_parent and not CHARACTER_ADDED & expected_parent,
            "wrong reviewed character input membership")
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
        if name not in CHARACTER_CHANGED:
            require(inputs[name] == before, "unreviewed change outside character boundary: " + name)
    for name, (length, digest) in CHARACTER_INPUTS.items():
        require(len(inputs[name]) == length and sha(inputs[name]) == digest,
                "character source does not match reviewed input: " + name)
        require(directives(inputs[name]) == REVIEWED_DIRECTIVES.get(name, directives(parent[name]) if name in parent else ([], [])),
                "character compile-time input directive changed: " + name)
    return inputs


def current_manifest(inputs, baseline, checkout, parent):
    changes = [{"path": name, "before": identity(name, baseline[name]), "after": identity(name, body)}
               for name, body in sorted(inputs.items()) if name in baseline and body != baseline[name]]
    result = {"schema": "oxid-project-workflow-current-source-v1", "baseline_commit": BASELINE,
              "checkout": checkout, "changes": changes, "additions": sorted(PARENT_ADDED),
              "assertion_scope": "retained bounded scalar/record semantics and resources",
              "source_policy_parent": PREDECESSOR, "source_policy_inputs": PREDECESSOR_POLICIES,
              "character_reviewed_inputs": [{"path": name, "bytes": length, "sha256": digest}
                                          for name, (length, digest) in sorted(CHARACTER_INPUTS.items())],
              "character_changes": [{"path": name, "before": identity(name, parent[name]),
                                  "after": identity(name, inputs[name])} for name in sorted(CHARACTER_CHANGED)],
              "unchanged_baseline_inputs": len(baseline) - len(changes),
              "files": [identity(name, body) for name, body in sorted(inputs.items())]}
    for name in ("package", "workspace", "native_reproducibility", "native_optimization", "owned_enum",
                 "byte_field", "byte_buffer", "binary_io", "native_heap_safety", "hosted_execution",
                 "owned_buffer_transfer", "byte_string", "native_file", "program_arguments", "i64", "integer_widths", "byte_integer", "integer128", "floats", "characters"):
        key = name + ("_qualified_by_this_gate" if name.startswith("native_") and name != "native_file" else "_semantics_qualified_by_this_gate")
        result[key] = False
    return result


def run(args):
    require(__debug__ and not sys.flags.optimize, "qualification refuses optimized Python")
    repo, output = args.repo.resolve(strict=True), args.output.resolve()
    require(Path(__file__).resolve() == repo / "scripts/verify_character_compatibility.py",
            "run the controller from the selected checkout")
    require(output != repo and repo not in output.parents, "output must be outside repository")
    require(not output.exists(), "output must be new")
    output.mkdir(parents=True)
    report = {"schema": "oxid-project-workflow-current-result-v1", "status": "running",
              "scope": "retained bounded scalar/record semantics and resources on the character successor; character operations qualified separately",
              "semantic_pass": False, "compiler_executions": 0, "baseline_commit": BASELINE,
              "started_at_utc": datetime.datetime.now(datetime.timezone.utc).isoformat(),
              "controller": identity("scripts/verify_character_compatibility.py", Path(__file__).read_bytes())}
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
