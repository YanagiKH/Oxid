#!/usr/bin/env python3
"""Read-only identity check for this historical Unit3A replay package.

This checks bytes and records provenance. It never builds, runs the compiler,
compares observations, adapts a source binding, or grants a semantic pass.
"""
import argparse
import hashlib
import json
from pathlib import Path

PACKAGE = Path(__file__).resolve().parent
AUTHORITY = {
    'contracts-v2': '45e01b88abfd91a1a30eb80cfa65c033a1b2986f365d05caa44836c8051231bf',
    'element-boundary-supplement-v1': 'a8c8454ac11355708cebbd749ade26335ed477e6649da75089184c5b7162fb00',
}


def sha(path):
    if not path.is_file() or path.is_symlink():
        raise ValueError(f'not an ordinary file: {path}')
    result = hashlib.sha256()
    with path.open('rb') as source:
        for chunk in iter(lambda: source.read(1024 * 1024), b''):
            result.update(chunk)
    return result.hexdigest()


def safe_path(root, relative):
    part = Path(relative)
    if part.is_absolute() or not part.parts or any(p in ('.', '..') for p in part.parts):
        raise ValueError(f'unsafe manifest path: {relative}')
    candidate = root
    for component in part.parts:
        candidate /= component
        if candidate.is_symlink():
            raise ValueError(f'symlink in manifest path: {relative}')
    return candidate


def check_files(root, entries):
    for name, spec in entries.items():
        path = safe_path(root, name)
        expected = spec if isinstance(spec, str) else spec['sha256']
        if sha(path) != expected:
            raise ValueError(f'body hash mismatch: {name}')
        if isinstance(spec, dict) and path.stat().st_size != spec['bytes']:
            raise ValueError(f'body length mismatch: {name}')
    return len(entries)


