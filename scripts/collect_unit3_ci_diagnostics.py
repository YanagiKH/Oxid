#!/usr/bin/env python3
"""Bounded CI diagnosis; full qualification artifacts remain authoritative."""
import argparse
import gzip
import hashlib
import io
import json
import os
import pathlib
import stat
import tarfile

LOG_LIMIT = 16 * 1024
WRAPPER_LIMIT = 64 * 1024
FILE_LIMIT = 4 * 1024 * 1024
PAYLOAD_LIMIT = 15 * 1024 * 1024
TAR_LIMIT = 16 * 1024 * 1024
LOG_PATHS = tuple(
    f'native/native-component/build-{profile}/{stream}'
    for profile in ('debug', 'release') for stream in ('stdout', 'stderr')
)
RECEIPT_PATHS = (
    'package-verification.json', 'llvm-runtime/llvm-runtime-stage.json',
    'inputs/materialization.json',
    'bridge/bridge-receipt.json', 'native-auxiliary/auxiliary-transport.json',
    'native-inputs/native-inputs.json', 'observer/prepared-source.json',
    'source-build-debug/build.json', 'source-build-release/build.json',
    'source-plan.json', 'source-collection/collection.json',
    'native/native-prepared.json', 'native-build-wrapper.json',
    'native/native-component/qualified-build-tools.json',
    'native/native-component/build-debug/verified-build.json',
    'native/native-component/build-release/verified-build.json',
    'native-plan.json', 'native-collection/collection.json',
    'mutations-v2/prepared-mutation.json', 'mutations-v3/prepared-mutation.json',
    'mutation-build-v2-debug/build.json', 'mutation-build-v2-release/build.json',
    'mutation-build-v3-debug/build.json', 'mutation-build-v3-release/build.json',
    'mutation-build-v2-debug/mutation-build-view.json',
    'mutation-build-v2-release/mutation-build-view.json',
    'mutation-build-v3-debug/mutation-build-view.json',
    'mutation-build-v3-release/mutation-build-view.json',
    'mutation-plan.json', 'mutation-collection/collection.json',
    'comparison-view/prepared-comparison.json',
    'source-comparison/comparison.json', 'native-comparison/comparison.json',
    'mutation-comparison/comparison.json',
    'mutation-comparison/driver-boundaries/driver-attributions.json',
    'ci-evidence-index.json',
)


def emit_json(row):
    # The runner's legacy command parser can recognize ##[ anywhere in a line.
    # This escape preserves JSON-decoded text while removing that literal token.
    print(json.dumps(row, ensure_ascii=True).replace('##[', '##\\u005b'))


def read_fixed(root, name, limit):
    """Read a fixed regular leaf, with a byte cap before any JSON parsing."""
    path = root / name
    try:
        if root.is_symlink() or not root.is_dir():
            return None, 'missing-or-aliased-root', False
        for parent in path.parents:
            if parent == root:
                break
            if parent.is_symlink():
                return None, 'aliased-parent', False
        if path.is_symlink() or not path.resolve().is_relative_to(root):
            return None, 'aliased-or-escaping-path', False
        descriptor = os.open(path, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK)
        with os.fdopen(descriptor, 'rb') as stream:
            if not stat.S_ISREG(os.fstat(stream.fileno()).st_mode):
                return None, 'nonregular-file', False
            payload = stream.read(limit + 1)
        return payload[:limit], 'read', len(payload) > limit
    except (OSError, ValueError):
        return None, 'unavailable', False


def failure_logs(root):
    name = 'native-build-wrapper.json'
    payload, status, truncated = read_fixed(root, name, WRAPPER_LIMIT)
    if payload is not None and not truncated:
        try:
            document = json.loads(payload)
            error = document.get('error') if isinstance(document, dict) else None
            if isinstance(error, str):
                emit_json({'path': name, 'error': error[:LOG_LIMIT],
                           'truncated': len(error) > LOG_LIMIT})
            else:
                emit_json({'path': name, 'status': 'no-string-error-field'})
        except (ValueError, RecursionError):
            emit_json({'path': name, 'status': 'invalid-bounded-json'})
    else:
        emit_json({'path': name, 'status': 'over-byte-limit' if truncated else status,
                   'byte_limit': WRAPPER_LIMIT})
    for name in LOG_PATHS:
        payload, status, truncated = read_fixed(root, name, LOG_LIMIT)
        row = {'path': name, 'status': status, 'byte_limit': LOG_LIMIT,
               'truncated': truncated}
        if payload is not None:
            row['text'] = payload.decode('utf-8', errors='replace')
        emit_json(row)


