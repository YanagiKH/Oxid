#!/usr/bin/env python3
"""Replay frozen script tests and verify their genuine-event hosted certificate.

One retained suite requires its checkout to be the actual GitHub event commit.
That suite is verified against its successful original run, not impersonated by
changing GITHUB_SHA/GITHUB_EVENT_PATH during a later historical replay.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
import unittest
import urllib.error
import urllib.request

from verify_historical_event_evidence import verify_archive

REPOSITORY = "YanagiKH/Oxid"
BASELINE = "9ff8ef90a26f003f4616838dfc186fa3bfb6845c"
BASELINE_TREE = "50512c0ff428a802afcb1b834ee11f981811db1f"
RUN = 39226443899
ATTEMPT = 1
JOB = 117973774217
WORKFLOW = 321632921
HOSTED_MODULE = "test_published_formatter_source"
SOURCE_PINS = {
    ".github/workflows/ci.yml": "f97c508b3e620cf2144c3b8c3b9488ff8bc442c7e99b4d41887475a8a062fdbc",
    "scripts/test_published_formatter_source.py": "81c46045c946e6a6c9ef1e5defc5072c13bf7d3b3da5f853408ae0f55171a6ba",
    "tests/qualification/stdlib_feature_storage_activation_v3/hosted_controls.py":
        "57da469df9fcb3399b7a6ebeaf6808641f14d1d5b7b3127fb0fd543b1d34efa0",
}


def require(condition, message):
    if not condition:
        raise ValueError(message)


def validate_history(run, job):
    require(isinstance(run, dict) and isinstance(job, dict), "invalid GitHub metadata")
    for key, value in {"id": RUN, "run_attempt": ATTEMPT, "workflow_id": WORKFLOW,
                       "path": ".github/workflows/ci.yml", "head_sha": BASELINE,
                       "event": "schedule", "status": "completed", "conclusion": "success"}.items():
        require(type(run.get(key)) is type(value) and run[key] == value,
                "historical run mismatch: " + key)
    for key in ("repository", "head_repository"):
        require(isinstance(run.get(key), dict) and run[key].get("full_name") == REPOSITORY,
                "historical run repository mismatch")
    for key, value in {"id": JOB, "run_id": RUN, "run_attempt": ATTEMPT,
                       "head_sha": BASELINE, "name": "Full repository verification",
                       "status": "completed", "conclusion": "success"}.items():
        require(type(job.get(key)) is type(value) and job[key] == value,
                "historical job mismatch: " + key)
    require(isinstance(job.get("steps"), list), "missing historical job steps")
    steps = [step for step in job["steps"] if isinstance(step, dict)
             and step.get("name") == "Test feature status metadata validation"]
    require(len(steps) == 1 and steps[0].get("status") == "completed"
            and steps[0].get("conclusion") == "success", "historical script suite did not pass")
    return {"run_id": RUN, "run_attempt": ATTEMPT, "job_id": JOB,
            "head_sha": BASELINE, "tree_sha": BASELINE_TREE,
            "event": "schedule", "step": steps[0],
            "url": f"https://github.com/{REPOSITORY}/actions/runs/{RUN}/job/{JOB}"}


class NoRedirect(urllib.request.HTTPRedirectHandler):
    def redirect_request(self, req, fp, code, msg, headers, newurl):
        raise ValueError("unexpected redirect while verifying historical GitHub metadata")


def fetch_json(path):
    headers = {"Accept": "application/vnd.github+json", "X-GitHub-Api-Version": "2022-11-28",
               "User-Agent": "oxid-historical-qualification"}
    if token := os.environ.get("GITHUB_TOKEN"):
        headers["Authorization"] = "Bearer " + token
    request = urllib.request.Request("https://api.github.com/repos/" + REPOSITORY + path, headers=headers)
    with urllib.request.build_opener(NoRedirect).open(request, timeout=30) as response:
        raw = response.read(2 * 1024 * 1024 + 1)
    require(len(raw) <= 2 * 1024 * 1024, "historical metadata exceeds size limit")
    return json.loads(raw)


def validate_checkout(repo):
    def git(*words):
        return subprocess.check_output(["git", "-C", str(repo), *words], text=True).strip()
    require(git("rev-parse", "HEAD") == BASELINE, "historical checkout HEAD mismatch")
    require(git("rev-parse", "HEAD^{tree}") == BASELINE_TREE, "historical checkout tree mismatch")
    require(not git("diff", "HEAD", "--name-only"), "historical tracked files changed")
    for name, expected in SOURCE_PINS.items():
        path = repo / name
        require(path.is_file() and not path.is_symlink(), "missing historical source: " + name)
        require(hashlib.sha256(path.read_bytes()).hexdigest() == expected,
                "changed historical source: " + name)


class HistoricalLoader(unittest.TestLoader):
    def __init__(self, directory):
        super().__init__()
        self.directory = Path(directory).resolve()

    def _match_path(self, path, full_path, pattern):
        candidate = Path(full_path)
        if candidate.parent.resolve() == self.directory and candidate.stem == HOSTED_MODULE:
            return False
        return super()._match_path(path, full_path, pattern)


def run(args):
    repo, output = args.repo.resolve(strict=True), args.output.resolve()
    require(not output.exists() and not args.output.is_symlink(), "receipt must be new")
    report = {"schema": "oxid-historical-script-replay-v1", "status": "running",
              "scope": "historical source only; not current-source qualification",
              "baseline": BASELINE, "controller_sha256": hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
              "caller": {name: os.environ.get(name) for name in
                         ("GITHUB_SHA", "GITHUB_EVENT_NAME", "GITHUB_RUN_ID", "GITHUB_RUN_ATTEMPT")},
              "source_pins": SOURCE_PINS}
    try:
        validate_checkout(repo)
        report["retained_archive"] = verify_archive(args.archive)
        certificate = validate_history(fetch_json(f"/actions/runs/{RUN}/attempts/{ATTEMPT}"),
                                       fetch_json(f"/actions/jobs/{JOB}"))
        report["delegated"] = {"module": HOSTED_MODULE,
                               "method": "verified-original-genuine-event-run", "certificate": certificate}
        print(f"Historical event-bound suite: {HOSTED_MODULE} @ {BASELINE}; "
              f"verified original run {RUN}, job {JOB}; no event environment substitution", file=sys.stderr)
        directory = repo / "scripts"
        suite = HistoricalLoader(directory).discover(str(directory), pattern="test_*.py", top_level_dir=str(directory))
        require(suite.countTestCases() > 0, "no historical replay tests discovered")
        result = unittest.TextTestRunner(verbosity=2).run(suite)
        validate_checkout(repo)
        report["replayed"] = {"tests_run": result.testsRun, "failures": len(result.failures),
                              "errors": len(result.errors), "skipped": len(result.skipped)}
        report["status"] = "passed" if result.wasSuccessful() else "failed"
        return 0 if result.wasSuccessful() else 1
    except (OSError, ValueError, subprocess.SubprocessError, urllib.error.URLError) as error:
        report.update(status="failed", error=str(error))
        print(str(error), file=sys.stderr)
        return 1
    finally:
        output.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--repo", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--archive", type=Path, required=True)
    args = parser.parse_args()
    try:
        return run(args)
    except (OSError, ValueError) as error:
        print(str(error), file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
