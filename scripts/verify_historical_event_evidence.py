#!/usr/bin/env python3
"""Authenticate the retained original GitHub event's source-control evidence.

This verifies archived results; it never runs them or qualifies current source.
Live run/job metadata is verified separately by run_historical_script_tests.py.
"""
import hashlib
import io
import json
import os
from pathlib import Path, PurePosixPath
import re
import stat
import zipfile

ARCHIVE_RELATIVE = Path("docs/evidence/historical-event-9ff8ef9/typed-project-unit2-evidence.zip")
ARCHIVE_BYTES = 2251614
ARCHIVE_SHA256 = "a3395ec1a79a7bb28f1372c1cf40b0745b6583a575d18b0428a1d475c577f88f"
BASELINE = "9ff8ef90a26f003f4616838dfc186fa3bfb6845c"
BASELINE_TREE = "50512c0ff428a802afcb1b834ee11f981811db1f"
RUN = 39226443899
JOB = 117973774217
ARTIFACT = 12180618442
PREFIX = "typed-project-unit2-results/"
STEM = "source-controls-package-json-v1-"
CASE_FILES = (
    "runner.stderr", "runner.stdout", "unit2/runner.stderr", "unit2/runner.stdout",
    "unit2/run/materialize.stderr", "unit2/run/materialize.stdout",
    "unit2/run/python-assertions.stderr", "unit2/run/python-assertions.stdout",
    "unit2/command.json", "unit2/invocation.json", "unit2/prepared.json",
    "unit2/run/assembly.json", "unit2/run/commands.json", "unit2/run/invocation.json",
    "unit2/run/prepared.json", "host-command.json", "tree-summary.json",
)
ORIGINAL_PATHS = ("checkout.json",) + tuple(
    f"case-{number}/{name}" for number in (1, 2, 3) for name in CASE_FILES
) + ("complete.json",)
OLD_INPUTS = ("result.json", "plan.json", "command.json", "resource-seam.json", "runner.stdout", "runner.stderr")
COUNTS = {
    "source_unit": 37, "source_integration": 15, "genuine_view": 14,
    "ci_transport": 20, "original_public_lifecycle": 6,
    "additional_public_lifecycle_pending": 36, "fresh_preparations": 3,
    "preparation_child_launches": 12, "git_metadata_launches": 2,
    "maximum_live_preparation_processes_including_host": 4,
    "compiler_executions": 0, "execution_qualified": False,
}


def require(condition, message):
    if not condition:
        raise ValueError(message)


def digest(raw):
    return hashlib.sha256(raw).hexdigest()


def flat(name):
    return STEM + name.replace("/", "--")


REQUIRED_MEMBERS = frozenset([flat(name) for name in (*ORIGINAL_PATHS, "index.json")] + list(OLD_INPUTS))


def exact_fields(value, expected, label):
    require(type(value) is dict, "invalid " + label)
    for key, item in expected.items():
        require(type(value.get(key)) is type(item) and value[key] == item,
                label + " mismatch: " + key)


def decode(raw):
    def pairs(items):
        result = {}
        for key, value in items:
            require(key not in result, "duplicate historical JSON key")
            result[key] = value
        return result

    def invalid_constant(value):
        raise ValueError("non-finite historical JSON value: " + value)

    return json.loads(raw, object_pairs_hook=pairs, parse_constant=invalid_constant)


