"""Inactive exact two-argument CI workflow transport.

The unchanged oldest inventory and its three historical-to-M adaptations remain
separate from the new M-to-C source-manifest argument update. No commands execute.
"""
import copy
import json
import re

from adapters import need, sha, verify
import ci_inventory as retained
import published_transition as source

M_WORKFLOW_SHA = "81dfdfe82d9e477c4b6535a10967fc8d6861bcbcea5f90d341c694797ced3269"
M_WORKFLOW_BYTES = 67408
M_DISPOSITIONS_SHA = "b69f0585ffd35d889023e3bcefe668db142b0d3adbd7a18cfa3823c88ffa96f0"
M_DISPOSITIONS_BYTES = 191055
BYTE_SOURCE_SHA = "402db5018af489c30b2a57ed3ef558c055013af2b727a3ad0eb39ffc42125efa"
M_SOURCE_SHA = source.PREDECESSOR_SHA
ARGUMENT_LINES = (755, 768)
COMMAND_IDS = ("ci.yml:L751", "ci.yml:L764")
UNIT1_OLD = b"tests/fixtures/typed_project_unit1_independent/run.py --repo . --profile both"
UNIT1_CURRENT = b"tests/qualification/lexer_reservation_current/unit1_run_current.py --repo . --profile both"


def _digest(value):
    need(type(value) is str and re.fullmatch("[0-9a-f]{64}", value) is not None,
         "invalid fixed source-manifest digest")
    return value.encode("ascii")


def same(actual, expected, message):
    """Preserve recursive type-strict equality for complete sealed plans."""
    need(type(actual) is type(expected), message)
    if isinstance(expected, dict):
        need(actual.keys() == expected.keys(), message)
        for key in expected:
            same(actual[key], expected[key], message)
    elif isinstance(expected, list):
        need(len(actual) == len(expected), message)
        for left, right in zip(actual, expected):
            same(left, right, message)
    else:
        need(actual == expected, message)


def descriptor(value, path):
    need(type(value) is dict and set(value) == {"path", "bytes", "sha256"}
         and type(value["path"]) is str and value["path"] == path
         and type(value["bytes"]) is int and value["bytes"] > 0,
         "invalid fixed complete artifact descriptor")
    _digest(value["sha256"])


def unique(pairs):
    value = {}
    for key, item in pairs:
        need(key not in value, "duplicate current disposition key")
        value[key] = item
    return value


def workflow_successor(oldest, middle, current_source_sha):
    """Derive bytes without accepting an arbitrary workflow modification."""
    digest = _digest(current_source_sha)
    need(current_source_sha not in (M_SOURCE_SHA, BYTE_SOURCE_SHA),
         "wrong or repeated workflow source stage")
    need(sha(oldest) == retained.WORKFLOW_SHA,
         "oldest approved workflow changed")
    need(len(middle) == M_WORKFLOW_BYTES and sha(middle) == M_WORKFLOW_SHA,
         "published-main workflow changed")
    need(oldest.count(BYTE_SOURCE_SHA.encode()) == 2
         and oldest.count(UNIT1_OLD) == 1,
         "changed historical workflow adaptation scope")
    recovered_middle = oldest.replace(BYTE_SOURCE_SHA.encode(), M_SOURCE_SHA.encode())
    recovered_middle = recovered_middle.replace(UNIT1_OLD, UNIT1_CURRENT, 1)
    need(recovered_middle == middle,
         "historical three-adaptation workflow mapping changed")
    lines = middle.splitlines(keepends=True)
    old_argument = b"--source-manifest-sha256 " + M_SOURCE_SHA.encode()
    new_argument = b"--source-manifest-sha256 " + digest
    found = tuple(i + 1 for i, line in enumerate(lines) if old_argument in line)
    need(found == ARGUMENT_LINES and middle.count(old_argument) == 2
         and middle.count(M_SOURCE_SHA.encode()) == 2,
         "missing, duplicated or relocated current source argument")
    for number in ARGUMENT_LINES:
        line = lines[number - 1]
        need(line.count(old_argument) == 1, "duplicate source argument line")
        lines[number - 1] = line.replace(old_argument, new_argument, 1)
    result = b"".join(lines)
    need(result.count(new_argument) == 2
         and result.replace(new_argument, old_argument) == middle,
         "workflow inverse failed exact published-main recovery")
    return result


