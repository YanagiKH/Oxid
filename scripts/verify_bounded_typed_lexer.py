#!/usr/bin/env python3
"""Compare the bounded Oxid ASCII lexer with an independently built real lexer."""
import argparse
import hashlib
import json
from pathlib import Path
import shutil
import subprocess

ROOT = Path(__file__).resolve().parents[1]
MESSAGES = {1: "unterminated string literal", 2: "unterminated block comment"}


def decode(raw, source):
    if len(source) > 128 or not source.isascii():
        raise ValueError("source outside bounded ASCII domain")
    if len(raw) != 395 or raw[:4] != b"OXL1":
        raise ValueError("invalid transcript extent or magic")
    tag, count, start, end = raw[4:8]
    rows = list(zip(raw[8:137], raw[137:266], raw[266:395]))
    if tag:
        if tag not in MESSAGES or count or not 0 <= start < end == len(source):
            raise ValueError("invalid diagnostic header")
        opener = b'"' if tag == 1 else b"/*"
        if not source[start:].startswith(opener):
            raise ValueError("diagnostic opener does not match source")
        if any(any(row) for row in rows):
            raise ValueError("diagnostic retained token prefix")
        return {"diagnostic": {"tag": tag, "start": start, "end": end}}
    if start or end or not 1 <= count <= 129:
        raise ValueError("invalid success header")
    if any(any(row) for row in rows[count:]):
        raise ValueError("nonzero unused token row")
    prior = 0
    for index, (kind, lo, hi) in enumerate(rows[:count]):
        if lo != prior or not lo <= hi <= len(source):
            raise ValueError("noncontiguous source coverage")
        if index == count - 1:
            if (kind, lo, hi) != (47, len(source), len(source)):
                raise ValueError("invalid final EOF")
        elif not 1 <= kind <= 46 or lo == hi:
            raise ValueError("invalid used token")
        prior = hi
    return {"tokens": [list(row) for row in rows[:count]]}


def canonical(raw):
    facts = json.loads(raw)
    if facts["status"] == "ok":
        if any(t["file_id"] != 0 for t in facts["tokens"]):
            raise ValueError("unexpected canonical source ID")
        return {"tokens": [[t["id"], t["start"], t["end"]] for t in facts["tokens"]]}
    d = facts["diagnostic"]
    if (facts["status"] != "diagnostic" or d["code"] != "E0100" or d["stage"] != "lex"
            or d["severity"] != "error" or d["secondary"] or d["notes"]
            or d["primary"]["file_id"] != 0 or d["primary"]["path"] != "stdin.ox"):
        raise ValueError("unexpected canonical diagnostic")
    tags = [tag for tag, message in MESSAGES.items() if message == d["message"]]
    if len(tags) != 1:
        raise ValueError("unknown canonical lexical diagnostic")
    return {"diagnostic": {"tag": tags[0], "start": d["primary"]["start"], "end": d["primary"]["end"]}}


