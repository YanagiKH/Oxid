#!/usr/bin/env python3
"""Replay every unchanged owned-vector source-policy control at its fixed source."""
import argparse
import hashlib
from pathlib import Path
import subprocess
import sys
import types


REVISION = "0e8f23f7481c7ec4e24cd416ed30607f8eb75daf"
ENGINE = "run_record_vector_predecessor_tests.py"
ENGINE_SHA = "6dc9ee36351bdafab40399038133c8547dfb89ac0d0f33076b76b41ef374b75c"
MODULES = {"test_owned_record_vector_compatibility": (
    "0981b5cc24a281f892ee1c8b6d26cbfc3c2a7d0f4a4dfa8c2743714c35723f3b", 21)}
POLICIES = {
    "verify_owned_record_vector_compatibility.py": "16c7affd778deda3d61f9a4e4aebd66c9e01d842c6afe3e9c1a20de57caab6e0",
    "verify_registry_compatibility.py": "d6bd23496f38bde3d89e7e97a2d724b73a0f233cfef99a6dc9fad752bdec9766",
    ENGINE: ENGINE_SHA,
}


def load_engine(repo):
    path = Path(repo) / "scripts" / ENGINE
    if path.is_symlink() or not path.is_file():
        raise ValueError("unsafe pinned predecessor replay engine")
    body = path.read_bytes()
    if hashlib.sha256(body).hexdigest() != ENGINE_SHA:
        raise ValueError("changed pinned predecessor replay engine")
    module = types.ModuleType("_owned_text_owned_vector_predecessor_replay")
    module.__file__ = str(path)
    exec(compile(body, str(path), "exec"), module.__dict__)
    module.REVISION = REVISION
    module.MODULES = MODULES
    module.POLICIES = POLICIES
    return module


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    try:
        return load_engine(args.repo).run(args)
    except (OSError, ValueError, subprocess.TimeoutExpired) as error:
        print(str(error), file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
