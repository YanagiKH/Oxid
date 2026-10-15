"""Pinned hosted successor entry; retained v1 controls run in their genuine view."""
from pathlib import Path
import hashlib
import os
import stat
import sys
import types
sys.dont_write_bytecode = True
BODY = {'path': 'tests/qualification/package_json_current_v1/hosted_controls.py', 'bytes': 184125, 'sha256': '50b9dbe7ea427964607fcb6450bb9033989e45e6a81f490c4b3099934ad05486'}


def load_tests(loader, tests, pattern):
    root = Path(__file__).absolute().parents[1]
    path = root / BODY['path']
    for parent in (path, *path.parents):
        if parent.is_symlink():
            raise ValueError('aliased hosted successor entry')
    before = path.lstat()
    if not stat.S_ISREG(before.st_mode) or before.st_nlink != 1 or before.st_mode & 0o111:
        raise ValueError('unsafe hosted successor entry')
    fields = ('st_dev','st_ino','st_mode','st_nlink','st_size','st_mtime_ns','st_ctime_ns')
    stamp = lambda value: tuple(getattr(value,field) for field in fields)
    fd = os.open(path,os.O_RDONLY | getattr(os,'O_NOFOLLOW',0) | getattr(os,'O_NONBLOCK',0) | getattr(os,'O_BINARY',0))
    try:
        opened = os.fstat(fd)
        left, right = stamp(before), stamp(opened)
        matches = left == right
        if os.name == 'nt' and hasattr(before,'st_birthtime_ns') and hasattr(opened,'st_birthtime_ns'):
            matches = left[:-1] == right[:-1] and before.st_birthtime_ns == opened.st_birthtime_ns
        if not matches:
            raise ValueError('hosted successor entry changed before read')
        with os.fdopen(fd,'rb',closefd=False) as stream:
            raw = stream.read(BODY['bytes']+1)
        if stamp(os.fstat(fd)) != stamp(opened):
            raise ValueError('hosted successor entry changed during read')
    finally:
        os.close(fd)
    if stamp(path.lstat()) != stamp(before) or len(raw)!=BODY['bytes'] or hashlib.sha256(raw).hexdigest()!=BODY['sha256']:
        raise ValueError('unapproved complete hosted successor body')
    module = types.ModuleType('package_json_hosted_controls')
    module.__file__ = str(path)
    exec(compile(raw,str(path),'exec'),module.__dict__)
    return module.load_tests(loader,tests,pattern)
