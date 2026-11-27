#!/usr/bin/env python3
"""Compare optimized Oxid and Rust UTF-8 streaming on fixed, equal-output inputs."""
from __future__ import annotations

import argparse
import hashlib
import json
import os
from pathlib import Path
import platform
import random
import re
import shutil
import statistics
import subprocess
import tempfile
import time


ROOT = Path(__file__).resolve().parents[1]
APP = Path("apps/utf8-normalize")
LIBRARY = Path("packages/utf8_text")
MINIMUM_BYTES = 32 * 1024 * 1024
BOOTSTRAP_DRAWS = 2000
RSS_SAMPLES = 3
WORKLOADS = {
    "ascii-log": b"2026-11-25T17:30:00Z INFO worker completed task 42 in 7 ms\n",
    "mixed-newlines": b"A" * 4095 + "\r\n🦀\0雪\r".encode(),
    "multilingual": "繁體中文、こんにちは、γειά σου、مرحبا、🦀\r\n".encode(),
}


def digest(path: Path) -> str:
    with path.open("rb") as stream:
        return hashlib.file_digest(stream, "sha256").hexdigest()


def checked(command: list, cwd: Path, env: dict) -> str:
    result = subprocess.run(command, cwd=cwd, env=env, capture_output=True, timeout=180)
    if result.returncode:
        raise RuntimeError((result.stderr or result.stdout).decode("utf-8", errors="replace"))
    return result.stdout.decode("utf-8").strip()


def prepare(work: Path, oxid: Path, rustc: Path, env: dict) -> tuple[dict, dict]:
    for path in (APP, LIBRARY):
        shutil.copytree(ROOT / path, work / path, ignore=shutil.ignore_patterns(".oxid", "oxid.lock"))
    project = work / APP
    checked([oxid, "lock", "--offline"], project, env)
    checked([oxid, "build", "-p", "app", "--profile=release", "--frozen"], project, env)
    binary = project / "app/.oxid/bin/utf8-normalize"
    reference = project / "benchmark/reference.rs"
    rust = work / "reference"
    checked([rustc, "--edition=2024", "-C", "opt-level=2", reference, "-o", rust], work, env)
    inputs = sorted(file for root in (APP, LIBRARY) for file in (work / root).rglob("*")
                    if file.is_file() and ".oxid" not in file.relative_to(work).parts
                    and file.suffix in (".ox", ".rs", ".toml"))
    llvm = Path(env["OXID_LLVM_BIN"])
    linker = "ld64.lld" if platform.system() == "Darwin" else "ld.lld"
    tools = {name: {"version": checked([llvm / name, "--version"], work, env).splitlines()[0],
                    "sha256": digest(llvm / name)}
             for name in ("clang", "opt", linker)}
    report = {
        "system": platform.platform(), "arch": platform.machine(),
        "oxid_version": checked([oxid, "--version"], work, env),
        "rustc_version": checked([rustc, "--version", "--verbose"], work, env),
        "oxid_compiler_sha256": digest(oxid), "llvm": tools,
        "executables": {"oxid": digest(binary), "rust": digest(rust)},
        "source_sha256": {file.relative_to(work).as_posix(): digest(file) for file in inputs},
        "workspace_lock_sha256": digest(project / "oxid.lock"),
    }
    return {"oxid": [binary, "--stream"], "rust": [rust]}, report


def fixture(path: Path, sample: bytes) -> str:
    repeats = MINIMUM_BYTES // len(sample) + 1
    normalized = sample.replace(b"\r\n", b"\n").replace(b"\r", b"\n")
    blocks, remaining = divmod(repeats, 512)
    raw_block, normalized_block = sample * 512, normalized * 512
    expected = hashlib.sha256()
    with path.open("wb") as stream:
        for _ in range(blocks):
            stream.write(raw_block)
            expected.update(normalized_block)
        stream.write(sample * remaining)
        expected.update(normalized * remaining)
    return expected.hexdigest()


def measure(command: list, work: Path, source: Path, sink: Path) -> int:
    with source.open("rb") as stdin, sink.open("wb") as stdout:
        start = time.perf_counter_ns()
        result = subprocess.run(command, cwd=work, env={}, stdin=stdin, stdout=stdout,
                                stderr=subprocess.PIPE, timeout=120)
        elapsed = time.perf_counter_ns() - start
    if result.returncode or result.stderr:
        raise RuntimeError(f"{command[0].name} exited with {result.returncode}: {result.stderr!r}")
    return elapsed


def rss_bytes(report: bytes, system: str) -> int:
    if system == "Linux":
        match = re.fullmatch(rb"([0-9]+)", report.strip())
        matches = [match[1]] if match else []
        scale = 1024
    elif system == "Darwin":
        matches = re.findall(rb"^\s*([0-9]+)\s+maximum resident set size\s*$", report, re.MULTILINE)
        scale = 1
    else:
        raise ValueError(f"peak RSS measurement is unavailable on {system}")
    if len(matches) != 1 or int(matches[0]) == 0:
        raise ValueError(f"could not read peak RSS from {system} time output: {report!r}")
    return int(matches[0]) * scale


