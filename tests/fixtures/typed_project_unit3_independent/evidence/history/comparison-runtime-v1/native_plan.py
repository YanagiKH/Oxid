"""Source-only native plan admission; this module never observes a candidate.

Public interface:
  registry(native_root) -> {key: exact_selected_request}
  key(row) -> (id, profile, group, operation, format, fuel, noclobber)
  request_sha256(request) -> controller-compatible selected-request digest
  validate_rows(rows, material_root, native_root, full) -> selected registry

The caller verifies the plan's materialization FILE_IDENTITY and build bindings.
Here material_root is its parent, containing materialization.json, the original
requests.jsonl, and the one shared sources/ tree. Native component input needs
are exactly NATIVE_INPUTS below. Their identities are copied from the reviewed
minimal-component-v3.json (identity in PIN_PROVENANCE), so no ambient controller,
upstream checkout, toolchain, or mutable external hash table is consulted.

The 300-key roster follows the frozen controller's primary stages. Eight IDs
occur in both native and driver groups: their driver compile/json requests stay
distinct even though the existing portable wrapper selects native-default for
an unqualified compile/json invocation of those IDs. A collector must preserve
the planned group and selected-request identity; this gate never relabels them.

Run with PYTHONOPTIMIZE=0 python3 -B. All refusals are explicit exceptions, not
assert statements. The module reads source request metadata/bytes only, without
reading expected outcomes, diagnostics, observations, receipts, or tool outputs.
"""

from collections import Counter
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import re
import stat
import sys


class NativePlanError(ValueError):
    """A plan or its frozen source-only inputs failed admission."""


if not __debug__ or sys.flags.optimize or os.environ.get("PYTHONOPTIMIZE", "0") not in ("", "0"):
    raise NativePlanError("PROTOCOL_REFUSED: optimized Python is not admitted")


PIN_PROVENANCE = {
    "name": "minimal-component-v3.json",
    "schema": "unit3-portable-native-component-v3",
    "bytes": 5223,
    "sha256": "695e67a3074131d4da30af4cc5c1c17fe893ee7c6d123cedeed777db2427030b",
}
NATIVE_INPUTS = {
    "native-requests.json": {
        "bytes": 26931,
        "sha256": "c9828b3bb5f16d5f03c13475f769cec9726534a25e800777ec842f7feb909984",
    },
    "driver-requests.proposed.json": {
        "bytes": 10646,
        "sha256": "ab2a51daec389f659ee1aaa2ccf8d91b2184d94d712d646e6b6a1137006d20ac",
    },
    "fuel-requests.frozen.json": {
        "bytes": 3629,
        "sha256": "712d69349d7c641feca7a6c24d48c8f4f7ebd05578094594a19831c4c449d13d",
    },
    "no-clobber-requests.json": {
        "bytes": 2142,
        "sha256": "bf2955e64077d12513438ed2cad386287f7ab1f6c5ba6ab13c4e80ad98cc4f68",
    },
}
SHARED_REQUESTS = {
    "bytes": 97143,
    "sha256": "f405bf46d8945027cd1d79d3c125692b4c00b8fe57b97c0d55c68b83b9795113",
}
GROUP_COUNTS = {
    "native-default": 64,
    "native-fuel": 36,
    "reference-fuel": 36,
    "driver": 144,
    "no-clobber": 20,
}
KEY_FIELDS = ("id", "profile", "group", "operation", "format", "fuel", "noclobber")
ROW_FIELDS = frozenset(KEY_FIELDS + (
    "build", "receipt", "wrapper_receipt", "source_root", "entry", "output",
    "input_request_sha256",
))
PROFILES = ("debug", "release")
_SIMPLE_NAME = re.compile(r"[A-Za-z0-9][A-Za-z0-9_.-]*\Z")
_SHA256 = re.compile(r"[0-9a-f]{64}\Z")


def _require(condition, message):
    if not condition:
        raise NativePlanError(message)


