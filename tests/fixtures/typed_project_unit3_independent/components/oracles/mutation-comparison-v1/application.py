"""Independent structural proof that a frozen mutation request was applied.

This module consumes observations only. It never imports expected outcomes,
executes a compiler, resolves names, or accepts an applied marker as proof.
The caller authenticates artifacts and parses raw Debug with canonical(raw=True).
"""
from collections import Counter
from copy import deepcopy
import importlib.util
from pathlib import Path
import re
import sys

if not __debug__:
    raise RuntimeError("mutation application validation requires Python assertions")

_root = Path(__file__).resolve().parents[1]
_loader = importlib.util.spec_from_file_location("mutation_frozen_span_inventory", _root / "raw_span_oracle.py")
_spans = importlib.util.module_from_spec(_loader)
_loader.loader.exec_module(_spans)


class Invalid(ValueError):
    pass


class Missing(ValueError):
    pass


def require(test, reason):
    if not test:
        raise Invalid(reason)


def need(test, reason):
    if not test:
        raise Missing(reason)


def equal(a, b):
    """JSON comparison which does not silently identify True and integer 1."""
    if type(a) is not type(b):
        return False
    if isinstance(a, dict):
        return set(a) == set(b) and all(equal(a[k], b[k]) for k in a)
    if isinstance(a, list):
        return len(a) == len(b) and all(equal(x, y) for x, y in zip(a, b))
    return a == b


def raw_value(v):
    """Normalize generic marker payloads, independently of paths or expectations."""
    if isinstance(v, list):
        return [raw_value(x) for x in v]
    if not isinstance(v, dict):
        return v
    out = {k: raw_value(x) for k, x in v.items()}
    tag = out.get("tag")
    if tag == "Assign" and "items" in out:
        require(len(out["items"]) == 1, "malformed Assign marker")
        return out["items"][0]
    rename = {"Load": "place", "Copy": "operand", "Return": "value", "ReturnScalar": "value",
              "Scalar": "scalar", "Goto": "target", "StorageLive": "owner", "StorageEnd": "owner",
              "Discard": "owner", "OpenCall": "call", "ReturnOwned": "owner", "I32": "value", "Bool": "value"}
    if tag in rename and "items" in out:
        require(len(out["items"]) == 1, "malformed tuple marker")
        out[rename[tag]] = out.pop("items")[0]
    if tag == "Construct" and out.get("fields") and isinstance(out["fields"][0], list):
        out["fields"] = [{"field": f, "value": x} for f, x in out["fields"]]
    return out


def components(path):
    result = []
    for segment in path.split("."):
        found = re.fullmatch(r"([A-Za-z_][A-Za-z_0-9]*)(\[\d+\])*", segment)
        require(found is not None, "invalid structural path: " + path)
        result.append(segment.split("[")[0])
        result.extend(int(x) for x in re.findall(r"\[(\d+)\]", segment))
    return result


def at(tree, path):
    for key in components(path):
        tree = tree[key]
    return tree


def put(tree, path, value):
    keys = components(path)
    for key in keys[:-1]:
        tree = tree[key]
    tree[keys[-1]] = deepcopy(value)


def artifact(artifacts, name):
    need(name in artifacts, "require artifact " + name)
    v = artifacts[name]
    return v["value"] if isinstance(v, dict) and "identity" in v and "value" in v else v


def route_of(actual, raw):
    route = "owned" if "records" in raw else "scalar"
    require(actual.get("route", route) == route, "observation route disagrees with raw")
    return route


def source_files(actual):
    need("loaded" in actual and "sources" in actual["loaded"], "require observed loaded source map")
    files = actual["loaded"]["sources"]["files"]
    require([f["id"] for f in files] == list(range(len(files))), "source file ids are not dense")
    return files


def valid_span(files, span):
    if not (isinstance(span, list) and len(span) == 3 and all(type(x) is int for x in span)):
        return False
    f, start, end = span
    if not (0 <= f < len(files) and 0 <= start <= end):
        return False
    data = files[f]["text"].encode("utf-8")
    if end > len(data):
        return False
    try:
        data[:start].decode("utf-8")
        data[:end].decode("utf-8")
    except UnicodeDecodeError:
        return False
    return True


def span_text(files, span):
    require(valid_span(files, span), "selection uses an invalid observed source span")
    return files[span[0]]["text"].encode("utf-8")[span[1]:span[2]].decode("utf-8")


