#!/usr/bin/env python3
"""Stage the pinned Debian LLVM override libraries, never the host library tree."""
import argparse
import hashlib
import json
import os
import pathlib
import re
import stat
import subprocess
import sys

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parents[1]))
from common import SCHEMA, assertion_mode, bound_file, require, write_json

SOURCE_DIRECTORY = pathlib.Path('/usr/lib/x86_64-linux-gnu')
PACKAGE_VERSION = '1:19.1.7-3+b1'
BINARY_PACKAGES = {'libllvm19': 'libllvm19:amd64', 'libclang-cpp19': 'libclang-cpp19'}
RUNTIME_FILES = {
    'libLLVM.so.19.1': {
        'package': 'libllvm19', 'bytes': 129673080,
        'sha256': '3523f50f635d2a1ea47518a392451f2854b92823ceaf1345fb806099dd1a3b8a',
    },
    'libclang-cpp.so.19.1': {
        'package': 'libclang-cpp19', 'bytes': 71043800,
        'sha256': '0108c687d2f500d77687984c46c9fad663782603f5c623e3c14224c155b4fd21',
    },
}
ALIAS = 'libLLVM-19.so'
ALIAS_TARGET = 'libLLVM.so.19.1'
MAXIMUM_FILE = 256 * 1024 * 1024
MAXIMUM_TOTAL = 512 * 1024 * 1024


def command(argv):
    result = subprocess.run(argv, text=True, capture_output=True, timeout=20,
                            env={**os.environ, 'LC_ALL': 'C'})
    require(result.returncode == 0, f'qualification command failed: {argv[0]}')
    require(len(result.stdout) <= 65536 and len(result.stderr) <= 65536,
            'qualification command output limit')
    return result.stdout


def package_inventory(root):
    records = []
    for package in sorted({row['package'] for row in RUNTIME_FILES.values()}):
        selector = BINARY_PACKAGES[package]
        status = command(['dpkg-query', '--show', '--showformat',
                          '${binary:Package}\t${Version}\t${Architecture}\t${db:Status-Status}\n',
                          selector])
        require(status == f'{selector}\t{PACKAGE_VERSION}\tamd64\tinstalled\n',
                f'wrong installed LLVM package version/architecture/status: {package}')
        owned = command(['dpkg-query', '--listfiles', selector]).splitlines()
        names = sorted(name for name, row in RUNTIME_FILES.items()
                       if row['package'] == package)
        if package == 'libllvm19':
            names.append(ALIAS)
        paths = sorted(str(root / name) for name in names)
        for path in paths:
            require(owned.count(path) == 1, f'required LLVM package path missing: {path}')
            owner = command(['dpkg-query', '--search', path])
            require(owner == f'{selector}: {path}\n',
                    f'required LLVM path owner differs: {path}')
        records.append({'package': package, 'binary_package': selector, 'version': PACKAGE_VERSION,
                        'architecture': 'amd64', 'status': 'installed',
                        'owned_paths': paths})
    return records


def checked_file(path, expected, destination=None):
    """No-follow open rejects substituted links, directories and blocking FIFOs."""
    require(path.resolve() == path, f'aliased or escaping required LLVM file: {path}')
    descriptor = os.open(path, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK)
    with os.fdopen(descriptor, 'rb') as stream:
        metadata = os.fstat(stream.fileno())
        require(stat.S_ISREG(metadata.st_mode), f'nonregular required LLVM file: {path}')
        require(metadata.st_size == expected['bytes'] <= MAXIMUM_FILE,
                f'required LLVM file size differs: {path}')
        digest = hashlib.sha256()
        size = 0
        output = destination.open('xb') if destination is not None else None
        try:
            while chunk := stream.read(min(1024 * 1024, expected['bytes'] - size + 1)):
                size += len(chunk)
                require(size <= expected['bytes'], f'required LLVM file grew: {path}')
                digest.update(chunk)
                if output is not None:
                    output.write(chunk)
        finally:
            if output is not None:
                output.close()
    identity = {'bytes': size, 'sha256': digest.hexdigest()}
    require(identity == {key: expected[key] for key in ('bytes', 'sha256')},
            f'required LLVM file identity differs: {path}')
    return identity


