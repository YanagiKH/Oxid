#!/usr/bin/env python3
"""Replay immutable RFC0030 integer-oracle batches, with bounded helper adaptation.

This standalone runner qualifies only the supplied executable. Exact-head build
and pinned Debian staging admission belong to verify_bounded_u8_native.py.
Original fixtures and their mathematical expectations are never rewritten.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess
import time

ORACLE = Path(__file__).resolve().parents[1] / 'tests/qualification/bounded_u8_current/oracle'
OPS = ('==', '!=', '<', '<=', '>', '>=')
KEY_COUNT = 256 * 256 * len(OPS)
ELF_ENV = {'PATH': '/usr/bin:/bin', 'LANG': 'C', 'LC_ALL': 'C'}
# Immutable independently designed source roster; these are input identities,
# never evidence that any compiler executed them successfully.
COVERAGE_SHA256 = '0ebd5c0280bb4537ee9fee6e8ffda7bf088aa53dc01045369592774c5c2815d3'
ROUNDTRIP_SHA256 = {'scalar': 'e0e9eee05f7f03dead426def0682a0d786632e330431d20bd7a6a593c0f4b306', 'owned': '13c9e480b158f685dc64e6799e014eca05deece728a22a625e9565041cdf5909'}


def require(value, message):
    if not value:
        raise ValueError(message)


def sha(data):
    return hashlib.sha256(data).hexdigest()


def identity(path):
    return {'path': str(path), 'bytes': path.stat().st_size, 'sha256': sha(path.read_bytes())}


def save(path, value):
    path.write_text(json.dumps(value, indent=2, sort_keys=True) + '\n')


def truth(left, right):
    return (left == right, left != right, left < right, left <= right, left > right, left >= right)


def expected_stream():
    digest = hashlib.sha256()
    for left in range(256):
        for right in range(256):
            for operator, result in enumerate(truth(left, right)):
                digest.update(bytes((left, right, operator, int(result))))
    return digest.hexdigest()


def partition_body(left, part, low, high):
    """Independent integer expectations validate every frozen partition body."""
    j, b = f'j{left}_{part}', f'b{left}_{part}'
    lines = [f'let mut {j} = {low};', f'while {j} < {high} {{',
             f'let {b} = {j}.to_u8_checked();']
    expected = truth(left, low)
    require(all(truth(left, right) == expected for right in range(low, high)),
            'oracle partition crosses a mathematical comparison boundary')
    for operator, (spelling, result) in enumerate(zip(OPS, expected)):
        lines.append(f'if (a{left} {spelling} {b}) != {str(result).lower()} {{ return -(1 + ({left} * 256 + {j}) * 6 + {operator}); }}')
    lines += ['checked = checked + 6;', f'{j} = {j} + 1;', '}']
    return '\n'.join(lines) + '\n'


def partition_helpers(source, route, batch):
    """Move byte-identical oracle partitions to bounded, nonrecursive helpers."""
    require(route in ('scalar', 'owned') and type(batch) is int and 0 <= batch < 32,
            'invalid route/batch')
    marker = 'struct Marker {}\n' if route == 'owned' else ''
    original = [marker + 'fn main() -> i32 {\nlet mut checked = 0;\n']
    helpers, calls, seals = [], [], []
    for left in range(batch * 8, batch * 8 + 8):
        binding = f'let n{left} = {left};\nlet a{left} = n{left}.to_u8_checked();\n'
        original.append(binding)
        for part, (low, high) in enumerate(((0, left), (left, left + 1), (left + 1, 256))):
            if low == high:
                continue
            body = partition_body(left, part, low, high)
            original.append(body)
            name = f'partition_{left}_{part}'
            helpers.append(f'fn {name}() -> i32 {{\nlet mut checked = 0;\n{binding}{body}return checked;\n}}\n')
            calls.append(f'checked = preserve_failure(checked, {name}());')
            seals.append({'left': left, 'right_range': [low, high], 'partition': part,
                          'operators': list(OPS), 'keys': (high - low) * len(OPS),
                          'unchanged_original_body_sha256': sha(body.encode())})
    original.append('return checked;\n}\n')
    require(source == ''.join(original), 'frozen source does not match independent integer partition oracle')
    combiner = 'fn preserve_failure(a:i32,b:i32)->i32{if a<0{return a;}if b<0{return b;}return a+b;}\n'
    main = 'fn main()->i32{\nlet mut checked=0;\n' + '\n'.join(calls) + '\nreturn checked;\n}\n'
    return marker + combiner + ''.join(helpers) + main, seals


def mark_coverage(bitmap, seals):
    for seal in seals:
        left = seal['left']
        low, high = seal['right_range']
        require(type(left) is int and 0 <= left < 256 and type(low) is int
                and type(high) is int and 0 <= low < high <= 256,
                'invalid coverage range')
        require(seal['operators'] == list(OPS) and seal['keys'] == (high - low) * len(OPS),
                'incomplete operator coverage')
        for right in range(low, high):
            for operator in range(len(OPS)):
                key = (left * 256 + right) * len(OPS) + operator
                require(bitmap[key] == 0, 'duplicate pair/operator coverage')
                bitmap[key] = 1


def admit_oracle(oracle=ORACLE):
    data = (oracle / 'coverage.json').read_bytes()
    require(sha(data) == COVERAGE_SHA256, 'frozen oracle coverage authority changed')
    manifest = json.loads(data)
    require(manifest['unique_pair_operator_keys'] == KEY_COUNT
            and manifest['expected_stream_sha256'] == expected_stream(), 'mathematical oracle digest differs')
    expected_names = {f'pairs-{route}-{batch:02}.ox' for route in ('scalar', 'owned') for batch in range(32)}
    rows = manifest['batches']
    require(len(rows) == 64 and {row['file'] for row in rows} == expected_names,
            'missing, duplicate or unexpected oracle batches')
    require({path.name for path in oracle.iterdir()} == expected_names | {
        'coverage.json', 'roundtrip-scalar.ox', 'roundtrip-owned.ox'}, 'oracle directory membership differs')
    for row in rows:
        match = re.fullmatch(r'pairs-(scalar|owned)-(\d{2})\.ox', row['file'])
        route, batch = match[1], int(match[2])
        require(row['route'] == route and row['left_range'] == [batch * 8, batch * 8 + 8]
                and row['right_range'] == [0, 256] and row['operators'] == list(OPS)
                and row['expected_i32'] == 12288, 'oracle batch metadata differs')
        data = (oracle / row['file']).read_bytes()
        require(len(data) == row['bytes'] and sha(data) == row['source_sha256'], 'frozen oracle source changed')
        partition_helpers(data.decode(), route, batch)
    for route in ('scalar', 'owned'):
        require(sha((oracle / f'roundtrip-{route}.ox').read_bytes()) == ROUNDTRIP_SHA256[route],
                'frozen roundtrip source changed')
    return manifest


def invoke(directory, label, argv, *, cwd, env, expected_status=0, timeout=180):
    argv = [str(value) for value in argv]
    started, error = time.monotonic(), None
    try:
        result = subprocess.run(argv, cwd=cwd, env=env, capture_output=True, timeout=timeout)
        status, stdout, stderr = result.returncode, result.stdout, result.stderr
    except (subprocess.TimeoutExpired, OSError) as caught:
        error = str(caught)
        status = None
        stdout, stderr = getattr(caught, 'stdout', None) or b'', getattr(caught, 'stderr', None) or b''
    out, err = directory / (label + '.stdout'), directory / (label + '.stderr')
    out.write_bytes(stdout)
    err.write_bytes(stderr)
    save(directory / (label + '.json'), {'argv': argv, 'cwd': str(cwd), 'status': status,
        'error': error, 'elapsed_seconds': time.monotonic() - started,
        'stdout_sha256': sha(stdout), 'stderr_sha256': sha(stderr),
        'environment': ELF_ENV if env == ELF_ENV else {
            'mode': 'inherited', 'OXID_LLVM_BIN': env.get('OXID_LLVM_BIN'),
            'PYTHONDONTWRITEBYTECODE': env.get('PYTHONDONTWRITEBYTECODE'),
            'OXID_U8_NATIVE_EVIDENCE': env.get('OXID_U8_NATIVE_EVIDENCE'),
            'OXID_UNARY_NATIVE_EVIDENCE': env.get('OXID_UNARY_NATIVE_EVIDENCE')}})
    require(status == expected_status, f'{label}: unexpected status {status}; retained original streams')
    return stdout, stderr


def run_case(root, binary, original, route, batch, env):
    directory = root / f'batch-{batch:02}'
    directory.mkdir()
    source = directory / 'original.ox'
    source.write_bytes(original)
    expected = b'12288\n'
    require(invoke(directory, 'reference-original', [binary, 'run', source, '--edition=typed-preview'],
                   cwd=directory, env=env) == (expected, b''), 'original reference oracle mismatch')
    if batch == 0:
        refused_output = directory / 'original-program'
        stdout, stderr = invoke(directory, 'compile-original', [binary, 'compile', source,
            '--edition=typed-preview', '--backend=llvm', '--message-format=json', f'--output={refused_output}'],
            cwd=directory, env=env, expected_status=1)
        rows = [json.loads(line) for line in stdout.splitlines()]
        diagnostic = [row for row in rows if row.get('kind') == 'diagnostic']
        message = ('native preview locals per function limit exceeded (256)' if route == 'scalar'
                   else 'native owned scalar slots per function limit exceeded (256)')
        require(not stderr and len(diagnostic) == 1 and diagnostic[0]['code'] == 'E0700'
                and diagnostic[0]['stage'] == 'native-admission' and diagnostic[0]['message'] == message
                and not refused_output.exists(), 'original native cap/refusal endpoint changed')
    adapted, seals = partition_helpers(original.decode(), route, batch)
    candidate = directory / 'partition-helpers.ox'
    candidate.write_text(adapted)
    save(directory / 'partition-seals.json', seals)
    require(invoke(directory, 'reference-adapted', [binary, 'run', candidate, '--edition=typed-preview'],
                   cwd=directory, env=env) == (expected, b''), 'adapted reference oracle mismatch')
    elf = compile_run(directory, binary, candidate, expected, env)
    return {'requested_source': identity(source), 'compiled_source': identity(candidate),
            'elf': elf, 'partitions': seals, 'expected_checks': 12288}, seals


def compile_run(directory, binary, source, expected, env):
    sourcefree = directory / 'source-free'
    sourcefree.mkdir()
    executable = sourcefree / 'program'
    stdout, stderr = invoke(directory, 'compile', [binary, 'compile', source, '--edition=typed-preview',
        '--backend=llvm', '--message-format=json', f'--output={executable}'], cwd=directory, env=env)
    rows = [json.loads(line) for line in stdout.splitlines()]
    require(not stderr and rows and rows[-1].get('kind') == 'compile-summary'
            and rows[-1].get('success') is True, 'missing successful native compile summary')
    require(executable.read_bytes()[:4] == b'\x7fELF' and list(sourcefree.iterdir()) == [executable],
            'source-free ELF is missing or directory contains other inputs')
    before = identity(executable)
    require(invoke(directory, 'execute', [executable], cwd=sourcefree, env=ELF_ENV) == (expected, b''),
            'native integer oracle mismatch')
    require(identity(executable) == before and list(sourcefree.iterdir()) == [executable],
            'source-free ELF or execution directory changed')
    return before


def run(binary, root, route, env, oracle=ORACLE):
    manifest = admit_oracle(oracle)
    binary = binary.resolve(strict=True)
    before = identity(binary)
    bitmap, results = bytearray(KEY_COUNT), []
    for batch in range(32):
        path = oracle / f'pairs-{route}-{batch:02}.ox'
        result, seals = run_case(root, binary, path.read_bytes(), route, batch, env)
        mark_coverage(bitmap, seals)
        results.append(result)
        save(root / 'pair-progress.json', results)
        print(f'{route} batch {batch:02}: 12288 reference/native checks; {sum(bitmap)} unique keys', flush=True)
    require(all(bitmap) and sum(bitmap) == KEY_COUNT, 'incomplete pair/operator coverage')
    (root / 'coverage.bin').write_bytes(bitmap)
    directory = root / 'roundtrip'
    directory.mkdir()
    source = directory / 'input.ox'
    source.write_bytes((oracle / f'roundtrip-{route}.ox').read_bytes())
    require(invoke(directory, 'reference', [binary, 'run', source, '--edition=typed-preview'],
                   cwd=directory, env=env) == (b'256\n', b''), 'roundtrip reference mismatch')
    elf = compile_run(directory, binary, source, b'256\n', env)
    require(identity(binary) == before and admit_oracle(oracle) == manifest,
            'compiler or frozen oracle changed during execution')
    summary = {'schema': 'rfc0030-exhaustive-corpus-1', 'status': 'passed', 'route': route,
        'compiler': before, 'pair_batches': 32, 'pair_operator_keys': KEY_COUNT,
        'expected_stream_sha256': manifest['expected_stream_sha256'],
        'coverage_bitmap_sha256': sha(bitmap), 'roundtrip_values': 256,
        'roundtrip': {'source': identity(source), 'elf': elf}, 'batches': results,
        'adaptation': 'Byte-identical original integer-oracle partition bodies moved into bounded helpers; immutable originals run separately; all limits unchanged.',
        'isolation': 'Source-free cwd and minimal ELF environment; not filesystem isolation.'}
    save(root / 'summary.json', summary)
    return summary


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--compiler', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--route', choices=('scalar', 'owned'), required=True)
    args = parser.parse_args()
    root = args.output.resolve()
    root.mkdir(parents=True, exist_ok=False)
    result = {'status': 'failed', 'scope': 'supplied executable only', 'route': args.route}
    try:
        run(args.compiler, root, args.route, dict(os.environ, PYTHONDONTWRITEBYTECODE='1'))
        result['status'] = 'passed'
    except Exception as error:
        result['error'] = str(error)
        raise
    finally:
        save(root / 'result.json', result)


if __name__ == '__main__':
    main()