def foreign_span(actual, old):
    files = source_files(actual)
    require(len(files) > 1, "foreign-span request has no other loaded file")
    target = (old[0] + 1) % len(files)
    same = [target, old[1], old[2]]
    if valid_span(files, same):
        return same
    programs = actual["loaded"].get("programs", [])
    matching = [p for p in programs if p.get("source", {}).get("file") == target]
    need(len(matching) == 1, "require observed AST tokens for foreign file's checked first identifier")
    tokens = [t for t in matching[0]["tokens"] if t["kind"]["tag"] == "Ident"]
    require(bool(tokens), "foreign file has no checked identifier")
    span = tokens[0]["span"]
    require(valid_span(files, span), "foreign identifier span is invalid")
    return span


def marker(applied, path, before, after, aliases=()):
    require(isinstance(applied, dict) and set(applied) == {"path", "before", "after"}, "malformed application marker")
    labels = {path, *aliases}
    # Canonical raw nests enum payloads under kind; the reviewed marker may omit
    # that single representation layer, never an index, field, or category.
    labels.add(path.replace(".kind.", "."))
    require(applied["path"] in labels, "marker path does not name the independently selected edit: " + path)
    require(equal(raw_value(applied["before"]), before), "marker before value disagrees with actual edit")
    require(equal(raw_value(applied["after"]), after), "marker after value disagrees with actual edit")


def one_edit(before, after, path, value, applied, aliases=()):
    old = at(before, path)
    require(not equal(old, value), "requested edit is a no-op")
    wanted = deepcopy(before)
    put(wanted, path, value)
    require(equal(wanted, after), "raw after differs from the single permitted edit at " + path)
    marker(applied, path, old, value, aliases)
    return {"path": path, "proof": "exact-whole-raw-edit"}


def functions(raw):
    return enumerate(raw["functions"])


def statements(raw):
    for fi, f in functions(raw):
        for bi, b in enumerate(f["blocks"]):
            for si, s in enumerate(b["statements"]):
                yield fi, bi, si, s, f"functions[{fi}].blocks[{bi}].statements[{si}]"


def calls(raw, route):
    for fi, f in functions(raw):
        if route == "owned":
            for ci, c in enumerate(f["calls"]):
                yield fi, ci, c, f"functions[{fi}].calls[{ci}]"
        else:
            for bi, b in enumerate(f["blocks"]):
                t = b["terminator"]
                if t is not None and t["kind"]["tag"] == "Call":
                    yield fi, bi, t["kind"], f"functions[{fi}].blocks[{bi}].terminator.kind"


def unique(values, reason):
    values = list(values)
    require(len(values) == 1, reason + " (observed candidates: " + str(len(values)) + ")")
    return values[0]


def first(values, reason):
    return next(iter(values), None) or _absent(reason)


def _absent(reason):
    raise Invalid(reason)


def signature(raw, route, fid):
    f = raw["functions"][fid]
    if route == "scalar":
        params = [x["ty"] for x in f["locals"][:f["param_count"]]]
    else:
        params = []
        for p in f["parameters"]:
            tag = p["tag"]
            i = p["scalar"] if tag == "Scalar" else p["items"][0]
            if tag == "Scalar":
                params.append([tag, f["locals"][i]["ty"]])
            elif tag == "Owned":
                params.append([tag, f["owners"][i]["record"]])
            elif tag == "Reference":
                r = f["references"][i]
                params.append([tag, r["record"], r["kind"]])
            else:
                raise Invalid("unknown observed parameter kind " + tag)
    return [f["result"], params]


def named_function(raw, files, name, filename=None):
    return unique((fi for fi, f in functions(raw)
                   if span_text(files, f["span"]) == name
                   and (filename is None or Path(files[f["span"][0]]["path"]).name == filename)),
                  "require unique observed " + str(filename) + " function " + name)


def successors(block):
    t = block["terminator"]
    if t is None:
        return []
    k = t["kind"]
    if k["tag"] == "Branch":
        return [k["then_block"], k["else_block"]]
    if k["tag"] == "Goto":
        return [k["target"]]
    if k["tag"] in ("Invoke", "Call"):
        return [k["continuation"]]
    return []


