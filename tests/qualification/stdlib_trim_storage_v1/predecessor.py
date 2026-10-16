"""Fresh, finite accepted-main view; historical helpers execute at genuine paths.

The view contains all accepted regular non-executable blobs, with the two
unrelated executable entrypoints explicitly excluded. It is not a Git checkout.
The selected source bodies MUST come from the exact two-source inverse.
"""
from contextlib import contextmanager
import json
import os
from pathlib import Path
import stat
import sys
import tempfile
import threading
import types

import generation as g

VIEW_NAME = g.ROOT + 'accepted-view-v1.json'
PREFIX = g.ROOT + 'predecessor-v1/'
EXCLUDED = ('install.sh', 'scripts/verify_repo.py')
_LEASE = threading.Lock()


# Fixed storage relocation, independent of any caller-supplied seal or mapping.
_STRINGS = 'stdlib/strings.ox'
_ARCHIVE = 'tests/qualification/stdlib_trim_storage_v1/archive/stdlib/strings.ox'
_VIEW_PIN = (1023905, 'ddcf5e3c09cda020aca4f1b639d2a66d7357f843f130054800d20228606ad76a')
_STRINGS_ROW = {
    'path': 'stdlib/strings.ox', 'read_from': 'stdlib/strings.ox',
    'bytes': 2345, 'mode': '100644',
    'sha256': '6ecb6b4c96acd0e788bf49db9a480c930d4235e317af8ffc596bbe8868fb65fd',
    'git_blob': '9f81e678fa63bd44b3387db2e1f06e2279b2d72f',
}


def _strings_archive(repo, row):
    g.need(g.encoded(row) == g.encoded(_STRINGS_ROW), 'fixed historical strings row')
    body = g.regular(repo, _ARCHIVE, limit=2345)
    g.pin(body, (2345, _STRINGS_ROW['sha256']), 'fixed historical strings bytes')
    g.need(g.gitid('blob', body) == _STRINGS_ROW['git_blob'], 'fixed historical strings blob')
    return body


def capture(repo, restored, descriptor):
    raw = g.regular(repo, VIEW_NAME)
    g.pin(raw, _VIEW_PIN, 'immutable accepted-view descriptor')
    g.need(_STRINGS not in restored, 'strings cannot be restored source')
    g.need(g.identity(VIEW_NAME, raw) == descriptor, 'fixed accepted-view descriptor')
    value = json.loads(raw)
    g.need(set(value) == {'schema', 'accepted_main', 'excluded_executables', 'files', 'logical_bytes'}
           and value['schema'] == 'oxid-package-json-accepted-view-v1'
           and value['accepted_main'] == g.MAIN and value['excluded_executables'] == list(EXCLUDED),
           'accepted view provenance')
    tree = g.verify_git(json.loads(g.regular(repo, g.ROOT + 'source-proof/accepted-main.json')),
                        g.MAIN, 'refs/heads/main', 3417)
    rows = g.rows_map(value['files'])
    g.need(set(rows) == set(tree) - set(EXCLUDED), 'complete finite accepted view membership')
    bodies = {}
    for name, row in rows.items():
        g.need(set(row) == {'path', 'bytes', 'sha256', 'mode', 'git_blob', 'read_from'}
               and type(row['bytes']) is int and row['mode'] == tree[name]['mode'] == '100644',
               'accepted view fields/mode')
        if name == _STRINGS:
            body = _strings_archive(repo, row)
        elif name in restored:
            g.need(row['read_from'] is None, 'selected body must be recovered by inverse')
            body = restored[name]
        else:
            g.need(row['read_from'] in (name, PREFIX + name), 'unapproved accepted alias')
            g.need(0 <= row['bytes'] <= 16 * 1024 * 1024, 'bounded accepted body size')
            body = g.regular(repo, row['read_from'], limit=row['bytes'])
        g.need(g.identity(name, body) == {k: row[k] for k in ('path', 'bytes', 'sha256')}
               and g.gitid('blob', body) == row['git_blob'] == tree[name]['sha'],
               'accepted view body: ' + name)
        bodies[name] = body
    g.need(sum(map(len, bodies.values())) == value['logical_bytes'], 'accepted view byte count')
    return bodies


def check(root, bodies):
    expected_dirs = {p.as_posix() for name in bodies for p in Path(name).parents if str(p) != '.'}
    found, dirs, pending = set(), set(), ['']
    root_info = root.lstat()
    g.need(stat.S_ISDIR(root_info.st_mode) and not root.is_symlink()
           and not getattr(root_info, 'st_file_attributes', 0) & 0x400, 'accepted view root')
    while pending:
        parent = pending.pop()
        with os.scandir(root / parent) as entries:
            for entry in entries:
                name = parent + '/' + entry.name if parent else entry.name
                info = entry.stat(follow_symlinks=False)
                g.need(not getattr(info, 'st_file_attributes', 0) & 0x400, 'accepted view reparse point')
                if name in expected_dirs:
                    g.need(stat.S_ISDIR(info.st_mode) and name not in dirs, 'accepted view directory')
                    dirs.add(name); pending.append(name)
                else:
                    g.need(name in bodies and name not in found, 'unexpected accepted view member')
                    g.need(g.regular(root, name, limit=len(bodies[name])) == bodies[name], 'changed accepted view member')
                    found.add(name)
    g.need(found == set(bodies) and dirs == expected_dirs, 'accepted view exact closure')


@contextmanager
def view(repo, restored, descriptor):
    g.need(_LEASE.acquire(blocking=False), 'overlapping accepted predecessor view')
    try:
        with _view(repo, restored, descriptor) as root:
            yield root
    finally:
        _LEASE.release()


@contextmanager
def _view(repo, restored, descriptor):
    bodies = capture(repo, restored, descriptor)
    parent = Path(tempfile.gettempdir()).resolve(strict=True)
    with tempfile.TemporaryDirectory(prefix='oxid-package-json-predecessor-', dir=parent) as directory:
        root = Path(directory) / 'repository'; root.mkdir()
        for name, body in bodies.items():
            target = root / name; target.parent.mkdir(parents=True, exist_ok=True)
            with target.open('xb') as stream:
                stream.write(body)
        check(root, bodies)
        root_stamp = (root.stat().st_dev, root.stat().st_ino)
        # Helpers have fixed historical import aliases. Restore all aliases that
        # were actually replaced, before closing the view, even on exceptions.
        modules = dict(sys.modules)
        error = None
        try:
            yield root
        except BaseException as primary:
            error = primary
            raise
        finally:
            for name, module in list(sys.modules.items()):
                filename = getattr(module, '__file__', None)
                if isinstance(filename, str) and Path(filename).absolute().is_relative_to(root):
                    if name in modules:
                        sys.modules[name] = modules[name]
                    else:
                        del sys.modules[name]
            try:
                g.need((root.lstat().st_dev, root.lstat().st_ino) == root_stamp, 'replaced predecessor root')
                check(root, bodies)
            except BaseException as preservation:
                if error is not None:
                    raise BaseExceptionGroup('operation and predecessor preservation failed', [error, preservation]) from None
                raise


def load(root, name, module_name):
    raw = g.regular(root, name)
    module = types.ModuleType(module_name)
    module.__file__ = str(root / name)
    exec(compile(raw, module.__file__, 'exec'), module.__dict__)
    return module