def verify_members(members):
    """Validate the exact finite receipt subset, without executing archived code."""
    require(type(members) is dict and set(members) == REQUIRED_MEMBERS, "historical receipt membership differs")
    require(all(type(raw) is bytes and len(raw) <= 1048576 for raw in members.values()),
            "historical member exceeds byte limit")
    index = decode(members[flat("index.json")])
    exact_fields(index, {
        "schema": "oxid-package-json-source-controls-flat-evidence-v1",
        "status": "passed-source-controls-only", "absent_expected_paths": ["error.json"],
        "aggregate_bytes_before_index": 240175, "aggregate_limit_bytes": 33554432,
        "file_limit_including_index": 55, "source_or_execution_qualification_from_exports": False,
        "completion_rule": "require this final index, all exact entries and unchanged old Unit2 inputs",
    }, "historical index")
    entries, old = index.get("entries"), index.get("original_unit2_inputs")
    require(type(entries) is list and len(entries) == 53, "historical index must have all 53 entries")
    require(type(old) is list and len(old) == 6, "historical index must preserve six old inputs")

    def descriptor(row, name):
        require(type(row) is dict and set(row) == {"path", "bytes", "sha256"}, "invalid historical descriptor")
        exact_fields(row, {"path": name, "bytes": len(members[name]), "sha256": digest(members[name])},
                     "historical member " + name)

    for entry, original in zip(entries, ORIGINAL_PATHS):
        require(type(entry) is dict and entry.get("original_path") == original, "historical entry roster differs")
        descriptor(entry.get("flat_file"), flat(original))
    for row, name in zip(old, OLD_INPUTS):
        descriptor(row, name)
    require(sum(len(members[flat(name)]) for name in ORIGINAL_PATHS) == index["aggregate_bytes_before_index"],
            "historical aggregate byte count differs")

    checkout = decode(members[flat("checkout.json")])
    complete = decode(members[flat("complete.json")])
    exact_fields(checkout, {
        "checkout_head": BASELINE, "checkout_tree": BASELINE_TREE,
        "event_name": "schedule", "form": "event-checkout",
        "controls_stage": "accepted-formatter-predecessor", "full_checkpoint_reconstruction": False,
    }, "historical checkout")
    exact_fields(checkout.get("actual_current_source"), {"source_only": True, "execution_qualified": False},
                 "historical source scope")
    exact_fields(complete, {"status": "passed-source-controls-only", **COUNTS}, "historical completion")
    require(json.dumps(complete.get("checkout"), sort_keys=True) == json.dumps(checkout, sort_keys=True),
            "historical checkout and completion disagree")
    exact_fields(complete.get("accepted_fixture_view"), {
        "files": 2724, "logical_bytes": 78507317,
        "scope": "entire predecessor control batch; closed before terminal export",
        "separate_from_nested_view_effect_audit": True,
    }, "historical accepted view")

    result, plan, command = [decode(members[name]) for name in OLD_INPUTS[:3]]
    for receipt in (result, plan):
        exact_fields(receipt, {"schema": "oxid-current-archive-binding-v1", "action": "run-unit2"},
                     "original Unit2 receipt")
    exact_fields(result, {"status": "passed-current-unit2", "semantic_pass": True,
                          "direct_current_execution": True}, "original Unit2 result")
    exact_fields(plan, {"status": "planned", "current_source_members": 376,
                        "repository": "/home/runner/work/Oxid/Oxid"}, "original Unit2 plan")
    require(type(result.get("invocation_id")) is str
            and re.fullmatch("[0-9a-f]{32}", result["invocation_id"]) is not None
            and plan.get("invocation_id") == result["invocation_id"], "original Unit2 invocation differs")
    for key in ("controller_sha256", "current_source_sha256", "adapter_package_sha256"):
        require(type(result.get(key)) is str and re.fullmatch("[0-9a-f]{64}", result[key]) is not None
                and plan.get(key) == result[key], "original Unit2 source binding differs: " + key)
    exact_fields(result, {"plan_sha256": digest(members["plan.json"]),
                          "command_sha256": digest(members["command.json"]),
                          "resource_seam_sha256": digest(members["resource-seam.json"])}, "original Unit2 links")
    exact_fields(command, {"plan_sha256": digest(members["plan.json"]), "exit_status": 0,
                           "cwd": plan["repository"]}, "original Unit2 command")
    for stream in ("stdout", "stderr"):
        descriptor(command.get(stream), "runner." + stream)
    return {
        "status": "verified-original-source-controls-only", "checkout": checkout,
        "counts": dict(COUNTS), "tests": 92, "indexed_entries": len(entries), "old_unit2_inputs": len(old),
        "receipt_members": len(members), "index_sha256": digest(members[flat("index.json")]),
        "complete_sha256": digest(members[flat("complete.json")]),
    }


def verify_archive(path: Path) -> dict:
    """Verify one exact original artifact and return its historical-only receipt."""
    path = Path(path)
    before = path.lstat()
    require(stat.S_ISREG(before.st_mode) and before.st_size == ARCHIVE_BYTES, "wrong historical archive file or size")
    fd = os.open(path, os.O_RDONLY | getattr(os, "O_NOFOLLOW", 0) | getattr(os, "O_NONBLOCK", 0)
                 | getattr(os, "O_BINARY", 0))
    try:
        opened = os.fstat(fd)
        require(stat.S_ISREG(opened.st_mode) and (opened.st_dev, opened.st_ino, opened.st_size)
                == (before.st_dev, before.st_ino, ARCHIVE_BYTES), "historical archive changed before read")
        with os.fdopen(fd, "rb", closefd=False) as stream:
            raw = stream.read(ARCHIVE_BYTES + 1)
    finally:
        os.close(fd)
    require(len(raw) == ARCHIVE_BYTES and digest(raw) == ARCHIVE_SHA256, "historical archive digest differs")
    with zipfile.ZipFile(io.BytesIO(raw)) as archive:
        infos = archive.infolist()
        names = [info.filename for info in infos]
        require(len(infos) == len(set(names)) == 146 and sum(info.file_size for info in infos) == 4866606,
                "historical archive roster or byte envelope differs")
        for info in infos:
            name = PurePosixPath(info.filename)
            require(not name.is_absolute() and str(name) == info.filename and ".." not in name.parts
                    and "\\" not in info.filename and not info.is_dir()
                    and stat.S_ISREG(info.external_attr >> 16)
                    and 0 <= info.file_size <= 1048576 and info.compress_size <= ARCHIVE_BYTES
                    and info.compress_type == zipfile.ZIP_DEFLATED and not (info.flag_bits & 1),
                    "unsafe historical archive member")
        controls = {name[len(PREFIX):] for name in names if name.startswith(PREFIX + STEM)}
        require(controls == {flat(name) for name in (*ORIGINAL_PATHS, "index.json")},
                "historical controls namespace differs")
        require({PREFIX + name for name in REQUIRED_MEMBERS} <= set(names), "missing historical receipt")
        result = verify_members({name: archive.read(PREFIX + name) for name in REQUIRED_MEMBERS})
    return {
        "schema": "oxid-historical-event-evidence-v1", **result,
        "scope": "original historical event source controls; not current-source or release qualification",
        "run_id": RUN, "run_attempt": 1, "job_id": JOB, "artifact_id": ARTIFACT,
        "archive": {"path": str(path), "bytes": len(raw), "sha256": ARCHIVE_SHA256,
                    "members": len(infos), "uncompressed_bytes": sum(info.file_size for info in infos)},
    }
