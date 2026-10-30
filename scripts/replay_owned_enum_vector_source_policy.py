#!/usr/bin/env python3
"""Replay every unchanged enum-vector source-policy control at its parent."""
import argparse
import hashlib
from pathlib import Path
import subprocess
import sys
import types


REVISION = "698043182916dda7276c363e20a98dfbb5b22ed3"
PINS_PROVISIONAL = True
ENGINE = "run_record_vector_predecessor_tests.py"
ENGINE_SHA = "6dc9ee36351bdafab40399038133c8547dfb89ac0d0f33076b76b41ef374b75c"
MODULES = {"test_owned_enum_vector_compatibility": (
    "6a4bf9d412a9ae94acb7c01ae29633764bd56f39aeb27e117d55a60ed7374a0f", 16)}
POLICIES = {
    "verify_owned_enum_vector_compatibility.py": '3505f8238f1d24754916075fb1ad46aa022c740cd71c9f10a5509656b7e9877c',
    "verify_offline_vendor_compatibility.py": "ee64cc7f63051764594a5866101cff9987ca7ddf20b764e7e1aa88f975c9b7a1",
    "verify_owned_text_compatibility.py": "61bf3bf55b9c1f958fdf48fcc89e747cdf61700109ab979b098851249d9fa109",
    "verify_owned_record_vector_compatibility.py": "16c7affd778deda3d61f9a4e4aebd66c9e01d842c6afe3e9c1a20de57caab6e0",
    "replay_owned_text_source_policy.py": "341673ed92fdc3b2f3269ad0396411a061397110b663d067cc7d92def65b01b6",
    ENGINE: ENGINE_SHA,
}


def load_engine(repo):
    path = Path(repo) / "scripts" / ENGINE
    if path.is_symlink() or not path.is_file():
        raise ValueError("unsafe pinned predecessor replay engine")
    body = path.read_bytes()
    if hashlib.sha256(body).hexdigest() != ENGINE_SHA:
        raise ValueError("changed pinned predecessor replay engine")
    module = types.ModuleType("_generic_vector_enum_predecessor_replay")
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
        if PINS_PROVISIONAL:
            raise ValueError("provisional enum-vector replay pins refuse qualification")
        return load_engine(args.repo).run(args)
    except (OSError, ValueError, subprocess.TimeoutExpired) as error:
        print(str(error), file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
