#!/usr/bin/env python3
"""Published-main formatter source dispatcher with a retained execution chain."""
from pathlib import Path as _Path
import hashlib as _hashlib
import os as _os
import stat as _stat
import sys as _sys
import types as _types

_sys.dont_write_bytecode = True
_PACKAGE = _Path(__file__).absolute().parent
_REPOSITORY = _PACKAGE.parents[2]
_HELPER_SPECS = [['adapters', {'path': 'tests/qualification/lexer_reservation_current/adapters.py', 'bytes': 10711, 'sha256': '0520fbf2c651f9b22897a19937c54749b0153ee234bae424dc4132d48e198e2a'}], ['source_transition', {'path': 'tests/qualification/lexer_reservation_current/source_transition.py', 'bytes': 12008, 'sha256': 'e642061c863ac1b836eeaeea72c8fdc1152097be4dc285ee2d071ea3372da9a0'}], ['seal', {'path': 'tests/qualification/lexer_reservation_current/seal.py', 'bytes': 783, 'sha256': 'fa8c298cc5cc65dee984034190ab2e1fe945812e7a873004e14b3d3e242cf878'}], ['current', {'path': 'tests/qualification/lexer_reservation_current/current.py', 'bytes': 16325, 'sha256': 'f15abe15b48cf6a5b7a196ec77c45d8c34c8e0fca6d98dfbf2bb35fcabbcf850'}], ['published_transition', {'path': 'tests/qualification/published_formatter_current/published_transition.py', 'bytes': 12447, 'sha256': '069f7610fe2ffd3e51c47b1d2ed8f6db61f774dccb1dabf6977b24472e973a45'}], ['published_view', {'path': 'tests/qualification/published_formatter_current/published_view.py', 'bytes': 12882, 'sha256': '11a381be0f642138663d46c928b48b434332cb8152cf5006dd452fd178076e9b'}], ['published_current', {'path': 'tests/qualification/published_formatter_current/published_current.py', 'bytes': 32290, 'sha256': '11f96d500198406d29e1a1e0880ade13dd8e2afd799315a08d5febcf5e889393'}], ['generation', {'path': 'tests/qualification/package_json_current_v1/generation.py', 'bytes': 13680, 'sha256': '66ca7aee20a7d17b332ab3c2f8bad72d5882e7a52f07d86ebbd6ccaa85cae9ea'}], ['predecessor', {'path': 'tests/qualification/package_json_current_v1/predecessor.py', 'bytes': 6089, 'sha256': '5de6efa9af91ae2aec153a5496454de0ca6d0fa785c657ba3d12f999564b8eef'}], ['package_current', {'path': 'tests/qualification/package_json_current_v1/package_current.py', 'bytes': 17965, 'sha256': 'f15b58cdb23562e572f4e558f068c5a4dab810ac442a486525db6f0764126355'}]]
_SOURCE_SEAL = {'source_manifest': {'path': 'package-json-source-v1.json', 'bytes': 76328, 'sha256': 'd33ecc4e41975a062790540ba9e9a270d7c6043a31f8f4b9a8d7a044c79530b0'}, 'predecessor_view': {'path': 'tests/qualification/package_json_current_v1/accepted-view-v1.json', 'bytes': 1023374, 'sha256': '9a7d944fa3dcaf4d4764d73f290571ccd8658166e2a19c9c7f6a65d07d73fc47'}}
_RETAINED_PATH = 'tests/fixtures/typed_project_source_binding_byte_storage_v1/run.py'
_RETAINED_PIN = {'bytes': 216279, 'sha256': '584e8ee937cb2d3ef62d59d3eb69a79bfb6afce23df86de3df305f81afae7c76'}


