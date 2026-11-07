#!/usr/bin/env python3
"""Replay the complete unchanged Windows-setup policy successor at its fixed source.

All 35 scalar-library/source-policy tests remain mandatory, including their
SDK and download-bound guards. Historical success does not qualify current
compiler, download, SDK, or Windows runtime behavior.
"""
import argparse
import hashlib
from pathlib import Path
import re
import subprocess
import sys
import types


REVISION = "c693662501c7629c67aabaa2675dbb6b5a76d460"
ENGINE = "run_record_vector_predecessor_tests.py"
ENGINE_SHA = "6dc9ee36351bdafab40399038133c8547dfb89ac0d0f33076b76b41ef374b75c"
MODULES = {
    "test_scalar_library_compatibility": (
        "cb5186db4f64fc82c043e0cded2a642914bc64c2e17993cd79aff57e7a809c73", 35),
}
POLICIES = {
    "verify_scalar_library_compatibility.py":
        "29c1d76604adfeddf2006213922ceb59947f7a193454ffb542df2ba0f7597119",
    "verify_file_seek_compatibility.py":
        "eec5daba34c6380781289a3bb01ed58e44e359a4b03fd23288f950be0ca99e38",
    "run_file_seek_predecessor_tests.py":
        "dadfefe4bca26f008456a5c018b53e3ba5697a22a7618714e0a738aa825614a0",
    ENGINE: ENGINE_SHA,
}
DENIAL_TEST = ("test_provisional_pins_refuse_qualification_before_any_process "
               "(test_scalar_library_compatibility.FrozenFileSeekReplay."
               "test_provisional_pins_refuse_qualification_before_any_process) ... ")
EXPECTED_DENIAL = (DENIAL_TEST + "ValueError: provisional scalar-library pins permit "
                   "prepare-only admission, not qualification\nok\n").encode()
DENIAL_SUCCESS = (DENIAL_TEST + "ok\n").encode()


def load_engine(repo):
    path = Path(repo) / "scripts" / ENGINE
    if path.is_symlink() or not path.is_file():
        raise ValueError("unsafe predecessor replay engine")
    body = path.read_bytes()
    if hashlib.sha256(body).hexdigest() != ENGINE_SHA:
        raise ValueError("changed predecessor replay engine")
    module = types.ModuleType("_windows_download_predecessor_replay")
    module.__file__ = str(path)
    exec(compile(body, str(path), "exec"), module.__dict__)
    module.REVISION = REVISION
    module.MODULES = MODULES
    module.POLICIES = POLICIES
    original_validate = module.validate_execution

    def validate(selected, source, child):
        # This one forced provisional negative is part of the exact source.
        # Join only its unique verbose-test record for validation. The engine
        # retains the original child stdout and stderr bytes before this call.
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
