#!/usr/bin/env python3
"""Exercise private registry publication, identity, offline builds and deployment."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import signal
import subprocess
import sys
import time

ROOT = Path(__file__).resolve().parents[1]
HELPERS = ROOT / "scripts/registry_acceptance"
MAX_STREAM = 8 * 1024 * 1024


def require(condition, message):
    if not condition:
        raise ValueError(message)


def digest(path):
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def save(path, value):
    path.write_text(json.dumps(value, ensure_ascii=False, indent=2, sort_keys=True) + "\n")


def phase(name, script, arguments, directory, report):
    transcript = directory / (name + "-transcript")
    transcript.mkdir()
    output = directory / name
    argv = [sys.executable, "-B", str(HELPERS / script), *map(str, arguments), str(output)]
    if name == "versions":
        argv = [sys.executable, "-B", str(HELPERS / script), str(arguments[0]), "--output", str(output)]
    if name == "application" and report["native"]:
        argv.append("--native")
    row = {"name": name, "argv": argv, "cwd": str(ROOT), "status": "running"}
    report["phases"].append(row)
    save(directory / "result.json", report)
    start = time.monotonic()
    child = None
    try:
        with (transcript / "stdout").open("wb") as stdout, (transcript / "stderr").open("wb") as stderr:
            child = subprocess.Popen(argv, cwd=ROOT, stdin=subprocess.DEVNULL,
                                     stdout=stdout, stderr=stderr, start_new_session=True)
            while child.poll() is None:
                require(time.monotonic() - start < 600, name + ": phase deadline exceeded")
                require(stdout.tell() <= MAX_STREAM and stderr.tell() <= MAX_STREAM,
                        name + ": phase transcript exceeded its allowance")
                time.sleep(0.05)
            row["exit"] = child.wait()
        row.update(stdout_sha256=digest(transcript / "stdout"), stderr_sha256=digest(transcript / "stderr"))
        require(row["exit"] == 0, name + ": failed; inspect retained command streams")
        require((transcript / "stderr").stat().st_size == 0, name + ": unexpected stderr")
        require((output / "result.json").stat().st_size <= MAX_STREAM, name + ": oversized result")
        result = json.loads((output / "result.json").read_text())
        require(result.get("status") == "passed", name + ": result is not successful")
        row.update(status="passed", result_sha256=digest(output / "result.json"),
                   recorded_commands=len(result["commands"]))
        if name == "application":
            require(result.get("cases_per_execution") == 12 and result.get("nominal_diamond_shared") is True,
                    "registered application did not verify the full independent corpus")
            require(len(result.get("deployments", [])) == (3 if report["native"] else 0),
                    "native deployment coverage differs from the requested mode")
        return output
    except BaseException as error:
        row.update(status="failed", error=repr(error))
        raise
    finally:
        if child is not None and child.poll() is None:
            os.killpg(child.pid, signal.SIGTERM)
            try:
                child.wait(timeout=2)
            except subprocess.TimeoutExpired:
                os.killpg(child.pid, signal.SIGKILL)
                child.wait(timeout=2)
        row["seconds"] = time.monotonic() - start
        save(transcript / "command.json", row)
        save(directory / "result.json", report)


def verify(args):
    require(sys.platform == "linux", "private registry acceptance currently requires Linux")
    output = args.output.absolute()
    require(not output.exists() and not output.is_symlink(), "output must be a fresh directory")
    require(output != ROOT and ROOT not in output.parents, "output must be outside the repository")
    compilers = [path.resolve(strict=True) for path in args.compilers]
    require(compilers and len(compilers) == len(set(compilers)), "compiler paths must be distinct")
    require(all(path.is_file() and os.access(path, os.X_OK) for path in compilers), "compiler must be executable")
    inputs = [Path(__file__), *HELPERS.glob("*.py"), *ROOT.joinpath("apps/byte-count").rglob("*.ox"),
              *ROOT.joinpath("apps/byte-count").rglob("oxid.toml")]
    pins = {str(path.relative_to(ROOT)): digest(path) for path in inputs}
    output.mkdir(parents=True)
    report = {"schema": "oxid-typed-private-registry-acceptance-v1", "status": "running", "native": args.native,
              "scope": "experimental exact-version private file registry; no general release qualification",
              "source_sha256": pins, "compilers": []}
    try:
        for index, compiler in enumerate(compilers):
            directory = output / f"compiler-{index}"
            directory.mkdir()
            selected = {"path": str(compiler), "sha256": digest(compiler), "native": args.native,
                        "status": "running", "phases": []}
            report["compilers"].append(selected)
            trust = phase("trust", "trust.py", [compiler], directory, selected)
            phase("cache", "cache.py", [compiler, trust], directory, selected)
            phase("versions", "versions.py", [compiler], directory, selected)
            phase("application", "byte_count.py", [compiler, ROOT], directory, selected)
            require(digest(compiler) == selected["sha256"], "compiler changed during acceptance")
            selected["status"] = "passed"
        require(all(digest(ROOT / name) == value for name, value in pins.items()), "acceptance inputs changed")
        report["status"] = "passed"
        print(json.dumps({"status": "passed", "compilers": len(compilers), "native": args.native}))
        return 0
    except (OSError, ValueError, KeyError, subprocess.TimeoutExpired) as error:
        report.update(status="failed", error=repr(error))
        print(str(error), file=sys.stderr)
        return 1
    finally:
        save(output / "result.json", report)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("compilers", type=Path, nargs="+")
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--native", action="store_true")
    args = parser.parse_args()
    try:
        return verify(args)
    except (OSError, ValueError) as error:
        print(str(error), file=sys.stderr)
        return 1


if __name__ == "__main__":
    raise SystemExit(main())
