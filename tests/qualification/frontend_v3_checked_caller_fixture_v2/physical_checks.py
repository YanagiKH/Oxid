"""Linux fixture reader. No producer execution or qualification entry point."""
from dataclasses import dataclass
import hashlib
import os
from pathlib import Path
import stat
import sys


class PhysicalReject(ValueError):
    pass


def need(ok, reason):
    if not ok:
        raise PhysicalReject(reason)


def supported():
    need(sys.platform == 'linux', 'Linux fixture only')
    for name in ('O_NOFOLLOW', 'O_NONBLOCK', 'O_CLOEXEC', 'O_DIRECTORY'):
        need(hasattr(os, name), 'missing Linux flag: ' + name)
    need({os.open, os.stat, os.rmdir} <= os.supports_dir_fd, 'missing dir_fd operations')
    need(os.listdir in os.supports_fd and os.stat in os.supports_follow_symlinks,
         'missing fd listing or non-following stat')
    need(hasattr(os.stat_result, 'st_mtime_ns') and hasattr(os.stat_result, 'st_ctime_ns'),
         'missing nanosecond stat timestamps')


def flags(directory=False):
    supported()
    return os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK | os.O_CLOEXEC | (os.O_DIRECTORY if directory else 0)


def directory_identity(info):
    need(stat.S_ISDIR(info.st_mode) and not getattr(info, 'st_file_attributes', 0), 'aliased/non-directory')
    return tuple(getattr(info, key) for key in ('st_dev', 'st_ino', 'st_mode', 'st_uid', 'st_gid'))


def file_identity(info):
    need(stat.S_ISREG(info.st_mode) and not getattr(info, 'st_file_attributes', 0), 'aliased/nonregular file')
    need(info.st_nlink == 1, 'file link count differs from one')
    return tuple(getattr(info, key) for key in (
        'st_dev', 'st_ino', 'st_mode', 'st_nlink', 'st_uid', 'st_gid', 'st_size', 'st_mtime_ns', 'st_ctime_ns'))


def canonical(path):
    need(type(path) is str and path.startswith('/'), 'exact absolute path required')
    parts = () if path == '/' else tuple(path[1:].split('/'))
    components(parts)
    return parts


def components(parts):
    need(type(parts) is tuple, 'exact component tuple required')
    alphabet = frozenset('abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789._-')
    need(all(type(p) is str and p and p not in ('.', '..') and set(p) <= alphabet for p in parts),
         'noncanonical components')


@dataclass(frozen=True)
class ExpectedFile:
    permission_mode: int
    bytes: int
    sha256: str


@dataclass(frozen=True)
class RootAnchor:
    path: str
    fd: int
    identity: tuple
    chain: tuple
    directories: tuple = ()
    files: tuple = ()


# Private synchronous instrumentation; never available as a consumer operation.
def _observe(phase, **values):
    pass


def _finish(errors):
    if errors:
        raise BaseExceptionGroup('physical fixture operation failed', errors)


def open_anchor(path):
    parts = canonical(path)
    opened = []
    errors = []
    anchor = None
    try:
        fd = os.open('/', flags(True)); opened.append(fd)
        chain = [('/', directory_identity(os.fstat(fd)))]
        current = ''
        for part in parts:
            before = directory_identity(os.stat(part, dir_fd=fd, follow_symlinks=False))
            fd = os.open(part, flags(True), dir_fd=fd); opened.append(fd)
            need(directory_identity(os.fstat(fd)) == before, 'directory changed during open')
            current += '/' + part
            chain.append((current, before))
        anchor = RootAnchor(path, fd, directory_identity(os.fstat(fd)), tuple(chain))
        check_anchor(anchor)
    except BaseException as error:
        errors.append(error)
    # Keep only the root descriptor after successful construction.
    retained = anchor.fd if not errors else None
    for fd in reversed(opened[:-1] if retained is not None else opened):
        try:
            os.close(fd)
        except BaseException as error:
            errors.append(error)
    if errors and retained is not None:
        try:
            os.close(retained)
        except BaseException as error:
            errors.append(error)
    _finish(errors)
    return anchor


