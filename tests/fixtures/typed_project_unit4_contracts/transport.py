#!/usr/bin/env python3
"""Verify immutable Unit4 authority bytes before exclusive materialization.
This transports source-only contracts; it never runs a compiler or an oracle.
"""
import argparse
import hashlib
import io
import json
import os
from pathlib import Path, PurePosixPath
import re
import stat
import tarfile
import zlib

MAX_MANIFEST_BYTES = 1024 * 1024
MAX_ARCHIVE_BYTES = 8 * 1024 * 1024
MAX_TAR_BYTES = 32 * 1024 * 1024
MAX_MEMBER_BYTES = 8 * 1024 * 1024
MAX_DECODED_BYTES = 128 * 1024 * 1024
MAX_MEMBERS = 512
MAX_NAME_BYTES = 512
SCHEMA = 'oxid-unit4-contract-transport-v1'
TRUSTED_MANIFEST_SHA256 = '631bd4dcc5e3c2e2887debddf8a8657af5247fb985a8d6fcc45cbf78b701c687'
TRUSTED_CONTRACTS = {
    'public': {'logical_path':'/workspace/shared/oxid-reset-recovery-20261003/new-contracts/public-v3/PUBLIC-FREEZE-v3.json', 'sha256':'12b40d321014719805f4864f297ba3598a3c4c5ec6f416de99a0218de68f2a85'},
    'parser': {'logical_path':'/workspace/shared/oxid-reset-recovery-20261003/new-contracts/parser/package-freeze.json', 'sha256':'7a2ec4fdf43bf94f3348a04251a9c04b77610b0e6bdd7318ce62dc2c5a5c4027'},
}

class Reject(ValueError):
    pass

def need(condition, message):
    if not condition:
        raise Reject(message)

def sha(data):
    return hashlib.sha256(data).hexdigest()

def regular(info):
    return stat.S_ISREG(info.st_mode) and not (
        getattr(info, 'st_file_attributes', 0) & getattr(stat, 'FILE_ATTRIBUTE_REPARSE_POINT', 0x400)
    )

def read_bounded(path, limit):
    path = Path(path)
    before = path.lstat()
    need(regular(before), 'input is not a regular file: ' + str(path))
    need(before.st_size <= limit, 'input byte limit: ' + str(path))
    flags = os.O_RDONLY | getattr(os, 'O_BINARY', 0) | getattr(os, 'O_NOFOLLOW', 0)
    fd = os.open(path, flags)
    try:
        opened = os.fstat(fd)
        need(regular(opened) and (opened.st_dev, opened.st_ino) == (before.st_dev, before.st_ino), 'input changed before open')
        chunks = []
        total = 0
        while True:
            chunk = os.read(fd, min(65536, limit + 1 - total))
            if not chunk:
                break
            chunks.append(chunk)
            total += len(chunk)
            need(total <= limit, 'input grew past limit')
        after = os.fstat(fd)
        fields = ('st_dev', 'st_ino', 'st_size', 'st_mtime_ns', 'st_ctime_ns')
        need(all(getattr(opened, x) == getattr(after, x) for x in fields), 'input changed during read')
        need(total == opened.st_size, 'input length changed')
        return b''.join(chunks)
    finally:
        os.close(fd)

def unique_object(pairs):
    result = {}
    for key, value in pairs:
        need(key not in result, 'duplicate JSON key')
        result[key] = value
    return result

def integer(value, limit, name):
    need(type(value) is int and 0 <= value <= limit, 'invalid ' + name)
    return value

def digest(value):
    need(isinstance(value, str) and re.fullmatch('[0-9a-f]{64}', value), 'invalid SHA256')
    return value

def archive_name(value):
    need(isinstance(value, str) and len(value.encode('utf-8')) <= MAX_NAME_BYTES, 'member name length')
    need(value and not value.startswith('/') and not value.endswith('/') and '\\' not in value, 'unsafe member path')
    parts = value.split('/')
    for part in parts:
        need(part not in ('', '.', '..') and re.fullmatch('[A-Za-z0-9_.-]+', part), 'unsafe member component')
        need(not part.endswith('.') and part.split('.')[0].upper() not in {'CON','PRN','AUX','NUL',*(f'COM{i}' for i in range(1,10)),*(f'LPT{i}' for i in range(1,10))}, 'nonportable member component')
    need(PurePosixPath(value).as_posix() == value, 'noncanonical member path')
    return value

def decode_gzip(data, limit):
    need(type(limit) is int and 0 <= limit <= MAX_DECODED_BYTES, 'decoded byte limit')
    try:
        decoder = zlib.decompressobj(31)
        result = decoder.decompress(data, limit + 1)
    except zlib.error as error:
        raise Reject('invalid gzip stream') from error
    need(len(result) <= limit, 'gzip decoded bytes exceeded bound')
    need(decoder.eof and not decoder.unused_data and not decoder.unconsumed_tail, 'truncated, concatenated or trailing gzip data')
    return result

