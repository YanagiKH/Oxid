#!/usr/bin/env python3
"""Preserve Unit3 bytes even when a failed/cancelled collector is still unwinding.

Only a stable, successful qualification gets a complete index. Files are copied
one at a time (1 MiB read buffer, at most one file of temporary disk space), then
indexed and archived from that same snapshot. No compiler inputs are changed.
"""
import contextlib
import errno
import hashlib
import io
import json
import os
from pathlib import Path
import stat
import tarfile
import tempfile

INDEX = 'ci-evidence-index.json'
ARCHIVE = 'typed-project-unit3-evidence.tar.gz'
PACKAGE_INPUTS = ('package-manifest.json', 'manifests/selected-current.json',
                  'manifests/core-v1.json')
CHUNK = 1024 * 1024


class IncompleteEvidence(RuntimeError):
    """A successful qualification did not leave stable, complete evidence."""

    def __init__(self, document):
        super().__init__('partial Unit3 evidence preserved; successful qualification requires stable complete evidence')
        self.document = document


class ChangedPath(RuntimeError):
    pass


def identity(value):
    return (value.st_dev, value.st_ino, value.st_mode, value.st_size,
            value.st_mtime_ns, value.st_ctime_ns)


def source_call(operation, *args, **kwargs):
    try:
        return operation(*args, **kwargs)
    except (FileNotFoundError, NotADirectoryError) as error:
        raise ChangedPath(f'{type(error).__name__}: {error}') from error


def source_open(name, flags, parent=None):
    try:
        return os.open(name, flags | os.O_NOFOLLOW, dir_fd=parent)
    except OSError as error:
        if error.errno in (errno.ENOENT, errno.ENOTDIR, errno.ELOOP):
            raise ChangedPath(f'path unavailable: {error}') from error
        raise


def directory_open(name, parent=None):
    return source_open(name, os.O_RDONLY | os.O_DIRECTORY, parent)


@contextlib.contextmanager
def parent_directory(root_fd, name):
    """Anchor every component; never follow a racing ancestor symlink."""
    fd = os.dup(root_fd)
    try:
        parts = Path(name).parts
        for part in parts[:-1]:
            child = directory_open(part, fd)
            os.close(fd)
            fd = child
        yield fd, parts[-1]
    finally:
        os.close(fd)


def add_issue(issues, name, phase, error, scope='evidence'):
    # Do not catch permission, I/O, disk-space, archive, or programming errors.
    issues.append({'path': name, 'scope': scope, 'phase': phase,
                   'reason': type(error).__name__, 'detail': str(error)})


TRANSIENT = (FileNotFoundError, NotADirectoryError, ChangedPath)


def inventory(root_fd, issues, observe):
    result = {}

    def walk(fd, prefix):
        with os.scandir(fd) as children:
            names = sorted(entry.name for entry in children)
        for leaf in names:
            name = prefix + leaf
            if name == INDEX:
                continue  # Generated metadata is added exactly once, last.
            try:
                observe('stat', name)
                value = os.stat(leaf, dir_fd=fd, follow_symlinks=False)
                if stat.S_ISDIR(value.st_mode):
                    child = directory_open(leaf, fd)
                    try:
                        opened = os.fstat(child)
                        if (value.st_dev, value.st_ino) != (opened.st_dev, opened.st_ino):
                            raise ChangedPath('directory changed during inventory')
                        walk(child, name + '/')
                    finally:
                        os.close(child)
                elif stat.S_ISREG(value.st_mode) or stat.S_ISLNK(value.st_mode):
                    cache = 'target' in Path(name).parts or 'source-target' in Path(name).parts
                    if cache:
                        if not stat.S_ISREG(value.st_mode) or not leaf.startswith('oxid-'):
                            continue
                        descriptor = source_open(leaf, os.O_RDONLY | os.O_NONBLOCK, fd)
                        with os.fdopen(descriptor, 'rb') as source:
                            if identity(os.fstat(source.fileno())) != identity(value):
                                raise ChangedPath('cache candidate changed while opening')
                            magic = source.read(4)
                            if identity(os.fstat(source.fileno())) != identity(value):
                                raise ChangedPath('cache candidate changed during selection')
                        if magic != b'\x7fELF':
                            continue
                    result[name] = value
            except TRANSIENT as error:
                add_issue(issues, name, 'inventory', error)

    walk(root_fd, '')
    return result


