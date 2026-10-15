"""Inactive exact published-main predecessor view.

The fixed caller supplies a reviewed view-manifest descriptor and already
admitted source bytes. There is no directory search, environment-selected
authority, live-input fallback, source-tree copy cache, or retained module cache.
The context owns the temporary view; callers must finish every use before exit.
"""
from contextlib import contextmanager
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import stat
import tempfile
import threading

from adapters import need, verify
import source_transition as retained

VIEW_NAME = "tests/qualification/published_formatter_current/predecessor-view-v1.json"
PRESERVED_PREFIX = "tests/qualification/published_formatter_current/predecessor-v1/"
M_HEAD = "15eac633e167c261ba0246ced49cfdc1c6cef434"
M_TREE = "f66d5ae31272268c29f520ec523ee1fa31c15ee9"
SOURCE_NAME = "tests/fixtures/typed_project_source_binding/current-source.json"
SOURCE_SHA = "9432c61fc4f63b760e5f55599aedb24067a206e40e8b44b0911392c00cda7261"
MAX_FILE_BYTES = 2 * 1024 * 1024
MAX_VIEW_BYTES = 38003640
MAX_VIEW_FILES = 681
VIEW_FIELDS = frozenset((
    "schema", "published_main", "selected_source_manifest", "files",
    "directories", "logical_bytes",
))
ROW_FIELDS = frozenset((
    "path", "bytes", "sha256", "mode", "git_blob", "read_from",
))
_VIEW_LEASE = threading.Lock()


def _relative(name):
    retained.relative(name)
    return name


def _identity(body):
    return hashlib.sha1(
        b"blob " + str(len(body)).encode() + b"\0" + body).hexdigest()


def _body_descriptor(row):
    return {key: row[key] for key in ("path", "bytes", "sha256")}


def _read_regular(root, name, limit=MAX_FILE_BYTES):
    name = _relative(name)
    root = Path(root)
    for path in (*reversed(root.parents), root):
        need(not path.is_symlink(), "symlink predecessor source ancestor")
    target = root
    for part in PurePosixPath(name).parts:
        target = target / part
        need(not target.is_symlink(), "symlink predecessor source member")
    before = target.lstat()
    need(stat.S_ISREG(before.st_mode) and before.st_nlink == 1
         and not before.st_mode & 0o111
         and 0 <= before.st_size <= limit,
         "unsafe or oversized predecessor source member")
    flags = (os.O_RDONLY | getattr(os, "O_NOFOLLOW", 0)
             | getattr(os, "O_NOATIME", 0) | getattr(os, "O_NONBLOCK", 0))
    fd = os.open(target, flags)
    try:
        opened = os.fstat(fd)
        need(_stat_identity(opened) == _stat_identity(before),
             "predecessor source changed before read")
        chunks, count = [], 0
        while True:
            chunk = os.read(fd, min(262144, limit + 1 - count))
            if not chunk:
                break
            chunks.append(chunk)
            count += len(chunk)
            need(count <= limit, "predecessor source grew beyond limit")
        after = os.fstat(fd)
        need(_stat_identity(after) == _stat_identity(opened)
             and count == opened.st_size,
             "predecessor source changed during read")
    finally:
        os.close(fd)
    need(_stat_identity(target.lstat()) == _stat_identity(before),
         "predecessor source name changed during read")
    return b"".join(chunks)


def _stat_identity(value):
    return (value.st_dev, value.st_ino, value.st_mode, value.st_nlink, value.st_size,
            value.st_mtime_ns, value.st_ctime_ns)


def _directories(names):
    return sorted({str(parent) for name in names
                   for parent in PurePosixPath(name).parents
                   if str(parent) != "."})


def _old_inputs(admitted):
    need(isinstance(admitted, dict), "explicit admitted source is required")
    # Historical captures contain an older field also called predecessor_inputs.
    # The outer stage must select its exact M capture rather than relabel that
    # older map. Pure transition results use their explicitly named inverse.
    if "published_predecessor" in admitted:
        prior = admitted["published_predecessor"]
        need(isinstance(prior, dict) and isinstance(prior.get("inputs"), dict),
             "missing complete published predecessor capture")
        result = prior["inputs"]
        current = prior["current"]
    else:
        need(admitted.get("source_only") is True
             and admitted.get("execution_qualified") is False,
             "wrong source-only transition stage")
        result = admitted["predecessor_inputs"]
        current = admitted["predecessor"]
    need(isinstance(result, dict) and len(result) == 376
         and all(isinstance(body, bytes) for body in result.values()),
         "wrong complete predecessor input map")
    rows = retained.rows_map(current["files"])
    need(list(rows) == sorted(result), "predecessor source membership changed")
    for name, body in result.items():
        verify(body, rows[name], name)
    return result


