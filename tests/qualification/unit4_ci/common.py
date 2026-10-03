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
INPUTS_SHA = '4dffb6c9320ee7eed2808a05156b37cf07083e326a4fa362b6ca5568b2db8a90'
CURRENT_SHA = '2ce302c0d3182b2b7c57d9cc99d65b9d3b40fd60761331fdec9db5ec8cf3bd5e'
HISTORICAL_HEAD = 'd9e6b9bf172abd5e15da7212c9e6224e29ccc768'
PUBLIC = 'tests/qualification/unit4_public_v3'
HOSTED = 'tests/qualification/unit4_hosted_capability'
PARSER = 'tests/fixtures/typed_project_unit4_parser_portable/frozen/v3'
TRANSPORT = 'tests/fixtures/typed_project_unit4_contracts'
SOURCE = 'tests/fixtures/typed_project_source_binding'
AMENDMENT = 'tests/fixtures/typed_project_unit4_public_location_amendment_v1'
OBSERVER_PATCH = 'tests/fixtures/typed_project_unit4_independent/components/lifecycle/observer-additive-v1.patch'
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


def admit(repo, expected_head, event_sha, committed=True):
    need(not sys.flags.optimize and __debug__, 'optimized Python is forbidden')
    repo = Path(repo).resolve()
    for label, value in (('head', expected_head), ('event SHA', event_sha)):
        need(re.fullmatch('[0-9a-f]{40}', value) is not None, 'invalid ' + label)
    need(sha((HERE / 'inputs.json').read_bytes()) == INPUTS_SHA, 'stale integration input manifest')
    inputs = read(HERE / 'inputs.json')
    verify_package(repo, inputs)
    source_path = repo / SOURCE / 'current-source.json'
    need(identity(source_path)['sha256'] == CURRENT_SHA, 'unapproved source120 manifest')
    source = read(source_path)
    need(len(source['files']) == 120, 'source120 count')
    for row in source['files']:
        verify(repo / relative(row['path']), row)
    actual = sorted(p.relative_to(repo).as_posix() for sub in ('src', 'native') for p in (repo / sub).rglob('*') if p.is_file())
    expected = sorted(row['path'] for row in source['files'] if row['path'].startswith(('src/', 'native/')))
    need(actual == expected, 'source120 exact membership')
    head = git(repo, 'rev-parse', 'HEAD')
    need(head == expected_head, 'checkout is not expected event head')
    names = [row['path'] for row in inputs['files']] + [row['path'] for row in source['files']]
    integration = sorted(p for p in HERE.iterdir() if p.is_file())
    names += [p.relative_to(repo).as_posix() for p in integration]
    names.append('.github/workflows/ci.yml')
    if committed:
        tracked = set(git(repo, 'ls-tree', '-r', '--name-only', head).splitlines())
        need(set(names) <= tracked, 'uncommitted qualification input')
        git(repo, 'diff', '--exit-code', head, '--', *names)
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
