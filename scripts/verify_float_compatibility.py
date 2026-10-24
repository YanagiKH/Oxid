#!/usr/bin/env python3
"""Replay retained narrow assertions after floating-point support.

The complete integer-bit predecessor and fourteen preceding source policies
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

PREDECESSOR = '20e029bd309db58aec04e04adcd91bea801d1c82'
PREDECESSOR_POLICY = 'scripts/verify_integer_bit_compatibility.py'
PREDECESSOR_POLICY_SHA = '34319c05343781386c75669ffc3d33ba3755d17d75f4c8c69710b3920dfaad1f'
PREDECESSOR_POLICY_BYTES = 24827
FLOAT_CHANGED = frozenset(('build.rs', 'src/frontend/ast.rs', 'src/frontend/declaration_index.rs', 'src/frontend/declaration_index/enum_views.rs', 'src/frontend/declaration_index/source_owner.rs', 'src/frontend/declaration_index/u8_reservation.rs', 'src/frontend/format.rs', 'src/frontend/format/ast_tests.rs', 'src/frontend/hir.rs', 'src/frontend/native.rs', 'src/frontend/oir/comparison_tests.rs', 'src/frontend/oir/execute.rs', 'src/frontend/oir/lower.rs', 'src/frontend/oir/mod.rs', 'src/frontend/oir/native.rs', 'src/frontend/oir/native_emit_work.rs', 'src/frontend/oir/owned/array_native_tests.rs', 'src/frontend/oir/owned/array_reference_boundary_tests.rs', 'src/frontend/oir/owned/array_reference_tests.rs', 'src/frontend/oir/owned/array_tests.rs', 'src/frontend/oir/owned/enum_consumer_fixtures.rs', 'src/frontend/oir/owned/enum_match_tests.rs', 'src/frontend/oir/owned/enum_native_tests.rs', 'src/frontend/oir/owned/enum_reference_tests.rs', 'src/frontend/oir/owned/execute.rs', 'src/frontend/oir/owned/execute_values.rs', 'src/frontend/oir/owned/native.rs', 'src/frontend/oir/owned/plan.rs', 'src/frontend/oir/owned/reviewer_array_reference_tests.rs', 'src/frontend/oir/owned/shape.rs', 'src/frontend/oir/owned/source/array_pipeline_rows.rs', 'src/frontend/oir/owned/source/candidate_adapter.rs', 'src/frontend/oir/owned/source/hir.rs', 'src/frontend/oir/owned/source/lower.rs', 'src/frontend/oir/owned/source/mod.rs', 'src/frontend/oir/owned/source/resolve.rs', 'src/frontend/oir/owned/source/typeck.rs', 'src/frontend/oir/owned_types.rs', 'src/frontend/oir/owned_types/enums.rs', 'src/frontend/oir/source/association.rs', 'src/frontend/oir/source/hir_import/candidate.rs', 'src/frontend/oir/source/hir_import/candidate/typed_compare.rs', 'src/frontend/oir/source/sealed.rs', 'src/frontend/oir/verify.rs', 'src/frontend/parser.rs', 'src/frontend/parser/arrays.rs', 'src/frontend/parser/conversions.rs', 'src/frontend/parser/enums.rs', 'src/frontend/typeck.rs', 'tests/typed_frontend.rs'))
FLOAT_ADDED = frozenset(('native/float_format.rs', 'native/typed_float.c', 'src/frontend/oir/owned/execute_floats.rs', 'src/frontend/oir/owned/native_floats.rs', 'src/frontend/oir/owned/source/float_tests.rs'))
FLOAT_INPUTS = {'build.rs': (1649, '1b165d401459d0e1799dafe6d368b2646aa78c63258fef5762e050d3d6d65f41'),
 'native/float_format.rs': (1458, '2648b90a9d7e294fe94aff48b9746fe17fbb36f0ee0c180b0284a078e8df7d8b'),
 'native/typed_float.c': (610, '270c7c41e5c825a3bb8217543cacb71f15dbd292c58e0d164edf836c480a2e28'),
 'src/frontend/ast.rs': (48736, 'e97a08de94cf4cc526f3320fe7d0543100c9fd6a54fd53c716bc6a18b6a1889a'),
 'src/frontend/declaration_index.rs': (77592,
                                       '6e29b795b4f2293f10aa96249e7a67d879b54ae55ee1476950fcf12b26059193'),
 'src/frontend/declaration_index/enum_views.rs': (11328,
                                                  'df7da9f7b39353ba366567fc8670b168d8d58ba632d9ef77f39ab4323c61ad1b'),
 'src/frontend/declaration_index/source_owner.rs': (23224,
                                                    '24ba2c1c4e0675198e9fa4c78d193327b405d1e19ef8d5310880111e53f13fd2'),
 'src/frontend/declaration_index/u8_reservation.rs': (32019,
                                                      '0097b05168c1396bf62bd6123c3465988a8326a9c6ecad0b92c5537c63066346'),
 'src/frontend/format.rs': (51003, '78ae009d3ba893b610d979fc113e2795b2bbc3ccac36b0e6a5ed5e9e8bfa873a'),
 'src/frontend/format/ast_tests.rs': (68390,
                                      'b055b71b36c9cfa869d0183d010c492e1270fc46c8dada2921efe2143e91c5e0'),
 'src/frontend/hir.rs': (31767, '162bf38eaa6e531c847ec2b0f94f78d204a805492435dd5ec024db3f9b499376'),
 'src/frontend/native.rs': (19239, '806606091bdb3284b5d8a984f9b248233e364bff94ff69894ab923512be758d9'),
 'src/frontend/oir/comparison_tests.rs': (9826,
                                          '496732f3fc0bc9b8af162cef15bbd6a07ed3b38ca07bc228be3490cf98208a1c'),
 'src/frontend/oir/execute.rs': (22641,
                                 'f563a744ac4f26a4f899f22df5bc0bc5eae71c15da6959a8eea8739e0b867938'),
 'src/frontend/oir/lower.rs': (42390,
                               'dd990a19987341af19a00e0142fb2bda1106306a44249b9401e6dd645e28a9d5'),
 'src/frontend/oir/mod.rs': (23658, '2df95c01257c37467669f76a1c9de9d12c7ef57027dbab14de4357b661b2fb7f'),
 'src/frontend/oir/native.rs': (106749,
                                'd4344f9010ab00dd89ccb6497b0c8aad847406843ae99892a4e870a1da5d69ad'),
 'src/frontend/oir/native_emit_work.rs': (29290,
                                          'bd8c594c7a6933ec3f9f2faa0a66d4836b6954d6de242a761b74bf8ab817ef5b'),
 'src/frontend/oir/owned/array_native_tests.rs': (52534,
                                                  '4b0149907e2c2a5ff4f5648a49fdbdbe473db88643afd8277c0dacfe614e9259'),
 'src/frontend/oir/owned/array_reference_boundary_tests.rs': (34030,
                                                              '3528103291f022c36d5b2cba6e8751e5c859c9ef02beda4b89bf32f1e5692d48'),
 'src/frontend/oir/owned/array_reference_tests.rs': (45903,
                                                     'b872c15ffd104521c02c07fa646f8016260536b501d2ece210e16c50e68f5e6c'),
 'src/frontend/oir/owned/array_tests.rs': (34043,
                                           'c64b8ab62d9645a0e2f5f8021c98e78ba330f4ea5e767516b1dbff8bc85599b9'),
 'src/frontend/oir/owned/enum_consumer_fixtures.rs': (31700,
                                                      '0d41e7250807d9ac047a0e2d4c536efb63707000dfdf87ab29c4ff84210a64d8'),
 'src/frontend/oir/owned/enum_match_tests.rs': (50080,
                                                'ab3b4d1d3bf47e6a4821e72d14ddfac2b36c46a81aa080178cf4133df64a91a0'),
 'src/frontend/oir/owned/enum_native_tests.rs': (62643,
                                                 'bd5aa7b68be9059455ffc1a50f9cc3bc4f99de5887047eef1c34aa702f324b85'),
 'src/frontend/oir/owned/enum_reference_tests.rs': (43630,
                                                    '320b675d668c778a9d224dddd2173f2caf737644d24bd9912f69107cf1328cc1'),
 'src/frontend/oir/owned/execute.rs': (188517,
                                       '7bcdafcf00f972a8f25a3a3ba9ebff479a7ca1df8b9acac6adfca25ec876418d'),
 'src/frontend/oir/owned/execute_floats.rs': (6268,
                                              'b929dc1c8e4fdd396685b2b6a860b3199b5c2421de89a38682bf2409bf499e7b'),
 'src/frontend/oir/owned/execute_values.rs': (13296,
                                              '09212f8914b94020ff95377d318850d38719aea1a77f0e122f22a33f67c8234e'),
 'src/frontend/oir/owned/native.rs': (234543,
                                      '159e95412e384b2cc3cc4b42b781471e1fa95518f5a448dc6a67c9546a8385b8'),
 'src/frontend/oir/owned/native_floats.rs': (2269,
                                             'e4cf298bdeda82e949ede7c26a5c5b034609ef60b73bd4d8f1cfc610a86021df'),
 'src/frontend/oir/owned/plan.rs': (30474,
                                    '53d8b7da9077467cf2b69b7c2fb3e7e7cce04d62543747ea0b538d0590c1d86c'),
 'src/frontend/oir/owned/reviewer_array_reference_tests.rs': (66143,
                                                              'c99aa0d5c52147db7ed88ed2ddd35e816d0c206c976cdc455287d84d82bd7b3e'),
 'src/frontend/oir/owned/shape.rs': (56324,
                                     '21eb92a70a53583b2c61e9789b89a11b0212b6bb6183580511a8c398f06f6aec'),
 'src/frontend/oir/owned/source/array_pipeline_rows.rs': (55649,
                                                          '2a563586a3644832a382608d7bf44a137504e5bb713bcd651e944a9daade41f5'),
 'src/frontend/oir/owned/source/candidate_adapter.rs': (43366,
                                                        'c9b2f7314ee6fc4f2c5b9637a8e2aa8ef76b2371e3f2814946f7382ff1404e6c'),
 'src/frontend/oir/owned/source/float_tests.rs': (3179,
                                                  '985f9a57a564da41fff74f97ddb9f0dca2983740445d0b5043f942a9cfc3558b'),
 'src/frontend/oir/owned/source/hir.rs': (6654,
                                          'a7878f923685cf40f1a33419c78ad56667d1d77f0a6e0a6d58ff9deccabe94d4'),
 'src/frontend/oir/owned/source/lower.rs': (116802,
                                            '606b8152f4d5bae31e477361df47252ad2da9725b87f00f113bb9f122375e1e0'),
 'src/frontend/oir/owned/source/mod.rs': (2115,
                                          'dae6ef11dfe252e8277a328be3e5eb96fbd6347cf75ccf1d4587100df44067cd'),
 'src/frontend/oir/owned/source/resolve.rs': (147828,
                                              '94a371162445e81309790afb0cd6fddd3f4ca4eab5dd512c1d376c3d63b87ac7'),
 'src/frontend/oir/owned/source/typeck.rs': (213012,
                                             'afaec6ca552a65fcd1527610638515dedd2b0196ef2c7e5b2677de7643cac2df'),
 'src/frontend/oir/owned_types.rs': (79946,
                                     'dc4c314ff93c8ff51fc7d48afdc14073086fb0a77ac6cebd194dd02350fe74e9'),
 'src/frontend/oir/owned_types/enums.rs': (57640,
                                           'bc69862e06b5f7c37758fcfd9742f1d3112ebe99648c80dd5a8208debb3feaa8'),
 'src/frontend/oir/source/association.rs': (33181,
                                            '7abc5ea0230b58bf602563575698fdf5ed9ce8b182f2f78aad6422624163a035'),
 'src/frontend/oir/source/hir_import/candidate.rs': (52784,
                                                     '40d486dc884e17988042fa648b282c7a3117dfd9a61eb305a2c8cc7a871a31d5'),
 'src/frontend/oir/source/hir_import/candidate/typed_compare.rs': (17590,
                                                                   '92847c7a82874f86c5727e30ae1bf0237c168bede3cdcb8cdd4144f19adbfce7'),
 'src/frontend/oir/source/sealed.rs': (23628,
                                       'faf88f696b73aa5fbebfb2320b093d883db70dee569a8c4793f312b6072d23fd'),
 'src/frontend/oir/verify.rs': (52541,
                                'e59fb3825cf2596cc39bb8ebacabcd2a69d92d3c9a61160316addd5d1d447719'),
 'src/frontend/parser.rs': (86476, '6a8f7b8bdfc81762c5e905cab72ae404b077cae61213d04670c4d65305650a1e'),
 'src/frontend/parser/arrays.rs': (10639,
                                   '3c6281ef9e18996d8a582b1a95ef29ebf1d124d742744772e3a6650092d312bf'),
 'src/frontend/parser/conversions.rs': (3490,
                                        '150d2a5fc35b5c7206fc1dd1a813238dcf9d8484dae04a0a820f10d726131c75'),
 'src/frontend/parser/enums.rs': (16576,
                                  '590be9c6912df60b9c38418c70dba560f3e13dd956f6b92de6d116942e7f8e94'),
 'src/frontend/typeck.rs': (36048, '663b37e205f806e53d98a86de8c9cb37a675178ebcaa6031c6badb658a1af850'),
 'tests/typed_frontend.rs': (16553, '39f3a042f36373ee4c740e50529fc02b18ef443666700519291e7fc0900bee35')}
REVIEWED_DIRECTIVES = {'native/float_format.rs': ([], []),
 'native/typed_float.c': ([], []),
 'src/frontend/native.rs': ([b'include_str!("../../native/typed_preview.c")',
                             b'include_str!("../../native/typed_float.c")',
                             b'include_bytes!(concat!(env!("OUT_DIR")',
                             b'include_str!("../../native/typed_integer128.c")',
                             b'include_str!("../../native/typed_bytebuf.c")',
                             b'include_str!("../../native/experimental_bytebuf.c")',
                             b'include_str!("../../native/experimental_files.c")',
                             b'include_str!("../../native/experimental_arguments.c")',
                             b'include_str!("../../native/experimental_bytebuf.h")'],
                            []),
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
 'src/frontend/oir/owned/execute_floats.rs': ([], []),
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
 'src/frontend/oir/owned/native_floats.rs': ([], []),
 'src/frontend/oir/owned/source/float_tests.rs': ([], [])}

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
PARENT_ADDED = PREDECESSOR_ADDED | FLOAT_ADDED
directives = library.directives
PREDECESSOR_POLICIES = {**library.PREDECESSOR_POLICIES, PREDECESSOR_POLICY: PREDECESSOR_POLICY_SHA}


def authenticate_policies(repo, parent):
    for name, expected in PREDECESSOR_POLICIES.items():
        before = regular(parent, name)
        require(sha(before) == expected, "wrong pinned predecessor source policy: " + name)
        require(regular(repo, name) == before, "predecessor source policy changed: " + name)


OBSERVER_ADAPTER = "scripts/float_compatibility_observer.py"
OBSERVER_ADAPTER_BYTES = 4060
OBSERVER_ADAPTER_SHA = '9dcc12063886d4ced2906cab239d147c4554266c7a6bed28659a11a3a49ec101'
def authenticate_observer_adapter(repo):
    body = regular(repo, OBSERVER_ADAPTER)
    require(len(body) == OBSERVER_ADAPTER_BYTES and sha(body) == OBSERVER_ADAPTER_SHA,
            "changed pinned floating-point observer adapter")
    adapter = types.ModuleType("_float_observer_successor")
    adapter.__file__ = str(repo / OBSERVER_ADAPTER)
    exec(compile(body, adapter.__file__, "exec"), adapter.__dict__)
    return adapter, body
run_file_observer_controls = library.run_file_observer_controls
run_i64_observer_controls = library.run_i64_observer_controls
run_integer_width_observer_controls = library.run_integer_width_observer_controls
run_integer128_observer_controls = library.run_integer128_observer_controls


def run_float_observer_controls(repo, output, captured, seam, original, adapter):
    """Run floating-point exclusions in addition to all unchanged prior observer controls."""
    run = output / "unit2/run"
    controls = output / "float-observer-controls"
    controls.mkdir()
    protocol_body = regular(Path(seam["resource_package_root"]), "protocol.py")
    require(protocol_body == captured["historical_bytes"]["protocol.py"], "changed control protocol")
    protocol = types.ModuleType("_float_observer_protocol")
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
        for label, filters in (("list", ["--list", adapter.FLOAT_CONTROL_FILTER]),
                               ("run", [adapter.FLOAT_CONTROL_FILTER, "--test-threads=1", "--color", "never"])):
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
        names = list(adapter.FLOAT_CONTROL_NAMES)
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
    return {"float_observer_control_tests_per_profile": 4,
            "float_observer_control_receipts": receipts,
            "test_function_executions": 2 * (1 + 21 + 6 + 3 + 3 + 2 + 14 + 4 + 4)}


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
    expected = expected_parent | FLOAT_ADDED
    require(set(parent) == expected_parent, "wrong predecessor compiler input membership")
    require(set(FLOAT_INPUTS) == FLOAT_CHANGED | FLOAT_ADDED
            and FLOAT_CHANGED <= expected_parent and not FLOAT_ADDED & expected_parent,
            "wrong reviewed floating-point input membership")
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
        if name not in FLOAT_CHANGED:
            require(inputs[name] == before, "unreviewed change outside floating-point boundary: " + name)
    for name, (length, digest) in FLOAT_INPUTS.items():
        require(len(inputs[name]) == length and sha(inputs[name]) == digest,
                "floating-point source does not match reviewed input: " + name)
        require(directives(inputs[name]) == REVIEWED_DIRECTIVES.get(name, directives(parent[name]) if name in parent else ([], [])),
                "floating-point compile-time input directive changed: " + name)
    return inputs


def current_manifest(inputs, baseline, checkout, parent):
    changes = [{"path": name, "before": identity(name, baseline[name]), "after": identity(name, body)}
               for name, body in sorted(inputs.items()) if name in baseline and body != baseline[name]]
    result = {"schema": "oxid-project-workflow-current-source-v1", "baseline_commit": BASELINE,
              "checkout": checkout, "changes": changes, "additions": sorted(PARENT_ADDED),
              "assertion_scope": "retained bounded scalar/record semantics and resources",
              "source_policy_parent": PREDECESSOR, "source_policy_inputs": PREDECESSOR_POLICIES,
              "float_reviewed_inputs": [{"path": name, "bytes": length, "sha256": digest}
                                          for name, (length, digest) in sorted(FLOAT_INPUTS.items())],
              "float_changes": [{"path": name, "before": identity(name, parent[name]),
                                  "after": identity(name, inputs[name])} for name in sorted(FLOAT_CHANGED)],
              "unchanged_baseline_inputs": len(baseline) - len(changes),
              "files": [identity(name, body) for name, body in sorted(inputs.items())]}
    for name in ("package", "workspace", "native_reproducibility", "native_optimization", "owned_enum",
                 "byte_field", "byte_buffer", "binary_io", "native_heap_safety", "hosted_execution",
                 "owned_buffer_transfer", "byte_string", "native_file", "program_arguments", "i64", "integer_widths", "byte_integer", "integer128", "floats"):
        key = name + ("_qualified_by_this_gate" if name.startswith("native_") and name != "native_file" else "_semantics_qualified_by_this_gate")
        result[key] = False
    return result


def run(args):
    require(__debug__ and not sys.flags.optimize, "qualification refuses optimized Python")
    repo, output = args.repo.resolve(strict=True), args.output.resolve()
    require(Path(__file__).resolve() == repo / "scripts/verify_float_compatibility.py",
            "run the controller from the selected checkout")
    require(output != repo and repo not in output.parents, "output must be outside repository")
    require(not output.exists(), "output must be new")
    output.mkdir(parents=True)
    report = {"schema": "oxid-project-workflow-current-result-v1", "status": "running",
              "scope": "retained bounded scalar/record semantics and resources on the floating-point successor; floating-point operations qualified separately",
              "semantic_pass": False, "compiler_executions": 0, "baseline_commit": BASELINE,
              "started_at_utc": datetime.datetime.now(datetime.timezone.utc).isoformat(),
              "controller": identity("scripts/verify_float_compatibility.py", Path(__file__).read_bytes())}
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
