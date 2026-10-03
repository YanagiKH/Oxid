#!/usr/bin/env python3
"""Observer-only application tests. No subprocess or compiler execution.

Actual smoke specimens remain byte-identical. Additional cases below modify
in-memory copies of observed positive raw; they establish validator sensitivity,
not mutation execution coverage or expected compiler outcomes.
"""
from copy import deepcopy
import gzip
import hashlib
import importlib.util
import json
from pathlib import Path

import application as app

HERE = Path(__file__).resolve().parent
ROOT = HERE.parents[1]
REQUESTS = json.loads((HERE.parent / "mutation-requests.json").read_text())["mutations"]
BY_ID = {r["id"]: r for r in REQUESTS}
FROZEN_EXPECTED = {r["id"]: r["expected"] for r in json.loads((HERE.parent / "mutation-expectations.json").read_text())["mutations"]}


def parser():
    spec = importlib.util.spec_from_file_location("application_test_debug_parser", ROOT / "typed-project-unit3-observer/parse_debug.py")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def baseline(case):
    path = ROOT / "typed-project-unit3-observer/full-qualification-union-v1/debug" / case / "receipt.json"
    receipt = json.loads(path.read_text())
    entry = next(x for x in receipt["artifacts"] if Path(x["path"]).name == "observations.json")
    if Path(entry["path"]).exists():
        data = Path(entry["path"]).read_bytes()
    else:
        data = gzip.decompress(Path(entry["compressed"]["path"]).read_bytes())
    assert len(data) == entry["bytes"] and hashlib.sha256(data).hexdigest() == entry["sha256"]
    return json.loads(data)


def raw_case(request, base, path, value, marker_path=None, marker_before=None, marker_after=None):
    before = base["raw_before_audit"]
    after = deepcopy(before)
    app.put(after, path, value)
    mutant = deepcopy(base)
    mutant["raw_before_audit"] = after
    if "raw" in mutant:
        mutant["raw"] = deepcopy(after)
    artifacts = {"mutation-before-raw.debug": before, "mutation-after-raw.debug": after}
    marker = {"path": marker_path or path,
              "before": deepcopy(app.at(before, path) if marker_before is None else marker_before),
              "after": deepcopy(value if marker_after is None else marker_after)}
    if request["mutation"]["kind"] == "replace-diagnostic-origin":
        artifacts.update({name: after for name in ("generic-probe-input-original.debug", "generic-probe-input-clone.debug")})
    return request, base, mutant, artifacts, marker


def check(args, wanted="applied"):
    result = app.validate(*args)
    assert result["status"] == wanted, (args[0], result)
    return result