def disposition_successor(audit, oldest, middle, old_raw, source_descriptor,
                          workflow_descriptor):
    """Retain every original disposition and bind only the genuine new identities."""
    need(len(old_raw) == M_DISPOSITIONS_BYTES and sha(old_raw) == M_DISPOSITIONS_SHA,
         "published-main disposition record changed")
    old = json.loads(old_raw)
    original = retained.inventory(audit, oldest)
    retained.validate_plan(original, old["entries"])
    need(old["entry_count"] == len(original) == 173
         and old["current_source_manifest_sha256"] == M_SOURCE_SHA
         and old["compiler_head"] == source.HISTORICAL_SOURCE["head"]
         and old["compiler_tree"] == source.HISTORICAL_SOURCE["tree"],
         "wrong retained source disposition provenance")
    descriptor(source_descriptor, source.CURRENT_NAME)
    current_sha = source_descriptor["sha256"]
    workflow = workflow_successor(oldest, middle, current_sha)
    descriptor(workflow_descriptor, ".github/workflows/ci.yml")
    verify(workflow, workflow_descriptor, workflow_descriptor["path"])
    expected = copy.deepcopy(old)
    expected.update({
        "schema": "oxid-published-main-direct-ci-dispositions-v1",
        "status": "source-only-successor-pending-independent-execution",
        "compiler_head": source.SOURCE_CHECKPOINT["head"],
        "compiler_tree": source.SOURCE_CHECKPOINT["tree"],
        "current_source_manifest_sha256": current_sha,
        "published_main": copy.deepcopy(source.PUBLISHED_MAIN),
        "source_checkpoint": copy.deepcopy(source.SOURCE_CHECKPOINT),
        "published_main_workflow": {
            "path": ".github/workflows/ci.yml", "bytes": M_WORKFLOW_BYTES,
            "sha256": M_WORKFLOW_SHA,
        },
        "published_main_dispositions": {
            "path": "direct-consumer-dispositions.json",
            "bytes": M_DISPOSITIONS_BYTES, "sha256": M_DISPOSITIONS_SHA,
        },
        "current_workflow": copy.deepcopy(workflow_descriptor),
        "workflow_source_argument_lines": list(ARGUMENT_LINES),
    })
    changed = []
    for row, prior, original_row in zip(expected["entries"], old["entries"], original):
        need({key: prior[key] for key in original_row} == original_row,
             "retained original command association changed")
        need(prior["active_command_sha256"] == sha(prior["active_command"].encode()),
             "retained active command hash changed")
        if row["id"] in COMMAND_IDS:
            before = "--source-manifest-sha256 " + M_SOURCE_SHA
            after = "--source-manifest-sha256 " + current_sha
            need(row["active_command"].count(before) == 1,
                 "wrong complete current native command")
            row["active_command"] = row["active_command"].replace(before, after, 1)
            row["active_command_sha256"] = sha(row["active_command"].encode())
            changed.append(row["id"])
        else:
            need(M_SOURCE_SHA not in row["active_command"],
                 "unexpected additional current source argument")
    need(tuple(changed) == COMMAND_IDS, "wrong changed-command roster")
    retained.validate_plan(original, expected["entries"])
    need(expected["retained_workflow_scope"] == old["retained_workflow_scope"]
         and expected["disposition_counts"] == old["disposition_counts"]
         and sum(expected["disposition_counts"].values()) == 173,
         "changed host/native/profile/failure or consumer obligations")
    return expected


def validate(audit, oldest, middle, current, old_raw, current_raw,
             source_descriptor, workflow_descriptor, dispositions_descriptor):
    """Authenticate complete bytes and exact metadata; return no execution claim."""
    expected = disposition_successor(
        audit, oldest, middle, old_raw, source_descriptor, workflow_descriptor)
    need(current == workflow_successor(oldest, middle, source_descriptor["sha256"]),
         "unapproved current workflow byte change")
    verify(current, workflow_descriptor, ".github/workflows/ci.yml")
    descriptor(dispositions_descriptor, "published-direct-consumer-dispositions-v1.json")
    verify(current_raw, dispositions_descriptor, dispositions_descriptor["path"])
    same(json.loads(current_raw, object_pairs_hook=unique), expected,
         "changed complete current disposition or command association")
    return {"entry_count": 173, "changed_commands": list(COMMAND_IDS),
            "argument_lines": list(ARGUMENT_LINES),
            "source_only": True, "execution_qualified": False}

