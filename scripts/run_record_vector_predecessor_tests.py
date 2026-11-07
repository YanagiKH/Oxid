#!/usr/bin/env python3
"""Replay both unchanged pre-Windows vector and Copy source-policy suites."""
import argparse
import ast
import hashlib
import json
from pathlib import Path
import re
import subprocess
import sys
import tempfile

REVISION = "6c3161640dbcc36a484ebdd23eb2594ccde4e16a"
MODULES = {
    "test_checked_copy_compatibility": (
        "e16894e15eed4bbe27e32370c9e29bdffb309680b4eb5b94f42aafea8ecb2ae2", 20),
    "test_typed_vector_compatibility": (
        "87b834d9c64d369716843cd1428aea54d940c94f50f5c4c00033c031a1f0ce1b", 18),
}
POLICIES = {
    "verify_checked_copy_compatibility.py":
        "d64883191139491e31e594516d4e8a62d90e7df18bd31a8a7cfa80a8dc1e77ef",
    "verify_typed_vector_compatibility.py":
        "6fb58c5296eb64d9462e849cbc06056d6852dd1a9323e124a219b5fc407166ca",
}


def require(condition, message):
    if not condition:
        raise ValueError(message)


def sha(body):
    return hashlib.sha256(body).hexdigest()


def git(repo, *args):
    child = subprocess.run(["git", "--no-replace-objects", "-C", str(repo), *args],
                           capture_output=True, timeout=30)
    require(child.returncode == 0, "cannot inspect predecessor checkout")
    return child.stdout


def identity(repo):
    require(git(repo, "rev-parse", "HEAD").decode().strip() == REVISION,
            "wrong predecessor replay revision")
    require(not git(repo, "status", "--porcelain=v1", "--untracked-files=all"),
            "predecessor replay must be clean")
    pins = {**{name + ".py": pin for name, (pin, _) in MODULES.items()}, **POLICIES}
    bodies = {}
    for name, pin in pins.items():
        path = repo / "scripts" / name
        require(not path.is_symlink() and path.is_file(), "unsafe predecessor input")
        bodies[name] = path.read_bytes()
        require(sha(bodies[name]) == pin, "changed pinned predecessor input: " + name)
    return bodies


def validate_execution(module, body, child):
    names = {module + "." + cls.name + "." + method.name
             for cls in ast.parse(body).body if isinstance(cls, ast.ClassDef)
             for method in cls.body if isinstance(method, ast.FunctionDef)
             and method.name.startswith("test_")}
    count = MODULES[module][1]
    require(len(names) == count, "predecessor test census changed")
    require(child.returncode == 0 and child.stdout == b"", "predecessor controls failed")
    text = child.stderr.decode("utf-8")
    executed = re.findall(r"^test_[^\n]+ \(([^\n]+)\) \.\.\. ok$", text, re.M)
    require(set(executed) == names and len(executed) == count,
            "partial predecessor test execution")
    require(re.findall(r"^Ran (\d+) tests in .+$", text, re.M) == [str(count)]
            and re.findall(r"^OK$", text, re.M) == ["OK"],
            "missing predecessor terminal success")
    return sorted(names)


def run(args):
    repo = args.repo.resolve(strict=True)
    output = args.output.resolve()
    require(not output.exists() and output != repo and repo not in output.parents,
            "output must be new and outside repository")
    output.mkdir(parents=True)
    bodies = identity(repo)
    result = {"schema": "oxid-source-policy-predecessor-replay-v1",
              "revision": REVISION, "historical_results_apply_to_current": False,
              "policies": POLICIES, "modules": {}, "status": "running"}
    try:
        for module, (pin, count) in MODULES.items():
            with tempfile.TemporaryDirectory(prefix="oxid-record-vector-replay-bytecode-") as cache:
                command = [sys.executable, "-I", "-B", "-X", "pycache_prefix=" + cache,
                           "-m", "unittest", "discover", "-s", str(repo / "scripts"),
                           "-p", module + ".py", "-v"]
                child = subprocess.run(command, cwd=repo, capture_output=True, timeout=120)
            (output / (module + ".stdout")).write_bytes(child.stdout)
            (output / (module + ".stderr")).write_bytes(child.stderr)
            row = {"sha256": pin, "argv": command, "exit_status": child.returncode,
                   "stdout_sha256": sha(child.stdout), "stderr_sha256": sha(child.stderr)}
            result["modules"][module] = row
            row["tests"] = validate_execution(module, bodies[module + ".py"], child)
            require(identity(repo) == bodies, "predecessor inputs changed during replay")
            row["test_executions"] = count
        result.update(status="passed", test_executions=sum(row[1] for row in MODULES.values()))
    except (Exception, KeyboardInterrupt) as error:
        result.update(status="failed", error=f"{type(error).__name__}: {error}")
        raise
    finally:
        (output / "result.json").write_text(json.dumps(result, indent=2, sort_keys=True) + "\n")
    print(json.dumps({"status": "passed", "predecessor": REVISION, "tests": result["test_executions"]}))
    return 0


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    arguments = parser.parse_args()
    try:
        raise SystemExit(run(arguments))
    except (OSError, ValueError, subprocess.TimeoutExpired) as error:
        print(str(error), file=sys.stderr)
        raise SystemExit(1)