def _capture(repo, admitted, seal):
    need(type(seal) is dict and set(seal) == {"path", "bytes", "sha256"}
         and type(seal["path"]) is str and seal["path"] == VIEW_NAME
         and type(seal["bytes"]) is int and 0 < seal["bytes"] <= 512 * 1024
         and type(seal["sha256"]) is str and len(seal["sha256"]) == 64
         and all(c in "0123456789abcdef" for c in seal["sha256"]),
         "missing fixed predecessor-view descriptor")
    raw = _read_regular(repo, VIEW_NAME, 512 * 1024)
    verify(raw, seal, VIEW_NAME)
    value = json.loads(raw)
    need(set(value) == VIEW_FIELDS
         and value["schema"] == "oxid-published-main-predecessor-view-v1"
         and value["published_main"] == {"head": M_HEAD, "tree": M_TREE}
         and value["selected_source_manifest"] ==
         {"path": SOURCE_NAME, "bytes": 74328, "sha256": SOURCE_SHA},
         "wrong published predecessor view provenance")
    rows = value["files"]
    need(isinstance(rows, list) and len(rows) == MAX_VIEW_FILES
         and all(set(row) == ROW_FIELDS for row in rows),
         "wrong finite predecessor view fields or cardinality")
    names = [_relative(row["path"]) for row in rows]
    need(names == sorted(set(names))
         and value["directories"] == _directories(names)
         and len(value["directories"]) == 111
         and value["logical_bytes"] == MAX_VIEW_BYTES,
         "wrong exact predecessor view roster")
    old_inputs = _old_inputs(admitted)
    selected_rows = []
    bodies = {}
    for row in rows:
        name = row["path"]
        need(row["mode"] == "100644"
             and type(row["bytes"]) is int
             and 0 <= row["bytes"] <= MAX_FILE_BYTES,
             "unsafe predecessor row mode or size")
        location = row["read_from"]
        if name in old_inputs:
            need(location is None, "selected predecessor must come from exact inverse")
            body = old_inputs[name]
            selected_rows.append(name)
        else:
            need(location == name or location == PRESERVED_PREFIX + name,
                 "unapproved predecessor alias or source location")
            body = _read_regular(repo, location)
        verify(body, _body_descriptor(row), name)
        need(_identity(body) == row["git_blob"],
             "predecessor body differs from reviewed published Git blob")
        bodies[name] = body
    need(selected_rows == sorted(old_inputs)
         and sum(map(len, bodies.values())) == MAX_VIEW_BYTES,
         "incomplete predecessor source or byte roster")
    need(hashlib.sha256(bodies[SOURCE_NAME]).hexdigest() == SOURCE_SHA,
         "predecessor facade source alias changed")
    need(retained.rows_map(json.loads(bodies[SOURCE_NAME])["files"]) ==
         {name: {"path": name, "bytes": len(body),
                 "sha256": hashlib.sha256(body).hexdigest()}
          for name, body in sorted(old_inputs.items())},
         "predecessor facade does not describe exact recovered source")
    return value, bodies


def _verify_view(root, value, bodies):
    expected_files, expected_dirs = set(bodies), set(value["directories"])
    need(stat.S_ISDIR(root.lstat().st_mode), "unsafe materialized predecessor root")
    files, directories, pending = set(), set(), [""]
    while pending:
        parent = pending.pop()
        need(len(directories) <= 111 and len(files) <= MAX_VIEW_FILES,
             "materialized predecessor traversal exceeded finite roster")
        with os.scandir(root / parent) as entries:
            for entry in entries:
                relative = parent + "/" + entry.name if parent else entry.name
                # Reject an unknown subtree before traversing any part of it.
                need(relative in expected_files or relative in expected_dirs,
                     "unexpected materialized predecessor member")
                info = entry.stat(follow_symlinks=False)
                if relative in expected_dirs:
                    need(stat.S_ISDIR(info.st_mode) and relative not in directories,
                         "unsafe materialized predecessor directory")
                    directories.add(relative)
                    pending.append(relative)
                else:
                    need(stat.S_ISREG(info.st_mode) and info.st_nlink == 1
                         and not info.st_mode & 0o111 and relative not in files,
                         "unsafe materialized predecessor file")
                    files.add(relative)
    need(files == expected_files and directories == expected_dirs,
         "missing or extra materialized predecessor file/directory")
    for row in value["files"]:
        body = _read_regular(root, row["path"])
        verify(body, _body_descriptor(row), row["path"])
        need(body == bodies[row["path"]], "materialized predecessor body changed")


@contextmanager
def predecessor_view(repo, admitted=None, seal=None):
    """Yield one fresh exact M view, and remove it only after use completes.

    The caller must perform current admission first and pass the fixed seal.
    This context returns only a Path. It never returns a loaded module, mutates
    module globals, or retains a view for a later operation.
    """
    need(seal is not None, "fixed predecessor-view seal is required")
    need(_VIEW_LEASE.acquire(blocking=False), "overlapping published predecessor view")
    try:
        value, bodies = _capture(Path(repo), admitted, seal)
        # Host temp aliases (notably /var on macOS) are canonicalized only for
        # our new generated directory. Input-source symlink rules stay strict.
        temporary_parent = Path(tempfile.gettempdir()).resolve(strict=True)
        need(temporary_parent.is_dir()
             and all(not p.is_symlink() for p in (temporary_parent, *temporary_parent.parents)),
             "unsafe canonical predecessor temporary parent")
        with tempfile.TemporaryDirectory(prefix="oxid-published-main-",
                                         dir=temporary_parent) as directory:
            root = Path(directory) / "repository"
            root.mkdir()
            for name, body in sorted(bodies.items()):
                path = root / name
                path.parent.mkdir(parents=True, exist_ok=True)
                with path.open("xb") as stream:
                    stream.write(body)
            _verify_view(root, value, bodies)
            try:
                yield root
            except BaseException as operation_error:
                try:
                    _verify_view(root, value, bodies)
                except BaseException as preservation_error:
                    raise BaseExceptionGroup(
                        "operation and predecessor preservation both failed",
                        [operation_error, preservation_error]) from None
                raise
            else:
                _verify_view(root, value, bodies)
    finally:
        _VIEW_LEASE.release()

