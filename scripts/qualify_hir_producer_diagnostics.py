#!/usr/bin/env python3
"""Qualify source-validated static failures from freshly built v1/v2 producers.

Expected user diagnostics come only from ordinary source check/run/compile.
Positive producer frames are captured from the real Oxid programs, never made
by this controller. C replay helpers are used only for explicitly negative
wire/source controls. Every invocation preserves inputs, streams and identities.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import time

import build_hir_producers_v2 as v2


ROOT = Path(__file__).resolve().parents[1]
ROSTER = ROOT / "tests/fixtures/bounded_typed_static/cases.json"
ROSTER_SHA256 = "985c6f5286b86591bdb5945ef13df66bce7b5bbdd2ff4cdb8e8a4cd7742a2df3"
# Named 48-case subset observed from the unchanged authored 81-source roster.
# These numbers are wire coverage assertions, not generated diagnostic text.
KINDS = {
    "scope_ends": 1, "parameter_duplicate": 3, "function_duplicate": 3,
    "parameter_function_conflict": 3, "later_function_conflict": 3,
    "local_duplicate": 3, "initializer_precedes_duplicate": 1,
    "self_initializer": 1, "unknown_local": 1, "call_target_first": 2,
    "assignment_target_first": 1, "arguments_left_first": 1,
    "later_resolution_before_type": 1, "unreachable_resolution": 1,
    "false_loop_resolution": 1, "logical_rhs_resolution": 1,
    "break_placement": 6, "continue_placement": 7, "positive_range": 5,
    "negative_range": 5, "unknown_type_before_body": 4,
    "global_duplicate_before_type": 3, "annotation_block_order": 4,
    "parameter_type_before_result": 4, "public_unknown_type": 4,
    "type_display_64": 4, "type_display_65": 4, "pending_bad_return": 8,
    "pending_immutable_assignment": 14, "pending_wrong_arity": 10,
    "pending_missing_return": 11, "pending_unreachable": 12,
    "canonical-012": 9, "canonical-016": 13, "canonical-021": 14,
    "canonical-043": 8, "canonical-044": 1, "canonical-045": 3,
    "canonical-054": 8, "canonical-056": 11, "rhs_before_immutable": 8,
    "inner_before_unit_equality": 8, "argument_inner_before_arity": 8,
    "first_argument_type": 8, "first_function_completion": 11,
    "unreachable_before_inner_type": 12, "ordered_bool_mismatch": 8,
    "initializer_before_annotation": 8,
}
OPERATIONS = ("check", "run", "compile")


def require(condition, message):
    if not condition:
        raise RuntimeError(message)


def digest(data):
    return hashlib.sha256(data).hexdigest()


def load_cases():
    data = ROSTER.read_bytes()
    require(digest(data) == ROSTER_SHA256, "authored roster changed")
    cases = json.loads(data)["cases"]
    require(len(cases) == 81 and len({c["name"] for c in cases}) == 81,
            "expected exact 81-source roster")
    require(set(KINDS) <= {c["name"] for c in cases} and len(KINDS) == 48
            and set(KINDS.values()) == set(range(1, 15)), "diagnostic roster changed")
    return cases


def extra_cases(version):
    cases = [
        ("multiple-type-errors", b"fn a()->i32{return true;}fn b()->bool{return 1;}", 8),
        ("zero-actual-arguments", b"fn f(x:i32)->(){return;}fn g()->(){f();return;}", 10),
        ("long-unknown-local", b"fn f()->(){" + b"a" * 65 + b";return;}", 1),
        ("long-unknown-function", b"fn f()->(){" + b"a" * 65 + b"();return;}", 2),
    ]
    if version == 2:
        cases += [
            ("real-primary-end255", b" " * (255 - len(b"fn f()->(){}")) + b"fn f()->(){}", 11),
            ("long-unknown-type129", b"pub fn f(x:" + b"A" * 129 + b")->(){return;}", 4),
        ]
    return cases


def json_rows(result):
    require(result.returncode == 1 and not result.stderr,
            "JSON failure must exit 1 with empty stderr")
    rows = [json.loads(line) for line in result.stdout.splitlines()]
    require(rows and all(isinstance(row, dict) for row in rows), "invalid JSON rows")
    return rows


def without_provenance(rows):
    # Only the producer-specific evidence records are allowed to differ.
    return [row for row in rows if row.get("kind") != "hir-producer"]


def without_text_provenance(stderr, hashes, opa, wire):
    prefix = b"".join((
        f"experimental HIR producer {role} sha256={hashes[role]} spawned=true leader_reaped=true "
        f"stop=exited exit=0 signal=null captured_stdout={len(data)} captured_stderr=0\n"
    ).encode() for role, data in (("parser", opa), ("static", wire)))
    require(stderr.startswith(prefix), "text provenance does not match genuine producer captures")
    return stderr[len(prefix):]


def validate_failure(rows, operation, code=None):
    canonical = without_provenance(rows)
    require(len(canonical) >= 2, "missing diagnostics and failure summary")
    diagnostics, summary = canonical[:-1], canonical[-1]
    require(all(row.get("kind") == "diagnostic" and row.get("severity") == "error"
                for row in diagnostics), "unexpected output or execution record")
    require(summary.get("kind") == operation + "-summary" and summary.get("success") is False
            and type(summary.get("errors")) is int and summary["errors"] == len(diagnostics),
            "wrong failure summary")
    if code is not None:
        require(len(diagnostics) == 1 and diagnostics[0].get("code") == code,
                "expected fail-closed diagnostic " + code)
    return canonical


def validate_provenance(rows, source, opa, wire, hashes, version):
    records = [row for row in rows if row.get("kind") == "hir-producer"]
    require(len(records) == 2, "both genuine producer executions are required")
    ast = b"AST" + str(version).encode() + bytes([len(source)]) + source + opa
    for role, record, data, output in zip(("parser", "static"), records, (source, ast), (opa, wire)):
        require(record.get("role") == role and record.get("executable_sha256") == hashes[role]
                and record.get("input_sha256") == digest(data)
                and type(record.get("input_written")) is int and record["input_written"] == len(data)
                and record.get("captured_stdout_sha256") == digest(output)
                and type(record.get("captured_stdout_bytes")) is int and record["captured_stdout_bytes"] == len(output)
                and type(record.get("captured_stderr_bytes")) is int and record["captured_stderr_bytes"] == 0
                and record.get("captured_stderr_sha256") == digest(b"")
                and record.get("spawned") is True and record.get("leader_reaped") is True
                and record.get("stop") == "exited" and type(record.get("exit_status")) is int
                and record["exit_status"] == 0 and "signal" in record and record["signal"] is None,
                "producer execution identity mismatch: " + role)


def mutations(opa, wire, version):
    """Damage actual captured bytes. None of these are positive evidence."""
    def changed(offset, value):
        data = bytearray(wire)
        data[offset] = value
        return bytes(data)
    other_version = ord("2" if version == 1 else "1")
    result = {
        "kind-zero": changed(1565, 0), "kind-unknown": changed(1565, 15),
        "kind-wrong": changed(1565, 11),
        "primary-start": changed(1566, wire[1566] + 1),
        "primary-end": changed(1567, wire[1567] - 1),
        "primary-end255": changed(1567, 255),
        "unexpected-secondary": changed(1568, 1),
        "secondary-end255": changed(1569, 255),
        "wrong-label": changed(1570, 4),
        "expected-type": changed(1571, 1), "actual-type": changed(1572, 3),
        "reserved-expected-count": changed(1573, 1),
        "reserved-actual-count": changed(1574, 1),
        "wrong-row-count": changed(1564, wire[1564] + 1),
        "wrong-tag": changed(1563, 2), "wrong-stf-magic": changed(1559, ord("X")),
        "wrong-stf-version": changed(1562, other_version),
        "wrong-opa-version": changed(3, other_version),
        "changed-opa-prefix": changed(0, ord("X")),
        "inactive-opa-cell": changed(1558, 1),
        "truncated-empty": b"", "truncated-opa": wire[:1558],
        "truncated-header": wire[:-1], "trailing-byte": wire + b"X",
        "trailing-success-columns": wire + bytes(1032),
    }
    require(all(data != wire for data in result.values()), "mutation did not change capture")
    require(wire[:1559] == opa, "mutation seed prefix differs from actual OPA")
    return result


def expected_recipe():
    selected = [(case['name'], case['source'].encode('ascii'), KINDS[case['name']])
                for case in load_cases() if case['name'] in KINDS]
    captures, parities, refusals = [], [], []
    negative_names = (
        'kind-zero kind-unknown kind-wrong primary-start primary-end primary-end255 '
        'unexpected-secondary secondary-end255 wrong-label expected-type actual-type '
        'reserved-expected-count reserved-actual-count wrong-row-count wrong-tag '
        'wrong-stf-magic wrong-stf-version wrong-opa-version changed-opa-prefix '
        'inactive-opa-cell truncated-empty truncated-opa truncated-header trailing-byte '
        'trailing-success-columns stale-source same-length-stale-source agreeing-inactive-opa '
        'missing-required-secondary second-error-before-first false-error-valid-source supplied-artifact-tag1'
    ).split()
    for version in (1, 2):
        for name, source, kind in selected + extra_cases(version):
            captures.append((version, name, source, kind))
            parities.extend(f'v{version}/parity/{name}/{operation}-{mode}'
                            for operation in OPERATIONS for mode in ('json', 'text'))
        refusals.extend(f'v{version}/refusals/{name}/{operation}-{mode}'
                        for name in negative_names for operation in OPERATIONS for mode in ('json', 'text'))
    return captures, parities, refusals


def validate_recipe(summary, expected):
    require(isinstance(summary, dict) and type(summary.get('schema_version')) is int
            and summary['schema_version'] == 1 and summary.get('status') == 'passed',
            'diagnostic qualification did not pass')
    for key, value in expected.items():
        require(summary.get(key) == value, 'diagnostic qualification identity mismatch: ' + key)
    captures, parities, refusals = expected_recipe()
    actual = summary.get('captures')
    require(isinstance(actual, list) and len(actual) == len(captures), 'incomplete diagnostic captures')
    for record, (version, name, source, kind) in zip(actual, captures):
        require(isinstance(record, dict) and type(record.get('version')) is int and record['version'] == version
                and record.get('name') == name and type(record.get('kind')) is int and record['kind'] == kind
                and record.get('source_sha256') == digest(source), 'diagnostic capture recipe mismatch')
    require(summary.get('parity_cases') == parities, 'incomplete or changed diagnostic parity recipe')
    require(summary.get('refusal_cases') == refusals, 'incomplete or changed diagnostic refusal recipe')
    require(type(summary.get('llvm_invocations')) is int and summary['llvm_invocations'] == 0,
            'diagnostic qualification invoked LLVM')


def compiler_command(compiler, operation, source, option=None, value=None, output=None, json_mode=True):
    argv = [compiler, operation, source, '--edition=typed-preview']
    if option:
        argv += [option, value]
    if json_mode:
        argv += ['--message-format=json']
    if operation == 'compile':
        argv += ['--backend=llvm', '--output', output]
    return [str(arg) for arg in argv]


def refusal_inputs(root, version):
    """Expected negative wire mutations of genuine retained producer captures."""
    def capture(name):
        directory = root / f'v{version}/captures/{name}'
        return ((directory / 'source.ox').read_bytes(), (directory / 'parser/stdout').read_bytes(),
                (directory / 'static/stdout').read_bytes())
    source, opa, wire = capture('pending_bad_return')
    cases = {name: (source, opa, damaged) for name, damaged in mutations(opa, wire, version).items()}
    cases['stale-source'] = (source.replace(b'true', b'false'), opa, wire)
    cases['same-length-stale-source'] = (source.replace(b'true', b'1234'), opa, wire)
    inactive = bytearray(opa)
    inactive[1558] = 1
    cases['agreeing-inactive-opa'] = (source, bytes(inactive), bytes(inactive) + wire[1559:])
    duplicate_source, duplicate_opa, duplicate_wire = capture('parameter_duplicate')
    duplicate = bytearray(duplicate_wire)
    duplicate[1568:1571] = bytes(3)
    cases['missing-required-secondary'] = (duplicate_source, duplicate_opa, bytes(duplicate))
    multiple_source, multiple_opa, multiple_wire = capture('multiple-type-errors')
    second = bytearray(multiple_wire)
    second[1566:1568] = bytes([multiple_source.rindex(b'1'), multiple_source.rindex(b'1') + 1])
    second[1571:1573] = bytes([1, 2])
    cases['second-error-before-first'] = (multiple_source, multiple_opa, bytes(second))
    valid = b'fn main()->i32{return 42;}'
    actual = (root / f'v{version}/valid-source-parser/stdout').read_bytes()
    require(len(actual) == 1559 and actual[:5] == b'OPA' + str(version).encode() + b'\0',
            'valid-source parser capture changed')
    forged = bytearray(actual + wire[1559:])
    forged[1564] = actual[8]
    forged[1566:1568] = bytes([valid.index(b'42'), valid.index(b'42') + 2])
    forged[1571:1573] = bytes([1, 2])
    cases['false-error-valid-source'] = (valid, actual, bytes(forged))
    cases['supplied-artifact-tag1'] = (source, opa, wire)
    return cases


def validate_summary(root, expected):
    """Re-admit the complete retained recipe, including raw canonical comparisons."""
    summary = json.loads((root / 'summary.json').read_text())
    validate_recipe(summary, expected)
    require(summary.get('receipts_sha256') == digest((root / 'receipts.json').read_bytes()),
            'diagnostic receipts changed')
    receipts = json.loads((root / 'receipts.json').read_text())
    require(isinstance(receipts, list) and receipts, 'missing diagnostic receipts')
    indexed = {}
    for row in receipts:
        require(isinstance(row, dict) and isinstance(row.get('name'), str), 'invalid diagnostic receipt')
        name = row['name']
        require(name not in indexed and not Path(name).is_absolute() and '..' not in Path(name).parts,
                'duplicate or invalid diagnostic receipt name')
        require(isinstance(row.get('argv'), list) and row['argv']
                and all(isinstance(arg, str) for arg in row['argv']), 'invalid diagnostic command')
        require(type(row.get('status')) is int and row['status'] in (0, 1), 'failed diagnostic command')
        for stream in ('stdout', 'stderr'):
            require(row.get(stream + '_sha256') == digest((root / name / stream).read_bytes()),
                    'diagnostic receipt stream mismatch: ' + name)
        if row.get('stdin_sha256') is not None:
            require(row['stdin_sha256'] == digest((root / name / 'stdin.bin').read_bytes()),
                    'diagnostic receipt stdin mismatch: ' + name)
        indexed[name] = row

    def result(name, status, guarded):
        require(name in indexed and indexed[name]['status'] == status
                and indexed[name].get('guarded') is guarded, 'missing or changed diagnostic execution: ' + name)
        return subprocess.CompletedProcess(indexed[name]['argv'], status,
            (root / name / 'stdout').read_bytes(), (root / name / 'stderr').read_bytes())

    hashes = {}
    compiler = None
    for version in (1, 2):
        bundle = root / f'v{version}-build/bundle'
        hashes[version] = {role: digest((bundle / role).read_bytes()) for role in ('parser', 'static')}
        require((bundle / 'manifest.txt').read_text() == f'OXID-HIR-PRODUCERS-{version}\n' + ''.join(
            role + ' ' + hashes[version][role] + '\n' for role in ('parser', 'static')),
            'diagnostic producer manifest mismatch')
        for role in ('parser', 'static'):
            build = result(f'v{version}-build/{role}-build', 0, False)
            if compiler is None:
                compiler = build.args[0]
            sources = ROOT / 'fixtures/typed-lexer-samples' if version == 1 else root / 'v2-build/sources'
            source_name = 'parser_main.ox' if role == 'parser' else 'ast_static_main.ox'
            require(build.args == [compiler, 'compile', str(sources / source_name),
                    '--edition=typed-preview', '--backend=llvm', '--entry-mode=process', '--output', str(bundle / role)],
                    'diagnostic producer build command changed')
            require(digest(Path(build.args[0]).read_bytes()) == expected['compiler_sha256'],
                    'diagnostic producer built by another compiler')
    recipe, _, _ = expected_recipe()
    for record, (version, name, source, kind) in zip(summary['captures'], recipe):
        label = f'v{version}/captures/{name}'
        path = root / label / 'source.ox'
        require(path.read_bytes() == source, 'diagnostic source capture changed')
        parser = result(label + '/parser', 0, False)
        static = result(label + '/static', 0, False)
        require(parser.args == [str(root / f'v{version}-build/bundle/parser')]
                and static.args == [str(root / f'v{version}-build/bundle/static')],
                'diagnostic capture did not execute actual rebuilt producers')
        opa, wire = parser.stdout, static.stdout
        require(not parser.stderr and not static.stderr and len(opa) == 1559 and len(wire) == 1575
                and opa[:5] == b'OPA' + str(version).encode() + b'\0'
                and wire[:1559] == opa and wire[1559:1564] == b'STF' + str(version).encode() + b'\1'
                and wire[1565] == kind and record.get('opa_sha256') == digest(opa)
                and record.get('wire_sha256') == digest(wire), 'diagnostic producer capture mismatch')
        ast = b'AST' + str(version).encode() + bytes([len(source)]) + source + opa
        require((root / label / 'parser/stdin.bin').read_bytes() == source
                and (root / label / 'static/stdin.bin').read_bytes() == ast,
                'diagnostic actual producer inputs changed')
        if name == 'real-primary-end255':
            require(wire[1567] == 255, 'missing actual endpoint 255')
        for operation in OPERATIONS:
            for mode in ('json', 'text'):
                pair = f'v{version}/parity/{name}/{operation}-{mode}'
                ordinary = result(pair + '/ordinary', 1, True)
                producer = result(pair + '/producer', 1, True)
                target = root / (pair.replace('/', '-') + '.elf')
                require(ordinary.args == compiler_command(compiler, operation, path, output=target, json_mode=mode == 'json')
                        and producer.args == compiler_command(compiler, operation, path, '--experimental-hir-producers',
                            root / f'v{version}-build/bundle', target, mode == 'json'),
                        'diagnostic parity command changed')
                if mode == 'json':
                    canonical = validate_failure(json_rows(ordinary), operation)
                    rows = json_rows(producer)
                    validate_provenance(rows, source, opa, wire, hashes[version], version)
                    require(validate_failure(rows, operation) == canonical, 'retained diagnostic JSON parity mismatch')
                    if name == 'multiple-type-errors':
                        require(len(canonical) >= 3, 'retained multi-error vector is incomplete')
                else:
                    require((producer.stdout, without_text_provenance(producer.stderr, hashes[version], opa, wire))
                            == (ordinary.stdout, ordinary.stderr), 'retained diagnostic text parity mismatch')
                require(target.read_bytes() == b'KEEP-EXISTING-OUTPUT' if mode == 'json' else not target.exists(),
                        'diagnostic parity replaced or published output')
    for version in (1, 2):
        valid_parser = result(f'v{version}/valid-source-parser', 0, False)
        require(valid_parser.args == [str(root / f'v{version}-build/bundle/parser')]
                and not valid_parser.stderr
                and (root / f'v{version}/valid-source-parser/stdin.bin').read_bytes() == b'fn main()->i32{return 42;}',
                'valid-source parser execution changed')
    negatives = {version: refusal_inputs(root, version) for version in (1, 2)}
    for name in summary['refusal_cases']:
        version = int(name[1])
        control = name.split('/')[2]
        operation, mode = name.rsplit('/', 1)[1].split('-')
        refusal = result(name, 1, True)
        code = 'E0702' if '/supplied-artifact-tag1/' in name else 'E0703'
        source, opa, wire = negatives[version][control]
        path = root / name.rsplit('/', 1)[0] / 'source.ox'
        require(path.read_bytes() == source, 'diagnostic refusal source changed')
        target = path.parent / (operation + ('-existing' if mode == 'json' else '-absent') + '.elf')
        selected = root / f'v{version}/negative-bundles/{control}'
        option = '--experimental-hir-producers'
        if code == 'E0702':
            selected = root / f'v{version}/captures/pending_bad_return/static/stdout'
            option = '--experimental-hir-import'
        require(refusal.args == compiler_command(compiler, operation, path, option, selected, target, mode == 'json'),
                'diagnostic refusal command changed')
        if mode == 'json':
            rows = json_rows(refusal)
            validate_failure(rows, operation, code)
            records = [row for row in rows if row.get('kind') == 'hir-producer']
            if code == 'E0702':
                require(not records, 'supplied artifact unexpectedly executed producers')
            else:
                selected_hashes = {role: digest((selected / role).read_bytes()) for role in ('parser', 'static')}
                require((selected / 'manifest.txt').read_text() == f'OXID-HIR-PRODUCERS-{version}\n' + ''.join(
                    role + ' ' + selected_hashes[role] + '\n' for role in ('parser', 'static')),
                    'diagnostic negative producer manifest changed')
                if control == 'stale-source':
                    require(len(records) == 1 and records[0].get('role') == 'parser'
                            and records[0].get('input_sha256') == digest(source)
                            and records[0].get('captured_stdout_sha256') == digest(opa)
                            and records[0].get('executable_sha256') == selected_hashes['parser'],
                            'stale-source parser refusal identity mismatch')
                else:
                    validate_provenance(rows, source, opa, wire, selected_hashes, version)
        else:
            require(not refusal.stdout and code.encode() in refusal.stderr, 'retained text refusal mismatch')
        require(target.read_bytes() == b'KEEP-EXISTING-OUTPUT' if mode == 'json' else not target.exists(),
                'diagnostic refusal replaced or published output')
    require(not (root / 'unexpected-llvm-invocation').exists(), 'retained LLVM guard was triggered')
    require(not list((root / 'snapshots').iterdir()), 'retained producer workspace was not cleaned')
    return summary


class Run:
    def __init__(self, compiler, llvm, output, cc):
        self.compiler = compiler.resolve(strict=True)
        self.llvm = llvm.resolve(strict=True)
        self.root = output.resolve()
        require(not self.root.is_relative_to(ROOT), "evidence must be outside checkout")
        self.root.mkdir()  # Never overwrite earlier successful or failed evidence.
        self.cc = cc
        self.receipts, self.parities, self.refusals, self.captures = [], [], [], []
        self.env = dict(os.environ, OXID_LLVM_BIN=str(self.llvm))
        self.snapshots = self.root / "snapshots"
        self.snapshots.mkdir()
        self.env["TMPDIR"] = str(self.snapshots)
        self.identities = self.input_identities()
        (self.root / 'input-identities.json').write_text(json.dumps(self.identities, indent=2) + '\n')
        self.guard = self.root / "llvm-guard"
        self.guard.mkdir()
        self.marker = self.root / "unexpected-llvm-invocation"
        for name in ("clang", "opt", "ld.lld", "llvm-as", "llc"):
            path = self.guard / name
            path.write_text("#!/bin/sh\nprintf 'LLVM called\\n' >> " +
                            "'" + str(self.marker).replace("'", "'\\''") + "'\nexit 97\n")
            path.chmod(0o755)
        self.save()

    def input_identities(self):
        return dict(compiler_sha256=digest(self.compiler.read_bytes()),
                    harness_sha256=digest(Path(__file__).read_bytes()),
                    builder_sha256=digest(Path(v2.__file__).read_bytes()),
                    roster_sha256=digest(ROSTER.read_bytes()),
                    source_manifest_sha256=digest(v2.MANIFEST.read_bytes()),
                    source_head=subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT).decode().strip(),
                    source_tree=subprocess.check_output(["git", "rev-parse", "HEAD^{tree}"], cwd=ROOT).decode().strip())

    def save(self):
        (self.root / "receipts.json").write_text(json.dumps(self.receipts, indent=2) + "\n")

    def call(self, name, argv, data=None, status=0, guarded=False):
        directory = self.root / name
        directory.mkdir(parents=True)
        command = [str(arg) for arg in argv]
        env = dict(self.env)
        if guarded:
            env["OXID_LLVM_BIN"] = str(self.guard)
        if data is not None:
            (directory / "stdin.bin").write_bytes(data)
        start = time.monotonic()
        try:
            result = subprocess.run(command, cwd=ROOT, env=env, input=data,
                                    capture_output=True, timeout=120)
            stdout, stderr, actual = result.stdout, result.stderr, result.returncode
        except subprocess.TimeoutExpired as error:
            stdout, stderr, actual = error.stdout or b"", error.stderr or b"", "outer-timeout"
        except OSError as error:
            stdout, stderr, actual = b"", str(error).encode(), "spawn-failed"
        (directory / "stdout").write_bytes(stdout)
        (directory / "stderr").write_bytes(stderr)
        self.receipts.append(dict(name=name, argv=command, status=actual,
            elapsed_seconds=time.monotonic() - start, guarded=guarded,
            stdin_sha256=digest(data) if data is not None else None,
            stdout_sha256=digest(stdout), stderr_sha256=digest(stderr)))
        self.save()
        require(actual == status, f"{name}: expected {status}, got {actual}: {stdout!r} {stderr!r}")
        if guarded:
            require(not self.marker.exists(), "invalid source reached LLVM")
            require(not list(self.snapshots.iterdir()), "producer/native workspace leaked")
        return result

    def manifest(self, bundle, version):
        hashes = {role: digest((bundle / role).read_bytes()) for role in ("parser", "static")}
        (bundle / "manifest.txt").write_text(f"OXID-HIR-PRODUCERS-{version}\n" + "".join(
            role + " " + hashes[role] + "\n" for role in ("parser", "static")))
        return hashes

    def build(self, version):
        directory = self.root / f"v{version}-build"
        directory.mkdir()
        sources = ROOT / "fixtures/typed-lexer-samples"
        if version == 2:
            sources = directory / "sources"
            v2.materialize(sources)
        bundle = directory / "bundle"
        bundle.mkdir()
        for role, source in (("parser", "parser_main.ox"), ("static", "ast_static_main.ox")):
            self.call(f"v{version}-build/{role}-build", [self.compiler, "compile", sources / source,
                "--edition=typed-preview", "--backend=llvm", "--entry-mode=process", "--output", bundle / role])
        return bundle, self.manifest(bundle, version)

    def command(self, operation, source, option=None, value=None, output=None, json_mode=True):
        return compiler_command(self.compiler, operation, source, option, value, output, json_mode)

    def capture(self, version, name, source, bundle, expected_kind=None):
        label = f"v{version}/captures/{name}"
        directory = self.root / label
        directory.mkdir(parents=True)
        path = directory / "source.ox"
        path.write_bytes(source)
        parser = self.call(label + "/parser", [bundle / "parser"], source)
        require(not parser.stderr, "actual parser emitted stderr")
        opa = parser.stdout
        if len(opa) != 1559:
            require(expected_kind is None and name in {"canonical-036", "canonical-037", "canonical-038", "canonical-039"}
                    and len(opa) == 11 and opa[:4] == b"OPA" + str(version).encode() and opa[4] != 0,
                    "roster unexpectedly failed parsing")
            return None
        require(opa[:5] == b"OPA" + str(version).encode() + b"\0" and opa[10] == len(source),
                "actual parser did not return expected successful frame")
        ast = b"AST" + str(version).encode() + bytes([len(source)]) + source + opa
        static = self.call(label + "/static", [bundle / "static"], ast)
        wire = static.stdout
        require(not static.stderr and wire[:1559] == opa, "actual static prefix/stream mismatch")
        if expected_kind is None:
            require(len(wire) == 2607 and wire[1559:1564] == b"STF" + str(version).encode() + b"\0",
                    "unlisted source became a static diagnostic")
            return None
        require(len(wire) == 1575 and wire[1559:1564] == b"STF" + str(version).encode() + b"\1"
                and wire[1565] == expected_kind, "actual static diagnostic kind/frame mismatch: " + name)
        record = dict(version=version, name=name, source_sha256=digest(source),
                      opa_sha256=digest(opa), wire_sha256=digest(wire), kind=wire[1565])
        self.captures.append(record)
        return path, opa, wire

    def parity(self, version, name, source, capture, bundle, hashes):
        path, opa, wire = capture
        for operation in OPERATIONS:
            for json_mode in (True, False):
                label = f"v{version}/parity/{name}/{operation}-" + ("json" if json_mode else "text")
                target = self.root / (label.replace("/", "-") + ".elf")
                # Exercise both pre-existing and absent output paths.
                if json_mode:
                    target.write_bytes(b"KEEP-EXISTING-OUTPUT")
                ordinary = self.call(label + "/ordinary", self.command(operation, path,
                    output=target, json_mode=json_mode), status=1, guarded=True)
                producer = self.call(label + "/producer", self.command(operation, path,
                    "--experimental-hir-producers", bundle, target, json_mode), status=1, guarded=True)
                if json_mode:
                    expected = validate_failure(json_rows(ordinary), operation)
                    rows = json_rows(producer)
                    actual = validate_failure(rows, operation)
                    validate_provenance(rows, source, opa, wire, hashes, version)
                    require(actual == expected, "canonical diagnostic/summary parity failed: " + label)
                    if name == "multiple-type-errors":
                        require(len(expected) >= 3, "multi-error control did not produce multiple diagnostics")
                    require(target.read_bytes() == b"KEEP-EXISTING-OUTPUT", "existing output was replaced")
                else:
                    require((producer.stdout, without_text_provenance(producer.stderr, hashes, opa, wire))
                            == (ordinary.stdout, ordinary.stderr),
                            "text diagnostic parity failed: " + label)
                    require(not target.exists(), "invalid source published native output")
                self.parities.append(label)

    def helper(self, path, output):
        # Negative controls only: no synthetic bytes are admitted as positive captures.
        source = path.with_suffix(".c")
        source.write_text("#include <unistd.h>\nstatic unsigned char output[]={" +
            ",".join(map(str, output or b"\0")) + "};\nint main(void){char b[4096];"
            "while(read(0,b,sizeof b)>0){};return write(1,output," + str(len(output)) + ")<0?3:0;}\n")
        self.call(str(path.relative_to(self.root)) + "-build", [self.cc, "-O0", source, "-o", path])

    def refuse(self, version, name, source, bundle, option="--experimental-hir-producers", code="E0703"):
        directory = self.root / f"v{version}/refusals/{name}"
        directory.mkdir(parents=True)
        path = directory / "source.ox"
        path.write_bytes(source)
        for operation in OPERATIONS:
            for json_mode in (True, False):
                label = str(directory.relative_to(self.root)) + "/" + operation + ("-json" if json_mode else "-text")
                target = directory / (operation + ("-existing" if json_mode else "-absent") + ".elf")
                if json_mode:
                    target.write_bytes(b"KEEP-EXISTING-OUTPUT")
                result = self.call(label, self.command(operation, path, option, bundle,
                    target, json_mode), status=1, guarded=True)
                if json_mode:
                    validate_failure(json_rows(result), operation, code)
                    require(target.read_bytes() == b"KEEP-EXISTING-OUTPUT", "refusal replaced existing output")
                else:
                    require(not result.stdout and code.encode() in result.stderr,
                            "text refusal executed or omitted expected diagnostic")
                    require(not target.exists(), "refusal published a new output")
                self.refusals.append(label)

    def negatives(self, version, captures, bundle):
        path, opa, wire = captures["pending_bad_return"]
        source = path.read_bytes()
        directory = self.root / f"v{version}/negative-bundles"
        directory.mkdir(parents=True)
        frames = mutations(opa, wire, version)
        for name, damaged in frames.items():
            selected = directory / name
            selected.mkdir()
            (selected / "parser").write_bytes((bundle / "parser").read_bytes())
            (selected / "parser").chmod(0o755)
            self.helper(selected / "static", damaged)
            self.manifest(selected, version)
            self.refuse(version, name, source, selected)
        # Agreeing parser/static prefixes that are stale still require current-source validation.
        for name, current in (("stale-source", source.replace(b"true", b"false")),
                              ("same-length-stale-source", source.replace(b"true", b"1234"))):
            selected = directory / name
            selected.mkdir()
            self.helper(selected / "parser", opa)
            self.helper(selected / "static", wire)
            self.manifest(selected, version)
            self.refuse(version, name, current, selected)
        # The two frames agree with one another, but carry an inactive OPA cell.
        # This must test complete current-source/OPA validation, not prefix equality.
        inactive = bytearray(opa)
        inactive[1558] = 1
        selected = directory / "agreeing-inactive-opa"
        selected.mkdir()
        self.helper(selected / "parser", bytes(inactive))
        self.helper(selected / "static", bytes(inactive) + wire[1559:])
        self.manifest(selected, version)
        self.refuse(version, "agreeing-inactive-opa", source, selected)
        # A genuine binding error must retain its exact secondary label and span.
        duplicate_path, _, duplicate_wire = captures["parameter_duplicate"]
        duplicate = bytearray(duplicate_wire)
        duplicate[1568:1571] = bytes(3)
        selected = directory / "missing-required-secondary"
        selected.mkdir()
        (selected / "parser").write_bytes((bundle / "parser").read_bytes())
        (selected / "parser").chmod(0o755)
        self.helper(selected / "static", bytes(duplicate))
        self.manifest(selected, version)
        self.refuse(version, "missing-required-secondary", duplicate_path.read_bytes(), selected)
        # Report the genuine source's second error first. Structurally plausible
        # bytes must not override the ordinary checker's diagnostic ordering.
        multiple_path, _, multiple_wire = captures["multiple-type-errors"]
        multiple_source = multiple_path.read_bytes()
        second = bytearray(multiple_wire)
        second[1566:1568] = bytes([multiple_source.rindex(b"1"), multiple_source.rindex(b"1") + 1])
        second[1571:1573] = bytes([1, 2])
        selected = directory / "second-error-before-first"
        selected.mkdir()
        (selected / "parser").write_bytes((bundle / "parser").read_bytes())
        (selected / "parser").chmod(0o755)
        self.helper(selected / "static", bytes(second))
        self.manifest(selected, version)
        self.refuse(version, "second-error-before-first", multiple_source, selected)
        # Fresh genuine valid-source OPA plus forged failure: must not become success fallback.
        valid = b"fn main()->i32{return 42;}"
        actual = self.call(f"v{version}/valid-source-parser", [bundle / "parser"], valid).stdout
        forged = bytearray(actual + wire[1559:])
        forged[1564] = actual[8]
        forged[1566:1568] = bytes([valid.index(b"42"), valid.index(b"42") + 2])
        forged[1571:1573] = bytes([1, 2])
        selected = directory / "false-error-valid-source"
        selected.mkdir()
        (selected / "parser").write_bytes((bundle / "parser").read_bytes())
        (selected / "parser").chmod(0o755)
        self.helper(selected / "static", bytes(forged))
        self.manifest(selected, version)
        self.refuse(version, "false-error-valid-source", valid, selected)
        # The supplied-artifact API remains success-only for all operations/formats.
        self.refuse(version, "supplied-artifact-tag1", source,
                    path.parent / "static/stdout", "--experimental-hir-import", "E0702")

    def finish(self):
        require(self.input_identities() == self.identities, "qualification inputs changed")
        require(len(self.captures) == 106 and len(self.parities) == 636
                and len(self.refusals) == 384, "incomplete diagnostic qualification recipe")
        require(not self.marker.exists(), "LLVM guard was triggered")
        report = dict(self.identities, schema_version=1, status="passed",
                      captures=self.captures, parity_cases=self.parities,
                      refusal_cases=self.refusals, llvm_invocations=0,
                      receipts_sha256=digest((self.root / "receipts.json").read_bytes()))
        (self.root / "summary.json").write_text(json.dumps(report, indent=2) + "\n")
        try:
            validate_summary(self.root, self.identities)
        except Exception as error:
            report.update(status='failed', error=str(error))
            (self.root / 'summary.json').write_text(json.dumps(report, indent=2) + '\n')
            raise
        print("PASS: 106 genuine diagnostic captures; 636 text/JSON parity checks; 384 fail-closed checks", flush=True)

    def run(self):
        roster = load_cases()
        for version in (1, 2):
            bundle, hashes = self.build(version)
            captures = {}
            for case in roster:
                name, source = case["name"], case["source"].encode("ascii")
                capture = self.capture(version, name, source, bundle, KINDS.get(name))
                if capture is not None:
                    captures[name] = capture
                    self.parity(version, name, source, capture, bundle, hashes)
            for name, source, kind in extra_cases(version):
                capture = self.capture(version, name, source, bundle, kind)
                captures[name] = capture
                if name == "real-primary-end255":
                    require(len(source) == 255 and capture[2][1567] == 255,
                            "boundary evidence is not a genuine primary endpoint 255")
                self.parity(version, name, source, capture, bundle, hashes)
            self.negatives(version, captures, bundle)
            print(f"v{version}: actual diagnostics and negative controls passed", flush=True)
        self.finish()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--compiler", type=Path, required=True)
    parser.add_argument("--llvm-bin", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--cc", default="cc")
    args = parser.parse_args()
    run = None
    try:
        run = Run(args.compiler, args.llvm_bin, args.output, args.cc)
        run.run()
    except Exception as error:
        if run is not None:
            (run.root / 'failure.json').write_text(json.dumps(dict(
                status='failed', error=str(error), identities=run.identities,
                completed_captures=len(run.captures), completed_parity_checks=len(run.parities),
                completed_refusal_checks=len(run.refusals),
                receipts_sha256=digest((run.root / 'receipts.json').read_bytes())), indent=2) + '\n')
        raise


if __name__ == "__main__":
    main()