def references_owner(k, owner, f):
    tag = k["tag"]
    if tag in ("StorageLive", "StorageEnd", "Discard", "ReturnOwned"):
        return k["owner"] == owner
    if tag in ("Construct", "MoveInitialize", "Replace"):
        return k["destination"] == owner or k.get("source") == owner
    if tag == "PrepareOwned":
        return k["source"] == owner
    if tag in ("ReadField", "WriteField"):
        return k["base"].get("tag") == "Owner" and k["base"]["items"] == [owner]
    if tag == "PrepareBorrow":
        base = f["loans"][k["loan"]]["authority"]
        return base.get("tag") == "Owner" and base["items"] == [owner]
    if tag == "Invoke":
        for slot in f["calls"][k["call"]]["arguments"]:
            if slot["tag"] == "Owned" and slot["items"] == [owner]:
                return True
            if slot["tag"] == "Borrow":
                authority = f["loans"][slot["items"][0]]["authority"]
                if authority.get("tag") == "Owner" and authority["items"] == [owner]:
                    return True
    return False


def no_later_owner_access(f, bi, si, owner):
    """All reachable raw paths after the selected exit, including back edges."""
    pending = [(bi, si + 1)]
    seen = set()
    while pending:
        current, start = pending.pop()
        if (current, start) in seen:
            continue
        seen.add((current, start))
        require(0 <= current < len(f["blocks"]), "invalid cleanup successor")
        b = f["blocks"][current]
        require(not any(references_owner(s["kind"], owner, f) for s in b["statements"][start:]),
                "removed lexical owner is accessed or made live again after the exit")
        if b["terminator"] is not None:
            require(not references_owner(b["terminator"]["kind"], owner, f), "removed owner is transferred after cleanup")
        pending.extend((target, 0) for target in successors(b))


