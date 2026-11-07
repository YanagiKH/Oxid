#!/usr/bin/env python3
"""Replay the unchanged Copy debug policies on their fixed source.

This preserves their complete source-admission assertions; the result does not
qualify the successor compiler or its Windows native behavior.
"""
import argparse
import hashlib
from pathlib import Path
import re
import subprocess
import sys
import types

REVISION = "9a5ecaaeb5fa50aff2296bf990d63167143cf312"
ENGINE = "run_record_vector_predecessor_tests.py"
ENGINE_SHA = "6dc9ee36351bdafab40399038133c8547dfb89ac0d0f33076b76b41ef374b75c"
MODULES = {
    "test_copy_record_debug_compatibility": (
        "8fb0a586a790371179f98ffdba89090d5e5d092d0a14dd5a91aa951d0130b549", 21),
}
POLICIES = {
    "verify_copy_record_debug_compatibility.py":
        "9d4c5981b6f027d782a4a7e7505cc2cce812c6c689e8fe8bbb592b5745b7f688",
    ENGINE: ENGINE_SHA,
}
DENIAL_TEST = ("test_provisional_pins_refuse_qualification_before_any_process "
               "(test_copy_record_debug_compatibility.FrozenG6Replay."
               "test_provisional_pins_refuse_qualification_before_any_process) ... ")
EXPECTED_DENIAL = (DENIAL_TEST + "ValueError: provisional Copy-record-debug pins permit "
                   "prepare-only admission, not qualification\nok\n").encode()
DENIAL_SUCCESS = (DENIAL_TEST + "ok\n").encode()


def load_engine(repo):
    path = Path(repo) / "scripts" / ENGINE
    if path.is_symlink() or not path.is_file():
        raise ValueError("unsafe predecessor replay engine")
    body = path.read_bytes()
    if hashlib.sha256(body).hexdigest() != ENGINE_SHA:
        raise ValueError("changed predecessor replay engine")
    module = types.ModuleType("_copy_record_debug_predecessor_replay")
    module.__file__ = str(path)
    exec(compile(body, str(path), "exec"), module.__dict__)
    module.REVISION = REVISION
    module.MODULES = MODULES
    module.POLICIES = POLICIES
    original_validate = module.validate_execution

    def validate(selected, source, child):
        # One source-pinned negative test writes its expected refusal between
        # unittest's name and its terminal `ok`. Preserve the raw stream in the
        # engine's artifact, and join only this exact, unique observation.
        if child.stderr.count(EXPECTED_DENIAL) != 1:
            raise ValueError("missing or ambiguous expected predecessor denial")
        normalized = child.stderr.replace(EXPECTED_DENIAL, DENIAL_SUCCESS, 1)
        names = original_validate(selected, source, subprocess.CompletedProcess(
            child.args, child.returncode, child.stdout, normalized))
        for line in normalized.decode("utf-8").splitlines():
            if (not line or line == "-" * 70 or line == "OK"
                    or re.fullmatch(r"Ran 21 tests in .+", line)
                    or re.fullmatch(r"test_\w+ \([\w.]+\) \.\.\. ok", line)):
                continue
            raise ValueError("unrecognized predecessor test output")
        return names

    module.validate_execution = validate
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
