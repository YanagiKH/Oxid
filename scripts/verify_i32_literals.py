#!/usr/bin/env python3
"""Independent exact-integer/CLI oracle for the experimental literal-only i32 slice.

Usage: python3 scripts/verify_i32_literals.py target/release/oxid
Uses Python integers for expected values, not the production resolver or legacy VM.
Fixtures live only in a temporary directory; no .ox discovery entries are added.
"""
import json
import os
from pathlib import Path
import random
import subprocess
import sys
import tempfile


def main():
    binary = str(Path(sys.argv[1]).resolve())
    rng = random.Random(0x132DEC)
    # This is test-oracle bigint conversion, not a runtime/compiler dependency.
    sys.set_int_max_str_digits(0)
    counts = {"literal_cases": 0, "unsupported_cases": 0, "source_cases": 0, "invocations": 0}
    with tempfile.TemporaryDirectory(prefix="oxid-i32-oracle-") as directory:
        root = Path(directory)
        env = dict(os.environ, OXID_CACHE_DIR=str(root / "cache"))
        env.pop("OXID_PATH", None)

        def run(source, operation="run"):
            (root / "input.ox").write_text(source, encoding="utf-8")
            process = subprocess.run(
                [binary, operation, "input.ox", "--edition=typed-preview", "--message-format=json"],
                cwd=root, env=env, input=b"", capture_output=True, timeout=10,
            )
            counts["invocations"] += 1
            assert not process.stderr, process.stderr
            records = [json.loads(line) for line in process.stdout.splitlines()]
            assert records
            for record in records:
                assert record["schema_version"] == 1
                assert record["edition"] == "typed-preview"
            summary = records[-1]
            assert summary["kind"] == f"{operation}-summary", records
            assert len(list(root.iterdir())) == 1, "unexpected output/cache file"
            return process.returncode, records

        def success(source, expected, functions=1):
            for operation in ("check", "run"):
                code, records = run(source, operation)
                assert code == 0 and len(records) == 1, (source, code, records)
                summary = records[0]
                assert summary["success"] is True and summary["errors"] == 0
                if operation == "check":
                    assert summary["functions"] == functions
                else:
                    result = summary["result"]
                    assert result == {"type": "i32", "value": expected}, (source, records)
                    assert type(result["value"]) is int, "JSON numeric value must be an integer"

        def failure(source, expected_code, stage, origin):
            start = source.index(origin)
            start_byte = len(source[:start].encode())
            end_byte = start_byte + len(origin.encode())
            for operation in ("check", "run"):
                code, records = run(source, operation)
                assert code == 1 and len(records) == 2, (code, records)
                diagnostic, summary = records
                assert diagnostic["kind"] == "diagnostic"
                assert diagnostic["code"] == expected_code and diagnostic["stage"] == stage, (source, records)
                primary = diagnostic["primary"]
                assert (primary["start"], primary["end"]) == (start_byte, end_byte), (source, records)
                assert primary["line"] == source[:start].count("\n") + 1
                assert primary["column"] == len(source[:start].rsplit("\n", 1)[-1]) + 1
                assert summary["success"] is False and summary["errors"] == 1
                assert summary["result" if operation == "run" else "functions"] is None

        magnitudes = [0, 1, 2, 2147483646, 2147483647, 2147483648, 2147483649, 9007199254740992, 9007199254740993]
        magnitudes += [rng.randrange(0, 2**33) for _ in range(160)]
        magnitudes += [rng.randrange(0, 2**31) for _ in range(80)]
        cases = []
        for magnitude in magnitudes:
            for negative in (False, True):
                digits = "0" * rng.randrange(0, 30) + str(magnitude)
                sign = rng.choice(["-", "- ", "- /* 雪 */ ", "- // é\r\n "]) if negative else ""
                expected = int(("-" if negative else "") + digits)
                cases.append((sign + digits, expected))
        cases += [("0" * 65536, 0), ("-" + "0" * 65536, 0), ("0" * 65526 + "2147483647", 2147483647), ("9" * 65536, int("9" * 65536))]
        for literal, expected in cases:
            source = f"// é\r\nfn main() -> i32 {{ return {literal}; }}"
            if -(2**31) <= expected < 2**31:
                success(source, expected)
            else:
                failure(source, "E0203", "resolve", literal)
            counts["literal_cases"] += 1

        for literal in ["1i32", "1_000", "0xff", "0o7", "0b1", "1.0", "1e9", "9" * 200 + "i32", "１", "١", "1é", "1١", "- /* 雪 */ 1.0"]:
            failure(f"fn main() -> i32 {{ return {literal}; }}", "E0101", "parse", literal)
            counts["unsupported_cases"] += 1

        values = [-(2**31), -(2**31) + 1, -1, 0, 1, 2**31 - 2, 2**31 - 1]
        for _ in range(80):
            selected = [rng.choice(values) for _ in range(4)]
            for a in (False, True):
                for b in (False, True):
                    # Expected selection is a Python table, independent of printed source CFG.
                    expected = selected[2 * int(a) + int(b)]
                    main_source = f"fn main() -> i32 {{ let n = choose({str(a).lower()}, {str(b).lower()}); return copy(n); }}"
                    helpers = f"""fn copy(value: i32) -> i32 {{ let saved: i32 = (value); return saved; }}
fn choose(a: bool, b: bool) -> i32 {{
    if a {{ if b {{ return copy({selected[3]}); }} return {selected[2]}; }}
    else {{ if b {{ return {selected[1]}; }} return copy({selected[0]}); }}
}}
fn decoy() -> i32 {{ return 123; }}"""
                    source = (main_source + helpers) if rng.choice([True, False]) else (helpers + main_source)
                    success(source, expected, functions=4)
                    counts["source_cases"] += 1
    print(json.dumps({"status": "pass", **counts}, sort_keys=True))


if __name__ == "__main__":
    main()