def raw_mutation(spec, baseline, mutant, artifacts, applied):
    m = spec["mutation"]
    kind = m["kind"]
    before = artifact(artifacts, "mutation-before-raw.debug")
    after = artifact(artifacts, "mutation-after-raw.debug")
    need("raw_before_audit" in baseline, "require baseline raw_before_audit for pre-edit join")
    require(equal(before, baseline["raw_before_audit"]), "pre-edit raw does not equal unmutated baseline")
    if "raw_before_audit" in mutant:
        require(equal(after, mutant["raw_before_audit"]), "post-edit raw does not equal raw submitted to association audit")
    else:
        raise Missing("require mutant raw_before_audit to bind the edited raw to the real source association path")
    if "raw" in mutant:
        require(equal(after, mutant["raw"]), "post-edit raw does not equal immutable checked consumer raw")
    route = route_of(baseline, before)
    files = source_files(baseline)
    inv = _spans.inventory(before, route)
    edit = lambda p, v, aliases=(): one_edit(before, after, p, v, applied, aliases)

    if kind == "replace-span":
        rows = [r for r in inv["spans"] if r["category"] == m["category"]]
        n = m["occurrence"]
        require(type(n) is int and 0 <= n < len(rows), "requested span category occurrence absent")
        row = rows[n]
        out = edit(row["path"], foreign_span(baseline, row["span"]))
        return dict(out, category=m["category"], occurrence=n)
    if kind in ("replace-span-file", "replace-span-range"):
        candidates = [(fi, f) for fi, f in functions(before) if f["locals"]]
        if m.get("start") == "first multibyte scalar start + 1":
            candidates = [(fi, f) for fi, f in candidates if any(ord(c) > 127 for c in files[f["span"][0]]["text"])]
        fi, f = first(candidates, "nondeclaration range selection absent")
        p = f"functions[{fi}].locals[0].span"
        v = deepcopy(at(before, p))
        if kind == "replace-span-file":
            require(m["value"] == "number_of_files", "unsupported file replacement")
            v[0] = len(files)
        elif m.get("start") == "first multibyte scalar start + 1":
            data = files[v[0]]["text"].encode("utf-8")
            i = next((i for i, byte in enumerate(data) if byte >= 192), None)
            require(i is not None, "multibyte source scalar absent")
            v[1] = v[2] = i + 1
        else:
            if "start" in m:
                v[1] = m["start"]
            if "end" in m:
                v[2] = len(files[v[0]]["text"].encode("utf-8")) + 1 if m["end"] == "file_length+1" else m["end"]
        require(not valid_span(files, v), "range/file fault is still valid")
        return edit(p, v)
    if kind == "set-function-id":
        require(m["value"] == "function_count", "unknown function-id value")
        return edit(f"functions[{m['index']}].id", len(before["functions"]))
    if kind == "set-record-id":
        return edit(f"records[{m['index']}].id", m["value"])
    if kind == "set-field-id":
        return edit(f"records[{m['record']}].fields[{m['field']}].id", m["value"])
    if kind in ("append-duplicate-function", "append-duplicate-field"):
        p = "functions" if kind == "append-duplicate-function" else f"records[{m['record']}].fields"
        old = at(before, p)
        index = m["index"] if kind == "append-duplicate-function" else m["field"]
        wanted = deepcopy(before)
        put(wanted, p, old + [old[index]])
        require(equal(wanted, after), "raw is not the exact requested appended duplicate")
        marker(applied, p + ".length", len(old), len(old) + 1)
        return {"path": p + f"[{len(old)}]", "proof": "exact-appended-duplicate"}
    if kind in ("call-target", "remove-last-call-argument", "call-result-slot-type"):
        sites = list(calls(before, route))
        if kind == "call-target" and "replacement" in m:
            left = named_function(before, files, "helper", "left.ox")
            right = named_function(before, files, "helper", "right.ox")
            require(equal(signature(before, route, left), signature(before, route, right)), "replacement helper signature differs")
            sites = [site for site in sites if site[2]["target"] == left]
            fi, ci, c, p = first(sites, "no call to the original left helper")
            return edit(p + ".target", right)
        if kind == "call-target":
            require(m["value"] == "function_count", "unknown target value")
            fi, ci, c, p = first(sites, "no observed call target")
            return edit(p + ".target", len(before["functions"]))
        require(route == "scalar", "scalar-call mutation applied to owned route")
        if kind == "remove-last-call-argument":
            fi, ci, c, p = first((s for s in sites if s[2]["args"]), "no nonempty call argument list")
            return edit(p + ".args", c["args"][:-1])
        fi, ci, c, p = first((s for s in sites if before["functions"][s[0]]["locals"][s[2]["destination"]]["ty"]["tag"] in ("I32", "Bool")), "no scalar call result slot")
        p = f"functions[{fi}].locals[{c['destination']}].ty"
        old = at(before, p)["tag"]
        return edit(p, {"tag": {"I32": "Bool", "Bool": "I32"}[old]})
    if kind == "call-argument-mode":
        require(m["argument"] == 0 and m["value"] == "Exclusive", "unsupported borrow-mode request")
        candidates = []
        for fi, ci, c, p in calls(before, route):
            if c["parent"] is None and c["arguments"] and c["arguments"][0]["tag"] == "Borrow":
                loan = c["arguments"][0]["items"][0]
                if before["functions"][fi]["loans"][loan]["kind"]["tag"] == "Shared":
                    candidates.append((fi, ci, loan))
        fi, ci, loan = first(candidates, "no outer shared borrow argument")
        return edit(f"functions[{fi}].loans[{loan}].kind", {"tag": "Exclusive"},
                    [f"functions[{fi}].calls[{ci}].argument[0].loan.kind"])
    if kind == "projection-field-record":
        fi, bi, si, s, p = first((s for s in statements(before) if s[3]["kind"]["tag"] in ("ReadField", "WriteField")), "no field projection")
        field = s["kind"]["field"]
        require(field[0] != m["record"], "field already belongs to replacement record")
        return edit(p + ".kind.field", [m["record"], field[1]])
    if kind == "owner-kind":
        candidates = [(fi, oi) for fi, f in functions(before) for oi, o in enumerate(f["owners"]) if o["kind"]["tag"] == "StagedArgument"]
        fi, oi = first(candidates, "no StagedArgument owner")
        return edit(f"functions[{fi}].owners[{oi}].kind", {"tag": m["value"]})
    if kind == "ReturnOwned-source":
        fi = named_function(before, files, "helper", "left.ox")
        f = before["functions"][fi]
        owner = unique((i for i, o in enumerate(f["owners"]) if o["kind"]["tag"] == "Parameter"), "ambiguous parameter owner")
        bi, block = unique(((bi, b) for bi, b in enumerate(f["blocks"]) if b["terminator"] and b["terminator"]["kind"]["tag"] == "ReturnOwned"), "ambiguous callee ReturnOwned")
        old = block["terminator"]["kind"]["owner"]
        require(f["owners"][owner]["record"] == f["owners"][old]["record"], "return replacement has a different record")
        # Prove the move lies on every path reaching this return. Merely finding
        # one move in an unrelated block would accept the wrong semantic fault.
        pending = [(f["entry"], False)]
        seen = set()
        reached = False
        while pending:
            idx, moved = pending.pop()
            if (idx, moved) in seen:
                continue
            seen.add((idx, moved))
            for st in f["blocks"][idx]["statements"]:
                k = st["kind"]
                if k["tag"] == "MoveInitialize" and k["source"] == owner:
                    moved = True
                elif k["tag"] in ("Construct", "MoveInitialize", "Replace") and k["destination"] == owner:
                    moved = False
                elif k["tag"] == "StorageLive" and k["owner"] == owner:
                    moved = False
            if idx == bi:
                require(moved, "parameter is not already moved on every path to return")
                reached = True
            pending.extend((target, moved) for target in successors(f["blocks"][idx]))
        require(reached, "selected ReturnOwned is unreachable")
        return edit(f"functions[{fi}].blocks[{bi}].terminator.kind.owner", owner)
    if kind == "nested-call-parent":
        fi, ci, c, p = first((s for s in calls(before, route) if s[2]["parent"] is not None), "no nested call")
        return edit(p + ".parent", [ci, c["parent"][1]])
    if kind == "remove-outer-PrepareBorrow":
        fi, bi, si, s, p = first((s for s in statements(before) if s[3]["kind"]["tag"] == "PrepareBorrow" and before["functions"][s[0]]["calls"][s[3]["kind"]["call"]]["parent"] is None), "no outer PrepareBorrow")
        k = s["kind"]
        f = before["functions"][fi]
        slot = f["calls"][k["call"]]["arguments"][k["argument"]]
        require(slot == {"tag": "Borrow", "items": [k["loan"]]} and k["loan"] < len(f["loans"]), "PrepareBorrow is not joined to retained descriptors")
        return sequence_edit(before, after, applied, fi, bi, si, "remove", "removed, descriptors retained")
    if kind in ("remove-required-StorageEnd", "remove-StorageEnd", "duplicate-StorageEnd", "swap-two-independent-StorageEnd"):
        edge = m["edge"]
        matches = []
        for fi, f in functions(before):
            for bi, b in enumerate(f["blocks"]):
                t = b["terminator"]
                if t is None:
                    continue
                text = span_text(files, t["span"]).strip()
                wanted = text == "}" if edge == "if join" else bool(re.match(r"^" + re.escape(edge) + r"\b", text))
                if not wanted:
                    continue
                if edge in ("continue", "break", "if join"):
                    require(t["kind"]["tag"] == "Goto", "cleanup edge is not a raw goto")
                else:
                    require(t["kind"]["tag"] in ("ReturnScalar", "ReturnOwned"), "cleanup return edge is not a return")
                ends = [(si, st["kind"]["owner"]) for si, st in enumerate(b["statements"])
                        if st["kind"]["tag"] == "StorageEnd" and st["span"] == t["span"]
                        and f["owners"][st["kind"]["owner"]]["kind"]["tag"] == "Local"]
                if ends:
                    matches.append((fi, bi, ends))
        fi, bi, ends = first(matches, "no lexical StorageEnd on requested " + edge + " edge")
        si, owner = ends[0]
        if kind == "remove-StorageEnd":
            no_later_owner_access(before["functions"][fi], bi, si, owner)
        if kind == "swap-two-independent-StorageEnd":
            require(len(ends) >= 2 and owner != ends[1][1], "cleanup swap needs two distinct lexical owners")
            f = before["functions"][fi]
            b = f["blocks"][bi]
            sj, other = ends[1]
            for index, st in enumerate(b["statements"][si + 1:], si + 1):
                if index != sj:
                    require(not references_owner(st["kind"], owner, f) and not references_owner(st["kind"], other, f), "cleanup owners have intervening or later lifetime effects before the edge")
            require(not references_owner(b["terminator"]["kind"], owner, f) and not references_owner(b["terminator"]["kind"], other, f), "cleanup edge still uses one of the ended owners")
            return sequence_edit(before, after, applied, fi, bi, si, "swap", None, ends[1][0])
        mode = "duplicate" if kind == "duplicate-StorageEnd" else "remove"
        return sequence_edit(before, after, applied, fi, bi, si, mode, "duplicated once" if mode == "duplicate" else "removed once")
    if kind == "replace-diagnostic-origin":
        fi = named_function(before, files, "main")
        _, bi, si, s, p = unique((s for s in statements(before) if s[0] == fi and s[3]["kind"]["tag"] == "ReadField"), "main ReadField metadata selection is ambiguous")
        field = m["field"]
        require(field in ("primary", "cause") and s["diagnostic_origins"] is not None, "diagnostic origin absent")
        old = s["diagnostic_origins"][field]
        value = foreign_span(baseline, old) if m["replacement"] == "valid child-file span" else before["functions"][fi]["span"]
        require(valid_span(files, value), "replacement metadata span is invalid")
        if m["replacement"] == "valid child-file span":
            require(value[0] != old[0] and any(module["file"] == value[0] and module["parent"] is not None for module in baseline["loaded"]["modules"]), "replacement metadata is not in a child module")
        else:
            require(value[0] == old[0], "same-file metadata replacement changed file")
        result = edit(p + ".diagnostic_origins." + field, value, [p + ".diagnostic_origins"])
        for name in ("generic-probe-input-original.debug", "generic-probe-input-clone.debug"):
            require(equal(artifact(artifacts, name), after), "generic probe did not consume exact mutated raw")
        return result
    raise Missing("no structural evidence rule for mutation kind " + kind)


