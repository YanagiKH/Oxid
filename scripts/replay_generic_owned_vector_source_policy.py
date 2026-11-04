#!/usr/bin/env python3
"""Replay every unchanged generic-vector source-policy control at its parent."""
import argparse
import hashlib
from pathlib import Path
import subprocess
import sys
import types


REVISION = "1ed9a4b9c373b62d115f31ef9abf1b84debd2e3e"
PINS_PROVISIONAL = False
ENGINE = "run_record_vector_predecessor_tests.py"
ENGINE_SHA = "6dc9ee36351bdafab40399038133c8547dfb89ac0d0f33076b76b41ef374b75c"
MODULES = {"test_generic_owned_vector_compatibility": (
    "916fcbd204b221fabf1286eb48e08149a9b47b2cca40aa10a2d2ee0e69959be2", 16)}
POLICIES = {
    "verify_generic_owned_vector_compatibility.py": "8e80cbdb0b42e01dd4e125d36215dc94ff37422a0e10786216ec02ad66831ad0",
    "replay_owned_enum_vector_source_policy.py": "11a1d669c636c43b9163ed0e86a73785827628b93a83f4608bebfb262e2abbb1",
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
    module = types.ModuleType("_windows_local_package_generic_predecessor_replay")
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
            raise ValueError("provisional generic-vector replay pins refuse qualification")
        return load_engine(args.repo).run(args)
    except (OSError, ValueError, subprocess.TimeoutExpired) as error:
        print(str(error), file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