def _path(value, label, *, physical_leaf=True):
    _require(isinstance(value, (str, Path)), f"{label}: path must be text or Path")
    text = str(value)
    _require(text and not any(c in text for c in "\0\r\n\\"), f"{label}: invalid path")
    path = Path(text)
    _require(path.is_absolute() and str(path) == text, f"{label}: path must be normalized and absolute")
    _require(all(part not in (".", "..", "") for part in text.split("/")[1:]), f"{label}: noncanonical path")
    try:
        physical = path.resolve() if physical_leaf else path.parent.resolve() / path.name
    except (OSError, RuntimeError) as error:
        raise NativePlanError(f"{label}: cannot resolve physical path") from error
    _require(path == physical, f"{label}: symlinked or aliased path")
    return path


def _root(value, label):
    path = _path(value, label)
    _require(path.is_dir(), f"{label}: directory missing")
    return path


def _relative(value, label):
    _require(isinstance(value, str) and value and not any(c in value for c in "\0\r\n\\"), f"{label}: invalid relative path")
    path = PurePosixPath(value)
    _require(not path.is_absolute() and all(p not in ("", ".", "..") for p in value.split("/")), f"{label}: unsafe relative path")
    _require(str(path) == value, f"{label}: noncanonical relative path")
    return Path(*path.parts)


def _bytes(path, maximum, expected=None):
    path = _path(path, "input")
    try:
        info = path.stat()
        _require(stat.S_ISREG(info.st_mode), f"not a regular input file: {path}")
        _require(info.st_size <= maximum, f"input exceeds byte bound: {path}")
        if expected is not None:
            _require(info.st_size == expected["bytes"], f"input byte size changed: {path}")
        with path.open("rb") as stream:
            data = stream.read(maximum + 1)
    except OSError as error:
        raise NativePlanError(f"cannot read input file: {path}") from error
    _require(len(data) <= maximum and len(data) == info.st_size, f"input changed while reading: {path}")
    if expected is not None:
        _require(len(data) == expected["bytes"] and hashlib.sha256(data).hexdigest() == expected["sha256"], f"input identity changed: {path}")
    return data


def _object(pairs):
    result = {}
    for name, value in pairs:
        _require(name not in result, f"duplicate JSON key: {name}")
        result[name] = value
    return result


def _json(data, label):
    try:
        return json.loads(data, object_pairs_hook=_object)
    except (UnicodeDecodeError, json.JSONDecodeError) as error:
        raise NativePlanError(f"invalid JSON: {label}") from error


def request_sha256(request):
    """Match run.py exactly: default json.dumps spacing and sort_keys=True."""
    return hashlib.sha256(json.dumps(request, sort_keys=True).encode()).hexdigest()


def key(row):
    """Return the complete native invocation identity, including profile."""
    _require(isinstance(row, dict), "native row must be an object")
    _require(all(name in row for name in KEY_FIELDS), "native key field missing")
    for name in ("id", "profile", "group", "operation", "format"):
        _require(isinstance(row[name], str), f"native {name} must be text")
    _require(row["profile"] in PROFILES, "unknown native profile")
    _require(row["group"] in GROUP_COUNTS, "unknown native group")
    _require(row["operation"] in ("check", "run", "compile"), "unknown native operation")
    _require(row["format"] in ("text", "json"), "unknown native format")
    _require(row["fuel"] is None or type(row["fuel"]) is int, "native fuel must be an integer or null")
    _require(row["noclobber"] is None or row["noclobber"] in ("file", "symlink"), "unknown native no-clobber kind")
    return tuple(row[name] for name in KEY_FIELDS)