def sequence_edit(before, after, applied, fi, bi, si, mode, marker_after, sj=None):
    parent = f"functions[{fi}].blocks[{bi}].statements"
    p = parent + f"[{si}]"
    old = at(before, parent)
    values = deepcopy(old)
    if mode == "remove":
        values.pop(si)
        marker_before = old[si]
    elif mode == "duplicate":
        values.insert(si, deepcopy(values[si]))
        marker_before = old[si]
    else:
        values[si], values[sj] = values[sj], values[si]
        marker_before = [old[si], old[sj]]
        marker_after = [values[si], values[sj]]
        require(not equal(marker_before, marker_after), "cleanup swap is a no-op")
    wanted = deepcopy(before)
    put(wanted, parent, values)
    require(equal(wanted, after), "raw statement sequence differs beyond the requested " + mode)
    marker(applied, p, marker_before, marker_after)
    return {"path": p, "proof": "exact-statement-" + mode}


def verify_audit_inventory(audit, inv):
    wanted = Counter([("declaration", d["path"]) for d in inv["declarations"]] + [("span", r["path"]) for r in inv["spans"]])
    spans = {r["path"]: r["span"] for r in inv["spans"]}
    for phase, key in (("count", "count_visits"), ("validate", "validate_visits")):
        visits = audit[key]
        require(Counter((v["kind"], v["path"]) for v in visits) == wanted, "baseline visitor does not exhaust independent inventory")
        for v in visits:
            require(v["succeeded"] is True and v["pass"] == phase, "baseline visitor failed or has wrong phase")
            require(equal(v["span"], spans[v["path"]] if v["kind"] == "span" else None), "baseline visitor span does not equal stored raw field")
    require(equal(audit["visits"], audit["count_visits"] + audit["validate_visits"]), "audit journal partition disagrees")


