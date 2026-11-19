#!/usr/bin/env python3
"""Replay the unchanged source-pinned ARM64 host-test policy controls on their exact parent.

This is Python control evidence for that parent, not current compiler semantics.
"""
import argparse
import ast
import hashlib
import json
from pathlib import Path
import re
import subprocess
import sys
import tempfile

REVISION = "18314e95d6afa3d8a4da7970dbddc828f4f6cbac"
MODULE = "test_arm64_host_compatibility"
MODULE_SHA = "91b9cb1ae16da868ee873a36f6efef8ef93745bfed3b8684340c828cf77421a1"
POLICY_SHA = "9ae4f0ffe92ec86c1612cdb87da65995b88dd12229e836828c8e23d549de6bf4"


def require(condition, message):
    if not condition:
        raise ValueError(message)


def sha(body):
    return hashlib.sha256(body).hexdigest()


def git(repo, *args):
    result = subprocess.run(["git", "--no-replace-objects", "-C", str(repo), *args],
                            capture_output=True, timeout=30)
    require(result.returncode == 0, "cannot inspect replay checkout")
    return result.stdout


def identity(repo):
    require(git(repo, "rev-parse", "HEAD").decode().strip() == REVISION, "wrong predecessor replay revision")
    require(not git(repo, "status", "--porcelain=v1", "--untracked-files=all"), "predecessor replay must be clean")
    module = repo / "scripts" / (MODULE + ".py")
    policy = repo / "scripts/verify_arm64_host_compatibility.py"
    require(not module.is_symlink() and not policy.is_symlink(), "symlink replay input")
    body = module.read_bytes()
    require(sha(body) == MODULE_SHA and sha(policy.read_bytes()) == POLICY_SHA, "changed pinned predecessor replay input")
    return body


def run(args):
    repo = args.repo.resolve(strict=True)
    output = args.output.resolve()
    require(not output.exists() and output != repo and repo not in output.parents, "output must be new and outside repository")
    output.mkdir(parents=True)
    body = identity(repo)
    names = {MODULE + "." + cls.name + "." + method.name
             for cls in ast.parse(body).body if isinstance(cls, ast.ClassDef)
             for method in cls.body if isinstance(method, ast.FunctionDef) and method.name.startswith("test_")}
    require(len(names) == 20, "predecessor test census changed")
    with tempfile.TemporaryDirectory(prefix="oxid-arm64-host-replay-bytecode-") as cache:
        command = [sys.executable, "-I", "-B", "-X", "pycache_prefix=" + cache,
                   "-m", "unittest", "discover", "-s", str(repo / "scripts"),
                   "-p", MODULE + ".py", "-v"]
        child = subprocess.run(command, cwd=repo, capture_output=True, timeout=120)
    (output / "stdout").write_bytes(child.stdout)
    (output / "stderr").write_bytes(child.stderr)
    result = {"schema": "oxid-source-policy-predecessor-replay-v1", "revision": REVISION,
              "module_sha256": MODULE_SHA, "policy_sha256": POLICY_SHA,
              "controller_sha256": sha(Path(__file__).read_bytes()),
              "argv": command, "exit_status": child.returncode,
              "stdout_sha256": sha(child.stdout), "stderr_sha256": sha(child.stderr),
              "historical_results_apply_to_current": False}
    (output / "command.json").write_text(json.dumps(result, indent=2) + "\n")
    require(child.returncode == 0 and child.stdout == b"", "predecessor controls failed; see retained streams")
    text = child.stderr.decode("utf-8")
    executed = re.findall(r"^test_[^\n]+ \(([^\n]+)\) \.\.\. ok$", text, re.M)
    require(set(executed) == names and len(executed) == len(names), "partial predecessor test execution")
    require(re.findall(r"^Ran (\d+) tests in .+$", text, re.M) == ["20"]
            and re.findall(r"^OK$", text, re.M) == ["OK"], "missing predecessor terminal success")
    require(identity(repo) == body, "predecessor replay input changed")
    result.update(status="passed", tests=sorted(names), test_executions=20)
    (output / "result.json").write_text(json.dumps(result, indent=2, sort_keys=True) + "\n")
    print(json.dumps({"status": "passed", "predecessor": REVISION, "tests": 20}))
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
