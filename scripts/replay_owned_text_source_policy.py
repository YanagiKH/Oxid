#!/usr/bin/env python3
"""Replay all unchanged owned-text source-policy controls at their fixed source."""
import argparse
import hashlib
from pathlib import Path
import subprocess
import sys
import types


REVISION = "7279208226a3d4185c0b996491332519971e916e"
ENGINE = "run_record_vector_predecessor_tests.py"
ENGINE_SHA = "6dc9ee36351bdafab40399038133c8547dfb89ac0d0f33076b76b41ef374b75c"
MODULES = {"test_owned_text_compatibility": (
    "1fe80493856cf720c76525321ca7530f54c782282b95793d29ef47a889946ebc", 14)}
POLICIES = {
    "verify_owned_text_compatibility.py": "61bf3bf55b9c1f958fdf48fcc89e747cdf61700109ab979b098851249d9fa109",
    "verify_owned_record_vector_compatibility.py": "16c7affd778deda3d61f9a4e4aebd66c9e01d842c6afe3e9c1a20de57caab6e0",
    "replay_owned_record_vector_source_policy.py": "97bd65e326f23f57aba448fe1775a22b50b87f579ef22509930ceccb375b9826",
    ENGINE: ENGINE_SHA,
}


def load_engine(repo):
    path = Path(repo) / "scripts" / ENGINE
    if path.is_symlink() or not path.is_file():
        raise ValueError("unsafe pinned predecessor replay engine")
    body = path.read_bytes()
    if hashlib.sha256(body).hexdigest() != ENGINE_SHA:
        raise ValueError("changed pinned predecessor replay engine")
    module = types.ModuleType("_offline_vendor_owned_text_predecessor_replay")
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