def registry(native_root):
    """Read four pinned source-only files and enumerate the frozen 300 keys."""
    root = _root(native_root, "native root")
    inputs = {
        name: _json(_bytes(root / name, identity["bytes"], identity), name)
        for name, identity in NATIVE_INPUTS.items()
    }
    native = inputs["native-requests.json"]
    driver = inputs["driver-requests.proposed.json"]
    fuel = inputs["fuel-requests.frozen.json"]
    controls = inputs["no-clobber-requests.json"]["requests"]
    _require((len(native), len(driver), len(fuel), len(controls)) == (32, 12, 4, 10), "frozen native input cardinality changed")
    # This is deliberately the controller's native+driver merge order. For
    # overlapping IDs no-clobber receives the unchanged driver request.
    all_requests = {item["id"]: item for item in native + driver}
    result = {}

    def add(request, profile, group, operation, fmt="json", budget=None, noclobber=None):
        identity = (request["id"], profile, group, operation, fmt, budget, noclobber)
        _require(identity not in result, f"duplicate frozen native key: {identity}")
        result[identity] = request

    for profile in PROFILES:
        for request in native:
            add(request, profile, "native-default", "compile")
        for request in driver:
            for operation in ("check", "run", "compile"):
                for fmt in ("text", "json"):
                    add(request, profile, "driver", operation, fmt)
        for request in fuel:
            for budget in request["fuel_budgets"]:
                add(request, profile, "native-fuel", "compile", budget=budget)
                add(request, profile, "reference-fuel", "run", "text", budget=budget)
        for control in controls:
            kind = "file" if control["output_kind"] == "regular" else "symlink"
            add(all_requests[control["id"]], profile, "no-clobber", "compile", control["format"], noclobber=kind)
    _require(len(result) == 300 and dict(Counter(identity[2] for identity in result)) == GROUP_COUNTS, "frozen native roster cardinality changed")
    return result


def _materialized_requests(material_root):
    root = _root(material_root, "material root")
    material = _json(_bytes(root / "materialization.json", 256 * 1024), "materialization.json")
    _require(isinstance(material, dict), "materialization must be an object")
    _require(material.get("schema") == "oxid-unit3-portable-v1-materialized" and material.get("status") == "materialized", "wrong shared materialization")
    _require(material.get("source_root") == str(root), "materialization root rebound")
    _require(material.get("requests") == {"path": str(root / "requests.jsonl"), **SHARED_REQUESTS}, "materialization request identity rebound")
    raw_requests = _bytes(root / "requests.jsonl", SHARED_REQUESTS["bytes"], SHARED_REQUESTS)
    requests = [_json(line, "requests.jsonl row") for line in raw_requests.splitlines()]
    _require(len(requests) == 152, "shared request roster changed")
    by_id, declared = {}, {}
    for request in requests:
        identifier = request["id"]
        _require(identifier not in by_id, "duplicate shared source ID")
        by_id[identifier] = request
        source_root = _relative(request["source_root"], "request source root")
        _require(str(source_root) == "sources/" + identifier, "shared source root mismatch")
        _relative(request["entry"], "request entry")
        for member in request["source_files"]:
            rel = source_root / _relative(member["path"], "source member")
            name = rel.as_posix()
            _require(name not in declared, "duplicate shared source member")
            declared[name] = {"path": name, "bytes": member["bytes"], "sha256": member["sha256"]}
    _require(len(declared) == 445, "shared source member count changed")
    members = material.get("members")
    _require(isinstance(members, list) and len(members) == 445, "materialization member count changed")
    recorded = {}
    for member in members:
        _require(isinstance(member, dict) and set(member) == {"path", "bytes", "sha256"}, "invalid materialization member fields")
        name = member["path"]
        _require(isinstance(name, str) and name not in recorded, "duplicate/invalid materialization member")
        _require(type(member["bytes"]) is int and isinstance(member["sha256"], str), "invalid materialization member identity")
        recorded[name] = member
    _require(recorded == declared, "shared source membership or identity changed")
    # Bound traversal and reject all source-tree aliases, including symlinked
    # directories and extra empty directories, without reading actual outputs.
    expected_dirs = {"sources"}
    for name in declared:
        expected_dirs.update(str(parent) for parent in PurePosixPath(name).parents if str(parent) != ".")
    pending, actual_files, actual_dirs, entries = [root / "sources"], set(), set(), 0
    while pending:
        directory = pending.pop()
        _require(not directory.is_symlink() and directory.is_dir(), "missing or aliased shared source directory")
        actual_dirs.add(directory.relative_to(root).as_posix())
        try:
            with os.scandir(directory) as children:
                for child in children:
                    entries += 1
                    _require(entries <= 1024, "shared source tree exceeds entry bound")
                    name = Path(child.path).relative_to(root).as_posix()
                    _require(not child.is_symlink(), "symlink in shared source tree")
                    if child.is_dir(follow_symlinks=False):
                        _require(name in expected_dirs, "extra shared source directory")
                        pending.append(Path(child.path))
                    else:
                        _require(child.is_file(follow_symlinks=False) and name in declared, "extra or nonregular shared source member")
                        actual_files.add(name)
        except OSError as error:
            raise NativePlanError("cannot enumerate shared source tree") from error
    _require(actual_files == set(declared) and actual_dirs == expected_dirs, "shared source tree membership changed")
    for name, member in declared.items():
        _bytes(root / name, member["bytes"], member)
    return root, by_id