@contextlib.contextmanager
def capture(root_fd, name, expected, observe):
    """Read at most the observed size, so a growing log cannot loop forever."""
    with parent_directory(root_fd, name) as (parent, leaf):
        before = source_call(os.stat, leaf, dir_fd=parent, follow_symlinks=False)
        if identity(before) != identity(expected):
            raise ChangedPath('path changed before capture')
        if stat.S_ISLNK(before.st_mode):
            target = source_call(os.readlink, leaf, dir_fd=parent)
            after = source_call(os.stat, leaf, dir_fd=parent, follow_symlinks=False)
            if identity(before) != identity(after):
                raise ChangedPath('symlink changed during capture')
            yield {'path': name, 'type': 'symlink', 'target': target}, before, None
            return
        descriptor = source_open(leaf, os.O_RDONLY | os.O_NONBLOCK, parent)
        with os.fdopen(descriptor, 'rb') as source, tempfile.TemporaryFile() as snapshot:
            if identity(os.fstat(source.fileno())) != identity(before):
                raise ChangedPath('file changed while opening')
            observe('read', name)
            digest = hashlib.sha256()
            remaining = before.st_size
            while remaining:
                chunk = source.read(min(CHUNK, remaining))
                if not chunk:
                    break
                snapshot.write(chunk)
                digest.update(chunk)
                remaining -= len(chunk)
            observe('after-read', name)
            if remaining or identity(os.fstat(source.fileno())) != identity(before):
                raise ChangedPath('file changed during read')
            snapshot.seek(0)
            yield {'path': name, 'bytes': before.st_size, 'sha256': digest.hexdigest()}, before, snapshot


def tar_info(name, value, row):
    info = tarfile.TarInfo(name)
    info.mode = stat.S_IMODE(value.st_mode)
    info.uid, info.gid, info.mtime = value.st_uid, value.st_gid, value.st_mtime
    if row.get('type') == 'symlink':
        info.type, info.linkname = tarfile.SYMTYPE, row['target']
    else:
        info.size = row['bytes']
    return info


