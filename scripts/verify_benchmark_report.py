#!/usr/bin/env python3
from __future__ import annotations

import json
import re
import sys
from pathlib import Path
from typing import Any


TOP_LEVEL_KEYS = ["schema_version", "oxid_version", "iterations", "benchmarks"]
BENCHMARK_KEYS = ["cold_start", "parser", "packaging", "runtime_operations"]
STATS_KEYS = ["unit", "samples", "min", "median", "max"]
PARSER_KEYS = [*STATS_KEYS, "source_bytes", "bytes_per_second"]


def unique_object(pairs: list[tuple[str, Any]]) -> dict[str, Any]:
    result: dict[str, Any] = {}
    for key, value in pairs:
        if key in result:
            raise ValueError(f"duplicate JSON key: {key}")
        result[key] = value
    return result


def require_integer(value: Any, label: str, *, positive: bool = False) -> int:
    if type(value) is not int:
        raise ValueError(f"{label} must be an integer")
    minimum = 1 if positive else 0
    if value < minimum:
        qualifier = "positive" if positive else "non-negative"
        raise ValueError(f"{label} must be {qualifier}")
    return value


def validate_stats(name: str, value: Any, iterations: int, *, parser: bool = False) -> None:
    if not isinstance(value, dict):
        raise ValueError(f"benchmarks.{name} must be an object")
    expected_keys = PARSER_KEYS if parser else STATS_KEYS
    if list(value) != expected_keys:
        raise ValueError(
            f"benchmarks.{name} fields must be ordered exactly as {expected_keys}, got {list(value)}"
        )
    if value["unit"] != "nanoseconds":
        raise ValueError(f"benchmarks.{name}.unit must be nanoseconds")
    samples = require_integer(value["samples"], f"benchmarks.{name}.samples", positive=True)
    if samples != iterations:
        raise ValueError(
            f"benchmarks.{name}.samples is {samples}, expected {iterations}"
        )
    minimum = require_integer(value["min"], f"benchmarks.{name}.min")
    median = require_integer(value["median"], f"benchmarks.{name}.median")
    maximum = require_integer(value["max"], f"benchmarks.{name}.max")
    if not minimum <= median <= maximum:
        raise ValueError(f"benchmarks.{name} timings are not ordered min <= median <= max")
    if parser:
        require_integer(value["source_bytes"], "benchmarks.parser.source_bytes", positive=True)
        require_integer(
            value["bytes_per_second"], "benchmarks.parser.bytes_per_second", positive=True
        )


def validate_report(path: Path, expected_iterations: int) -> str:
    text = path.read_text(encoding="utf-8")
    if not text.endswith("\n"):
        raise ValueError("benchmark report must end with a newline")
    report = json.loads(text, object_pairs_hook=unique_object)
    if not isinstance(report, dict):
        raise ValueError("benchmark report must be a JSON object")
    if list(report) != TOP_LEVEL_KEYS:
        raise ValueError(
            f"top-level fields must be ordered exactly as {TOP_LEVEL_KEYS}, got {list(report)}"
        )
    if report["schema_version"] != 1:
        raise ValueError("schema_version must be 1")
    version = report["oxid_version"]
    if not isinstance(version, str) or re.fullmatch(r"\d+\.\d+\.\d+(?:[-+][0-9A-Za-z.-]+)?", version) is None:
        raise ValueError("oxid_version must be a semantic version")
    iterations = require_integer(report["iterations"], "iterations", positive=True)
    if iterations != expected_iterations:
        raise ValueError(f"iterations is {iterations}, expected {expected_iterations}")
    benchmarks = report["benchmarks"]
    if not isinstance(benchmarks, dict) or list(benchmarks) != BENCHMARK_KEYS:
        actual = list(benchmarks) if isinstance(benchmarks, dict) else type(benchmarks).__name__
        raise ValueError(
            f"benchmarks fields must be ordered exactly as {BENCHMARK_KEYS}, got {actual}"
        )
    validate_stats("cold_start", benchmarks["cold_start"], iterations)
    validate_stats("parser", benchmarks["parser"], iterations, parser=True)
    validate_stats("packaging", benchmarks["packaging"], iterations)
    validate_stats("runtime_operations", benchmarks["runtime_operations"], iterations)
    return version


def main() -> int:
    if len(sys.argv) != 3:
        raise SystemExit("usage: verify_benchmark_report.py <report.json> <expected-iterations>")
    path = Path(sys.argv[1])
    expected_iterations = require_integer(int(sys.argv[2]), "expected iterations", positive=True)
    version = validate_report(path, expected_iterations)
    print(f"benchmark report schema is valid for Oxid {version} ({expected_iterations} samples)")
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main())
    except (OSError, ValueError, json.JSONDecodeError) as error:
        raise SystemExit(f"benchmark report validation failed: {error}") from error