def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def run(directory, command, data, cwd, clear=False):
    directory.mkdir()
    (directory / "input.bin").write_bytes(data)
    with (directory / "input.bin").open("rb") as stream:
        result = subprocess.run([str(x) for x in command], stdin=stream, stdout=subprocess.PIPE,
                                stderr=subprocess.PIPE, cwd=cwd, env={} if clear else None,
                                timeout=120, check=False)
        remaining = stream.read()
    (directory / "stdout").write_bytes(result.stdout)
    (directory / "stderr").write_bytes(result.stderr)
    receipt = {"command": [str(x) for x in command], "status": result.returncode,
               "input_sha256": digest(directory / "input.bin"), "remaining_hex": remaining.hex(),
               "cleared_environment": clear, "cwd": str(cwd)}
    (directory / "receipt.json").write_text(json.dumps(receipt, indent=2) + "\n")
    return result, remaining


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--oxid", type=Path, required=True)
    parser.add_argument("--observer", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--native", action="store_true")
    args = parser.parse_args()
    compiler, observer = args.oxid.resolve(strict=True), args.observer.resolve(strict=True)
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=False)
    empty = output / "source-free"
    empty.mkdir()
    sources = output / "source"
    sources.mkdir()
    members = ("main.ox", "tape.ox", "transcript.ox", "lexer.ox", "keywords.ox", "lexer_core.ox", "buffers.ox")
    for name in members:
        shutil.copyfile(ROOT / "fixtures/typed-lexer-samples" / name, sources / name)
    entry, binary = sources / "main.ox", output / "lexer"
    report = {"passed": False, "compiler_sha256": digest(compiler),
              "observer_sha256": digest(observer), "source_sha256": {n: digest(sources / n) for n in members},
              "native": args.native, "checks": [],
              "execution_scope": "One ELF, empty cwd, cleared environment, copied source renamed before native runs; no filesystem sandbox claim"}
    def save():
        (output / "report.json").write_text(json.dumps(report, indent=2) + "\n")
    save()
    setup = [("check", [compiler, "check", entry, "--edition", "typed-preview"])]
    if args.native:
        setup.append(("compile", [compiler, "compile", entry, "--edition", "typed-preview", "--backend", "llvm", "--entry-mode", "process", "--output", binary]))
    for name, command in setup:
        result, remaining = run(output / name, command, b"unread setup input", empty)
        if result.returncode or result.stderr or remaining != b"unread setup input":
            raise ValueError("setup failed: " + name)
    if args.native:
        if binary.read_bytes()[:4] != b"\x7fELF":
            raise ValueError("expected ELF")
        report["binary_sha256"] = digest(binary)
    authored = json.loads((ROOT / "tests/fixtures/bounded_typed_lexer/cases.json").read_text())
    cases = authored if isinstance(authored, list) else authored["cases"]
    cases += [{"name": "ascii-" + str(i), "input_hex": bytes([i]).hex()} for i in range(128)]
    words = "fn struct mod use pub let mut return break continue if while else true false import macro macro_rules const for loop match async await move ref unsafe extern enum trait impl type null and or as i32 bool".split()
    cases += [{"name": "keyword-" + w, "input_hex": (w + " " + w + "x").encode().hex()} for w in words]
    cases += [{"name": n, "input_hex": d.hex(), "refused": True} for n, d in
              (("nonascii", b"\xff"), ("nonascii-after-error", b'"\xff'),
               ("capacity129", b"x" * 129), ("overread130", b"x" * 130))]
    failed = False
    for case in cases:
        d = output / case["name"]
        d.mkdir()
        data = bytes.fromhex(case["input_hex"])
        expected = None
        if not case.get("refused"):
            result, remaining = run(d / "canonical", [observer], data, empty, True)
            if result.returncode or result.stderr or remaining:
                raise ValueError("canonical observer failed")
            expected = canonical(result.stdout)
            authored_expectation = {k: case[k] for k in ("tokens", "diagnostic") if k in case}
            if authored_expectation and expected != authored_expectation:
                raise ValueError("authored oracle mismatch: " + case["name"])
        modes = [("reference", [compiler, "run", entry, "--edition", "typed-preview", "--entry-mode", "process"])]
        if args.native:
            modes.append(("native", [binary]))
        for mode, command in modes:
            hidden = output / "retained-source"
            if mode == "native":
                sources.rename(hidden)
            try:
                result, remaining = run(d / mode, command, data, empty, mode == "native")
            finally:
                if mode == "native":
                    hidden.rename(sources)
            ok = (result.returncode == (64 if case.get("refused") else 0)
                  and not result.stderr and remaining == data[129:])
            if case.get("refused"):
                ok = ok and not result.stdout
            elif ok:
                ok = decode(result.stdout, data) == expected
            report["checks"].append({"case": case["name"], "mode": mode, "passed": ok})
            failed |= not ok
            save()
    report["passed"] = not failed
    save()
    return int(failed)


if __name__ == "__main__":
    raise SystemExit(main())
