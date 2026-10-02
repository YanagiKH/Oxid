#!/usr/bin/env python3
"""Fast regression controls for the independent Unit1 libtest runner.

Place this file beside run_reviewer.py or in the repository's scripts/ directory.
Run with Python's unittest runner or execute this file directly. For another
layout, set OXID_UNIT1_PACKAGE to the package directory. No Cargo build, compiler
fixture generation, network access, or deliberate timeout is involved.
"""

import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import textwrap
import unittest


FAKE_LIBTEST = textwrap.dedent(r'''\
#!/usr/bin/env python3
import json
import os
from pathlib import Path
import sys

names = json.loads((Path(__file__).parent / "roster.json").read_text())
mode = os.environ["OXID_UNIT1_PROTOCOL_CASE"]
if "--list" in sys.argv:
    if mode == "list-error":
        print("deliberate listing failure", file=sys.stderr)
        raise SystemExit(9)
    if mode == "zero-list":
        names = []
    elif mode == "duplicate-list":
        names[-1] = names[0]
    elif mode == "wrong-list":
        names[-1] = "frontend::project::reviewer_unit1::unexpected_replacement"
    for name in names:
        print(name + ": test")
    print(f"\n{len(names)} tests, 0 benchmarks")
    raise SystemExit(0)

test = sys.argv[1]
first = test == names[0]
if mode == "test-error" and first:
    print("deliberate test failure", file=sys.stderr)
    raise SystemExit(11)
if mode == "zero-run" and first:
    print("running 0 tests\n\ntest result: ok. 0 passed; 0 failed; "
          "0 ignored; 0 measured; 69 filtered out; finished in 0.00s")
    raise SystemExit(0)
if mode == "ignored" and first:
    print(f"running 1 test\ntest {test} ... ignored\n\n"
          "test result: ok. 0 passed; 0 failed; 1 ignored; "
          "0 measured; 68 filtered out; finished in 0.00s")
    raise SystemExit(0)
if mode == "wrong-executed" and first:
    test = "frontend::project::reviewer_unit1::wrong_executed_name"
print(f"running 1 test\ntest {test} ... ok\n\n"
      "test result: ok. 1 passed; 0 failed; 0 ignored; "
      "0 measured; 68 filtered out; finished in 0.00s")
''').removeprefix("\\\n")


def package_directory():
    override = os.environ.get("OXID_UNIT1_PACKAGE")
    if override:
        candidates = [Path(override)]
    else:
        here = Path(__file__).resolve()
        candidates = [
            here.parent,
            here.parents[1] / "tests/fixtures/typed_project_unit1_independent",
        ]
    for candidate in candidates:
        if all((candidate / name).is_file() for name in (
            "run_reviewer.py", "expected-test-roster.json"
        )):
            return candidate.resolve()
    raise RuntimeError(
        "Unit1 package not found; set OXID_UNIT1_PACKAGE to its directory"
    )


@unittest.skipUnless(os.name == "posix", "Fake libtest uses a POSIX executable script")
class Unit1RunnerProtocolTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.package = package_directory()
        cls.roster = json.loads(
            (cls.package / "expected-test-roster.json").read_text()
        )["tests"]
        if len(cls.roster) != 69 or len(set(cls.roster)) != 69:
            raise RuntimeError("The package must contain its exact 69-test roster")

    def run_case(self, mode):
        with tempfile.TemporaryDirectory(prefix="oxid-unit1-runner-test-") as temporary:
            work = Path(temporary)
            for name in ("run_reviewer.py", "expected-test-roster.json"):
                shutil.copyfile(self.package / name, work / name)
            (work / "roster.json").write_text(json.dumps(self.roster))
            binary = work / "fake-libtest"
            binary.write_text(FAKE_LIBTEST)
            binary.chmod(0o755)
            env = dict(os.environ)
            env["OXID_UNIT1_PROTOCOL_CASE"] = mode
            env["PYTHONDONTWRITEBYTECODE"] = "1"
            env.pop("PYTHONOPTIMIZE", None)
            process = subprocess.run(
                [sys.executable, str(work / "run_reviewer.py"),
                 "--binary", str(binary), "--tag", mode],
                cwd=work, env=env, capture_output=True, text=True, timeout=60,
            )
            result_path = work / f"{mode}-results.json"
            self.assertTrue(
                result_path.is_file(),
                f"Missing retained result: {process.stdout}\n{process.stderr}",
            )
            result = json.loads(result_path.read_text())
            return process, result

    def assert_rejected(self, mode, total=0, passed=0):
        process, result = self.run_case(mode)
        self.assertNotEqual(process.returncode, 0, process.stdout)
        self.assertEqual(result["status"], "failed")
        self.assertEqual((result["total"], result["passed"]), (total, passed))
        if total:
            self.assertEqual(sum(row["passed"] is False for row in result["tests"]), 1)

    def test_success_executes_exact_roster(self):
        process, result = self.run_case("success")
        self.assertEqual(process.returncode, 0, process.stderr)
        self.assertEqual(result["status"], "passed")
        self.assertEqual((result["total"], result["passed"]), (69, 69))
        self.assertEqual(sorted(row["test"] for row in result["tests"]),
                         sorted(self.roster))
        self.assertTrue(all(row["one_named_test_verified"] for row in result["tests"]))

    def test_zero_listing_is_rejected(self):
        self.assert_rejected("zero-list")

    def test_duplicate_listing_is_rejected(self):
        self.assert_rejected("duplicate-list")

    def test_substituted_listing_is_rejected(self):
        self.assert_rejected("wrong-list")

    def test_listing_failure_is_rejected(self):
        self.assert_rejected("list-error")

    def test_zero_execution_is_rejected(self):
        self.assert_rejected("zero-run", total=69, passed=68)

    def test_ignored_test_is_rejected(self):
        self.assert_rejected("ignored", total=69, passed=68)

    def test_wrong_executed_name_is_rejected(self):
        self.assert_rejected("wrong-executed", total=69, passed=68)

    def test_nonzero_test_exit_is_rejected(self):
        self.assert_rejected("test-error", total=69, passed=68)


if __name__ == "__main__":
    unittest.main()
