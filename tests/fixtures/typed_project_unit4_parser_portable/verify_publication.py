#!/usr/bin/env python3
"""Verify the sole admitted documentation projection of portable parser v3."""
from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path, PurePosixPath
import sys

PROJECTION_SHA256 = "c6cb277b8f9d9909a72a9f98d1b0a04e303659bd06311fea1a1dd48ceb369b04"
CHECKPOINT_SHA256 = "8a71652fa28e4c7580112f5e447b8eed7dce56497d1ece1e1b49a2e93dbb2af3"
AUTHORITY_SHA256 = "02b72b3dcf45c695e5c523d71bb1c83e15c082556cf36029829fefc7a71571b0"
ORIGINAL_DESIGN_SHA256 = "c97c554fbe79856a04219614461ecaa3aa568b3f2fde9bbd7bdf91b5d3c8de86"
PUBLISHED_DESIGN_SHA256 = "677905d1b53ce4dd60e28442e2ac7989fcbd5e8a1eaa2ad8376f9007330a80d9"


class Rejected(ValueError):
    pass


def require(condition, message):
    if not condition:
        raise Rejected(message)


def digest(raw):
    return hashlib.sha256(raw).hexdigest()


def read_regular(path):
    path = Path(path).absolute()
    require(not any(p.is_symlink() for p in (path, *path.parents)), "symlink ancestry: " + str(path))
    require(path.is_file(), "missing regular member: " + str(path))
    return path.read_bytes()


def load(raw):
    def pairs(items):
        result = {}
        for key, value in items:
            require(key not in result, "duplicate JSON key: " + key)
            result[key] = value
        return result
    return json.loads(raw, object_pairs_hook=pairs)


def relative(name):
    require(isinstance(name, str) and name and "\\" not in name, "invalid relative member")
    path = PurePosixPath(name)
    require(not path.is_absolute() and str(path) == name and ".." not in path.parts, "unsafe relative member")
    return name


def verify_member(root, row):
    require(set(row) == {"path", "bytes", "sha256"}, "invalid member identity")
    raw = read_regular(root / relative(row["path"]))
    require(type(row["bytes"]) is int and len(raw) == row["bytes"] and digest(raw) == row["sha256"],
            "member identity differs: " + row["path"])
    return raw


def verify(package):
    """Accept one exact published package; no generic Markdown exemption."""
    package = Path(package).absolute()
    frozen = package / "frozen/v3"
    projection_raw = read_regular(package / "publication-projection.json")
    require(digest(projection_raw) == PROJECTION_SHA256, "unapproved documentation projection")
    projection = load(projection_raw)
    checkpoint_raw = read_regular(frozen / "checkpoint.json")
    require(digest(checkpoint_raw) == CHECKPOINT_SHA256, "unapproved original checkpoint")
    require(verify_member(frozen, projection["original_checkpoint"]) == checkpoint_raw, "checkpoint identity")
    checkpoint = load(checkpoint_raw)
    members = checkpoint["files"]
    names = [relative(row["path"]) for row in members]
    require(len(names) == 67 and len(set(names)) == len(names), "original member roster")
    original = projection["original_member"]
    published = projection["published_member"]
    require(projection["projected_member"] == original["path"] == published["path"] == "DESIGN.md", "only DESIGN.md may be projected")
    require(original["sha256"] == ORIGINAL_DESIGN_SHA256 and published["sha256"] == PUBLISHED_DESIGN_SHA256, "documentation pins")
    require(next(row for row in members if row["path"] == "DESIGN.md") == original, "original DESIGN checkpoint binding")
    for row in members:
        verify_member(frozen, published if row["path"] == "DESIGN.md" else row)
    actual = []
    allowed_dirs = {str(parent) for name in names + ["checkpoint.json"] for parent in PurePosixPath(name).parents}
    for path in frozen.rglob("*"):
        require(not path.is_symlink(), "symlink in frozen tree")
        name = path.relative_to(frozen).as_posix()
        if path.is_file():
            actual.append(name)
        else:
            require(path.is_dir() and name in allowed_dirs, "extra directory or special member: " + name)
    require(sorted(actual) == sorted(names + ["checkpoint.json"]), "frozen membership differs")
    projected = read_regular(frozen / "DESIGN.md")
    replacement = projection["replacement"]
    old, new = replacement["old_utf8"].encode(), replacement["new_utf8"].encode()
    require(replacement["occurrences"] == 1 and projected.count(new) == 1 and old not in projected, "exact wording projection")
    restored = projected.replace(new, old)
    require(len(restored) == original["bytes"] and digest(restored) == original["sha256"], "projection does not restore original DESIGN")
    authority_raw = read_regular(frozen / "authority.json")
    require(digest(authority_raw) == AUTHORITY_SHA256, "execution authority differs")
    require("DESIGN.md" not in {row["path"] for row in load(authority_raw)["package_files"]}, "projected member is an execution input")
    verify_member(package, projection["top_level_readme"]["published"])
    return {"status": "pass", "original_checkpoint_sha256": CHECKPOINT_SHA256,
            "projection_sha256": PROJECTION_SHA256, "checkpoint_bound_members": 67,
            "unchanged_checkpoint_bound_members": 66, "projected_member": "DESIGN.md",
            "runtime_authority_sha256": AUTHORITY_SHA256,
            "compiler_builds": 0, "candidate_executions": 0}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--package", type=Path, default=Path(__file__).absolute().parent)
    args = parser.parse_args()
    print(json.dumps(verify(args.package), sort_keys=True, indent=2))


if __name__ == "__main__":
    try:
        main()
    except (Rejected, OSError, ValueError, KeyError, TypeError, StopIteration) as error:
        print(json.dumps({"status": "fail", "error": str(error)}), file=sys.stderr)
        raise SystemExit(1)