def measure_rss(timer: Path, command: list, work: Path, source: Path) -> int:
    system = platform.system()
    flags = {"Linux": ["--format=%M"], "Darwin": ["-l"]}.get(system)
    if flags is None:
        raise ValueError(f"peak RSS measurement is unavailable on {system}")
    with source.open("rb") as stdin:
        result = subprocess.run([timer, *flags, *command], cwd=work, env={"LC_ALL": "C"},
                                stdin=stdin, stdout=subprocess.DEVNULL, stderr=subprocess.PIPE,
                                timeout=120)
    if result.returncode:
        raise RuntimeError(f"{command[0].name} exited with {result.returncode}: {result.stderr!r}")
    return rss_bytes(result.stderr, system)


def compare(work: Path, commands: dict, runs: int, timer: Path) -> dict:
    reports = {}
    source, capture = work / "input.bin", work / "output.bin"
    for name, sample in WORKLOADS.items():
        expected = fixture(source, sample)
        for command in commands.values():
            measure(command, work, source, capture)
            if digest(capture) != expected:
                raise RuntimeError(f"{command[0].name} produced incorrect bytes for {name}")
        samples = {key: [] for key in commands}
        for run in range(runs):
            for key in commands if run % 2 == 0 else reversed(commands):
                samples[key].append(measure(commands[key], work, source, Path(os.devnull)))
        medians = {key: statistics.median(values) for key, values in samples.items()}
        memory = {key: [] for key in commands}
        for run in range(RSS_SAMPLES):
            for key in commands if run % 2 == 0 else reversed(commands):
                memory[key].append(measure_rss(timer, commands[key], work, source))
        peaks = {key: max(values) for key, values in memory.items()}
        reports[name] = {
            "input_bytes": source.stat().st_size, "input_sha256": digest(source),
            "output_sha256": expected, "times_ns": samples, "median_ns": medians,
            "oxid_over_rust": medians["oxid"] / medians["rust"],
            "rss_samples_bytes": memory, "peak_rss_bytes": peaks,
            "peak_rss_oxid_over_rust": peaks["oxid"] / peaks["rust"],
        }
    return reports


def record_confidence_intervals(report: dict) -> None:
    """Resample paired Oxid and Rust measurements from each benchmark round."""
    generator = random.Random(0)
    draws_by_workload = []

    def interval(samples):
        cuts = statistics.quantiles(samples, n=40, method="inclusive")
        return [cuts[0], cuts[-1]]

    for workload in report["workloads"].values():
        samples = workload["times_ns"]
        pairs = list(zip(samples["oxid"], samples["rust"], strict=True))
        draws = []
        for _ in range(BOOTSTRAP_DRAWS):
            oxid, rust = zip(*generator.choices(pairs, k=len(pairs)))
            draws.append(statistics.median(oxid) / statistics.median(rust))
        workload["oxid_over_rust_ci95"] = interval(draws)
        draws_by_workload.append(draws)
    report["geometric_mean_ratio_ci95"] = interval([
        statistics.geometric_mean(draw) for draw in zip(*draws_by_workload, strict=True)
    ])
    report["bootstrap_draws"] = BOOTSTRAP_DRAWS


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--oxid", type=Path, required=True)
    parser.add_argument("--rustc", type=Path, default=Path(shutil.which("rustc") or "rustc"))
    parser.add_argument("--llvm-bin", type=Path, required=True)
    parser.add_argument("--time-bin", type=Path, default=Path("/usr/bin/time"),
                        help="GNU time on Linux or system time on macOS")
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--runs", type=int, default=30)
    args = parser.parse_args()
    if args.runs < 1:
        parser.error("--runs must be positive")
    if args.output.exists():
        parser.error("--output already exists")
    try:
        env = dict(os.environ, OXID_LLVM_BIN=str(args.llvm_bin.resolve(strict=True)))
        timer = args.time_bin.resolve(strict=True)
        with tempfile.TemporaryDirectory(prefix="oxid-utf8-benchmark-") as directory:
            work = Path(directory)
            commands, report = prepare(work, args.oxid.resolve(strict=True), args.rustc.absolute(), env)
            report["workloads"] = compare(work, commands, args.runs, timer)
            report["rss_timer"] = {"path": str(timer), "sha256": digest(timer),
                                   "samples_per_workload": RSS_SAMPLES}
            report["geometric_mean_ratio"] = statistics.geometric_mean(
                workload["oxid_over_rust"] for workload in report["workloads"].values())
            record_confidence_intervals(report)
            if hasattr(os, "sched_getaffinity"):
                report["cpu_affinity"] = sorted(os.sched_getaffinity(0))
        with args.output.open("x", encoding="utf-8") as stream:
            json.dump(report, stream, indent=2)
            stream.write("\n")
    except (OSError, RuntimeError, ValueError, subprocess.TimeoutExpired) as error:
        parser.exit(1, f"UTF-8 benchmark failed: {error}\n")
    lower, upper = report["geometric_mean_ratio_ci95"]
    print(f"Oxid/Rust median ratio across workloads {report['geometric_mean_ratio']:.3f} "
          f"(95% confidence interval {lower:.3f} to {upper:.3f})")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