def check_request_tuples(requests, authority):
    """Derive the exact historical roster from frozen source authorities."""
    cases = json.loads((authority / 'contracts-v2/cases.json').read_text())['cases']
    selected = [case for case in cases if case['first_observation_slice'] == '3A']
    if len(selected) != 58 or len({case['id'] for case in selected}) != 58:
        raise ValueError('frozen frontend roster must contain 58 distinct cases')
    core = []
    for case in selected:
        root = safe_path(authority, 'contracts-v2/fixtures/' + case['id'])
        if not root.is_dir() or root.resolve(strict=True) != root:
            raise ValueError('frontend source directory is not canonical')
        mode = 'single' if len(case['files']) == 1 else 'project'
        core.append((case['id'], mode, str(root), '200000', '16777216'))
    cases = json.loads((authority / 'element-boundary-supplement-v1/cases.json').read_text())['cases']
    if len(cases) != 6 or len({case['id'] for case in cases}) != 6:
        raise ValueError('frozen supplement roster must contain 6 distinct cases')
    supplement = []
    for case in cases:
        root = safe_path(requests, 'supplement-fixtures/' + case['id'])
        if not root.is_dir() or root.resolve(strict=True) != root:
            raise ValueError('supplement source directory is not canonical')
        supplement.append((case['id'], 'single', str(root), '200000', '16777216'))
    root = safe_path(authority, 'contracts-v2/fixtures/grouped-complete-access-and-index')
    if not root.is_dir() or root.resolve(strict=True) != root:
        raise ValueError('limit-control source directory is not canonical')
    limits = [(label, 'single', str(root), str(rows), str(size))
              for label, rows, size in [('zero-rows', 0, 16777216),
                                        ('one-row', 1, 16777216),
                                        ('zero-bytes', 200000, 0),
                                        ('one-byte', 200000, 1)]]
    rosters = {'core.tsv': core, 'supplement.tsv': supplement, 'observer-limits.tsv': limits}
    for name, expected in rosters.items():
        path = safe_path(requests, name)
        if not path.is_file():
            raise ValueError('missing request roster: ' + name)
        with path.open('rb') as source:
            actual = source.read(256 * 1024 + 1)
        if len(actual) >= 256 * 1024:
            raise ValueError('request roster exceeds the historical input bound: ' + name)
        canonical = ('\n'.join('\t'.join(row) for row in expected) + '\n').encode('utf-8')
        if actual != canonical:
            raise ValueError('request tuple roster mismatch: ' + name)
    return {name: len(rows) for name, rows in rosters.items()}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--authority-root', type=Path, default=PACKAGE.parent)
    parser.add_argument('--source', type=Path)
    parser.add_argument('--stage', choices=('checkpoint4', 'assembled'))
    parser.add_argument('--binary', type=Path)
    parser.add_argument('--requests', type=Path)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    if bool(args.source) != bool(args.stage):
        parser.error('--source and --stage must be provided together')
    if args.binary and args.stage != 'assembled':
        parser.error('--binary requires --source and --stage assembled')
    args.authority_root = args.authority_root.resolve(strict=True)
    if args.output.exists():
        raise ValueError('receipt must be a new file')
    package_manifest = PACKAGE / 'package-manifest.json'
    package = json.loads(package_manifest.read_text())
    package_count = check_files(PACKAGE, package['files'])
    found = {str(p.relative_to(PACKAGE)) for p in PACKAGE.rglob('*') if p.is_file() or p.is_symlink()}
    if found != set(package['files']) | {'package-manifest.json'}:
        raise ValueError('package file roster differs')
    receipt = {'schema': 'oxid-unit3a-replay-input-check-v1',
               'package_manifest_sha256': sha(package_manifest),
               'package_files': package_count, 'compiler_runs': 0,
               'semantic_comparisons': 0, 'authorities': {}}
    for name, expected in AUTHORITY.items():
        folder = args.authority_root / name
        manifest = folder / 'freeze-manifest.json'
        if sha(manifest) != expected:
            raise ValueError(f'authority manifest mismatch: {name}')
        body = json.loads(manifest.read_text())
        receipt['authorities'][name] = {
            'manifest_sha256': expected,
            'files': check_files(folder, body['files'])}
    if args.source:
        source = args.source.resolve()
        if args.stage == 'assembled':
            manifest = PACKAGE / 'inputs/assembled-v3-files.json'
            entries = json.loads(manifest.read_text())
        else:
            manifest = PACKAGE / 'inputs/checkpoint4-manifest.json'
            entries = {row['path']: row for row in json.loads(manifest.read_text())['files']}
        found = {str(p.relative_to(source)) for p in source.rglob('*') if p.is_file() or p.is_symlink()}
        if found != set(entries):
            raise ValueError('historical source file roster differs; use a fresh exact archive, not a current checkout')
        receipt['source'] = {'stage': args.stage, 'manifest_sha256': sha(manifest),
                             'files': check_files(source, entries)}
    if args.binary:
        receipt['binary'] = {'sha256': sha(args.binary), 'bytes': args.binary.stat().st_size,
                             'claim': 'Local binary identity only; bind its build command/toolchain receipt separately'}
    if args.requests:
        requests = args.requests.resolve(strict=True)
        receipt['request_tuple_counts'] = check_request_tuples(requests, args.authority_root)
        names = {'core.tsv', 'supplement.tsv', 'observer-limits.tsv'}
        cases = json.loads((args.authority_root / 'element-boundary-supplement-v1/cases.json').read_text())['cases']
        for case in cases:
            name = 'supplement-fixtures/' + case['id'] + '/main.ox'
            path = safe_path(requests, name)
            if sha(path) != case['source_sha256'] or path.stat().st_size != case['source_bytes']:
                raise ValueError('materialized supplemental source mismatch: ' + name)
            names.add(name)
        found = {str(p.relative_to(requests)) for p in requests.rglob('*') if p.is_file() or p.is_symlink()}
        if found != names:
            raise ValueError('generated request/source roster differs')
        receipt['requests'] = {name: {'sha256': sha(safe_path(requests, name)),
                                     'bytes': (requests / name).stat().st_size} for name in sorted(names)}
    with args.output.open('x') as destination:
        json.dump(receipt, destination, indent=2)
        destination.write('\n')
    print(json.dumps({'input_identity': 'PASS', 'compiler_runs': 0, 'semantic_comparisons': 0}))


if __name__ == '__main__':
    main()
