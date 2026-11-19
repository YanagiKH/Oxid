#!/usr/bin/env python3
"""Replay unchanged registry source-policy controls on their fixed predecessor.

The complete historical assertions remain mandatory. These results do not
qualify the successor compiler or owned-record vector semantics.
"""
import argparse
import hashlib
from pathlib import Path
import re
import subprocess
import sys
import types


REVISION = "e77402de69911a9d45a55fff5a8a0dd8de61f3e5"
ENGINE = "run_record_vector_predecessor_tests.py"
ENGINE_SHA = "6dc9ee36351bdafab40399038133c8547dfb89ac0d0f33076b76b41ef374b75c"
MODULES = {
    "test_registry_compatibility": (
        "f499348d6de5226fb56802b6cb80f7b7d8b58df8bc25e8800792e8c70bb6e02b", 37),
}
POLICIES = {
    "verify_registry_compatibility.py":
        "d6bd23496f38bde3d89e7e97a2d724b73a0f233cfef99a6dc9fad752bdec9766",
    "verify_scalar_library_compatibility.py":
        "29c1d76604adfeddf2006213922ceb59947f7a193454ffb542df2ba0f7597119",
    "replay_scalar_library_source_policy.py":
        "3c7715c35918ebb34b5ace5adcd90b30caf8d915d4e1fd4f1a3d3aa0d3f40ecb",
    ENGINE: ENGINE_SHA,
}
DENIAL_TEST = ("test_provisional_pins_refuse_qualification_before_any_process "
               "(test_registry_compatibility.FrozenScalarLibraryReplay."
               "test_provisional_pins_refuse_qualification_before_any_process) ... ")
EXPECTED_DENIAL = (DENIAL_TEST + "ValueError: provisional private-registry pins permit "
                   "prepare-only admission, not qualification\nok\n").encode()
DENIAL_SUCCESS = (DENIAL_TEST + "ok\n").encode()


def load_engine(repo):
    path = Path(repo) / "scripts" / ENGINE
    if path.is_symlink() or not path.is_file():
        raise ValueError("unsafe predecessor replay engine")
    body = path.read_bytes()
    if hashlib.sha256(body).hexdigest() != ENGINE_SHA:
        raise ValueError("changed predecessor replay engine")
    module = types.ModuleType("_registry_predecessor_replay")
    module.__file__ = str(path)
    exec(compile(body, str(path), "exec"), module.__dict__)
    module.REVISION = REVISION
    module.MODULES = MODULES
    module.POLICIES = POLICIES
    original_validate = module.validate_execution

    def validate(selected, source, child):
        # The forced provisional refusal is one source-pinned negative test.
        # Normalize only its exact, unique verbose-test record for validation;
        # the unchanged engine preserves the original stdout/stderr artifacts.
        if child.stderr.count(EXPECTED_DENIAL) != 1:
            raise ValueError("missing or ambiguous expected predecessor denial")
        normalized = child.stderr.replace(EXPECTED_DENIAL, DENIAL_SUCCESS, 1)
        names = original_validate(selected, source, subprocess.CompletedProcess(
            child.args, child.returncode, child.stdout, normalized))
        for line in normalized.decode("utf-8").splitlines():
            if (not line or line == "-" * 70 or line == "OK"
                    or re.fullmatch(r"Ran " + str(MODULES[selected][1]) + r" tests in .+", line)
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
