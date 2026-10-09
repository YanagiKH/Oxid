#!/usr/bin/env python3
"""Small, closed admission and evidence transport primitives for Unit4 CI."""
import sys
sys.dont_write_bytecode = True
import gzip
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import platform
import re
import shutil
import stat
import subprocess
import tarfile
import tempfile

HERE = Path(__file__).resolve().parent
INPUTS_SHA = '6ff9bd447a9e6f05cfd41acd40afe4ff02808ae6bcd174363611349d11dc0b64'
CURRENT_SHA = '35ee91911bb62c38c831aecb97c918bd14d9516013f5da3e62f445a1153e1cc4'
ENUM_SHA = '21ebc2e9f7c1b29111b35488334850aa27317bfc2400ad32963c3d7e18a16669'
HISTORICAL_HEAD = 'd9e6b9bf172abd5e15da7212c9e6224e29ccc768'
PUBLIC = 'tests/qualification/unit4_public_v3'
HOSTED = 'tests/qualification/unit4_hosted_capability'
PARSER = 'tests/qualification/unit4_parser_current'
PARSER_FROZEN = 'tests/fixtures/typed_project_unit4_parser_portable/frozen/v3'
TRANSPORT = 'tests/fixtures/typed_project_unit4_contracts'
SOURCE = 'tests/fixtures/typed_project_source_binding'
AMENDMENT = 'tests/fixtures/typed_project_unit4_public_location_amendment_v1'
OBSERVER_PATCH = 'tests/qualification/unit4_public_v3/observer-u8-v1.patch'
RUNTIME_STAGE = 'tests/fixtures/typed_project_unit3_independent/portable/native-v1/stage_llvm_runtime.py'
SECTIONS = ('public', 'original', 'predecessors', 'lifecycle', 'guards')
PROFILES = ('debug', 'release')
HOSTS = ('Linux x86_64', 'macOS x86_64', 'macOS arm64', 'Windows x86_64')
SLUGS = dict(zip(HOSTS, ('linux-x86_64', 'macos-x86_64', 'macos-arm64', 'windows-x86_64')))
COMPACT_LIMIT = 32 * 1024**2
DECODED_LIMIT = 768 * 1024**2


class Reject(Exception):
    pass


def need(condition, message):
    if not condition:
        raise Reject(message)


def sha(raw):
    return hashlib.sha256(raw).hexdigest()


def canonical(value):
    return json.dumps(value, sort_keys=True, separators=(',', ':'), ensure_ascii=False).encode()


def unique(pairs):
    value = {}
    for key, item in pairs:
        need(key not in value, 'duplicate JSON key: ' + key)
        value[key] = item
    return value


def loads(raw):
    return json.loads(raw, object_pairs_hook=unique)


def read(path):
    return loads(Path(path).read_bytes())


def save(path, value):
    path = Path(path)
    with tempfile.NamedTemporaryFile(mode='w', encoding='utf-8', dir=path.parent, prefix=path.name + '.', suffix='.partial', delete=False) as stream:
        stream.write(json.dumps(value, indent=2, sort_keys=True, ensure_ascii=False) + '\n')
        temporary = Path(stream.name)
    os.replace(temporary, path)


def regular(path):
    path = Path(path).absolute()
    need(path.is_file() and not path.is_symlink(), 'missing/nonregular input: ' + str(path))
    for parent in path.parents:
        need(not parent.is_symlink(), 'symlink ancestry: ' + str(parent))
    return path


def identity(path):
    path = regular(path)
    digest = hashlib.sha256()
    size = 0
    with path.open('rb') as stream:
        while chunk := stream.read(1024 * 1024):
            size += len(chunk)
            digest.update(chunk)
    return {'path': str(path), 'bytes': size, 'sha256': digest.hexdigest()}


def verify(path, record):
    actual = identity(path)
    need(all(actual[k] == record[k] for k in ('bytes', 'sha256')), 'changed input: ' + str(path))
    return actual


def relative(name):
    need(isinstance(name, str) and name and '\\' not in name, 'invalid relative path')
    path = Path(name)
    need(not path.is_absolute() and all(p not in ('', '.', '..') for p in name.split('/')), 'unsafe relative path')
    return path


def fresh(path):
    path = Path(path).absolute()
    need(not path.exists() and not path.is_symlink(), 'occupied output: ' + str(path))
    for parent in path.parents:
        need(not parent.is_symlink(), 'symlink output ancestry')
    path.mkdir(parents=True, exist_ok=False)
    return path