def validate_rows(rows, material_root, native_root, full):
    """Admit native rows and return their key->exact selected-request mapping.

    Both modes verify the same frozen source inventory; bounded mode permits a
    nonempty subset of the 300 invocation keys, never a different roster. Build
    names are syntax-checked here; the caller binds and checks all used builds.
    No path is created and no receipt/output file contents are opened.
    """
    _require(type(full) is bool, "full must be a boolean")
    _require(isinstance(rows, list) and 0 < len(rows) <= 300, "native rows must be a nonempty list bounded by 300")
    allowed = registry(native_root)
    root, shared = _materialized_requests(material_root)
    selected, targets = {}, set()
    for row in rows:
        _require(isinstance(row, dict) and set(row) == ROW_FIELDS, "native row fields differ from PLAN_SCHEMA")
        identity = key(row)
        _require(identity in allowed, f"native key outside frozen roster: {identity}")
        _require(identity not in selected, f"duplicate native plan key: {identity}")
        _require(isinstance(row["build"], str) and _SIMPLE_NAME.fullmatch(row["build"]), "invalid native build name")
        request = allowed[identity]
        original = shared.get(row["id"])
        _require(original is not None, "native request absent from shared sources")
        for name in ("source_root", "entry", "source_files"):
            _require(request[name] == original[name], "native source request differs from shared materialization")
        source_root = root / _relative(original["source_root"], "source root")
        entry = source_root / _relative(original["entry"], "entry")
        _require(row["source_root"] == str(source_root) and row["entry"] == str(entry), "native source root or entry rebound")
        _require(isinstance(row["input_request_sha256"], str) and _SHA256.fullmatch(row["input_request_sha256"]), "invalid native input request hash")
        _require(row["input_request_sha256"] == request_sha256(request), "native selected request hash changed")
        row_targets = []
        for name in ("receipt", "wrapper_receipt"):
            _require(isinstance(row[name], str), f"{name}: plan path must be text")
            row_targets.append(_path(row[name], name))
        receipt = row_targets[0]
        if row["operation"] == "compile":
            output = receipt.parent / ("occupied-output" if row["noclobber"] else "program")
            _require(row["output"] == str(output), "native compile output path rebound")
            # occupied-output may intentionally be a symlink after collection.
            row_targets.append(_path(row["output"], "output", physical_leaf=row["noclobber"] != "symlink"))
        else:
            _require(row["output"] is None, "native check/run output must be null")
        for target in row_targets:
            _require(target not in targets, "duplicate native receipt/wrapper/output path")
            targets.add(target)
        selected[identity] = request
    if full:
        _require(set(selected) == set(allowed), "full native plan is missing frozen invocation keys")
    return selected
