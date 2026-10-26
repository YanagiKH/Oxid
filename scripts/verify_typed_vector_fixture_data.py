"""Authenticate vector oracle inputs without importing or executing their verifier."""
from __future__ import annotations

import hashlib
import json
from pathlib import Path


MANIFEST = "tests/fixtures/typed_vectors/cases.json"
MANIFEST_SHA256 = "d9cb2529ff4f7ba5735afdc7655e468fee34d7832b03d6549d6980059f684317"
ACCEPTED = (
    "bool", "i8", "i16", "i32", "i64", "i128", "u8", "u16", "u32", "u64",
    "u128", "isize", "usize", "char", "f32", "f64", "unit",
)
REJECTED = (
    "moved_owner", "wrong_element", "owned_element", "unused_unconstrained_generic",
    "bare_vector_type", "wrong_type_arity", "missing_new_type", "vector_is_not_byte_buffer",
    "overlapping_call_loans", "vector_not_copy",
)
SCENARIOS = {"projected-vector": 85, "dynamic-range": 32, "array-ranges": 205}
MAX_INPUT_BYTES = 64 * 1024


def _unique_object(pairs):
    result = {}
    for key, value in pairs:
        if key in result:
            raise RuntimeError("duplicate vector fixture-data key: " + key)
        result[key] = value
    return result


def _read(root: Path, relative: str) -> tuple[Path, bytes]:
    if (not isinstance(relative, str) or not relative or "\\" in relative or ":" in relative
            or any(part in {"", ".", ".."} for part in relative.split("/"))):
        raise RuntimeError("invalid vector fixture-data path: " + repr(relative))
    path = root
    if path.is_symlink():
        raise RuntimeError("symlink in vector fixture-data root")
    for part in relative.split("/"):
        path = path / part
        if path.is_symlink():
            raise RuntimeError("symlink in vector fixture-data path: " + str(path))
    if not path.is_file():
        raise RuntimeError("missing or non-file vector fixture-data input: " + str(path))
    if path.stat().st_size > MAX_INPUT_BYTES:
        raise RuntimeError("vector fixture-data input exceeds size bound: " + str(path))
    with path.open("rb") as stream:
        body = stream.read(MAX_INPUT_BYTES + 1)
    if len(body) > MAX_INPUT_BYTES:
        raise RuntimeError("vector fixture-data input exceeds size bound: " + str(path))
    return path, body


def fixture_data_sources(root: Path) -> set[Path]:
    """Return only the thirty pinned DATA files; adjacent files remain language inputs."""
    try:
        _, body = _read(root, MANIFEST)
        if hashlib.sha256(body).hexdigest() != MANIFEST_SHA256:
            raise RuntimeError("vector fixture-data manifest digest mismatch")
        manifest = json.loads(body, object_pairs_hook=_unique_object)
        if (not isinstance(manifest, dict) or set(manifest) != {"schema", "cases", "scenarios"}
                or type(manifest["schema"]) is not int or manifest["schema"] != 1
                or not isinstance(manifest["cases"], list) or len(manifest["cases"]) != 27
                or not isinstance(manifest["scenarios"], list) or len(manifest["scenarios"]) != 3):
            raise RuntimeError("malformed vector fixture-data manifest")
        expected = {
            **{f"tests/fixtures/typed_vectors/accept/{name}.ox": (name, "accept") for name in ACCEPTED},
            **{f"tests/fixtures/typed_vectors/reject/{name}.ox": (name, "reject") for name in REJECTED},
            **{f"tests/fixtures/typed_vectors/scenarios/{name}.ox": (name, "scenario") for name in SCENARIOS},
        }
        sources = set()
        for rows, scenario in ((manifest["cases"], False), (manifest["scenarios"], True)):
            for row in rows:
                if not isinstance(row, dict) or not isinstance(row.get("source"), str):
                    raise RuntimeError("malformed vector fixture-data row")
                relative = row["source"]
                kind = "scenario" if scenario else row.get("status")
                if relative not in expected or expected[relative] != (row.get("name"), kind):
                    raise RuntimeError("vector fixture-data membership differs")
                fields = {"name", "source", "sha256"}
                fields |= ({"expected_result"} if scenario else {"status", "expected_result"}
                           if kind == "accept" else {"status", "requirement", "expected_diagnostics"})
                if set(row) != fields:
                    raise RuntimeError("malformed vector fixture-data row")
                if kind != "reject" and (type(row["expected_result"]) is not int
                        or row["expected_result"] != (SCENARIOS[row["name"]] if scenario else 42)):
                    raise RuntimeError("vector fixture-data expected result differs")
                digest = row["sha256"]
                if (not isinstance(digest, str) or len(digest) != 64
                        or any(char not in "0123456789abcdef" for char in digest)):
                    raise RuntimeError("malformed vector fixture-data digest")
                path, source = _read(root, relative)
                if path in sources:
                    raise RuntimeError("duplicate vector fixture-data source")
                if hashlib.sha256(source).hexdigest() != digest:
                    raise RuntimeError("vector fixture-data body identity mismatch: " + relative)
                sources.add(path)
        if sources != {root / name for name in expected}:
            raise RuntimeError("vector fixture-data membership differs")
        return sources
    except (OSError, ValueError) as error:
        raise RuntimeError("unreadable vector fixture-data input: " + str(error)) from error