def git(repo, *args):
    result = subprocess.run(['git', '-C', str(repo), *args], capture_output=True, timeout=60)
    need(result.returncode == 0, 'Git command failed: ' + ' '.join(args))
    return result.stdout.decode().strip()


def git_diff_paths(repo, head, names):
    """Check every path against HEAD without exceeding Windows' argv limit."""
    args = ['diff', '--exit-code', head, '--']
    # CreateProcess allows 32767 UTF-16 units including the terminating NUL.
    # Use a conservative bound on every host, including Python's exact Windows
    # quoting, the checkout path, and astral characters (two UTF-16 units).
    def units(argv):
        return len(subprocess.list2cmdline(argv).encode('utf-16-le')) // 2
    base = units(['git', '-C', str(repo), *args]) + 1
    batch, size = [], base
    for name in names:
        cost = 1 + units([name])
        need(base + cost <= 16000, 'qualification path exceeds Git command bound')
        if size + cost > 16000:
            git(repo, *args, *batch)
            batch, size = [], base
        batch.append(name)
        size += cost
    if batch:
        git(repo, *args, *batch)


def measured_host():
    system = {'Darwin': 'macOS'}.get(platform.system(), platform.system())
    machine = {'AMD64': 'x86_64', 'aarch64': 'arm64'}.get(platform.machine(), platform.machine())
    name = system + ' ' + machine
    need(name in HOSTS, 'unsupported measured host: ' + name)
    return {'name': name, 'system': platform.system(), 'machine': platform.machine(),
            'release': platform.release(), 'python': sys.version, 'executable': sys.executable,
            'pointer_width': __import__('struct').calcsize('P') * 8,
            'effective_uid': os.geteuid() if hasattr(os, 'geteuid') else None}


def verify_package(repo, manifest):
    rows = manifest['files']
    names = [r['path'] for r in rows]
    need(names == sorted(set(names)) and rows, 'empty/duplicate/unordered package inventory')
    need(all('__pycache__' not in Path(name).parts and not name.endswith(('.pyc', '.pyo'))
             for name in names), 'generated Python cache is not a qualification input')
    for row in rows:
        verify(Path(repo) / relative(row['path']), row)
    for root in manifest['closed_roots']:
        base = Path(repo) / relative(root)
        need(base.is_dir() and not base.is_symlink(), 'missing/aliased component directory')
        actual = []
        for path in base.rglob('*'):
            need(not path.is_symlink() and (path.is_file() or path.is_dir()), 'unsafe component member')
            if path.is_file():
                actual.append(path.relative_to(repo).as_posix())
        expected = [name for name in names if name.startswith(root + '/')]
        need(sorted(actual) == expected, 'changed component membership: ' + root)
        expected_dirs = {Path(p).parent.as_posix() for p in expected}
        expected_dirs |= {parent.as_posix() for p in expected for parent in Path(p).parents if parent.as_posix().startswith(root)}
        need(all(p.relative_to(repo).as_posix() in expected_dirs for p in base.rglob('*') if p.is_dir()), 'extra component directory')


def relative_path_order(paths, root):
    # Path comparison folds case on Windows; committed names must stay exact.
    return sorted(paths, key=lambda path: path.relative_to(root).as_posix())