def check_anchor(anchor):
    need(type(anchor) is RootAnchor, 'exact root anchor required')
    need(directory_identity(os.fstat(anchor.fd)) == anchor.identity, 'held root changed')
    for name, identity in anchor.chain:
        need(directory_identity(os.stat(name, follow_symlinks=False)) == identity, 'root/ancestor name changed')


def read_regular(root_anchor, relative_components, expected_file):
    """Return exact bytes from one fd, or preserve all original failures."""
    need(type(expected_file) is ExpectedFile, 'exact expected file required')
    need(type(expected_file.bytes) is int and expected_file.bytes >= 0, 'invalid expected byte count')
    components(relative_components)
    need(bool(relative_components), 'file cannot be root')
    opened, errors, parents = [], [], []
    body = None
    file_fd = None
    before = None
    baseline = None
    parent_fd = root_anchor.fd
    try:
        check_anchor(root_anchor)
        admitted_directories = dict(root_anchor.directories)
        prefix = ()
        for part in relative_components[:-1]:
            prefix += (part,)
            need(prefix in admitted_directories, 'file ancestor was not established')
            identity = directory_identity(os.stat(part, dir_fd=parent_fd, follow_symlinks=False))
            need(identity == admitted_directories[prefix], 'admitted file ancestor changed')
            child = os.open(part, flags(True), dir_fd=parent_fd); opened.append(child)
            need(directory_identity(os.fstat(child)) == identity, 'file ancestor changed during open')
            parents.append((parent_fd, part, child, identity))
            parent_fd = child
        name = relative_components[-1]
        pre = os.stat(name, dir_fd=parent_fd, follow_symlinks=False)
        _observe('pre', info=pre, parent_fd=parent_fd, name=name)
        before = file_identity(pre)
        admitted_files = dict(root_anchor.files)
        need(relative_components in admitted_files, 'file baseline was not established')
        baseline = admitted_files[relative_components]
        need(before == baseline, 'admitted file identity changed')
        need(stat.S_IMODE(pre.st_mode) == expected_file.permission_mode and pre.st_size == expected_file.bytes,
             'file differs from expected mode/size')
        file_fd = os.open(name, flags(), dir_fd=parent_fd); opened.append(file_fd)
        info = os.fstat(file_fd); _observe('fd_before', info=info, fd=file_fd)
        need(file_identity(info) == before == baseline, 'file changed before body read')
        chunks, remaining = [], expected_file.bytes
        while remaining:
            chunk = os.read(file_fd, min(65536, remaining))
            need(type(chunk) is bytes and 0 < len(chunk) <= min(65536, remaining), 'early EOF/invalid read')
            chunks.append(chunk); remaining -= len(chunk)
        need(os.read(file_fd, 1) == b'', 'extra byte')
        body = b''.join(chunks)
        need(hashlib.sha256(body).hexdigest() == expected_file.sha256, 'file digest differs')
    except BaseException as error:
        errors.append(error)
    finally:
        if file_fd is not None:
            try:
                info = os.fstat(file_fd); _observe('fd_after', info=info, fd=file_fd)
                need(file_identity(info) == before == baseline, 'file changed after body read')
            except BaseException as error:
                errors.append(error)
        if before is not None:
            try:
                info = os.stat(relative_components[-1], dir_fd=parent_fd, follow_symlinks=False)
                _observe('post', info=info, parent_fd=parent_fd, name=relative_components[-1])
                need(file_identity(info) == before == baseline, 'original file name changed')
            except BaseException as error:
                errors.append(error)
        for parent, part, child, identity in reversed(parents):
            try:
                need(directory_identity(os.fstat(child)) == identity and
                     directory_identity(os.stat(part, dir_fd=parent, follow_symlinks=False)) == identity,
                     'file ancestor changed')
            except BaseException as error:
                errors.append(error)
        try:
            check_anchor(root_anchor)
        except BaseException as error:
            errors.append(error)
        for fd in reversed(opened):
            try:
                os.close(fd)
            except BaseException as error:
                errors.append(error)
    _finish(errors)
    return body
