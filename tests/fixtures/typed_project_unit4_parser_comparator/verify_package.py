#!/usr/bin/env python3
"""Verify this source checkpoint; this is not candidate qualification."""
import hashlib
import json
from pathlib import Path

HERE = Path(__file__).resolve().parent
CHECKPOINT_SHA = "78b152c74d9623ae2a66bc12c169bb6c9ed4077657d9bcde0e60978331a5afdc"


def require(condition, message):
    if not condition:
        raise ValueError(message)


def verify(root=HERE):
    root = Path(root).resolve()
    manifest = json.loads((root / "package-manifest.json").read_bytes())
    require(manifest["schema"] == "oxid-unit4-parser-comparator-publication-content-v1", "manifest schema")
    rows = manifest["files"]
    names = [row["path"] for row in rows]
    require(len(names) == len(set(names)), "duplicate package member")
    actual = sorted(p.relative_to(root).as_posix() for p in root.rglob("*") if p.is_file())
    require(actual == sorted(names + ["package-manifest.json"]), "complete package inventory")
    for row in rows:
        relative = Path(row["path"])
        require(not relative.is_absolute() and ".." not in relative.parts, "unsafe package member")
        path = root / relative
        require(not any(part.is_symlink() for part in [path, *path.parents] if part != root.parent), "symlink member")
        raw = path.read_bytes()
        require(len(raw) == row["bytes"] and hashlib.sha256(raw).hexdigest() == row["sha256"], "package identity: " + str(relative))
    frozen = root / "frozen/v8b"
    raw = (frozen / "checkpoint.json").read_bytes()
    require(hashlib.sha256(raw).hexdigest() == CHECKPOINT_SHA, "approved v8b checkpoint")
    checkpoint = json.loads(raw)
    for row in checkpoint["files"]:
        raw = (frozen / row["path"]).read_bytes()
        require(len(raw) == row["bytes"] and hashlib.sha256(raw).hexdigest() == row["sha256"], "frozen member: " + row["path"])
    return {"status": "pass", "package_files": len(names) + 1, "frozen_checkpoint_sha256": CHECKPOINT_SHA,
            "candidate_comparisons": 0, "compiler_invocations": 0}


if __name__ == "__main__":
    print(json.dumps(verify(), sort_keys=True, indent=2))