def _opened_identity_matches(before, opened, fields):
    # CPython on Windows can use creation time for lstat's ctime but change
    # time for fstat's ctime. Only this cross-API comparison uses birthtime;
    # same-API checks below retain the complete original metadata stamp.
    left = tuple(getattr(before, name) for name in fields)
    right = tuple(getattr(opened, name) for name in fields)
    if (_os.name == 'nt' and hasattr(before, 'st_birthtime_ns')
            and hasattr(opened, 'st_birthtime_ns')):
        return (left[:-1] == right[:-1]
                and before.st_birthtime_ns == opened.st_birthtime_ns)
    return left == right


def _read_regular(path):
    for parent in (*reversed(path.parents), path):
        if parent.is_symlink():
            raise ValueError('aliased published dispatcher dependency')
    before = path.lstat()
    if (not _stat.S_ISREG(before.st_mode) or before.st_nlink != 1
            or before.st_mode & 0o111 or before.st_size > 2 * 1024 * 1024):
        raise ValueError('unsafe published dispatcher dependency')
    fields = ('st_dev', 'st_ino', 'st_mode', 'st_nlink', 'st_size',
              'st_mtime_ns', 'st_ctime_ns')
    stamp = tuple(getattr(before, name) for name in fields)
    fd = _os.open(path, _os.O_RDONLY | getattr(_os, 'O_NOFOLLOW', 0)
                  | getattr(_os, 'O_NOATIME', 0) | getattr(_os, 'O_NONBLOCK', 0)
                  | getattr(_os, 'O_BINARY', 0))
    try:
        opened = _os.fstat(fd)
        if not _opened_identity_matches(before, opened, fields):
            raise ValueError('published dependency changed before read')
        opened_stamp = tuple(getattr(opened, name) for name in fields)
        chunks, remaining = [], before.st_size
        while remaining:
            piece = _os.read(fd, min(remaining, 65536))
            if not piece:
                raise ValueError('short published dependency read')
            chunks.append(piece)
            remaining -= len(piece)
        if _os.read(fd, 1) != b'':
            raise ValueError('published dependency grew')
        after = _os.fstat(fd)
        if tuple(getattr(after, name) for name in fields) != opened_stamp:
            raise ValueError('published dependency changed during read')
    finally:
        _os.close(fd)
    after_name = path.lstat()
    if tuple(getattr(after_name, name) for name in fields) != stamp:
        raise ValueError('published dependency name changed during read')
    return b''.join(chunks)


_bodies = {}
# Authenticate every executable dependency before executing the first helper.
for _name, _row in _HELPER_SPECS:
    _raw = _read_regular(_REPOSITORY / _row['path'])
    if (len(_raw) != _row['bytes']
            or _hashlib.sha256(_raw).hexdigest() != _row['sha256']):
        raise ValueError('changed pinned published dispatcher helper: ' + _name)
    _bodies[_name] = _raw
_retained_raw = _read_regular(_REPOSITORY / _RETAINED_PATH)
if (len(_retained_raw) != _RETAINED_PIN['bytes']
        or _hashlib.sha256(_retained_raw).hexdigest() != _RETAINED_PIN['sha256']):
    raise ValueError('changed immutable source-binding predecessor runner')

_modules = {}
for _name, _row in _HELPER_SPECS:
    _module = _types.ModuleType(_name)
    _module.__file__ = str(_REPOSITORY / _row['path'])
    _sys.modules[_name] = _module
    exec(compile(_bodies[_name], _module.__file__, 'exec'), _module.__dict__)
    _modules[_name] = _module
_runtime = _types.ModuleType('published_formatter_retained_execution')
_runtime.__file__ = str(_REPOSITORY / _RETAINED_PATH)
exec(compile(_retained_raw, _runtime.__file__, 'exec'), _runtime.__dict__)
_modules['package_current'].install(_runtime, _PACKAGE, _SOURCE_SEAL)
# Preserve the exact public historical API and all retained execution predicates.
globals().update({name: value for name, value in vars(_runtime).items()
                  if not name.startswith('__')})

if __name__ == '__main__':
    raise SystemExit(main())