def canonical_header(row):
    info = tarfile.TarInfo(row['archive_path'])
    info.size = row['bytes']
    info.mode = 0o644
    info.uid = info.gid = info.mtime = 0
    info.uname = info.gname = ''
    info.type = tarfile.REGTYPE
    try:
        return info.tobuf(format=tarfile.USTAR_FORMAT)
    except (ValueError, UnicodeError) as error:
        raise Reject('member is not representable as canonical USTAR') from error

def load(package_dir):
    package_dir = Path(package_dir)
    raw = read_bounded(package_dir / 'transport-manifest.json', MAX_MANIFEST_BYTES)
    need(sha(raw) == TRUSTED_MANIFEST_SHA256, 'untrusted or stale transport manifest')
    try:
        manifest = json.loads(raw, object_pairs_hook=unique_object)
    except (UnicodeError, json.JSONDecodeError) as error:
        raise Reject('invalid transport manifest') from error
    need(manifest.get('schema') == SCHEMA, 'transport schema')
    rows = manifest.get('members')
    need(isinstance(rows, list) and 0 < len(rows) <= MAX_MEMBERS, 'member inventory count')
    seen = set()
    logical_seen = set()
    total = 0
    for row in rows:
        need(isinstance(row, dict), 'member schema')
        name = archive_name(row.get('archive_path'))
        logical = row.get('logical_path')
        need(isinstance(logical, str) and logical.startswith('/workspace/') and logical[1:] == name, 'logical path mapping')
        need(name.casefold() not in seen and logical not in logical_seen, 'duplicate member or portable path collision')
        seen.add(name.casefold())
        logical_seen.add(logical)
        total += integer(row.get('bytes'), MAX_MEMBER_BYTES, 'member bytes')
        digest(row.get('sha256'))
        need(total <= MAX_TAR_BYTES, 'aggregate member byte limit')
        if 'decoded' in row:
            need(name.endswith('.gz') and isinstance(row['decoded'], dict), 'decoded member schema')
            integer(row['decoded'].get('bytes'), MAX_DECODED_BYTES, 'nested decoded bytes')
            digest(row['decoded'].get('sha256'))
        need(name.endswith('.gz') == ('decoded' in row), 'gzip decoded identity required')
    spellings = {}
    file_names = {r['archive_path'].casefold() for r in rows}
    for row in rows:
        pieces = row['archive_path'].split('/')
        for i in range(1, len(pieces)):
            prefix = '/'.join(pieces[:i])
            need(prefix.casefold() not in file_names, 'file-directory member collision')
            old = spellings.setdefault(prefix.casefold(), prefix)
            need(old == prefix, 'portable directory-case collision')
    need([r['archive_path'] for r in rows] == sorted(r['archive_path'] for r in rows), 'member order')
    outer = manifest.get('archive', {})
    need(outer.get('path') == 'authorities.tar.gz', 'archive filename')
    integer(outer.get('bytes'), MAX_ARCHIVE_BYTES, 'archive bytes')
    integer(outer.get('decoded_bytes'), MAX_TAR_BYTES, 'tar bytes')
    digest(outer.get('sha256'))
    digest(outer.get('decoded_sha256'))
    integer(outer.get('member_count'), MAX_MEMBERS, 'member count')
    integer(outer.get('member_bytes'), MAX_TAR_BYTES, 'aggregate member bytes')
    need(outer.get('member_count') == len(rows) and outer.get('member_bytes') == total, 'aggregate member inventory')
    blob = read_bounded(package_dir / outer['path'], MAX_ARCHIVE_BYTES)
    need(len(blob) == outer['bytes'] and sha(blob) == outer['sha256'], 'archive compressed identity')
    tar = decode_gzip(blob, outer['decoded_bytes'])
    need(len(tar) == outer['decoded_bytes'] and sha(tar) == outer['decoded_sha256'], 'archive decoded identity')
    contents = {}
    offset = 0
    for row in rows:
        need(tar[offset:offset+512] == canonical_header(row), 'noncanonical member header or inventory')
        offset += 512
        data = tar[offset:offset+row['bytes']]
        need(len(data) == row['bytes'] and sha(data) == row['sha256'], 'member content identity')
        offset += row['bytes']
        padding = (-row['bytes']) % 512
        need(tar[offset:offset+padding] == b'\0' * padding, 'nonzero member padding')
        offset += padding
        if 'decoded' in row:
            decoded = decode_gzip(data, row['decoded']['bytes'])
            need(len(decoded) == row['decoded']['bytes'] and sha(decoded) == row['decoded']['sha256'], 'nested decoded identity')
            del decoded
        contents[row['archive_path']] = data
    tail_size = ((offset + 1024 + 10239) // 10240) * 10240 - offset
    need(len(tar) - offset == tail_size and tar[offset:] == b'\0' * tail_size, 'extra member, trailing data or noncanonical tar ending')
    contracts = manifest.get('active_contracts', {})
    need(contracts == TRUSTED_CONTRACTS, 'untrusted or stale active contract identity')
    row_by_path = {r['logical_path']:r for r in rows}
    for item in contracts.values():
        row = row_by_path.get(item.get('logical_path'))
        need(row is not None and item.get('sha256') == row['sha256'], 'active contract identity')
    return manifest, contents

def safe_destination(destination):
    destination = Path(destination)
    need('..' not in destination.parts, 'destination parent escape')
    destination = Path(os.path.abspath(destination))
    need(destination.name not in ('', '.', '..'), 'destination name')
    for parent in reversed(destination.parents):
        info = parent.lstat()
        need(stat.S_ISDIR(info.st_mode) and not stat.S_ISLNK(info.st_mode) and not (getattr(info,'st_file_attributes',0) & getattr(stat,'FILE_ATTRIBUTE_REPARSE_POINT',0x400)), 'destination ancestry must be real directories')
    need(not os.path.lexists(destination), 'destination already exists')
    return destination

def verify_materialized(manifest, destination):
    destination = Path(destination)
    info = destination.lstat()
    need(stat.S_ISDIR(info.st_mode) and not stat.S_ISLNK(info.st_mode) and not (getattr(info,'st_file_attributes',0) & getattr(stat,'FILE_ATTRIBUTE_REPARSE_POINT',0x400)), 'materialized root kind')
    expected = {r['archive_path']:r for r in manifest['members']}
    expected_directories = {str(PurePosixPath(name).parent) for name in expected}
    expected_directories |= {str(parent) for name in expected for parent in PurePosixPath(name).parents if str(parent) != '.'}
    seen = set()
    for parent, directories, files in os.walk(destination, followlinks=False):
        for name in directories:
            p = Path(parent)/name
            need(p.relative_to(destination).as_posix() in expected_directories, 'extra materialized directory')
            info = p.lstat()
            need(stat.S_ISDIR(info.st_mode) and not stat.S_ISLNK(info.st_mode) and not (getattr(info,'st_file_attributes',0) & getattr(stat,'FILE_ATTRIBUTE_REPARSE_POINT',0x400)), 'materialized directory kind')
        for name in files:
            p = Path(parent)/name
            rel = p.relative_to(destination).as_posix()
            need(rel in expected and rel not in seen, 'extra materialized file')
            seen.add(rel)
            row = expected[rel]
            data = read_bounded(p, MAX_MEMBER_BYTES)
            need(len(data) == row['bytes'] and sha(data) == row['sha256'], 'materialized content identity')
    need(seen == set(expected), 'missing materialized file')

def materialize(package_dir, destination):
    # Every compressed/header/member/decoded identity is checked before mkdir.
    manifest, contents = load(package_dir)
    destination = safe_destination(destination)
    destination.mkdir(mode=0o700)
    made = {destination}
    for row in manifest['members']:
        path = destination / row['archive_path']
        current = destination
        for part in PurePosixPath(row['archive_path']).parts[:-1]:
            current = current / part
            if current not in made:
                current.mkdir(mode=0o700, exist_ok=False)
                made.add(current)
        flags = os.O_WRONLY | os.O_CREAT | os.O_EXCL | getattr(os,'O_BINARY',0) | getattr(os,'O_NOFOLLOW',0)
        fd = os.open(path, flags, 0o600)
        try:
            data = contents[row['archive_path']]
            position = 0
            while position < len(data):
                written = os.write(fd, data[position:position+65536])
                need(written > 0, 'short materialization write')
                position += written
            os.fsync(fd)
        finally:
            os.close(fd)
    verify_materialized(manifest, destination)
    return manifest

def resolve_original_path(manifest, destination, logical_path):
    # Call after load and verify_materialized; never fall back to original host paths.
    rows = {row['logical_path']:row for row in manifest['members']}
    need(logical_path in rows, 'unlisted logical authority path')
    return Path(destination) / rows[logical_path]['archive_path']

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('operation', choices=['verify','extract','resolve'])
    parser.add_argument('--package', type=Path, default=Path(__file__).resolve().parent)
    parser.add_argument('--destination', type=Path)
    parser.add_argument('--logical-path')
    args = parser.parse_args()
    try:
        if args.operation == 'extract':
            need(args.destination is not None, 'extract needs destination')
            manifest = materialize(args.package, args.destination)
        else:
            manifest, _ = load(args.package)
        if args.operation == 'resolve':
            need(args.destination is not None and args.logical_path is not None, 'resolve needs destination and logical path')
            verify_materialized(manifest, args.destination)
            print(resolve_original_path(manifest, args.destination, args.logical_path))
        else:
            print(json.dumps({'status':'VERIFIED','operation':args.operation,'members':len(manifest['members']),'archive_sha256':manifest['archive']['sha256'],'active_contracts':manifest['active_contracts']},sort_keys=True))
        return 0
    except (Reject, OSError, KeyError, TypeError) as error:
        parser.exit(1, 'transport rejected: ' + str(error) + '\n')

if __name__ == '__main__':
    raise SystemExit(main())
