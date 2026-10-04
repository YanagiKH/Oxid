#!/usr/bin/env python3
"""Check hand-authored formatter goldens through the public CLI, byte for byte."""
from __future__ import annotations

import json
from pathlib import Path
import subprocess
import sys
import tempfile

ROOT = Path(__file__).resolve().parents[1]


def verify(executable: Path) -> None:
    cases = json.loads((ROOT / "tests/fixtures/typed_formatter/cases.json").read_text(encoding="utf-8"))
    with tempfile.TemporaryDirectory(prefix="oxid-format-goldens-") as directory:
        cwd = Path(directory)
        path = cwd / "input.ox"

        def run(expected_code: int, stdout: bytes, stderr: bytes | None, *, check: bool = False) -> bytes:
            before = path.read_bytes()
            argv = [str(executable), "fmt", "--edition=typed-preview", str(path)]
            if check:
                argv.append("--check")
            result = subprocess.run(argv, cwd=cwd, capture_output=True, timeout=20)
            if (result.returncode != expected_code or result.stdout != stdout
                    or (stderr is not None and result.stderr != stderr)):
                raise RuntimeError(f"unexpected result for {case['id']} (check={check}): "
                                   f"code={result.returncode}, stdout={result.stdout!r}, stderr={result.stderr!r}")
            if path.read_bytes() != before or set(cwd.iterdir()) != {path}:
                raise RuntimeError(f"formatter changed input or directory for {case['id']}")
            return result.stderr

        for case in cases["positive"]:
            original = case["input"].encode("utf-8")
            expected = case["expected"].encode("utf-8")
            path.write_bytes(original)
            run(0, expected, b"")
            code = case["check_input_exit"]
            run(code, b"", b"typed-preview fmt: formatting required\n" if code else b"", check=True)
            path.write_bytes(expected)
            run(0, expected, b"")
            run(0, b"", b"", check=True)
        for case in cases["negative"]:
            path.write_bytes(case["input"].encode("utf-8"))
            expected = f"error[{case['expected_code']}] ({case['expected_stage']})".encode()
            for check in (False, True):
                if expected not in run(2, b"", None, check=check):
                    raise RuntimeError(f"missing diagnostic for {case['id']}")
    print(f"typed formatter goldens passed: {len(cases['positive'])} positive, "
          f"{len(cases['negative'])} negative; format/check/fixed-point/input immutability")


if __name__ == "__main__":
    try:
        verify(Path(sys.argv[1] if len(sys.argv) > 1 else ROOT / "target/release/oxid").resolve())
    except (OSError, RuntimeError, subprocess.TimeoutExpired) as error:
        print(f"formatter verification error: {error}", file=sys.stderr)
        raise SystemExit(1)