def admit(repo, expected_head, event_sha, committed=True):
    need(not sys.flags.optimize and __debug__, 'optimized Python is forbidden')
    repo = Path(repo).resolve()
    for label, value in (('head', expected_head), ('event SHA', event_sha)):
        need(re.fullmatch('[0-9a-f]{40}', value) is not None, 'invalid ' + label)
    need(sha((HERE / 'inputs.json').read_bytes()) == INPUTS_SHA, 'stale integration input manifest')
    inputs = read(HERE / 'inputs.json')
    verify_package(repo, inputs)
    source_path = repo / SOURCE / 'current-source.json'
    need(identity(source_path)['sha256'] == CURRENT_SHA, 'unapproved current source manifest')
    source = read(source_path)
    need(len(source['files']) == 363, 'current source count')
    for row in source['files']:
        verify(repo / relative(row['path']), row)
    actual = sorted(p.relative_to(repo).as_posix() for sub in ('src', 'native') for p in (repo / sub).rglob('*') if p.is_file())
    expected = sorted(row['path'] for row in source['files'] if row['path'].startswith(('src/', 'native/')))
    need(actual == expected, 'current source exact membership')
    head = git(repo, 'rev-parse', 'HEAD')
    need(head == expected_head, 'checkout is not expected event head')
    names = [row['path'] for row in inputs['files']] + [row['path'] for row in source['files']]
    integration = relative_path_order((p for p in HERE.iterdir() if p.is_file()), repo)
    names += [p.relative_to(repo).as_posix() for p in integration]
    names.append('.github/workflows/ci.yml')
    if committed:
        tracked = set(git(repo, 'ls-tree', '-r', '--name-only', head).splitlines())
        need(set(names) <= tracked, 'uncommitted qualification input')
        git_diff_paths(repo, head, names)
    publication_root = repo / 'tests/fixtures/typed_project_unit4_parser_portable'
    publication = module('_unit4_parser_publication', publication_root / 'verify_publication.py').verify(publication_root)
    need(publication['status'] == 'pass', 'portable original checkpoint/projection rejected')
    return {'checkout_head': head, 'checkout_tree': git(repo, 'rev-parse', 'HEAD^{tree}'),
            'event_sha': event_sha, 'source_manifest': identity(source_path),
            'source_only_tree': source['source_only_tree'], 'inputs': identity(HERE / 'inputs.json'),
            'workflow': identity(repo / '.github/workflows/ci.yml'),
            'integration': [{'path': p.name, **{k: v for k, v in identity(p).items() if k != 'path'}} for p in integration]}