def checked_alias(root):
    path = root / ALIAS
    require(path.is_symlink(), 'required LLVM alias is not a symlink')
    require(os.readlink(path) == ALIAS_TARGET, 'required LLVM alias target differs')
    target = path.resolve(strict=True)
    require(target == root / ALIAS_TARGET and target.is_file() and
            not (root / ALIAS_TARGET).is_symlink(),
            'required LLVM alias is nonregular, aliased or escaping')


def stage(output):
    root = SOURCE_DIRECTORY.absolute()
    require(root.resolve() == root and root.is_dir(), 'LLVM source directory is missing or aliased')
    out = pathlib.Path(output).absolute()
    require(out.resolve() == out and not out.exists() and not out.is_symlink(),
            'LLVM stage output already exists or is aliased')
    require(not out.is_relative_to(root) and not root.is_relative_to(out),
            'LLVM stage output overlaps source directory')
    require(sum(row['bytes'] for row in RUNTIME_FILES.values()) <= MAXIMUM_TOTAL,
            'LLVM stage total byte limit')
    packages = package_inventory(root)
    checked_alias(root)
    entries = []
    for name, expected in sorted(RUNTIME_FILES.items()):
        identity = checked_file(root / name, expected)
        dynamic = command(['readelf', '--dynamic', str(root / name)])
        sonames = re.findall(r'\(SONAME\).*\[([^\]]+)\]', dynamic)
        require(sonames == [name], f'required LLVM SONAME differs: {name}')
        entries.append({'path': name, 'kind': 'file', 'package': expected['package'],
                        'soname': name, **identity})
    entries.append({'path': ALIAS, 'kind': 'symlink', 'link': ALIAS_TARGET,
                    'package': 'libllvm19'})
    entries.sort(key=lambda row: row['path'])
    # Identity is independent of temporary workspace paths and timestamps.
    content = {'package_version': PACKAGE_VERSION, 'architecture': 'amd64', 'entries': entries}
    content_sha256 = hashlib.sha256(json.dumps(content, sort_keys=True,
                                              separators=(',', ':')).encode()).hexdigest()
    out.mkdir(parents=True, exist_ok=False)
    libraries = out / 'libraries'
    libraries.mkdir()
    for name, expected in sorted(RUNTIME_FILES.items()):
        checked_file(root / name, expected, libraries / name)
        checked_file(libraries / name, expected)
    checked_alias(root)
    (libraries / ALIAS).symlink_to(ALIAS_TARGET)
    checked_alias(libraries)
    require(sorted(path.name for path in libraries.iterdir()) == sorted([*RUNTIME_FILES, ALIAS]),
            'LLVM stage membership differs')
    receipt = {
        'schema': SCHEMA + '-llvm-runtime-stage', 'status': 'verified',
        'assertion_mode': assertion_mode(), 'helper': bound_file(__file__),
        'source_directory': str(root), 'library_directory': str(libraries),
        'packages': packages, 'content': content, 'content_sha256': content_sha256,
        'unique_target_bytes': sum(row['bytes'] for row in RUNTIME_FILES.values()),
        'dependency_scope': 'Pinned LLVM override libraries only; ordinary host loader/libc, C++ and other system dependencies remain required.',
    }
    # Only a complete, verified stage receives a success receipt.
    write_json(out / 'llvm-runtime-stage.json', receipt)
    return receipt


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument('--output', required=True)
    args = parser.parse_args()
    receipt = stage(args.output)
    print(json.dumps({'status': receipt['status'],
                      'library_directory': receipt['library_directory'],
                      'content_sha256': receipt['content_sha256']}))


if __name__ == '__main__':
    main()
