#!/usr/bin/env python3
"""Independent Python-bigint oracle for checked i32 arithmetic and profile parity.

Usage: python3 scripts/verify_i32_arithmetic.py target/debug/oxid target/release/oxid
One or more binaries are accepted. Every binary must match the same source model;
with two binaries all JSON records and exit codes must also agree exactly.
No production parser, compiler, VM, Rust overflow operator or f64 oracle is used.
"""
import json
import os
from pathlib import Path
import random
import subprocess
import sys
import tempfile

MIN = -(2**31)
MAX = 2**31 - 1


def leaf(value, call=False):
    return (f"id({value})" if call else str(value), value, None)


def binary(left, op, right):
    """Render an independent expression tree and evaluate in source order."""
    lhs, lv, le = left
    rhs, rv, re = right
    text = f"({lhs} {op} {rhs})"
    if le is not None:
        return text, None, 1 + le
    if re is not None:
        return text, None, len(lhs) + 4 + re
    # Python integer operators are arbitrary precision; range check every node.
    value = {"+": lambda: lv + rv, "-": lambda: lv - rv, "*": lambda: lv * rv}[op]()
    return (text, value, None) if MIN <= value <= MAX else (text, None, len(lhs) + 2)


def corpus():
    values = [MIN, MIN + 1, -46341, -46340, -2, -1, 0, 1, 2, 46340, 46341, MAX - 1, MAX]
    for left in values:
        for right in values:
            for op in "+-*":
                yield binary(leaf(left), op, leaf(right))
    rng = random.Random(0x132ADD)
    for _ in range(200):
        yield binary(leaf(rng.randint(MIN, MAX), True), rng.choice("+-*"), leaf(rng.randint(MIN, MAX), True))

    def tree(depth):
        if depth == 0 or rng.randrange(4) == 0:
            return leaf(rng.choice(values + [rng.randint(-1000, 1000)]), bool(rng.randrange(2)))
        return binary(tree(depth - 1), rng.choice("+-*"), tree(depth - 1))

    for _ in range(300):
        yield tree(4)
    for expr, expected in [
        ("1 + 2 * 3", 7), ("(1 + 2) * 3", 9), ("20 - 4 - 3", 13),
        ("20 - (4 - 3)", 19), ("2*3*4-5+6", 25), ("1--2", 3),
        ("-2147483648- -2147483648", 0), ("- /* 雪 */ 1 + 2", 1),
    ]:
        yield expr, expected, None


def main():
    if len(sys.argv) < 2:
        raise SystemExit(__doc__)
    binaries = [str(Path(arg).resolve()) for arg in sys.argv[1:]]
    count = {"source_cases": 0, "successful_results": 0, "overflow_results": 0, "invocations": 0, "binaries": len(binaries)}
    with tempfile.TemporaryDirectory(prefix="oxid-arithmetic-oracle-") as directory:
        root = Path(directory)
        env = dict(os.environ, OXID_CACHE_DIR=str(root / "cache"))
        env.pop("OXID_PATH", None)

        def invoke(source, operation, expected, error=None):
            (root / "input.ox").write_bytes(source.encode())
            previous = None
            for executable in binaries:
                result = subprocess.run(
                    [executable, operation, "input.ox", "--edition=typed-preview", "--message-format=json"],
                    cwd=root, env=env, input=b"", capture_output=True, timeout=10,
                )
                count["invocations"] += 1
                assert not result.stderr, (source, result.stderr)
                records = [json.loads(line) for line in result.stdout.splitlines()]
                observed = result.returncode, records
                if previous is not None:
                    assert observed == previous, (source, "profile mismatch", previous, observed)
                previous = observed
                summary = records[-1]
                assert summary["kind"] == f"{operation}-summary", records
                for record in records:
                    assert record["schema_version"] == 1 and record["edition"] == "typed-preview"
                if error is None:
                    assert result.returncode == 0 and len(records) == 1, (source, records)
                    assert summary["success"] and summary["errors"] == 0
                    if operation == "run":
                        assert summary["result"] == {"type": "i32", "value": expected}, (source, expected, records)
                        assert type(summary["result"]["value"]) is int
                else:
                    assert result.returncode == 1 and len(records) == 2, (source, records)
                    assert not summary["success"] and summary["errors"] == 1 and summary["result"] is None
                    diagnostic = records[0]
                    assert diagnostic["code"] == "E0604" and diagnostic["stage"] == "oir-run", (source, records)
                    primary = diagnostic["primary"]
                    start_byte = len(source[:error].encode())
                    assert (primary["start"], primary["end"]) == (start_byte, start_byte + 1), (source, error, records)
                    assert primary["line"] == source[:error].count("\n") + 1
                    assert primary["column"] == len(source[:error].rsplit("\n", 1)[-1]) + 1
                assert len(list(root.iterdir())) == 1, "unexpected generated/cache file"

        for expression, value, error in corpus():
            prefix = "// 雪\r\nfn id(x: i32) -> i32 { return x; } fn main() -> i32 { return "
            source = prefix + expression + "; }"
            invoke(source, "check", None)
            invoke(source, "run", value, len(prefix) + error if error is not None else None)
            count["source_cases"] += 1
            count["overflow_results" if error is not None else "successful_results"] += 1

        # Only the chosen source branch executes, but its helper calls/arithmetic
        # still follow the same independent model. The other arm always overflows.
        for chosen in [True, False]:
            for value in [MIN, -1, 0, 1, MAX]:
                expression = f"id({value}) + 0"
                arms = [f"return {expression};", "return 2147483647 + 1;"]
                if not chosen:
                    arms.reverse()
                source = f"fn id(x: i32) -> i32 {{ return x; }} fn main() -> i32 {{ if {str(chosen).lower()} {{ {arms[0]} }} else {{ {arms[1]} }} }}"
                invoke(source, "check", None)
                invoke(source, "run", value)
                count["source_cases"] += 1
                count["successful_results"] += 1
    print(json.dumps(count, sort_keys=True))


if __name__ == "__main__":
    main()
