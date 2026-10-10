"""Read-only complete approved direct-CI inventory; no command execution."""
import re
from adapters import binding, need, sha

AUDIT_SHA = "8e5791d93dadfdf41f42c266ab9f1a2921317d6f60f0a9fd7bd6d0396a712caa"
WORKFLOW_SHA = "7ae94a3f70dd543a28b780912d7ca48fced54d39e0ba0cfdcd2152b5d5b10c57"


def inventory(audit, workflow):
    need(sha(audit) == AUDIT_SHA and sha(workflow) == WORKFLOW_SHA,
         "approved complete CI inventory/checkpoint changed")
    rows, group = [], None
    for line in audit.decode().splitlines():
        if line.startswith("### "):
            group = line[4:]
        match = re.fullmatch(r"- L([0-9]+): `(.*)`", line)
        if match:
            number, command = int(match[1]), match[2]
            rows.append({"id": f"ci.yml:L{number}", "job_step": group,
                         "line": number, "command": command,
                         "command_sha256": sha(command.encode())})
    need(len(rows) == len({row["id"] for row in rows}) == 173,
         "missing or duplicated approved direct CI entry")
    return rows


def validate_plan(inventory_rows, planned):
    """Each direct entry needs a declared execution/preservation disposition.

    A historical-source gate must continue authenticating through the new outer
    inverse; this does not turn its historical execution into current semantics.
    A blocked private boundary remains blocked rather than silently waived.
    """
    need([row["id"] for row in planned] == [row["id"] for row in inventory_rows],
         "CI coverage plan omitted, reordered, or duplicated a direct consumer")
    allowed = {"current-execution", "historical-execution-through-outer-inverse",
               "evidence-preservation", "repository-or-artifact-check",
               "blocked-pending-allowed-public-interface"}
    for original, row in zip(inventory_rows, planned):
        need(row["command_sha256"] == original["command_sha256"]
             and row["disposition"] in allowed and bool(row["rationale"]),
             "missing or invalid direct CI consumer disposition")
    return True
