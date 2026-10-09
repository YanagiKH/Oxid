"""Admit exact prospective source data without treating it as language coverage."""
from __future__ import annotations

import hashlib
import json
import re
from pathlib import Path


# These immutable public manifests bind every package body, including the
# expectations. A new registration requires review; directories are not excluded.
SOURCE_DATA_MANIFESTS = (
    (
        "tests/fixtures/fixed_array_source_unit3/contracts-v2/freeze-manifest.json",
        "45e01b88abfd91a1a30eb80cfa65c033a1b2986f365d05caa44836c8051231bf",
    ),
    (
        "tests/fixtures/fixed_array_source_unit3/element-boundary-supplement-v1/freeze-manifest.json",
        "a8c8454ac11355708cebbd749ade26335ed477e6649da75089184c5b7162fb00",
    ),
    (
        "tests/fixtures/fixed_array_source_unit3/typing-contracts-v1/freeze-manifest.json",
        "2cba1dbd200d75fbeb33b504b08279d89d2baa1784f88718413305446d458c35",
    ),
    (
        "tests/fixtures/fixed_array_source_unit3/lowering-contracts-v1/freeze-manifest.json",
        "b29641a4ce04f32d02a9d8932af029d002cb7870ce6d27666ec4a9d414c4113a",
    ),
    (
        "tests/qualification/bounded_u8_current/source-data-manifest.json",
        "274be1a6024174cdf4805ebc06846f68e2b6187f18b2d4cf10319e5fcf5aeb09",
    ),
)


def _unique_object(pairs: list[tuple[str, object]]) -> dict:
    result = {}
    for key, value in pairs:
        if key in result:
            raise RuntimeError(f"duplicate fixture-data manifest key: {key}")
        result[key] = value
    return result


def _regular_path(root: Path, relative: str) -> Path:
    # Canonical portable paths only, with no symlinked directory or file.
    if (not isinstance(relative, str) or not relative or "\\" in relative or ":" in relative
            or any(part in {"", ".", ".."} for part in relative.split("/"))):
        raise RuntimeError(f"invalid fixture-data path: {relative!r}")
    path = root
    for part in relative.split("/"):
        path = path / part
        if path.is_symlink():
            raise RuntimeError(f"symlink in fixture-data path: {path}")
    if not path.is_file():
        raise RuntimeError(f"missing or non-file fixture-data input: {path}")
    return path


def fixture_data_sources(root: Path) -> set[Path]:
    """Validate all required inputs before returning the exact .ox data inventory."""
    sources = set()
    registered = set()
    for relative, expected_sha256 in SOURCE_DATA_MANIFESTS:
        manifest_path = _regular_path(root, relative)
        if manifest_path in registered:
            raise RuntimeError(f"duplicate fixture-data registration: {relative}")
        registered.add(manifest_path)
        try:
            content = manifest_path.read_bytes()
            if hashlib.sha256(content).hexdigest() != expected_sha256:
                raise RuntimeError(f"fixture-data manifest digest mismatch: {relative}")
            manifest = json.loads(content, object_pairs_hook=_unique_object)
            files = manifest.get("files") if isinstance(manifest, dict) else None
            if not isinstance(files, dict) or not files:
                raise RuntimeError(f"malformed fixture-data manifest: {relative}")
            for name, record in files.items():
                path = _regular_path(manifest_path.parent, name)
                if path in registered:
                    raise RuntimeError(f"duplicate fixture-data registration: {path}")
                registered.add(path)
                if (not isinstance(record, dict) or set(record) != {"bytes", "sha256"}
                        or type(record["bytes"]) is not int or record["bytes"] < 0
                        or not isinstance(record["sha256"], str)
                        or not re.fullmatch(r"[0-9a-f]{64}", record["sha256"])):
                    raise RuntimeError(f"malformed fixture-data identity: {path}")
                body = path.read_bytes()
                if len(body) != record["bytes"] or hashlib.sha256(body).hexdigest() != record["sha256"]:
                    raise RuntimeError(f"fixture-data body identity mismatch: {path}")
                if path.suffix == ".ox":
                    sources.add(path)
        except (OSError, ValueError) as error:
            raise RuntimeError(f"unreadable fixture-data input {relative}: {error}") from error
    return sources