def compact(root, destination, outcome):
    rows = []
    included = []
    total = 0
    for name in RECEIPT_PATHS + LOG_PATHS:
        limit = LOG_LIMIT if name in LOG_PATHS else FILE_LIMIT
        payload, status, truncated = read_fixed(root, name, limit)
        row = {'path': name, 'read_status': status, 'byte_limit': limit,
               'truncated': truncated, 'included': False}
        if payload is not None:
            if truncated and name not in LOG_PATHS:
                row['omission'] = 'receipt exceeds byte limit; full receipt remains in full archive'
            elif total + len(payload) > PAYLOAD_LIMIT:
                row['omission'] = 'compact payload budget exhausted; full bytes remain in full archive'
            else:
                row.update(included=True, bytes=len(payload), sha256=hashlib.sha256(payload).hexdigest(),
                           identity_scope='bounded log prefix' if truncated else 'complete file bytes')
                included.append((name, payload))
                total += len(payload)
                if name not in LOG_PATHS:
                    try:
                        document = json.loads(payload)
                        if isinstance(document, dict):
                            row['observed_fields'] = {
                                key: value for key, value in document.items()
                                if key in ('status', 'kind', 'scope', 'count', 'matched',
                                           'completed_rows', 'planned_rows', 'exit_code')
                                and type(value) in (str, int, bool, type(None))
                                and (not isinstance(value, str) or len(value) <= 256)
                            }
                    except (ValueError, RecursionError):
                        row['json_status'] = 'invalid-bounded-json'
        rows.append(row)
    summary = {
        'schema': 'unit3-compact-ci-diagnosis-v1', 'qualification_step_outcome': outcome,
        'purpose': 'Diagnostic receipt transport; this does not certify qualification',
        'binary_byte_replay': False,
        'binary_identity_limit': 'Receipt hashes identify binaries; this archive does not independently replay their bytes',
        'full_archive': 'typed-project-unit3-evidence.tar.gz',
        'missing_reports': 'Missing or omitted comparison reports provide no PASS evidence',
        'payload_limit': PAYLOAD_LIMIT, 'uncompressed_archive_limit': TAR_LIMIT,
        'included_payload_bytes': total, 'files': rows,
    }
    metadata = (json.dumps(summary, sort_keys=True, indent=2) + '\n').encode()
    if len(metadata) > 64 * 1024:
        raise RuntimeError('compact index exceeds metadata budget')
    buffer = io.BytesIO()
    with tarfile.open(fileobj=buffer, mode='w', format=tarfile.USTAR_FORMAT) as archive:
        for name, payload in included + [('compact-index.json', metadata)]:
            info = tarfile.TarInfo(name)
            info.size = len(payload)
            info.mode = 0o644
            archive.addfile(info, io.BytesIO(payload))
    raw = buffer.getvalue()
    if len(raw) > TAR_LIMIT:
        raise RuntimeError('compact archive exceeds uncompressed budget')
    packed = gzip.compress(raw, compresslevel=6, mtime=0)
    with destination.open('xb') as output:
        output.write(packed)
    digest = hashlib.sha256(packed).hexdigest()
    with destination.with_suffix(destination.suffix + '.sha256').open('x') as output:
        output.write(digest + '  ' + destination.name + '\n')
    emit_json({'diagnostic_archive': destination.name, 'included_files': len(included),
               'uncompressed_bytes': len(raw), 'compressed_bytes': len(packed),
               'sha256': digest, 'qualification_step_outcome': outcome,
               'binary_byte_replay': False})
    return summary


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('mode', choices=('failure-logs', 'compact'))
    mode = parser.parse_args().mode
    temporary = pathlib.Path(os.environ['RUNNER_TEMP']).resolve()
    root = temporary / 'typed-project-unit3'
    outcome = os.environ.get('UNIT3_STEP_OUTCOME', 'unknown')
    if mode == 'failure-logs':
        if outcome != 'failure':
            raise SystemExit('failure-log mode requires the actual failed qualification step')
        failure_logs(root)
    else:
        compact(root, temporary / 'typed-project-unit3-compact.tar.gz', outcome)


if __name__ == '__main__':
    main()