def preserve(root, package, outcome, *, _observe=lambda phase, name: None):
    """Create archive/index; return partial evidence only for failure/cancellation.

    _observe is a deterministic filesystem-fault seam, unavailable through CLI.
    Partial output is retained before raising for any other qualification outcome.
    """
    issues = []
    if not root.exists():
        add_issue(issues, '.', 'inventory', FileNotFoundError('evidence root missing'))
    root.mkdir(parents=True, exist_ok=True)
    root_fd = directory_open(root)
    try:
        root_identity = os.fstat(root_fd)
        selected = inventory(root_fd, issues, _observe)
        entries, inputs = [], {}
        archive_path = root.parent / ARCHIVE
        checksum_path = root.parent / 'typed-project-unit3-evidence.sha256'
        if archive_path.exists() or checksum_path.exists() or (root / INDEX).exists():
            raise FileExistsError('refusing to replace previously preserved evidence')
        with tempfile.TemporaryDirectory(prefix='unit3-archive-', dir=root.parent) as temporary:
            staged_archive = Path(temporary) / ARCHIVE
            with tarfile.open(staged_archive, 'w:gz', compresslevel=6, dereference=False) as archive:
                for name, value in sorted(selected.items()):
                    with contextlib.ExitStack() as stack:
                        try:
                            row, before, snapshot = stack.enter_context(
                                capture(root_fd, name, value, _observe))
                        except ChangedPath as error:
                            add_issue(issues, name, 'capture', error)
                            continue
                        _observe('archive', name)
                        # Archive errors must propagate, including FileNotFoundError.
                        # addfile reads only the private snapshot, never the mutable source.
                        archive.addfile(tar_info(name, before, row), snapshot)
                        entries.append(row)

                # Keep the original package identity fields, with explicit missing/unstable inputs.
                try:
                    package_fd = directory_open(package)
                except TRANSIENT as error:
                    add_issue(issues, '.', 'package-inputs', error, 'package_inputs')
                else:
                    try:
                        for name in PACKAGE_INPUTS:
                            try:
                                with parent_directory(package_fd, name) as (parent, leaf):
                                    value = source_call(os.stat, leaf, dir_fd=parent, follow_symlinks=False)
                                if not stat.S_ISREG(value.st_mode):
                                    raise ChangedPath('package identity is not a regular file')
                                with capture(package_fd, name, value, _observe) as (row, _, snapshot):
                                    inputs[name] = {key: row[key] for key in ('bytes', 'sha256')}
                                with parent_directory(package_fd, name) as (parent, leaf):
                                    if identity(source_call(os.stat, leaf, dir_fd=parent, follow_symlinks=False)) != identity(value):
                                        raise ChangedPath('package identity changed after capture')
                            except ChangedPath as error:
                                inputs.pop(name, None)
                                add_issue(issues, name, 'package-inputs', error, 'package_inputs')
                    finally:
                        os.close(package_fd)

                _observe('verify', '.')
                final = inventory(root_fd, issues, lambda phase, name: None)
                for name in sorted(selected.keys() | final.keys()):
                    if name not in selected:
                        add_issue(issues, name, 'verify', ChangedPath('path appeared after inventory'))
                    elif name not in final:
                        add_issue(issues, name, 'verify', FileNotFoundError('path disappeared after inventory'))
                    elif identity(selected[name]) != identity(final[name]):
                        add_issue(issues, name, 'verify', ChangedPath('path changed after inventory'))
                try:
                    current_root = root.lstat()
                    if (current_root.st_dev, current_root.st_ino) != (root_identity.st_dev, root_identity.st_ino):
                        raise ChangedPath('evidence root replaced during preservation')
                except TRANSIENT as error:
                    add_issue(issues, '.', 'verify', error)
                complete = outcome == 'success' and not issues
                document = {
                    'schema': 1, 'qualification_step_outcome': outcome,
                    'archive_status': 'complete' if complete else 'partial',
                    'archive_scope': 'Selected evidence observed during preservation; the index itself is the final unindexed member',
                    'partial_reasons': ([] if outcome == 'success' else ['qualification did not succeed'])
                                       + ([] if not issues else ['missing or unstable evidence paths']),
                    'preservation_issues': issues,
                    'semantic_receipt_label': 'archived core-v1 reconstructed by pinned inverse current-source changes, then the unchanged platform bridge',
                    'current_source_binding_receipt': 'source-binding/prepared.json',
                    'direct_current_semantic_execution': False,
                    'direct_successor_semantic_execution': False,
                    'package_inputs': inputs,
                    'expected_full_plan_rows': {'source': 304, 'native': 300, 'mutation': 220},
                    'driver_attributions': 'eight rows joined from the full native comparison; zero extra executions',
                    'retained_bytes': sum(row.get('bytes', 0) for row in entries),
                    'excluded': 'regenerable Rust target caches; exact oxid test executables are retained',
                    'files': entries,
                }
                payload = (json.dumps(document, indent=2) + '\n').encode()
                info = tarfile.TarInfo(INDEX)
                info.size, info.mode = len(payload), 0o644
                archive.addfile(info, io.BytesIO(payload))
            digest = hashlib.sha256()
            with staged_archive.open('rb') as source:
                while chunk := source.read(CHUNK):
                    digest.update(chunk)
            # No half-written tar is exposed as an uploadable artifact.
            os.link(staged_archive, archive_path)
            with checksum_path.open('x') as output:
                output.write(digest.hexdigest() + '  ' + archive_path.name + '\n')
            descriptor = os.open(INDEX, os.O_WRONLY | os.O_CREAT | os.O_EXCL | os.O_NOFOLLOW,
                                 0o644, dir_fd=root_fd)
            with os.fdopen(descriptor, 'wb') as output:
                output.write(payload)
    finally:
        os.close(root_fd)
    if not complete and outcome not in ('failure', 'cancelled'):
        raise IncompleteEvidence(document)
    return document


def emit_summary(document):
    print(json.dumps({'archive': ARCHIVE, 'archive_status': document['archive_status'],
                      'qualification_step_outcome': document['qualification_step_outcome'],
                      'files': len(document['files']),
                      'retained_bytes': document['retained_bytes'],
                      'preservation_issues': len(document['preservation_issues'])}), flush=True)


def main():
    root = Path(os.environ['RUNNER_TEMP']) / 'typed-project-unit3'
    try:
        document = preserve(root, Path('tests/fixtures/typed_project_unit3_independent'),
                            os.environ.get('UNIT3_STEP_OUTCOME', 'unknown'))
    except IncompleteEvidence as error:
        emit_summary(error.document)
        raise
    emit_summary(document)


if __name__ == '__main__':
    main()