def require_bind_denial(mutant, artifacts, stage="oir-project-bind"):
    diagnostics = mutant.get("diagnostics", [])
    need(bool(diagnostics), "require actual source-association denial for count seam")
    require(len(diagnostics) == 1 and diagnostics[0].get("stage") == stage and diagnostics[0].get("code") == "E0500", "seam did not fail at its exact source-association stage")
    require("raw-verifier-start.debug" not in artifacts and "raw" not in mutant, "count seam reached raw verification or a checked consumer")
    require(mutant["audit"].get("usage") is None, "count seam unexpectedly completed association usage")


def seam_mutation(spec, baseline, mutant, artifacts, applied):
    m = spec["mutation"]
    kind = m["kind"]
    if kind == "source-seal-seam-entry":
        need("entry" in baseline and "index" in baseline and "index" in mutant, "require baseline entry and observed declaration indexes")
        entry = baseline["entry"]
        require(entry == baseline["index"]["tables"]["root_main"], "baseline entry does not join frozen index")
        require(equal(baseline["index"], mutant["index"]), "entry seam changed declaration index")
        require(equal(artifact(artifacts, "mutation-entry-agreement.debug"), [m["value"], entry, entry]), "entry disagreement artifact is not exactly local/typed/frozen")
        require(entry != m["value"], "entry seam is a no-op")
        marker(applied, "constructor.local.entry", entry, m["value"])
        require(not mutant.get("audit", {}).get("visits"), "entry disagreement unexpectedly reached association traversal")
        return {"path": "constructor.local.entry", "proof": "local-typed-frozen-entry-agreement"}
    if kind == "original-source-map-file-id":
        files = source_files(baseline)
        require(len(files) == 1 and m["value"] == 1, "nonzero source control requires one original file and id 1")
        active = artifact(artifacts, "active-constructor-map.debug")["files"]
        require(len(active) == 2 and [f["id"] for f in active] == [0, 1], "constructor did not use exact two-file map")
        require(active[0]["text"] == "" and active[0]["path"] == "mutation-decoy.ox", "nonzero constructor decoy differs")
        require(all(equal(active[1][k], files[0][k]) for k in ("text", "path", "line_starts")), "nonzero constructor changed original source bytes or path")
        need("raw_before_audit" in baseline and "raw_before_audit" in mutant, "require pre-audit raw for both source constructors")
        before, after = baseline["raw_before_audit"], mutant["raw_before_audit"]
        route = route_of(baseline, before)
        inv = _spans.inventory(before, route)
        wanted = deepcopy(before)
        for row in inv["spans"]:
            require(row["span"][0] == 0, "baseline original span is not file zero")
            put(wanted, row["path"], [1, *row["span"][1:]])
        require(equal(wanted, after), "nonzero constructor changed raw beyond every stored source file id")
        if "raw" in mutant:
            require(equal(after, mutant["raw"]), "nonzero constructor consumer raw differs")
        marker(applied, "constructor.actual_file", 0, 1)
        return {"path": "constructor.actual_file", "proof": "active-map-and-exhaustive-span-rebase", "span_count": inv["Sspan"]}
    if kind == "original-source-constructor-substitution":
        files = source_files(baseline)
        require(len(files) == 1, "constructor substitution requires one original file")
        active = artifact(artifacts, "active-constructor-map.debug")["files"]
        substitution = m["substitution"]
        if substitution == "stale_parser_generation":
            marker(applied, "constructor.parser_generation", "actual-source", "different-source-generation")
            require(equal(active, files), "stale-parser substitution changed source map")
            need("constructor-parser-identity.debug" in artifacts,
                 "require constructor-parser-identity.debug containing actual source/substitute identities and AST belongs_to predicates; ordinary SourceProvenance Debug omits generation identity")
            identities = artifact(artifacts, "constructor-parser-identity.debug")
            require(isinstance(identities, list) and len(identities) == 5, "malformed parser identity artifact")
            source, substitute, original_matches_source, substituted_matches_substitute, substituted_matches_source = identities
            require(type(source) is int and type(substitute) is int and source > 0 and substitute > 0,
                    "parser identity artifact is not concrete positive allocation identities")
            require(source == files[0]["identity"] == active[0]["identity"] and source != substitute,
                    "parser identity does not join actual active source or is not a different generation")
            require(original_matches_source is True and substituted_matches_substitute is True and substituted_matches_source is False,
                    "actual AST belongs_to predicates do not prove the requested stale generation")
            require_bind_denial(mutant, artifacts, stage="resolve-project")
            require("raw_before_audit" not in mutant and not mutant["audit"]["visits"], "stale-parser rejection reached raw construction or association traversal")
            return {"path": "constructor.parser_generation", "proof": "actual-source-identities-belongs-to-and-early-bind-denial"}
        require(len(active) == 1 and active[0]["id"] == 0, "substitution source map shape changed")
        old, new = files[0], active[0]
        require(new["identity"] != old["identity"], "source map substitution retained original allocation identity")
        if substitution == "same_bytes_new_map":
            require(new["text"] == old["text"] and new["path"] != old["path"], "same-bytes substitution did not retain bytes with distinct path")
        elif substitution == "same_path_new_map":
            require(new["text"] == old["text"] and new["path"] == old["path"], "same-path substitution changed bytes or path")
        elif substitution == "same_offsets_other_map":
            require(new["path"] == old["path"] and new["text"] == " " * len(old["text"].encode("utf-8")), "same-offsets substitution is not a byte-length-preserving alternate map")
        else:
            raise Invalid("unknown constructor substitution")
        marker(applied, "constructor.map", "actual-map-allocation", "different-map-allocation")
        return {"path": "constructor.map", "proof": "observed-map-allocation-and-source-content", "substitution": substitution}
    if kind.startswith("audit-"):
        need("raw_before_audit" in baseline and "raw_before_audit" in mutant, "require unchanged pre-audit raw for traversal/count seam")
        before = baseline["raw_before_audit"]
        require(equal(before, mutant["raw_before_audit"]), "audit seam altered raw")
        inv = _spans.inventory(before, route_of(baseline, before))
        verify_audit_inventory(baseline["audit"], inv)
        require(equal(mutant["audit"]["visits"], mutant["audit"]["count_visits"] + mutant["audit"]["validate_visits"]), "mutant audit journal partition disagrees")
        if kind in ("audit-count-seam", "audit-count-seam-overflow"):
            name = "count.spans.before_finish" if kind == "audit-count-seam" else "count.spans.before_actual_increment"
            if kind == "audit-count-seam":
                require(equal(baseline["audit"]["count_visits"], mutant["audit"]["count_visits"]), "count-total seam changed count traversal")
                require(equal(baseline["audit"]["validate_visits"], mutant["audit"]["validate_visits"]), "count-total seam changed validation traversal")
                require(m["delta"] == 1, "count-total seam delta is not the frozen +1")
                marker(applied, name, inv["Sspan"], inv["Sspan"] + m["delta"])
            else:
                # The authenticated overlay places MAX immediately before the
                # real checked increment, at the first span in the count pass.
                # Its numeric read is corroborated by the independently checked
                # journal prefix; a marker alone cannot satisfy this proof.
                visits = baseline["audit"]["count_visits"]
                first_span = next(i for i, v in enumerate(visits) if v["kind"] == "span")
                wanted = deepcopy(visits[:first_span + 1])
                wanted[-1]["succeeded"] = False
                require(equal(mutant["audit"]["count_visits"], wanted), "overflow count journal is not the exact prefix ending at first failed span")
                require(mutant["audit"]["validate_visits"] == [], "overflow reached validation pass")
                completed_spans = sum(v["kind"] == "span" and v["succeeded"] for v in wanted)
                marker(applied, name, completed_spans, (1 << 64) - 1)
            require_bind_denial(mutant, artifacts)
            return {"path": name, "proof": "actual-counter-read-independent-journal-and-bind-denial", "span_count": inv["Sspan"]}
        rows = [r for r in inv["spans"] if r["category"] == m["category"]]
        row = first(rows, "requested visitor category is absent")
        count = baseline["audit"]["count_visits"]
        validate = baseline["audit"]["validate_visits"]
        require(equal(count, mutant["audit"]["count_visits"]), "visitor mutation changed count traversal")
        target = unique((i for i, v in enumerate(validate) if v["kind"] == "span" and v["path"] == row["path"]), "visitor occurrence is not unique")
        wanted = deepcopy(validate)
        repeated = kind == "audit-visitor-repeat-one-visit"
        if repeated:
            wanted.insert(target, deepcopy(wanted[target]))
        else:
            wanted.pop(target)
        require(equal(wanted, mutant["audit"]["validate_visits"]), "actual validation journal is not exactly one requested visit suppression/repetition")
        marker(applied, "validate.Construct.fields[0].value.span", [row["span"], "once"], [row["span"], "twice" if repeated else "omitted"])
        return {"path": row["path"], "proof": "independent-inventory-and-exact-visit-sequence", "category": m["category"]}
    raise Missing("driver-boundary mutation requires separately attributed driver operation/restore receipts; raw/applied markers do not prove driver actions")


