"""Execution/identity checks only; this module contains no language oracle."""
from collections import Counter
import hashlib
import json
from pathlib import Path, PurePosixPath
import re


class ProtocolError(ValueError):
    pass


def require(condition, message):
    if not condition:
        raise ProtocolError(message)


def digest(data):
    return hashlib.sha256(data).hexdigest()


def unchanged_file(path, expected_sha256):
    require(digest(Path(path).read_bytes()) == expected_sha256, "changed captured manifest or input")


def relative_file(root, name):
    """Resolve a manifest file without accepting traversal or symlink aliases."""
    require(isinstance(name, str) and bool(name), "empty manifest path")
    path = PurePosixPath(name)
    require(not path.is_absolute() and str(path) == name, "noncanonical manifest path")
    require(all(part not in (".", "..") for part in path.parts), "manifest traversal")
    target = Path(root)
    for part in path.parts:
        target = target / part
        require(not target.is_symlink(), "manifest symlink")
    require(target.is_file(), "missing manifest file: " + name)
    return target


def check_files(root, entries):
    require(isinstance(entries, list) and bool(entries), "empty file manifest")
    unique_ids([entry["path"] for entry in entries])
    for entry in entries:
        data = relative_file(root, entry["path"]).read_bytes()
        require(len(data) == entry["bytes"], "file length changed: " + entry["path"])
        require(digest(data) == entry["sha256"], "file hash changed: " + entry["path"])


def unique_ids(ids):
    require(bool(ids), "zero executions/identities")
    require(all(isinstance(name, str) and name for name in ids), "invalid identity")
    require(len(ids) == len(set(ids)), "duplicate identity")
    return set(ids)


def exact_ids(actual, expected):
    require(unique_ids(actual) == unique_ids(expected), "missing or unexpected identity")


def jsonl(path):
    lines = Path(path).read_text().splitlines()
    require(bool(lines) and all(line.strip() for line in lines), "empty observation stream")
    return [json.loads(line) for line in lines]


def observations(path, expected_ids):
    rows = jsonl(path)
    exact_ids([row["case"] for row in rows], expected_ids)
    require(all("normalization_error" not in row for row in rows), "normalization failed")
    return rows


def comparison(report, profile, expected_ids):
    require(report["profile"] == profile, "wrong comparison profile")
    exact_ids([row["case"] for row in report["results"]], expected_ids)
    require(report["cases"] == len(expected_ids), "wrong comparison count")
    require(report["counts"] == {"match": len(expected_ids)}, "nonmatching comparison")
    require(all(row["status"] == "match" and not row.get("failures")
                and not row.get("unavailable") for row in report["results"]),
            "incomplete comparison result")


def rust_listing(stdout, expected_names):
    names = re.findall(r"^([^\n]+): test$", stdout, re.MULTILINE)
    exact_ids(names, expected_names)
    totals = re.findall(r"^(\d+) tests?, (\d+) benchmarks?$", stdout, re.MULTILINE)
    require(totals == [(str(len(expected_names)), "0")], "missing Rust listing terminal")


def rust_success(stdout, expected_names):
    tests = re.findall(r"^test ([^\n]+) \.\.\. ([^\n]+)$", stdout, re.MULTILINE)
    exact_ids([name for name, _ in tests], expected_names)
    require(all(status == "ok" for _, status in tests), "Rust test did not succeed")
    starts = re.findall(r"^running (\d+) tests?$", stdout, re.MULTILINE)
    require(starts == [str(len(expected_names))], "wrong Rust start count")
    totals = re.findall(
        r"^test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored; "
        r"(\d+) measured; (\d+) filtered out; finished in [^\n]+$", stdout, re.MULTILINE)
    require(len(totals) == 1 and totals[0][:4] == (str(len(expected_names)), "0", "0", "0"),
            "missing or invalid Rust terminal success")
    return {"passed": len(expected_names), "failed": 0, "ignored": 0,
            "filtered": int(totals[0][4])}


BINDINGS = ("invocation_id", "source_inputs_sha256", "package_inputs_sha256",
            "queue_sha256", "queue_tsv_sha256", "profile", "binary_sha256")


def run_receipt(receipt, expected_binding, root, expected_artifacts):
    require(receipt.get("schema") == 1, "wrong receipt schema")
    require(receipt.get("status") == "passed" and receipt.get("exit_status") == 0,
            "receipt has no terminal success")
    for field in BINDINGS:
        require(bool(expected_binding.get(field)) and receipt.get(field) == expected_binding[field],
                "stale or wrong binding: " + field)
    exact_ids([entry["path"] for entry in receipt["artifacts"]], expected_artifacts)
    check_files(root, receipt["artifacts"])


def both_profiles(receipts):
    require(Counter(receipt["profile"] for receipt in receipts) == {"debug": 1, "release": 1},
            "both unique profiles are required")
