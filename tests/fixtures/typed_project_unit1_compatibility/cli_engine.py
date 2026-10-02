"""Public CLI observation and preservation runner. Never regenerates expectations."""
import argparse
import hashlib
import json
import os
import platform
import subprocess
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parent


def digest(data):
    return hashlib.sha256(data).hexdigest()


def loc(data, offset):
    before = data[:offset].decode("utf-8")
    return before.count("\n") + 1, len(before.rsplit("\n", 1)[-1]) + 1


def check_locations(record, data, path):
    errors = []
    spans = ([record["primary"]] if record.get("primary") else [])
    spans += [s["span"] for s in record.get("secondary", [])]
    for s in spans:
        try:
            assert 0 <= s["start"] <= s["end"] <= len(data)
            expected = (0, path, *loc(data, s["start"]), *loc(data, s["end"]))
            actual = (s["file_id"], s["path"], s["line"], s["column"], s["end_line"], s["end_column"])
            if actual != expected:
                errors.append(f"independent byte/Unicode location mismatch: {actual} != {expected}")
        except (AssertionError, UnicodeDecodeError) as exc:
            errors.append(f"invalid source span {s}: {exc}")
    return errors


def semantic_checks(case, operation, completed):
    expected = case["expected"][operation]
    errors = []
    if completed.returncode != expected["exit"]:
        errors.append(f"exit {completed.returncode} != {expected['exit']}")
    try:
        records = [json.loads(line) for line in completed.stdout.splitlines()]
    except Exception as exc:
        return [f"invalid NDJSON: {exc}"], None
    diagnostics = [r for r in records if r.get("kind") == "diagnostic"]
    summaries = [r for r in records if r.get("kind") == f"{operation}-summary"]
    if len(summaries) != 1 or records[-1:] != summaries or len(records) != len(diagnostics) + 1:
        errors.append("expected exactly one terminal operation summary and only diagnostic records before it")
    if completed.stderr:
        errors.append("JSON mode emitted stderr")
    for r in records:
        if type(r.get("schema_version")) is not int or r["schema_version"] != 1 or r.get("edition") != "typed-preview":
            errors.append("incorrect schema/edition")
    actual_codes = [[r["code"], r["stage"]] for r in diagnostics]
    if actual_codes != expected.get("diagnostics", []):
        errors.append(f"ordered code/stage {actual_codes} != {expected.get('diagnostics', [])}")
    if summaries:
        summary = summaries[0]
        if type(summary.get("success")) is not bool or summary["success"] != (expected["exit"] == 0):
            errors.append("summary success mismatch")
        if summary.get("errors") != len(diagnostics):
            errors.append("summary diagnostic count mismatch")
        key = "functions" if operation == "check" else "result"
        if summary.get(key) != expected.get(key):
            errors.append(f"{key} {summary.get(key)} != {expected.get(key)}")
    data = (ROOT / case["path"]).read_bytes()
    for r in diagnostics:
        errors += check_locations(r, data, case["path"])
    # Primary/secondary landmarks are authored from source grammar, independent
    # of emitted diagnostics. Span/line validation above covers every label.
    name = case["name"]
    landmarks = []
    if name in ("double_colon_return", "spaced_colons"):
        i = data.index(b":")
        landmarks = [(i, i + 1, None)]
    elif name == "colon_token_one_over":
        landmarks = [(100000, 100001, None)]
    elif name == "number_token_one_over":
        i = data.index(b"0")
        landmarks = [(i, i + 65537, None)]
    elif name == "unicode_runtime_overflow" and operation == "run":
        i = data.index(b"+")
        landmarks = [(i, i + 1, None)]
    elif name in ("scalar_duplicate_before_body", "unknown_signature_plus_duplicate", "origins_unicode_crlf", "origins_ascii_crlf"):
        token = b"repeat" if name == "scalar_duplicate_before_body" else b"same"
        first = data.index(token)
        second = data.index(token, first + len(token))
        landmarks = [(second, second + len(token), (first, first + len(token)))]
    elif name == "owned_declaration_order":
        f0 = data.index(b"same")
        f1 = data.index(b"same", f0 + 4)
        f2 = data.index(b"same", f1 + 4)
        a0 = data.index(b"A {")
        a1 = data.index(b"A {", a0 + 1)
        landmarks = [(f1, f1 + 4, (f0, f0 + 4)), (a1, a1 + 1, (a0, a0 + 1)), (f2, f2 + 4, (f0, f0 + 4))]
    if landmarks:
        if len(diagnostics) != len(landmarks):
            errors.append("landmark diagnostic count mismatch")
        for r, (start, end, secondary) in zip(diagnostics, landmarks):
            p = r.get("primary") or {}
            if (p.get("start"), p.get("end")) != (start, end):
                errors.append(f"primary does not cover independent landmark {(start, end)}")
            if secondary is not None:
                labels = r.get("secondary", [])
                if len(labels) != 1 or (labels[0]["span"]["start"], labels[0]["span"]["end"]) != secondary:
                    errors.append(f"secondary does not cover independent earlier declaration {secondary}")
    return errors, records


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--debug", type=Path, default=ROOT / "baseline-src/target/debug/oxid")
    ap.add_argument("--release", type=Path, default=ROOT / "baseline-src/target/release/oxid")
    ap.add_argument("--label", default="baseline")
    ap.add_argument("--compare", type=Path)
    ap.add_argument("--original-only", action="store_true")
    args = ap.parse_args()
    manifest = json.loads((ROOT / "suite-intent.json").read_text())
    cases = [c for c in manifest["cases"] if not args.original_only or c["group"] == "OriginalSingleFile"]
    outdir = ROOT / "evidence" / args.label
    outdir.mkdir(parents=True, exist_ok=False)
    env = os.environ.copy()
    env.update(LC_ALL="C", LANG="C", TZ="UTC")
    rows, semantic_failures = [], []
    for profile, binary in (("debug", args.debug), ("release", args.release)):
        binary = binary.resolve()
        binary_hash = digest(binary.read_bytes())
        for case in cases:
            data = (ROOT / case["path"]).read_bytes()
            assert digest(data) == case["sha256"], case["name"]
            for operation in ("check", "run"):
                for fmt in ("json", "text"):
                    argv = [str(binary), operation, "--edition", "typed-preview", "--message-format", fmt, case["path"]]
                    for repeat in (1, 2):
                        stem = f"{case['name']}-{profile}-{operation}-{fmt}-r{repeat}"
                        start = time.monotonic()
                        completed = subprocess.run(argv, cwd=ROOT, env=env, stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=60)
                        elapsed = time.monotonic() - start
                        for stream in ("stdout", "stderr"):
                            (outdir / f"{stem}.{stream}").write_bytes(getattr(completed, stream))
                        failures, records = semantic_checks(case, operation, completed) if fmt == "json" else ([], None)
                        if fmt == "text":
                            if completed.returncode != case["expected"][operation]["exit"]:
                                failures.append("text exit mismatch")
                            if case["name"] == "control_text_origin" and b"\x1b" in completed.stderr:
                                failures.append("raw ESC escaped source renderer")
                            if operation == "run" and case["expected"][operation]["exit"] == 0:
                                result = case["expected"][operation]["result"]
                                wanted = ("()" if result["type"] == "unit" else str(result["value"]).lower()).encode() + b"\n"
                                if completed.stdout != wanted or completed.stderr:
                                    failures.append("text result differs from independent arithmetic result")
                        row = dict(case=case["name"], group=case["group"], route=case["route"], profile=profile,
                                   operation=operation, format=fmt, repeat=repeat, binary=str(binary), binary_sha256=binary_hash,
                                   source=case["path"], source_sha256=case["sha256"], argv=argv,
                                   argv_sha256=digest(json.dumps(argv, separators=(",", ":")).encode()), cwd=str(ROOT),
                                   exit_code=completed.returncode, elapsed_seconds=elapsed,
                                   stdout_file=f"{stem}.stdout", stderr_file=f"{stem}.stderr",
                                   stdout_sha256=digest(completed.stdout), stderr_sha256=digest(completed.stderr),
                                   semantic_failures=failures, **({"records": records} if records is not None else {}))
                        rows.append(row)
                        semantic_failures.extend(dict(case=case["name"], profile=profile, operation=operation, format=fmt, repeat=repeat, error=e) for e in failures)
            print(f"{profile}: {case['name']}", flush=True)
    key = lambda r: (r["case"], r["operation"], r["format"])
    signature = lambda r: (r["exit_code"], r["stdout_sha256"], r["stderr_sha256"])
    grouped = {}
    for r in rows:
        grouped.setdefault(key(r), []).append(r)
    stability = [dict(case=k[0], operation=k[1], format=k[2], observations=len(v), equal=len({signature(r) for r in v}) == 1) for k, v in grouped.items()]
    comparison = []
    if args.compare:
        predecessor = json.loads(args.compare.read_text())
        before = {(*key(r), r["profile"], r["repeat"]): r for r in predecessor["rows"]}
        for r in rows:
            original = before[(*key(r), r["profile"], r["repeat"])]
            comparison.append(dict(case=r["case"], operation=r["operation"], format=r["format"], profile=r["profile"], repeat=r["repeat"], equal=signature(r) == signature(original)))
    result = dict(label=args.label, cases=len(cases), invocations=len(rows), platform=platform.platform(),
                  controlled_environment={k: env[k] for k in ("LC_ALL", "LANG", "TZ")},
                  source_intent_sha256=digest((ROOT / "suite-intent.json").read_bytes()),
                  runner_sha256=digest(Path(__file__).read_bytes()), rows=rows,
                  semantic_failures=semantic_failures, stability=stability, comparison=comparison,
                  repeated_and_profiles_identical=all(s["equal"] for s in stability),
                  preservation_identical=all(c["equal"] for c in comparison) if args.compare else None)
    (outdir / "manifest.json").write_text(json.dumps(result, indent=2) + "\n")
    print(json.dumps({k: result[k] for k in ("cases", "invocations", "semantic_failures", "repeated_and_profiles_identical", "preservation_identical")}, indent=2))
    return int(bool(semantic_failures) or not result["repeated_and_profiles_identical"] or (args.compare is not None and not result["preservation_identical"]))


if __name__ == "__main__":
    raise SystemExit(main())
