"""Authenticate the two Copy record oracle inputs without executing the compiler."""
from __future__ import annotations

import hashlib
import os
from pathlib import Path
import stat

INPUTS = {
    "tests/fixtures/copy_records/all_scalars.ox": (
        1563, "e5fc754a21363f266f86324feed623c53fd2c12e954a294a4f0f53f4188b017e"),
    "tests/fixtures/copy_records/wide_unicode.ox": (
        573, "1178c5b8ef43a75e8527936f76be6ee3b138e2512132f1776dbeea701c1a2b03"),
}


def _no_symlink(path: Path) -> None:
    if any(parent.is_symlink() for parent in (path, *path.parents)):
        raise RuntimeError("symlink in Copy record fixture-data path: " + str(path))


def _metadata(value):
    return (value.st_dev, value.st_ino, value.st_mode, value.st_nlink,
            value.st_size, value.st_mtime_ns, value.st_ctime_ns)


def fixture_data_sources(root: Path) -> set[Path]:
    """Return only the pinned files; adjacent .ox files remain language inputs."""
    sources = set()
    try:
        for relative, (length, digest) in INPUTS.items():
            path = root / relative
            _no_symlink(path)
            before = path.stat()
            if (not stat.S_ISREG(before.st_mode) or before.st_nlink != 1
                    or before.st_mode & 0o111 or before.st_size != length):
                raise RuntimeError("nonregular or changed Copy record fixture-data input: " + relative)
            descriptor = os.open(path, os.O_RDONLY | getattr(os, "O_NOFOLLOW", 0))
            with os.fdopen(descriptor, "rb") as stream:
                if _metadata(os.fstat(stream.fileno())) != _metadata(before):
                    raise RuntimeError("Copy record fixture-data changed before read: " + relative)
                body = stream.read(length + 1)
                after = os.fstat(stream.fileno())
            _no_symlink(path)
            if (_metadata(before) != _metadata(after) or _metadata(path.stat()) != _metadata(before)
                    or len(body) != length or hashlib.sha256(body).hexdigest() != digest):
                raise RuntimeError("Copy record fixture-data body identity mismatch: " + relative)
            sources.add(path)
        return sources
    except OSError as error:
        raise RuntimeError("unreadable Copy record fixture-data input: " + str(error)) from error