def module(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    value = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(value)
    return value


def public_modules(repo):
    sys.path.insert(0, str(Path(repo) / PUBLIC))
    import contracts
    import runtime
    import run as collector
    import guards
    from predecessors import Predecessors
    return contracts, runtime, collector, guards, Predecessors


def rows(path):
    with gzip.open(path, 'rt', encoding='utf-8') as stream:
        return [loads(line) for line in stream]


def write_rows(path, values):
    with gzip.open(path, 'wt', encoding='utf-8') as stream:
        for value in values:
            stream.write(json.dumps(value, sort_keys=True, ensure_ascii=False) + '\n')


def pack(directory, archive, limit=None):
    directory, archive = Path(directory), Path(archive)
    need(not archive.exists(), 'archive output occupied')
    with tarfile.open(archive, 'w:gz', compresslevel=6, format=tarfile.USTAR_FORMAT) as sink:
        for path in sorted(directory.rglob('*')):
            need(not path.is_symlink(), 'archive contains symlink')
            if path.is_file():
                sink.add(path, arcname=path.relative_to(directory).as_posix(), recursive=False)
    need(limit is None or archive.stat().st_size < limit, 'compact evidence exceeds32MiB')
    return identity(archive)


def unpack(archive, output):
    need(Path(archive).stat().st_size < COMPACT_LIMIT, 'oversized compact archive')
    output = fresh(output)
    names, total = set(), 0
    with tarfile.open(archive, 'r:*') as source:
        for member in source:
            name = relative(member.name).as_posix()
            need(member.isfile() and name not in names and member.size <= DECODED_LIMIT, 'unsafe/duplicate evidence member')
            names.add(name)
            total += member.size
            need(total <= DECODED_LIMIT and len(names) <= 12000, 'evidence decode bound')
            dest = output / name
            dest.parent.mkdir(parents=True, exist_ok=True)
            with source.extractfile(member) as stream, dest.open('xb') as target:
                shutil.copyfileobj(stream, target)
    return output


def restore_committed_checkout(repo, destination, expected_head):
    """Restore only verified tracked blobs through a fresh, stat-cache-free view."""
    repo = Path(repo).absolute()
    destination = Path(destination).absolute()
    need(repo not in destination.parents and destination != repo, 'restoration output must be outside the checkout')
    output = fresh(destination)
    report = {'schema': 'oxid-unit4-committed-checkout-v1', 'status': 'checking',
              'expected_head': expected_head, 'files': [], 'writes_started': False}
    try:
        need(re.fullmatch('[0-9a-f]{40}', expected_head) is not None, 'invalid expected checkout head')
        report['actual_head'] = git(repo, 'rev-parse', 'HEAD')
        need(report['actual_head'] == expected_head, 'checkout restoration head mismatch')
        def entries(raw, index=False):
            need(raw.endswith('\0'), 'empty or incomplete tracked inventory')
            result = {}
            for entry in raw[:-1].split('\0'):
                header, name = entry.split('\t', 1)
                fields = header.split(' ')
                need(len(fields) == 3, 'invalid tracked metadata')
                mode, oid, kind = fields if index else (fields[0], fields[2], fields[1])
                need(mode in ('100644', '100755') and kind == ('0' if index else 'blob'), 'unsupported tracked mode/type/stage')
                need(re.fullmatch('[0-9a-f]{40}', oid) is not None, 'invalid tracked object identity')
                relative(name)
                need(':' not in name and all(ord(c) >= 32 and ord(c) != 127 for c in name) and
                     '.git' not in [part.lower() for part in name.split('/')], 'unsafe tracked name')
                need(name not in result, 'duplicate tracked member')
                result[name] = {'mode': mode, 'oid': oid}
            return result
        head = entries(git(repo, 'ls-tree', '-rz', '--full-tree', expected_head))
        index = entries(git(repo, 'ls-files', '--stage', '-z'), index=True)
        need(index == head, 'index differs from the expected committed tree')
        view = fresh(output / 'source')
        command = ['git', '-C', str(repo), '-c', 'core.autocrlf=false', '-c', 'core.eol=lf',
                   'checkout-index', '--all', '--force', '--prefix=' + view.as_posix() + '/']
        process = subprocess.run(command, capture_output=True, timeout=120)
        report['materialization'] = {'argv': command, 'status': process.returncode,
                                     'stdout': process.stdout.decode(), 'stderr': process.stderr.decode()}
        need(process.returncode == 0, 'committed-byte materialization failed')
        expected_dirs = {parent.as_posix() for name in head for parent in Path(name).parents if parent != Path('.')}
        actual = []
        for path in view.rglob('*'):
            name = path.relative_to(view).as_posix()
            need(not path.is_symlink(), 'aliased materialized member')
            if path.is_dir(): need(name in expected_dirs, 'extra materialized directory')
            else:
                regular(path)
                actual.append(name)
        need(sorted(actual) == sorted(head), 'materialized membership differs from the committed tree')
        for name in sorted(head):
            path = regular(view / relative(name))
            size = path.stat().st_size
            blob = hashlib.sha1(('blob ' + str(size) + '\0').encode())
            digest = hashlib.sha256()
            total = 0
            with path.open('rb') as stream:
                while chunk := stream.read(1024 * 1024):
                    blob.update(chunk); digest.update(chunk); total += len(chunk)
            need(total == size and blob.hexdigest() == head[name]['oid'], 'materialized bytes differ from the committed blob: ' + name)
            materialized = {'path': str(path), 'bytes': total, 'sha256': digest.hexdigest()}
            before = identity(repo / relative(name))
            report['files'].append({'path': name, 'git': head[name], 'before': before, 'materialized': materialized})
        # Every input, target, member and mode has been checked before overwriting anything.
        report['writes_started'] = True
        for row in report['files']:
            verify(row['materialized']['path'], row['materialized'])
            target = repo / relative(row['path'])
            if any(row['before'][key] != row['materialized'][key] for key in ('bytes', 'sha256')):
                shutil.copyfile(row['materialized']['path'], target)
            row['after'] = verify(target, row['materialized'])
        need(git(repo, 'rev-parse', 'HEAD') == expected_head and entries(git(repo, 'ls-files', '--stage', '-z'), index=True) == head,
             'Git head/index changed during restoration')
        report['status'] = 'restored'
    except BaseException as error:
        report.update(status='failed', failure=str(error)[:2000])
        raise
    finally:
        save(output / 'restoration.json', report)
    return report


if __name__ == '__main__':
    import argparse
    parser = argparse.ArgumentParser()
    parser.add_argument('--restore-checkout', action='store_true', required=True)
    for name in ('repo', 'output', 'expected-head'):
        parser.add_argument('--' + name, required=True)
    args = parser.parse_args()
    try:
        result = restore_committed_checkout(args.repo, args.output, args.expected_head)
        print('Verified committed-byte restoration: ' + str(len(result['files'])) + ' tracked files')
    except (Reject, OSError, ValueError, subprocess.SubprocessError) as error:
        print('Unit4 checkout restoration rejected: ' + str(error), file=sys.stderr)
        raise SystemExit(1)
