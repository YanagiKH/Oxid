#!/usr/bin/env python3
"""Exercise synthetic full-width parser storage; this does not parse a grammar."""
import argparse
import json
from pathlib import Path
import shutil

from verify_bounded_typed_lexer import digest, run

ROOT = Path(__file__).resolve().parents[1]
MEMBERS = ("parser_admission.ox", "parser_banks.ox", "parser_probe_output.ox",
           "buffers.ox", "lexer_core.ox", "keywords.ox")
CASES = (
    ("empty", b"", 0),
    ("max_tokens", b"@" * 128, 0),
    ("function_source", b"pub fn step(x:i32)->i32{let mut y=x;while y<3{y=y+1;}if y==3{return y;}else{return 0;}}fn main()->i32{return step(1);}", 0),
    ("lex_error", b'"', 65),
    ("non_ascii", b"\xff", 64),
    ("capacity", b"x" * 130, 64),
)


def expected():
    columns = [
        [(2 + i % 20) + 64 * (i + 256 * (i + 1)) + 4194304 * (i + 2 if i < 127 else 0)
         for i in range(128)] + [0],
        [(i + 1) + 256 * (128 - i) for i in range(128)] + [0],
        [(i % 2) + 256 * (1 + i % 64) for i in range(128)] + [0],
    ]
    return b"PAB3" + bytes([128]) + b"".join(
        bytes((value // (256 ** plane)) % 256 for value in column)
        for column in columns for plane in range(4))


def verify_decoded(raw):
    if len(raw) != 1553 or raw[:5] != b"PAB3" + bytes([128]):
        return False
    columns = []
    for column in range(3):
        offset = 5 + column * 516
        columns.append([sum(raw[offset + plane * 129 + i] * (256 ** plane)
                            for plane in range(4)) for i in range(129)])
    for i in range(128):
        header, ab, cd = [column[i] for column in columns]
        span = header // 64 % 65536
        actual = [header % 64, span % 256, span // 256, ab % 256, ab // 256,
                  cd % 256, cd // 256, header // 4194304]
        if actual != [2 + i % 20, i, i + 1, i + 1, 128 - i, i % 2,
                      1 + i % 64, i + 2 if i < 127 else 0]:
            return False
    return all(column[128] == 0 for column in columns)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--oxid", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--native", action="store_true")
    args = parser.parse_args()
    compiler = args.oxid.resolve(strict=True)
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=False)
    source = output / "source"
    source.mkdir()
    empty = output / "empty"
    empty.mkdir()
    for name in MEMBERS:
        shutil.copyfile(ROOT / "fixtures/typed-lexer-samples" / name, source / name)
    entry, binary = source / "parser_admission.ox", output / "carrier"
    report = {"passed": False, "scope": "Synthetic carrier/control admission only; no parser implementation",
              "compiler_sha256": digest(compiler), "source_sha256": {n: digest(source / n) for n in MEMBERS},
              "native": args.native, "checks": []}
    def save():
        (output / "report.json").write_text(json.dumps(report, indent=2) + "\n")
    save()
    setup = [("check", [compiler, "check", entry, "--edition", "typed-preview"])]
    if args.native:
        setup.append(("compile", [compiler, "compile", entry, "--edition", "typed-preview",
                                  "--backend", "llvm", "--entry-mode", "process", "--output", binary]))
    for label, command in setup:
        result, remaining = run(output / label, command, b"unread setup", empty)
        if result.returncode or result.stderr or remaining != b"unread setup":
            raise ValueError("carrier setup failed: " + label)
    if args.native:
        if binary.read_bytes()[:4] != b"\x7fELF":
            raise ValueError("expected native ELF")
        report["binary_sha256"] = digest(binary)
    payload = expected()
    (output / "expected.bin").write_bytes(payload)
    for name, data, status in CASES:
        modes = [("reference", [compiler, "run", entry, "--edition", "typed-preview", "--entry-mode", "process"])]
        if args.native:
            modes.append(("native", [binary]))
        for mode, command in modes:
            result, remaining = run(output / (name + "-" + mode), command, data, empty, mode == "native")
            ok = result.returncode == status and not result.stderr and remaining == data[129:]
            ok = ok and result.stdout == (payload if status == 0 else b"")
            if status == 0:
                ok = ok and verify_decoded(result.stdout)
            report["checks"].append({"case": name, "mode": mode, "passed": bool(ok)})
            save()
    report["passed"] = all(row["passed"] for row in report["checks"])
    save()
    return int(not report["passed"])


if __name__ == "__main__":
    raise SystemExit(main())