def validate(spec, baseline_actual, mutant_actual, artifacts, applied):
    """Return applied, invalid, or evidence-missing without modifying inputs.

    artifacts is filename -> value (or filename -> {identity,value}). Raw inputs
    include mutation before/after and generic probe inputs parsed raw=True. An
    applied report proves application only, never the expected verifier outcome.
    """
    if "kind" in spec:
        spec = {"mutation": spec}
    kind = spec.get("mutation", {}).get("kind")
    result = {"status": "invalid", "kind": kind}
    try:
        require(set(spec) in ({"mutation"}, {"id", "positive_control", "mutation"}), "malformed mutation request")
        require(isinstance(applied, dict), "missing mutation-applied marker")
        require(equal(baseline_actual.get("loaded"), mutant_actual.get("loaded")), "loaded project changed between baseline and mutation")
        seam = kind.startswith("audit-") or kind in ("source-seal-seam-entry", "original-source-constructor-substitution", "original-source-map-file-id", "driver-boundary")
        detail = seam_mutation(spec, baseline_actual, mutant_actual, artifacts, applied) if seam else raw_mutation(spec, baseline_actual, mutant_actual, artifacts, applied)
        return dict(result, status="applied", **detail)
    except Missing as error:
        return dict(result, status="evidence-missing", requirement=str(error))
    except (Invalid, AssertionError, KeyError, IndexError, TypeError, ValueError, StopIteration) as error:
        return dict(result, reason=str(error) or type(error).__name__)