def run():
    reports = []
    pd = parser()
    paths = list((ROOT / "typed-project-unit3-mutations/smoke-v1").glob("*/*/receipt.json"))
    for path in sorted(paths):
        receipt = json.loads(path.read_text())
        base = json.loads(Path(receipt["baseline_observations"]["path"]).read_text())
        mutant = json.loads(Path(receipt["mutant_observations"]["path"]).read_text())
        artifacts = {k: pd.canonical(pd.parse(Path(v["identity"]["path"]).read_text()), raw=k.endswith("raw.debug") or k.startswith("generic-probe-input")) for k, v in receipt["mutation_artifacts"].items()}
        args = (receipt["request"], base, mutant, artifacts, receipt["mutation_applied"])
        frozen = json.dumps(args, sort_keys=True)
        positive = check(args)
        controls = []
        for field, replacement in (("path", "functions[999].span"), ("before", "invented-before"), ("after", "invented-after")):
            bad = list(deepcopy(args))
            bad[4][field] = replacement
            controls.append({"control": "changed-marker-" + field, "result": check(bad, "invalid")})
        if "mutation-after-raw.debug" in artifacts:
            bad = list(deepcopy(args))
            after = deepcopy(bad[3]["mutation-after-raw.debug"])
            after["functions"][0]["entry"] += 1
            bad[3]["mutation-after-raw.debug"] = after
            bad[2]["raw_before_audit"] = deepcopy(after)
            if "raw" in bad[2]:
                bad[2]["raw"] = deepcopy(after)
            for name in ("generic-probe-input-original.debug", "generic-probe-input-clone.debug"):
                if name in bad[3]:
                    bad[3][name] = deepcopy(after)
            controls.append({"control": "extra-raw-diff-with-consistent-joins", "result": check(bad, "invalid")})
        assert json.dumps(args, sort_keys=True) == frozen, "validator modified its inputs"
        reports.append({"type": "actual-smoke", "profile": receipt["profile"], "id": receipt["mutation_id"], "result": positive, "controls": controls})

    cache = {}
    def observed(case):
        if case not in cache:
            cache[case] = baseline(case)
        return cache[case]

    # Every frozen category/occurrence is checked on its own real positive raw.
    for request in REQUESTS:
        m = request["mutation"]
        if m["kind"] != "replace-span":
            continue
        base = observed(request["positive_control"])
        inv = app._spans.inventory(base["raw_before_audit"], base["route"])
        selected = [r for r in inv["spans"] if r["category"] == m["category"]][m["occurrence"]]
        value = app.foreign_span(base, selected["span"])
        args = raw_case(request, base, selected["path"], value)
        positive = check(args)
        wrong_category = list(deepcopy(args))
        wrong_category[0]["mutation"]["category"] = "owned.NONEXISTENT.span"
        check(wrong_category, "invalid")
        wrong_occurrence = list(deepcopy(args))
        wrong_occurrence[0]["mutation"]["occurrence"] = 999
        check(wrong_occurrence, "invalid")
        reports.append({"type": "in-memory-observed-raw", "id": request["id"], "result": positive, "rejected": ["wrong-category", "wrong-occurrence"]})

    # Explicit real observed sites make these fixtures independent of the
    # validator's selector loops. A mismatch fails instead of searching elsewhere.
    edits = {
        "bind-function_id": ("functions[0].id", lambda b: len(b["functions"])),
        "bind-record_id": ("records[0].id", lambda b: 1),
        "bind-field_id": ("records[0].fields[0].id", lambda b: [0, 1]),
        "raw-wrong_arity": ("functions[0].blocks[4].terminator.kind.args", lambda b: []),
        "raw-wrong_parameter_kind": ("functions[0].loans[0].kind", lambda b: {"tag": "Exclusive"}),
        "raw-wrong_nominal_field": ("functions[1].blocks[0].statements[0].kind.field", lambda b: [1, 0]),
        "raw-staging_owner_class": ("functions[0].owners[3].kind", lambda b: {"tag": "Temporary"}),
        "raw-return_transfer": ("functions[2].blocks[0].terminator.kind.owner", lambda b: 0),
        "raw-loan_parent": ("functions[0].calls[1].parent", lambda b: [1, 1]),
        "correspondence-wrong-target-owned": ("functions[0].calls[0].target", lambda b: 3),
    }
    for identity, (path, value) in edits.items():
        request = BY_ID[identity]
        base = observed(request["positive_control"])
        marker_path = "functions[0].calls[0].argument[0].loan.kind" if identity == "raw-wrong_parameter_kind" else None
        args = raw_case(request, base, path, value(base["raw_before_audit"]), marker_path=marker_path)
        result = check(args)
        reports.append({"type": "in-memory-observed-raw", "id": identity, "result": result})

    for identity, path, source_index in (("bind-function_count", "functions", 0), ("bind-field_count", "records[0].fields", 0)):
        request = BY_ID[identity]
        base = observed(request["positive_control"])
        old = app.at(base["raw_before_audit"], path)
        args = raw_case(request, base, path, old + [old[source_index]], marker_path=path + ".length", marker_before=len(old), marker_after=len(old) + 1)
        reports.append({"type": "in-memory-observed-raw", "id": identity, "result": check(args)})

    for identity in ("bind-invalid_file", "bind-reversed_range", "bind-past_eof", "bind-utf8_continuation"):
        request = BY_ID[identity]
        base = observed(request["positive_control"])
        raw = base["raw_before_audit"]
        files = base["loaded"]["sources"]["files"]
        fi = 0
        if identity == "bind-utf8_continuation":
            fi = next(i for i, f in enumerate(raw["functions"]) if f["locals"] and any(ord(c) > 127 for c in files[f["span"][0]]["text"]))
        path = f"functions[{fi}].locals[0].span"
        value = deepcopy(app.at(raw, path))
        if identity == "bind-invalid_file":
            value[0] = len(files)
        elif identity == "bind-reversed_range":
            value[1:] = [2, 1]
        elif identity == "bind-past_eof":
            value[2] = len(files[value[0]]["text"].encode()) + 1
        else:
            continuation = next(i for i, c in enumerate(files[value[0]]["text"].encode()) if 128 <= c < 192)
            value[1:] = [continuation, continuation]
        reports.append({"type": "in-memory-observed-raw", "id": identity, "result": check(raw_case(request, base, path, value))})

    # Statement surgery uses literal observed offsets, with marker before holding
    # the deleted/duplicated statement, not its enclosing sequence.
    surgeries = {
        "raw-loan_acquisition_missing": (0, 0, 7, "remove", "removed, descriptors retained"),
        "raw-cleanup_continue_live": (0, 6, 0, "remove", "removed once"),
        "raw-cleanup_duplicate_end": (0, 2, 6, "duplicate", "duplicated once"),
        "correspondence-missing-cleanup-return": (0, 2, 6, "remove", "removed once"),
        "correspondence-missing-cleanup-break": (0, 8, 0, "remove", "removed once"),
        "correspondence-missing-cleanup-join": (0, 6, 9, "remove", "removed once"),
        "correspondence-reverse-cleanup": (0, 6, 0, "swap", None),
    }
    for identity, (fi, bi, si, mode, ma) in surgeries.items():
        request = BY_ID[identity]
        base = observed(request["positive_control"])
        path = f"functions[{fi}].blocks[{bi}].statements"
        old = app.at(base["raw_before_audit"], path)
        value = deepcopy(old)
        mb = old[si]
        if mode == "remove":
            value.pop(si)
        elif mode == "duplicate":
            value.insert(si, deepcopy(value[si]))
        else:
            value[si], value[si + 1] = value[si + 1], value[si]
            mb, ma = [old[si], old[si + 1]], [value[si], value[si + 1]]
        args = raw_case(request, base, path, value, marker_path=path + f"[{si}]", marker_before=mb, marker_after=ma)
        reports.append({"type": "in-memory-observed-raw", "id": identity, "result": check(args)})

    for identity in ("metadata-same-file-primary", "metadata-same-file-cause"):
        request = BY_ID[identity]
        base = observed(request["positive_control"])
        path = "functions[0].blocks[0].statements[14].diagnostic_origins"
        args = raw_case(request, base, path + "." + request["mutation"]["field"], base["raw_before_audit"]["functions"][0]["span"], marker_path=path)
        reports.append({"type": "in-memory-observed-raw", "id": identity, "result": check(args)})

    # A real applicability gap in the immutable source-only request: its only
    # scalar call returns Unit, so the opposite I32/Bool result slot is absent.
    request = BY_ID["raw-wrong_scalar_result"]
    base = observed(request["positive_control"])
    assert base["raw_before_audit"]["functions"][0]["locals"][17]["ty"] == {"tag": "Unit"}
    fabricated = raw_case(request, base, "functions[0].locals[17].ty", {"tag": "Bool"})
    reports.append({"type": "frozen-request-applicability-gap", "id": request["id"], "result": check(fabricated, "invalid")})

    supplement = HERE.parent / "supplements/mutation-applicability-v1/requests.json"
    supplement_bytes = supplement.read_bytes()
    assert hashlib.sha256(supplement_bytes).hexdigest() == "628d08d0500d9b98acac658ad3c75ac311272cd493913b55c2e2598a48240bd8"
    supplemental = next(r for r in json.loads(supplement_bytes)["mutations"] if r["id"] == "raw-wrong_scalar_result")
    assert supplemental["mutation"] == request["mutation"]
    base = observed(supplemental["positive_control"])
    assert base["raw_before_audit"]["functions"][0]["locals"][1]["ty"] == {"tag": "I32"}
    args = raw_case(supplemental, base, "functions[0].locals[1].ty", {"tag": "Bool"})
    reports.append({"type": "reviewed-supplement-in-memory-observed-raw", "id": supplemental["id"], "result": check(args)})

    for identity in ("bind-missing_span_visit", "bind-duplicate_span_visit"):
        request = BY_ID[identity]
        base = observed(request["positive_control"])
        mutant = deepcopy(base)
        visits = mutant["audit"]["validate_visits"]
        target = next(i for i, v in enumerate(visits) if v["path"] == "functions[0].blocks[5].statements[3].kind.fields[0].value.span")
        span = visits[target]["span"]
        repeated = "duplicate" in identity
        if repeated:
            visits.insert(target, deepcopy(visits[target]))
        else:
            visits.pop(target)
        mutant["audit"]["visits"] = mutant["audit"]["count_visits"] + visits
        marker = {"path": "validate.Construct.fields[0].value.span", "before": [span, "once"], "after": [span, "twice" if repeated else "omitted"]}
        args = (request, base, mutant, {}, marker)
        reports.append({"type": "in-memory-observed-journal", "id": identity, "result": check(args)})

    for identity in ("bind-count_total", "bind-checked_count_overflow"):
        request = BY_ID[identity]
        base = observed(request["positive_control"])
        total = app._spans.inventory(base["raw_before_audit"], base["route"])["Sspan"]
        marker = ({"path": "count.spans.before_finish", "before": total, "after": total + 1} if identity == "bind-count_total" else {"path": "count.spans.before_actual_increment", "before": 0, "after": (1 << 64) - 1})
        mutant = deepcopy(base)
        mutant.pop("raw", None)
        mutant["audit"]["usage"] = None
        mutant["diagnostics"] = [{"code": "E0500", "stage": "oir-project-bind"}]
        if identity == "bind-checked_count_overflow":
            visits = mutant["audit"]["count_visits"]
            i = next(i for i, v in enumerate(visits) if v["kind"] == "span")
            mutant["audit"]["count_visits"] = visits[:i + 1]
            mutant["audit"]["count_visits"][-1]["succeeded"] = False
            mutant["audit"]["validate_visits"] = []
        mutant["audit"]["visits"] = mutant["audit"]["count_visits"] + mutant["audit"]["validate_visits"]
        args = (request, base, mutant, {}, marker)
        positive = check(args)
        bad = list(deepcopy(args))
        bad[4]["after"] += 1
        check(bad, "invalid")
        bad = list(deepcopy(args))
        bad[2]["audit"]["count_visits"].pop(0)
        bad[2]["audit"]["visits"] = bad[2]["audit"]["count_visits"] + bad[2]["audit"]["validate_visits"]
        check(bad, "invalid")
        reports.append({"type": "in-memory-counter-journal", "id": identity, "result": positive, "rejected": ["changed-numeric-counter", "changed-count-prefix"]})

    for substitution in ("same_bytes_new_map", "same_path_new_map", "same_offsets_other_map", "stale_parser_generation"):
        request = BY_ID["source-association-" + substitution]
        base = observed("control-original-scalar-pilot")
        active = deepcopy(base["loaded"]["sources"])
        new = active["files"][0]
        if substitution != "stale_parser_generation":
            new["identity"] += 20
            if substitution == "same_bytes_new_map":
                new["path"] = "different-allocation.ox"
            if substitution == "same_offsets_other_map":
                new["text"] = " " * len(new["text"].encode())
            marker = {"path": "constructor.map", "before": "actual-map-allocation", "after": "different-map-allocation"}
            wanted = "applied"
        else:
            marker = {"path": "constructor.parser_generation", "before": "actual-source", "after": "different-source-generation"}
            wanted = "evidence-missing"
        args = (request, base, deepcopy(base), {"active-constructor-map.debug": active}, marker)
        reports.append({"type": "in-memory-source-constructor", "id": request["id"], "result": check(args, wanted)})
        if substitution == "stale_parser_generation":
            mutant = deepcopy(base)
            mutant.pop("raw")
            mutant.pop("raw_before_audit")
            mutant["audit"] = {"visits": [], "count_visits": [], "validate_visits": [], "usage": None}
            mutant["diagnostics"] = [{"code": "E0500", "stage": FROZEN_EXPECTED[request["id"]]["stage"], "primary": None, "secondary": []}]
            identity = active["files"][0]["identity"]
            artifacts = {"active-constructor-map.debug": active,
                         "constructor-parser-identity.debug": [identity, identity + 1, True, True, False]}
            args = (request, base, mutant, artifacts, marker)
            result = check(args)
            for index, value in ((0, identity + 2), (1, identity), (2, False), (3, False), (4, True)):
                bad = list(deepcopy(args))
                bad[3]["constructor-parser-identity.debug"][index] = value
                check(bad, "invalid")
            reports.append({"type": "in-memory-parser-identity-evidence", "id": request["id"], "result": result, "rejected_identity_controls": 5})

    for request in REQUESTS:
        if request["mutation"]["kind"] == "driver-boundary":
            result = check((request, {}, {}, {}, {"path": "driver", "before": "old", "after": "new"}), "evidence-missing")
            reports.append({"type": "missing-attributed-driver-evidence", "id": request["id"], "result": result})

    output = {"schema": "unit3-mutation-application-observer-only-tests-v1", "actual_smokes": len(paths),
              "covered_frozen_ids": len({r["id"] for r in reports}), "frozen_request_count": len(REQUESTS),
              "reports": reports, "compiler_runs": 0, "original_specimens_modified": False}
    print(json.dumps(output, indent=2, sort_keys=True))


if __name__ == "__main__":
    run()
