#!/usr/bin/env python3
"""Independent, fail-closed comparator for the approved Unit4 parser contract.

This module does not build or execute Rust and never manufactures expectations
from observations. Source-only inventory and synthetic tests need no candidate.
"""
from __future__ import annotations

import argparse
import base64
import collections
import copy
import gzip
import hashlib
import json
from pathlib import Path
import re
import sys

FREEZE_SHA = "7a2ec4fdf43bf94f3348a04251a9c04b77610b0e6bdd7318ce62dc2c5a5c4027"
CONTRACT_SHA = "b19819e2e4af627dfe3877ef7753fe237aa7830b16d2a83197fae0ed02010cbc"
BASE_COMMIT = "d9e6b9bf172abd5e15da7212c9e6224e29ccc768"
DURABLE_SHA = "f2d63a287f4910676306e1eb3165958cc3978151961e6af54e11355def31fa5f"
DURABLE_COMMIT = "96e881116aa5d5cdd6d2aaaf0957f6c736c91571"
REVIEWED_OBSERVER_SHA = "738dc9b47747889414859b9286e376c762dc75292ed3dedc5306e9eed881f625"
REVIEWED_PRODUCTION_SHA = "2e96ea16fe01532366a4af1d69f4fc8c18dfb2cca305da131aced3c9565b167a"
OBSERVER_REVIEW_SHA = "cca95fe71b0c5271ac721967375537eb5a2984a38f9b7b7569422a6629cf9dfa"
HELPER_REBIND_SHA = "9708016e83207a33d27db3ee6a67a4186f05347d5464ecb59aeaad93e1240b8a"
HELPER_REVIEW_SHA = "e655e3740ad8be86f49a2d2f7ebadc84e4b3597dee4a7d218a8ad446b57dca85"
ORIGINAL_NORMALIZER_SHA = "ca4bb6e661a5197cf340e7f03ad97e50df3222fec6986328f80792ef0c3b8d41"
REVIEWED_NORMALIZER_SHA = "89c77305d8b51bebbe71b06c8e3b2d82a09ae76a05f53eadae5e2820d48b2822"
AMENDMENT_CHECKPOINT_SHA = "5737321c95c0c293f80f89455024611d250908a4c039364059ca393f33fd2679"
AMENDMENT_SHA = "70b7228c3b3c3cec074dc10a84e8e338ed9b443f6b95b1be883b9a43b225e451"
AMENDMENT_DESCRIPTOR_SHA = "9bf8a80cbb30c384a70fd5696e19428d2b0aad535740e7cbe2ce8992ef96f020"
AMENDMENT_REVIEW_SHA = "a87d877c7e31d76ca0431188e3b862b94432baeb6bf3469017bab65e307ff89f"
EFFECTIVE_DOCUMENT_SHA = "c2d4f8db28f7815b3e17ca13fec9a7da70f0923dd052656edb339bc0cd7740cd"
EFFECTIVE_CONTRACT_ID = "oxid-unit4-parser-v1-location-amendment-v1"
CONTRACT_ID = "oxid-unit4-parser-source-contract-20261003-v1"
SCHEMA = "oxid-unit4-parser-observation-v1"
PLATFORM = {"actual_runtime_os": "linux", "actual_runtime_architecture": "x86_64",
            "actual_pointer_width": 64, "rust_compiler_target": "x86_64-unknown-linux-gnu"}
PROFILES = ["debug", "release"]
EXPECTED_FIELDS = set("diagnostic_count_exact diagnostics_exact eof_tokens field_lookahead_inspections_bound field_pub_lookahead_calls field_pub_scans_exact field_scans_disjoint first_diagnostic_projection human_diagnostics_base64_exact nodes_exact non_eof_tokens parse_attempts path_segment_count_after recognition_transitions_count recognition_transitions_exact recognized_final relation_to_original required_order reserve_attempts_exact reserve_trace_exact result syntax_flavor".split())
RELATIONS = set("complete_ast local_ids node_count source_provenance diagnostic_fields_order_count json_diagnostic_bytes human_diagnostic_bytes".split())
EVENT_KINDS = set("node_attempt node_admit node_reject consume recognize recognize_already_true recover_enter recover_inspect recover_exit path_count_checked path_count_overflow path_cap_reject reserve append field_scan_enter field_scan_exit identifier_byte_scan namespace_work_debit".split())
TRIVIA = {"Trivia"}
SCAN_FIELDS = {"initiator_token", "inspected_tokens", "terminal_kind", "result_is_ident", "eof_charge_initiator"}
SCAN_EVIDENCE = {"start_seq", "end_seq", "source_bytes_read", "namespace_work_units", "reserve_calls", "append_calls"}


class Rejected(ValueError):
    def __init__(self, code, detail):
        self.code, self.detail = code, detail
        super().__init__(f"{code}: {detail}")


def require(condition, code, detail):
    if not condition:
        raise Rejected(code, detail)


def same(actual, expected):
    if type(actual) is not type(expected):
        return False
    if isinstance(expected, dict):
        return actual.keys() == expected.keys() and all(same(actual[k], v) for k, v in expected.items())
    if isinstance(expected, list):
        return len(actual) == len(expected) and all(same(a, b) for a, b in zip(actual, expected))
    return actual == expected


def equal(actual, expected, label):
    require(same(actual, expected), "mismatch", label)


def integer(value, label, minimum=0):
    require(type(value) is int and value >= minimum, "schema", label + " must be an integer")
    return value


def obj(value, label):
    require(type(value) is dict, "schema", label + " must be an object")
    return value


def fields(value, required, label, optional=()):
    obj(value, label)
    require(set(required) <= value.keys(), "missing-signal", label + ": " + str(sorted(set(required) - value.keys())))
    require(value.keys() <= set(required) | set(optional), "unknown-signal", label + ": " + str(sorted(value.keys() - set(required) - set(optional))))


def vector(value, label):
    require(type(value) is list, "schema", label + " must be an array")
    return value


def text(value, label):
    require(type(value) is str, "schema", label + " must be a string")
    return value


def sha(data):
    return hashlib.sha256(data).hexdigest()


def b64(value, label):
    try:
        result = base64.b64decode(text(value, label), validate=True)
    except (ValueError, TypeError) as e:
        raise Rejected("schema", "invalid base64 " + label) from e
    equal(base64.b64encode(result).decode(), value, "canonical base64 " + label)
    return result


def loads(data):
    def pairs(items):
        result = {}
        for k, v in items:
            require(k not in result, "duplicate-json-key", k)
            result[k] = v
        return result
    def nonfinite(value):
        raise Rejected("schema", "nonfinite JSON number " + value)
    try:
        return json.loads(data, object_pairs_hook=pairs, parse_constant=nonfinite)
    except (UnicodeError, json.JSONDecodeError) as e:
        raise Rejected("invalid-json", str(e)) from e


def read_json(path):
    return loads(Path(path).read_bytes())


def verify_file(root, record):
    fields(record, {"path", "bytes", "sha256"}, "file identity")
    relative = Path(text(record["path"], "path"))
    require(not relative.is_absolute() and ".." not in relative.parts, "identity", "unsafe identity path")
    path = (Path(root) / relative).resolve()
    require(path.is_relative_to(Path(root).resolve()), "identity", "identity path escapes root")
    raw = path.read_bytes()
    equal(len(raw), record["bytes"], "file size " + str(relative))
    equal(sha(raw), record["sha256"], "file hash " + str(relative))
    return raw


def load_contract(root):
    root = Path(root)
    frozen = (root / "package-freeze.json").read_bytes()
    equal(sha(frozen), FREEZE_SHA, "approved contract freeze")
    freeze = loads(frozen)
    equal(freeze["contract_id"], CONTRACT_ID, "contract id")
    for record in freeze["files"]:
        verify_file(root, record)
    raw = gzip.decompress((root / "parser-contract-new-v1.json.gz").read_bytes())
    equal(sha(raw), CONTRACT_SHA, "decoded contract")
    contract = loads(raw)
    equal(contract["case_count"], 248, "case count")
    equal(contract["required_profiles"], PROFILES, "profiles")
    equal(len(contract["cases"]), 248, "decoded roster count")
    ids = [case["id"] for case in contract["cases"]]
    require(len(set(ids)) == 248, "roster", "contract duplicate case")
    for case in contract["cases"]:
        require(case["expected"].keys() <= EXPECTED_FIELDS, "unsupported-expectation", case["id"])
        require(set(case["expected"].get("relation_to_original", [])) <= RELATIONS,
                "unsupported-expectation", "relation " + case["id"])
        source = b64(case["source"]["base64"], "source")
        equal(sha(source), case["source"]["sha256"], "case source hash")
        equal(len(source), case["source"]["bytes"], "case source length")
        Source(case)
    return contract


class Source:
    def __init__(self, case):
        self.data = b64(case["source"]["base64"], "source")
        self.path = case["source"]["path"]
        self.positions = {0: (1, 1)}
        at, line, column = 0, 1, 1
        for char in self.data.decode("utf-8"):
            at += len(char.encode("utf-8"))
            if char == "\n":
                line, column = line + 1, 1
            else:
                column += 1
            self.positions[at] = (line, column)

    def span(self, span, label="span"):
        vector(span, label)
        require(len(span) == 3, "origin", label + " triple")
        file_id, start, end = span
        equal(file_id, 0, label + " file id")
        integer(start, label + " start"); integer(end, label + " end")
        require(start <= end and start in self.positions and end in self.positions,
                "origin", label + " bounds/UTF-8 boundary")
        return start, end

    def location(self, loc, label="location"):
        fields(loc, {"file_id", "path", "start", "end", "line", "column", "end_line", "end_column"}, label)
        start, end = self.span([loc["file_id"], loc["start"], loc["end"]], label)
        equal(loc["path"], self.path, label + " display path")
        equal([loc["line"], loc["column"]], list(self.positions[start]), label + " start position")
        equal([loc["end_line"], loc["end_column"]], list(self.positions[end]), label + " end position")


# Lexical vocabulary is schema spelling. The independent scanner below uses
# source bytes, the specification's non-nested comments, ASCII identifiers and
# longest-match punctuation, never token results emitted by the compiler.
KEYWORDS = {word: word.title() for word in "fn let mut return if else while break continue struct pub mod use true false".split()}
PUNCTUATION = {"->": "Arrow", "==": "EqualEqual", "!=": "NotEqual", "<=": "LessEqual", ">=": "GreaterEqual",
               "&&": "AndAnd", "||": "OrOr", "(": "LParen", ")": "RParen", "{": "LBrace", "}": "RBrace",
               ":": "Colon", ";": "Semi", ",": "Comma", ".": "Dot",
               "=": "Equal", "+": "Plus", "-": "Minus", "*": "Star", "!": "Not",
               "<": "Less", ">": "Greater", "&": "Ampersand"}


def source_tokens(source, limits):
    data, tokens, pos, denied = source.data, [], 0, None
    while pos < len(data):
        start = pos
        if data[pos] in b" \t\r\n\v\f":
            while pos < len(data) and data[pos] in b" \t\r\n\v\f":
                pos += 1
            kind = "Trivia"
        elif data[pos:pos+2] == b"//":
            end = data.find(b"\n", pos + 2)
            pos, kind = (len(data) if end == -1 else end), "Trivia"
        elif data[pos:pos+2] == b"/*":
            end = data.find(b"*/", pos + 2)
            require(end != -1, "unsupported-source", "unterminated comment needs an explicit lexical oracle")
            pos, kind = end + 2, "Trivia"
        else:
            match = re.match(rb"[A-Za-z_][A-Za-z_0-9]*|[0-9]+", data[pos:])
            if match:
                word = match[0].decode("ascii")
                pos += len(match[0])
                kind = "Number" if word[0].isdigit() else KEYWORDS.get(word, "Ident")
            else:
                pair = data[pos:pos+2].decode("ascii", errors="replace")
                char = data[pos:pos+1].decode("ascii", errors="replace")
                spelling = pair if pair in PUNCTUATION else char
                require(spelling in PUNCTUATION, "unsupported-source", f"unmodeled lexical byte at {pos}")
                kind, pos = PUNCTUATION[spelling], pos + len(spelling)
        span = [0, start, pos]
        if len(tokens) == limits["non_eof_tokens"] or pos - start > limits["token_bytes"]:
            denied = span
            break
        tokens.append({"kind": kind, "span": span})
    if denied is None:
        tokens.append({"kind": "Eof", "span": [0, len(data), len(data)]})
    return {"tokens": tokens, "non_eof_tokens": sum(t["kind"] != "Eof" for t in tokens),
            "eof_tokens": sum(t["kind"] == "Eof" for t in tokens),
            "colon_spans": [t["span"] for t in tokens if t["kind"] == "Colon"],
            "maximum_token_bytes": max((t["span"][2] - t["span"][1] for t in tokens), default=0),
            "denied_token_span": denied}


def projection(actual, expected, label):
    if type(expected) is dict:
        obj(actual, label)
        for key, value in expected.items():
            require(key in actual, "missing-signal", label + "." + key)
            projection(actual[key], value, label + "." + key)
    else:
        equal(actual, expected, label)


def diagnostics(observation, source, limit):
    rows = vector(observation["diagnostics"], "diagnostics")
    require(len(rows) <= limit, "diagnostic-cap", "too many diagnostics")
    rendered = vector(observation["json_diagnostic_bytes_base64"], "JSON diagnostic bytes")
    equal(len(rendered), len(rows), "JSON diagnostic rendering count")
    for index, row in enumerate(rows):
        fields(row, {"schema_version", "edition", "kind", "severity", "code", "stage", "message", "primary", "secondary", "notes"}, "diagnostic")
        projection(row, {"schema_version": 1, "edition": "typed-preview", "kind": "diagnostic", "severity": "error"}, "diagnostic envelope")
        for key in ("code", "stage", "message"):
            text(row[key], key)
        source.location(row["primary"], "primary")
        for secondary in vector(row["secondary"], "secondary"):
            fields(secondary, {"span", "message"}, "secondary label")
            source.location(secondary["span"], "secondary origin")
            text(secondary["message"], "secondary message")
        for note in vector(row["notes"], "notes"):
            text(note, "diagnostic note")
        raw = b64(rendered[index], "JSON diagnostic rendering")
        require(b"\n" not in raw and b"\r" not in raw, "diagnostic-bytes", "rendering includes an envelope separator")
        equal(loads(raw), row, "actual JSON bytes decode to full diagnostic")
    human = b64(observation["human_diagnostic_bytes_base64"], "human diagnostic rendering")
    if not rows:
        equal(human, b"", "empty diagnostic stream")
    else:
        require(bool(human), "diagnostic-bytes", "nonempty diagnostics require human rendering")
    return rows


def field_scans(observation, source, tape):
    scans = vector(observation["field_pub_scans"], "field scans")
    visited, initiators = set(), set()
    for scan in scans:
        fields(scan, SCAN_FIELDS | SCAN_EVIDENCE, "field scan")
        start = integer(scan["initiator_token"], "scan initiator")
        require(start < len(tape) and tape[start]["kind"] == "Pub", "field-scan", "initiator must be real Pub")
        require(start not in initiators, "field-scan", "repeated initiating Pub")
        initiators.add(start)
        inspected = vector(scan["inspected_tokens"], "scan inspections")
        require(bool(inspected), "field-scan", "empty field scan")
        for index in inspected:
            integer(index, "inspection index")
        end = start + 1
        while end < len(tape) and tape[end]["kind"] in TRIVIA:
            end += 1
        require(end < len(tape), "field-scan", "scan lacks terminal token")
        equal(inspected, list(range(start + 1, end + 1)), "full actual forward scan range")
        require(not visited.intersection(inspected), "field-scan", "overlapping forward scans")
        visited.update(inspected)
        terminal = tape[end]["kind"]
        equal(scan["terminal_kind"], terminal, "scan terminal kind")
        equal(scan["result_is_ident"], terminal == "Ident", "scan result")
        equal(scan["eof_charge_initiator"], start if terminal == "Eof" else None, "EOF scan charge")
        begin, end_seq = integer(scan["start_seq"], "scan start seq"), integer(scan["end_seq"], "scan end seq")
        require(begin < end_seq < len(observation["events"]), "field-scan", "scan event boundary range")
        for index, kind in ((begin, "field_scan_enter"), (end_seq, "field_scan_exit")):
            event = observation["events"][index]
            equal([event["kind"], event["cursor"]], [kind, start], "scan linked to real event boundary")
        for key in ("source_bytes_read", "namespace_work_units", "reserve_calls", "append_calls"):
            equal(scan[key], 0, "no " + key + " during field scan")
    require(len(visited) <= observation["token_inventory"]["non_eof_tokens"], "field-scan", "extra logical scans exceed non-EOF token count")
    for read in vector(observation["field_current_token_reads"], "current token reads"):
        fields(read, {"cursor", "token_kind", "span"}, "current token read")
        index = integer(read["cursor"], "current token cursor")
        require(index < len(tape), "field-scan", "current token cursor outside tape")
        equal([read["token_kind"], read["span"]], [tape[index]["kind"], tape[index]["span"]], "current token read")
    return scans


def previous_significant(tape, cursor):
    cursor -= 1
    while cursor >= 0 and tape[cursor]["kind"] in TRIVIA:
        cursor -= 1
    return cursor


def next_significant(tape, cursor):
    cursor += 1
    while cursor < len(tape) and tape[cursor]["kind"] in TRIVIA:
        cursor += 1
    return cursor


def recognition_trigger(event, prior, tape, scans, source):
    trigger, cursor = event["production"], event["cursor"]
    token = tape[cursor]
    consumed = [e for e in prior if e["kind"] == "consume" and e["context"] == "grammar"]
    admitted = [e for e in prior if e["kind"] == "node_admit"]
    previous = previous_significant(tape, cursor)
    if trigger in {"top_public", "field_public", "module_public"}:
        require(previous >= 0 and tape[previous]["kind"] == "Pub", "recognition-trigger", "public trigger has no preceding Pub")
        require(any(e["cursor"] == previous and e["token_kind"] == "Pub" for e in consumed),
                "recognition-trigger", "recognition preceded actual Pub consumption")
        if trigger == "top_public":
            require(token["kind"] in {"Fn", "Struct"}, "recognition-trigger", "top_public is not Pub+Fn/Struct")
            declaration = "function" if token["kind"] == "Fn" else "record"
            require(not any(e["kind"] in {"node_attempt", "node_admit", "node_reject"} and e["production"] == declaration and e["cursor"] == cursor for e in prior),
                    "event-order", "top-level public recognition occurred after its declaration node gate")
        elif trigger == "field_public":
            require(token["kind"] == "Ident", "recognition-trigger", "field public requires Ident")
            public_consumes = [e for e in consumed if e["cursor"] == previous and e["token_kind"] == "Pub"]
            require(any(s["initiator_token"] == previous and s["result_is_ident"] and
                        any(s["end_seq"] < e["seq"] < event["seq"] for e in public_consumes) for s in scans),
                    "event-order", "successful field scan must finish before grammar Pub consumption and recognition")
            require(any(e["production"] == "field" and e["cursor"] == previous for e in admitted),
                    "event-order", "field node admission must precede recognition")
        else:
            require(token["kind"] == "Mod", "recognition-trigger", "module_public does not precede Mod")
            require(any(e["production"] == "module" and e["cursor"] == previous for e in admitted),
                    "event-order", "public module node admission must precede recognition")
    elif trigger in {"module_keyword", "import_keyword"}:
        keyword, production = ("Mod", "module") if trigger == "module_keyword" else ("Use", "import")
        # The real assignment can be immediately before or after the keyword
        # consume. Both obey the contract; admission must precede both.
        keyword_index = cursor if token["kind"] == keyword else previous
        require(keyword_index >= 0 and tape[keyword_index]["kind"] == keyword,
                "recognition-trigger", "missing module/import keyword")
        matching_gates = {keyword_index}
        before_keyword = previous_significant(tape, keyword_index)
        if production == "module" and before_keyword >= 0 and tape[before_keyword]["kind"] == "Pub":
            matching_gates.add(before_keyword)
        require(any(e["production"] == production and e["cursor"] in matching_gates for e in admitted),
                "event-order", "keyword recognition before node admission")
        require(any(e["cursor"] == keyword_index and e["token_kind"] == keyword for e in consumed),
                "recognition-trigger", "module/import recognition requires its consumed keyword")
    elif trigger in {"qualified_item_path", "qualified_borrow", "qualified_field"}:
        # An assignment may be at the first colon or at the path's name. Bind
        # the pair to that current name/colon rather than scanning all source.
        left = cursor if token["kind"] == "Colon" else next_significant(tape, cursor)
        require(left + 1 < len(tape) and tape[left]["kind"] == tape[left+1]["kind"] == "Colon",
                "recognition-trigger", "qualified trigger lacks adjacent colon tokens")
        equal(tape[left]["span"][2], tape[left+1]["span"][1], "source-adjacent contextual colons")
        name_index = previous_significant(tape, left)
        require(name_index >= 0 and tape[name_index]["kind"] == "Ident", "recognition-trigger", "path begins at name")
    else:
        raise Rejected("unsupported-expectation", "unrecognized grammar trigger " + str(trigger))


def events(observation, case, source, tape):
    rows = vector(observation["events"], "events")
    transitions = vector(observation["recognition_transitions"], "recognition transitions")
    require(len(transitions) <= 1, "recognition", "multiple false-to-true transitions")
    equal(transitions, [r for r in rows if r.get("kind") == "recognize"], "recognition transitions are exact event subsequence")
    recovery, recognized, nodes, pending_node = False, False, 0, None
    budget = integer(case.get("seam", {}).get("node_limit", case["limits"]["nodes"]), "initial node budget")
    targeted = case.get("seam", {}).get("reject_node_admission")
    target_attempts = 0
    if targeted is not None:
        fields(targeted, {"kind", "occurrence"}, "targeted real node-admission seam")
        text(targeted["kind"], "targeted production")
        integer(targeted["occurrence"], "targeted occurrence", 1)
    reserve_rows, scan_rows, scan_open = [], [], None
    segment = None
    last_rejection, last_cursor = None, 0
    for seq, event in enumerate(rows):
        fields(event, {"seq", "kind", "production", "cursor", "token_kind", "span", "cursor_span", "cursor_token_kind", "context", "detail"}, "event")
        equal(event["seq"], seq, "contiguous event sequence")
        kind, production, detail = event["kind"], event["production"], event["detail"]
        require(kind in EVENT_KINDS, "unknown-event", str(kind))
        text(production, "event production")
        if kind in {"consume", "recover_enter", "recover_inspect", "recover_exit", "field_scan_enter", "field_scan_exit"}:
            equal(detail, None, "no invented payload on structural event")
        cursor = integer(event["cursor"], "event cursor")
        require(cursor < len(tape), "event-origin", "cursor outside real tape")
        require(cursor >= last_cursor, "event-order", "parser cursor moved backward within one mode")
        last_cursor = cursor
        equal([event["cursor_token_kind"], event["cursor_span"]], [tape[cursor]["kind"], tape[cursor]["span"]], "actual cursor token identity")
        source.span(event["span"], "event span")
        if production != "path_segment":
            equal([event["token_kind"], event["span"]], [tape[cursor]["kind"], tape[cursor]["span"]], "event cursor token identity")
        else:
            require(event["span"] in [t["span"] for t in tape if t["kind"] == "Ident"],
                    "event-origin", "segment span must name a real identifier token")
        if kind == "recover_enter":
            require(not recovery, "recovery", "nested recovery entry")
            recovery = True
        equal(event["context"], "recovery" if recovery else "grammar", "actual recovery context")
        if kind == "recover_exit":
            require(recovery, "recovery", "recovery exit without entry")
            recovery = False
        if kind == "recover_inspect":
            require(recovery, "recovery", "inspection outside recovery")
        if recovery:
            require(kind not in {"node_attempt", "node_admit", "node_reject", "reserve", "append"},
                    "recovery-work", "recovery scanning cannot admit or append grammar")
        if kind in {"recognize", "recognize_already_true"}:
            require(not recovery, "recovery-recognition", "recovery must not activate grammar")
            equal(detail, {"before": recognized, "after": True}, "recognition before/after")
            equal(kind, "recognize_already_true" if recognized else "recognize", "sticky transition kind")
            recognition_trigger(event, rows[:seq], tape, observation["field_pub_scans"], source)
            recognized = True
        if kind == "node_attempt":
            require(pending_node is None, "node-ledger", "unfinished prior node attempt")
            equal(detail, nodes, "node attempt actual ledger")
            pending_node = event
            if targeted is not None and production == targeted["kind"]:
                target_attempts += 1
                if target_attempts == targeted["occurrence"]:
                    budget = nodes
            previous = previous_significant(tape, cursor)
            grammar_public = previous >= 0 and tape[previous]["kind"] == "Pub" and any(
                e["kind"] == "consume" and e["context"] == "grammar" and e["cursor"] == previous for e in rows[:seq])
            if production in {"function", "record"} and grammar_public:
                require(any(e["kind"] in {"recognize", "recognize_already_true"} and e["production"] == "top_public" and e["cursor"] == cursor for e in rows[:seq]),
                        "event-order", "public Fn/Struct node gate precedes grammar recognition")
            require(seq + 1 < len(rows) and rows[seq+1].get("kind") in {"node_admit", "node_reject"},
                    "node-ledger", "node decision must immediately follow real gate attempt")
        if kind in {"node_admit", "node_reject"}:
            require(pending_node is not None, "node-ledger", "node decision without attempt")
            equal([production, cursor, event["span"]],
                  [pending_node["production"], pending_node["cursor"], pending_node["span"]], "same node admission")
            if kind == "node_admit":
                nodes += 1
                require(nodes <= budget, "node-ledger", "effective node limit exceeded")
            else:
                require(nodes >= budget,
                        "node-ledger", "node rejected below actual permitted budget")
                last_rejection = event
            equal(detail, nodes, "node decision actual ledger")
            pending_node = None
        if kind == "path_count_checked":
            integer(detail, "path count")
            segment = {"event": event, "count": detail, "decision": None}
        if kind in {"path_count_overflow", "path_cap_reject"}:
            require(segment is not None, "path-ledger", "path rejection lacks checked-add event")
            equal(event["span"], segment["event"]["span"], "path rejection span")
            if kind == "path_count_overflow":
                equal(segment["count"], (1 << 64) - 1, "path checked-add overflow at usize::MAX")
                equal(detail, segment["count"], "overflow leaves count unchanged")
            else:
                equal(detail, segment["count"] + 1, "path cap next count")
                require(detail > case["limits"]["q"], "path-ledger", "premature path cap rejection")
            require(not any(e["kind"] in {"node_attempt", "node_admit", "reserve", "append"} for e in rows[segment["event"]["seq"]+1:seq]),
                    "event-order", "path reject must precede node/reserve/append")
            segment["decision"] = "rejected"
            last_rejection = event
        if production == "path_segment" and kind == "node_attempt":
            require(segment is not None and segment["decision"] is None, "path-ledger", "segment node gate lacks accepted count check")
            require(segment["count"] < case["limits"]["q"], "path-ledger", "path node charged after cap")
            equal(event["span"], segment["event"]["span"], "segment node count span")
        # Enforce the existing barrier before this event can itself become a
        # new allocator-failure barrier. A failed reserve cannot approve itself.
        if kind in {"reserve", "append"} and last_rejection is not None:
            require(any(e["kind"] == "node_attempt" for e in rows[last_rejection["seq"]+1:seq]),
                    "event-order", "reserve/append after rejected admission or allocator failure")
        if kind == "reserve":
            fields(detail, {"kind", "length", "element_bytes", "success"}, "reserve event")
            equal(production, detail["kind"], "reserve kind")
            integer(detail["length"], "reserve length", 1)
            integer(detail["element_bytes"], "reserve element bytes", 1)
            require(type(detail["success"]) is bool, "schema", "reserve success boolean")
            reserve_rows.append(detail)
            if not detail["success"]:
                last_rejection = event
        if kind == "append":
            integer(detail, "post-append length", 1)
        if kind == "consume" and not recovery and event["token_kind"] in {"Mod", "Use", "Pub"}:
            token_kind = event["token_kind"]
            next_cursor = next_significant(tape, cursor)
            public_module = token_kind == "Pub" and next_cursor < len(tape) and tape[next_cursor]["kind"] == "Mod"
            if token_kind in {"Mod", "Use"} or public_module:
                production_kind = "import" if token_kind == "Use" else "module"
                allowed_cursors = {cursor}
                if token_kind == "Mod":
                    allowed_cursors.add(previous_significant(tape, cursor))
                require(any(e["kind"] == "node_admit" and e["production"] == production_kind and e["cursor"] in allowed_cursors for e in rows[:seq]),
                        "event-order", "module/import keyword consumed before its node admission")
        if kind == "field_scan_enter":
            require(scan_open is None, "field-scan", "nested logical scan")
            require(tape[cursor]["kind"] == "Pub", "field-scan", "scan enter must point to Pub")
            require(any(e["kind"] == "node_admit" and e["production"] == "field" and e["cursor"] == cursor for e in rows[:seq]),
                    "event-order", "scan before field node admission")
            scan_open = event
        elif kind == "field_scan_exit":
            require(scan_open is not None, "field-scan", "scan exit without enter")
            equal(cursor, scan_open["cursor"], "scan boundary initiator")
            scan_rows.append(cursor)
            scan_open = None
        elif scan_open is not None:
            require(kind not in {"node_attempt", "node_admit", "reserve", "append", "identifier_byte_scan", "namespace_work_debit"},
                    "field-scan-work", "prohibited work inside logical field scan")
    require(not recovery, "recovery", "unclosed recovery scope")
    require(pending_node is None, "node-ledger", "unfinished node attempt")
    require(scan_open is None, "field-scan", "unclosed field scan")
    equal(reserve_rows, observation["reserve_trace"], "reserve trace is ordered actual event projection")
    equal(len(reserve_rows), observation["reserve_attempts"], "reserve attempt ledger")
    equal(scan_rows, [s["initiator_token"] for s in observation["field_pub_scans"]], "actual field scan event boundaries")
    lexical = observation["result"] == "lex_error"
    direct = observation["result"] == "direct_seam_error"
    equal(observation["recognized_initial"], None if lexical else False, "false initial recognition")
    equal(observation["recognized_final"], None if lexical or direct else recognized, "sticky final recognition from events")
    equal(observation["nodes_admitted"], None if lexical else nodes, "node ledger final")
    if lexical:
        equal(rows, [], "lex rejection must not enter parser")
    if direct:
        equal([r["kind"] for r in rows], ["path_count_checked", "path_count_overflow"], "direct checked-counter operation")
    return rows


def required_first_admission(case, observation, wanted):
    """Bind required_order to the source-declared first gate, never a subsequence.

    The frozen cases declare either a zero-budget first top-level admission or
    the first field admission in a record. The enclosing record's gate and
    consumes are part of the latter's causal prefix, not candidate-derived
    ordering expectations. Ordinary current-token reads live outside events.
    Global events() still validates the complete trace, including later retries.
    """
    tape = source_tokens(Source(case), case["limits"])["tokens"]
    significant = [(i, t["kind"]) for i, t in enumerate(tape) if t["kind"] not in {"Trivia", "Eof"}]
    require(bool(significant), "required-order", "first admission needs source grammar")
    seam = case.get("seam", {})
    prefix = []
    if "reject_node_admission" in seam:
        equal(seam["reject_node_admission"], {"kind": "field", "occurrence": 1}, "supported first field admission seam")
        equal([kind for _, kind in significant[:4]], ["Struct", "Ident", "LBrace", "Pub"], "first field source grammar")
        record, name, opening, cursor = [i for i, _ in significant[:4]]
        production = "field"
        prefix = [("node_attempt", "record", record), ("node_admit", "record", record),
                  ("consume", "token", record), ("consume", "token", name), ("consume", "token", opening)]
        equal(wanted, ["field_node_reject"], "supported first field order")
    else:
        equal(seam, {"node_limit": 0}, "supported first top-level admission seam")
        first, first_kind = significant[0]
        cursor = first
        if first_kind == "Pub":
            require(len(significant) >= 2, "required-order", "public admission lacks a declaration")
            declaration, kind = significant[1]
            require(kind in {"Mod", "Fn", "Struct"}, "required-order", "unsupported public admission grammar")
            if kind in {"Fn", "Struct"}:
                cursor = declaration
                prefix = [("consume", "token", first), ("recognize", "top_public", cursor)]
        else:
            kind = first_kind
            require(kind in {"Mod", "Use"}, "required-order", "unsupported first admission grammar")
        production = {"Mod": "module", "Use": "import", "Fn": "function", "Struct": "record"}[kind]
        equal(wanted, ["consume_pub", "recognize", "node_reject"] if prefix else ["node_reject"], "supported first top-level order")
    rows = observation["events"]
    first_attempt = next((i for i, event in enumerate(rows)
                          if event["kind"] == "node_attempt" and event["production"] == production), None)
    require(first_attempt is not None, "required-order", "missing first relevant node attempt")
    require(first_attempt + 1 < len(rows), "required-order", "missing first relevant node decision")
    bounded = rows[:first_attempt + 2]
    causal = prefix + [("node_attempt", production, cursor), ("node_reject", production, cursor)]
    equal([(event["kind"], event["production"], event["cursor"]) for event in bounded], causal,
          "full source-bound first admission causal prefix")
    require(all(event["context"] == "grammar" for event in bounded), "required-order", "first admission prefix must be grammar")
    projected = []
    for event in bounded:
        name = event["kind"]
        if name == "consume" and event["token_kind"] == "Pub":
            name = "consume_pub"
        if name == "node_reject" and event["production"] == "field":
            name = "field_node_reject"
        if name in wanted:
            projected.append(name)
    equal(projected, wanted, "required real event order around first admission")


def expected_predicates(case, observation):
    expected = case["expected"]
    require(expected.keys() <= EXPECTED_FIELDS, "unsupported-expectation", "unknown expected key")
    direct = {"result": "result", "recognized_final": "recognized_final", "syntax_flavor": "syntax_flavor",
              "parse_attempts": "parse_attempts", "diagnostics_exact": "diagnostics",
              "recognition_transitions_exact": "recognition_transitions", "nodes_exact": "nodes_admitted",
              "reserve_attempts_exact": "reserve_attempts", "reserve_trace_exact": "reserve_trace",
              "path_segment_count_after": "path_segment_count_after"}
    for key, value in expected.items():
        if key in direct:
            equal(observation[direct[key]], value, key)
        elif key == "human_diagnostics_base64_exact":
            equal(b64(observation["human_diagnostic_bytes_base64"], key), b64(value, key), key)
        elif key == "field_pub_scans_exact":
            equal([{k: s[k] for k in SCAN_FIELDS} for s in observation["field_pub_scans"]], value, key)
        elif key == "first_diagnostic_projection":
            require(bool(observation["diagnostics"]), "diagnostic-projection", "first diagnostic is absent")
            projection(observation["diagnostics"][0], value, key)
        elif key in {"eof_tokens", "non_eof_tokens"}:
            equal(observation["token_inventory"][key], value, key)
        elif key in {"diagnostic_count_exact", "recognition_transitions_count", "field_pub_lookahead_calls"}:
            field = {"diagnostic_count_exact": "diagnostics", "recognition_transitions_count": "recognition_transitions", "field_pub_lookahead_calls": "field_pub_scans"}[key]
            equal(len(observation[field]), value, key)
        elif key == "field_lookahead_inspections_bound":
            equal(value, "non_eof_tokens", "supported field scan bound")
            require(sum(len(s["inspected_tokens"]) for s in observation["field_pub_scans"]) <= observation["token_inventory"][value], "field-scan", key)
        elif key == "field_scans_disjoint":
            equal(value, True, "supported disjoint predicate")
            indices = [i for s in observation["field_pub_scans"] for i in s["inspected_tokens"]]
            equal(len(indices), len(set(indices)), key)
        elif key == "required_order":
            required_first_admission(case, observation, value)
        elif key == "relation_to_original":
            require(set(value) <= RELATIONS, "unsupported-expectation", "unknown original-mode relation")
        else:
            raise Rejected("unsupported-expectation", key)


def typed_ast(root):
    """Check each AST position against its public algebraic type, not just tags."""
    structures = {
        "Program": {"tokens": "[]Token", "functions": "[]Function", "expressions": "[]Expr", "records": "[]StructDecl", "items": "[]ItemId",
                    "modules": "[]ModuleDecl", "paths": "[]AbsolutePath", "path_segments": "[]Span", "imports": "[]ImportDecl", "source": "SourceProvenance", "project_syntax": "bool"},
        "SourceProvenance": {"text_len": "int", "file": "SourceFileId", "debug_non_exhaustive": "bool"},
        "Span": {"file": "SourceFileId", "start": "int", "end": "int"}, "Token": {"kind": "TokenKind", "span": "Span"},
        "AbsolutePath": {"span": "Span", "segment_start": "int", "segment_len": "int"},
        "ImportDecl": {"path": "PathId", "alias": "Span", "span": "Span"}, "Expr": {"kind": "ExprKind", "span": "Span"},
        "TypeSyntax": {"span": "Span", "kind": "TypeSyntaxKind"},
        "StructField": {"public": "?Span", "name": "Span", "ty": "TypeSyntax", "span": "Span"},
        "StructDecl": {"public": "?Span", "name": "Span", "fields": "[]StructField", "span": "Span", "end": "Span"},
        "ModuleDecl": {"name": "Span", "public": "?Span", "span": "Span"},
        "FieldInit": {"name": "Span", "value": "ExprId", "span": "Span"}, "Param": {"name": "Span", "ty": "TypeSyntax"},
        "Stmt": {"kind": "StmtKind", "span": "Span"}, "BodyBlock": {"body": "[]Stmt", "span": "Span", "end": "Span"},
        "Function": {"public": "?Span", "name": "Span", "params": "[]Param", "result": "TypeSyntax", "body": "BodyBlockId", "blocks": "[]BodyBlock", "end": "Span"},
    }
    enums = {
        "ItemPath": {"Unqualified": ["Span"], "Absolute": ["PathId"]},
        "ItemId": {kind: ["int"] for kind in ("Module", "Import", "Function", "Struct")},
        "ExprKind": {"Not": {"operand": "ExprId", "operator_span": "Span"},
                     "Logical": {"op": "LogicalOp", "left": "ExprId", "right": "ExprId", "operator_span": "Span"},
                     "Comparison": {"op": "ComparisonOp", "left": "ExprId", "right": "ExprId", "operator_span": "Span"},
                     "Arithmetic": {"op": "ArithmeticOp", "left": "ExprId", "right": "ExprId", "operator_span": "Span"},
                     "Bool": ["bool"], "Number": {"digits": "Span", "negative": "bool"}, "Unit": None, "Name": ["Span"],
                     "Call": {"callee": "ItemPath", "args": "[]Argument"}, "StructLiteral": {"record": "ItemPath", "fields": "[]FieldInit"},
                     "FieldRead": {"base": "Span", "field": "Span"}, "Group": ["ExprId"]},
        "TypeSyntaxKind": {"Name": ["ItemPath"], "Unit": None, "Reference": {"mutable": "bool", "referent": "ItemPath"}},
        "BorrowPlace": {"OwnerName": ["Span"], "ForwardedParameter": {"name": "Span", "star_span": "Span"}},
        "Argument": {"Value": ["ExprId"], "Borrow": {"mutable": "bool", "place": "BorrowPlace", "span": "Span"}},
        "StmtKind": {"Let": {"mutable": "bool", "name": "Span", "annotation": "?TypeSyntax", "init": "ExprId"},
                     "Assign": {"name": "Span", "operator_span": "Span", "value": "ExprId"},
                     "FieldAssign": {"base": "Span", "field": "Span", "target_span": "Span", "operator_span": "Span", "value": "ExprId"},
                     "Expr": ["ExprId"], "Return": ["?ExprId"], "Break": None, "Continue": None,
                     "While": {"condition": "ExprId", "body": "BodyBlockId"},
                     "If": {"condition": "ExprId", "then_block": "BodyBlockId", "else_block": "?BodyBlockId"}},
        "ArithmeticOp": dict.fromkeys(("Add", "Subtract", "Multiply")),
        "ComparisonOp": dict.fromkeys(("Equal", "NotEqual", "Less", "LessEqual", "Greater", "GreaterEqual")),
        "LogicalOp": dict.fromkeys(("And", "Or")),
        "TokenKind": dict.fromkeys(set(KEYWORDS.values()) | set(PUNCTUATION.values()) | {"Trivia", "Ident", "Number", "String", "Unsupported", "Invalid", "Eof"}),
    }
    def tuple_value(value, tag, rules, label):
        fields(value, {"tag", "items"}, label)
        equal(value["tag"], tag, label + " tag")
        items = vector(value["items"], label + " items")
        equal(len(items), len(rules), label + " arity")
        for item, rule in zip(items, rules):
            validate(item, rule, label + " item")
    def struct_value(value, tag, rules, label):
        fields(value, set(rules) | {"tag"}, label)
        equal(value["tag"], tag, label + " tag")
        for field, rule in rules.items():
            validate(value[field], rule, label + "." + field)
    def validate(value, rule, label):
        if rule == "bool":
            require(type(value) is bool, "ast-schema", label + " must be boolean")
        elif rule == "int":
            integer(value, label)
        elif rule.startswith("[]"):
            for index, child in enumerate(vector(value, label)):
                validate(child, rule[2:], f"{label}[{index}]")
        elif rule.startswith("?"):
            if value is not None:
                tuple_value(value, "Some", [rule[1:]], label)
        elif rule in {"SourceFileId", "ExprId", "PathId", "BodyBlockId"}:
            tuple_value(value, rule, ["int"], label)
        elif rule in structures:
            struct_value(value, rule, structures[rule], label)
        elif rule in enums:
            obj(value, label)
            tag = value.get("tag")
            require(type(tag) is str and tag in enums[rule], "ast-schema", label + " variant")
            rules = enums[rule][tag]
            if rules is None:
                equal(value, {"tag": tag}, label + " unit variant")
            elif isinstance(rules, list):
                tuple_value(value, tag, rules, label)
            else:
                struct_value(value, tag, rules, label)
        else:
            raise Rejected("unsupported-expectation", "unknown AST type " + rule)
    validate(root, "Program", "Program")


def ast_record(observation, source):
    ast = observation["ast"]
    if observation["result"] != "ok":
        equal(ast, None, "failure has no fabricated AST")
        equal(observation["syntax_flavor"], None, "failure has no syntax flavor")
        return
    fields(ast, {"canonical", "syntax_flavor", "belongs_to_source", "source_generation", "file_id", "node_count", "spans_and_ids_valid"}, "AST")
    equal(ast["syntax_flavor"], observation["syntax_flavor"], "AST flavor")
    equal(ast["syntax_flavor"], "ProjectSyntax" if observation["recognized_final"] else "OriginalSingleFile", "AST flavor from actual recognition")
    equal(ast["belongs_to_source"], True, "AST belongs_to source")
    equal(ast["spans_and_ids_valid"], True, "producer AST validity")
    equal(ast["source_generation"], observation["binding"]["source_generation"], "actual AST provenance generation")
    equal(ast["file_id"], 0, "AST file id")
    equal(ast["node_count"], observation["nodes_admitted"], "AST node count")
    root = ast["canonical"]
    typed_ast(root)
    root_fields = {"tag", "tokens", "functions", "expressions", "records", "items", "modules", "paths", "path_segments", "imports", "source", "project_syntax"}
    fields(root, root_fields, "canonical Program")
    equal(root["tag"], "Program", "canonical Program tag")
    equal(root["project_syntax"], observation["recognized_final"], "canonical flavor flag")
    for key in root_fields - {"tag", "source", "project_syntax"}:
        vector(root[key], "Program." + key)
    provenance = root["source"]
    equal(provenance, {"tag": "SourceProvenance", "text_len": len(source.data),
                       "file": {"tag": "SourceFileId", "items": [0]}, "debug_non_exhaustive": True}, "complete exposed source provenance")
    def span_object(span):
        return {"tag": "Span", "file": {"tag": "SourceFileId", "items": [span[0]]}, "start": span[1], "end": span[2]}
    equal(root["tokens"], [{"tag": "Token", "kind": {"tag": token["kind"]}, "span": span_object(token["span"])}
                           for token in observation["token_inventory"]["tokens"]], "AST retains exact ordered lossless token tape")
    item_fields = {"Module": "modules", "Import": "imports", "Function": "functions", "Struct": "records"}
    item_indices = collections.defaultdict(list)
    for item in root["items"]:
        fields(item, {"tag", "items"}, "ordered item ID")
        require(item["tag"] in item_fields and type(item["items"]) is list and len(item["items"]) == 1,
                "ast-id", "unknown item ID")
        index = integer(item["items"][0], "item ID")
        require(index < len(root[item_fields[item["tag"]]]), "ast-id", "item ID out of arena")
        item_indices[item["tag"]].append(index)
    for tag, arena in item_fields.items():
        equal(item_indices[tag], list(range(len(root[arena]))), "ordered declaration local IDs " + tag)
    for path in root["paths"]:
        fields(path, {"tag", "span", "segment_start", "segment_len"}, "absolute path")
        equal(path["tag"], "AbsolutePath", "path tag")
        start, length = integer(path["segment_start"], "path segment start"), integer(path["segment_len"], "path segment length", 1)
        require(length <= 34 and start + length <= len(root["path_segments"]), "ast-id", "path segment arena range")
    shape = {
        "Program": root_fields - {"tag"}, "Token": {"kind", "span"},
        "SourceProvenance": {"text_len", "file", "debug_non_exhaustive"}, "Span": {"file", "start", "end"},
        "AbsolutePath": {"span", "segment_start", "segment_len"}, "ImportDecl": {"path", "alias", "span"},
        "Expr": {"kind", "span"}, "TypeSyntax": {"span", "kind"},
        "StructField": {"public", "name", "ty", "span"}, "StructDecl": {"public", "name", "fields", "span", "end"},
        "ModuleDecl": {"name", "public", "span"}, "FieldInit": {"name", "value", "span"}, "Param": {"name", "ty"},
        "Stmt": {"kind", "span"}, "BodyBlock": {"body", "span", "end"},
        "Function": {"public", "name", "params", "result", "body", "blocks", "end"},
        "Not": {"operand", "operator_span"}, "Logical": {"op", "left", "right", "operator_span"},
        "Comparison": {"op", "left", "right", "operator_span"}, "Arithmetic": {"op", "left", "right", "operator_span"},
        "Number": {"digits", "negative"}, "Call": {"callee", "args"}, "StructLiteral": {"record", "fields"},
        "FieldRead": {"base", "field"}, "Reference": {"mutable", "referent"},
        "ForwardedParameter": {"name", "star_span"}, "Borrow": {"mutable", "place", "span"},
        "Let": {"mutable", "name", "annotation", "init"}, "Assign": {"name", "operator_span", "value"},
        "FieldAssign": {"base", "field", "target_span", "operator_span", "value"},
        "While": {"condition", "body"}, "If": {"condition", "then_block", "else_block"},
    }
    tuples = {"SourceFileId", "ExprId", "PathId", "BodyBlockId", "Unqualified", "Absolute", "Bool", "Name", "Group", "Some",
              "Module", "Import", "Function", "Struct", "OwnerName", "Value", "Expr", "Return"}
    leaves = set(KEYWORDS.values()) | set(PUNCTUATION.values()) | {"Trivia", "Ident", "Number", "String", "Unsupported", "Invalid", "Eof",
              "Unit", "Add", "Subtract", "Multiply", "Equal", "NotEqual", "Less", "LessEqual", "Greater", "GreaterEqual", "And", "Or", "Break", "Continue"}
    def walk(value, block_limit=None):
        if isinstance(value, list):
            for child in value:
                walk(child, block_limit)
        elif isinstance(value, dict):
            require("tag" in value, "ast-schema", "untagged canonical object")
            tag = value["tag"]
            if tag in tuples and "items" in value:
                fields(value, {"tag", "items"}, "canonical tuple " + tag)
                require(type(value["items"]) is list and len(value["items"]) == 1, "ast-schema", "canonical tuple arity")
            elif tag in shape and len(value) > 1:
                fields(value, shape[tag] | {"tag"}, "complete canonical structure " + tag)
            else:
                require(tag in leaves and set(value) == {"tag"}, "ast-schema", "unknown/incomplete canonical tag " + str(tag))
            if tag == "Span":
                fields(value, {"tag", "file", "start", "end"}, "canonical span")
                equal(value["file"], {"tag": "SourceFileId", "items": [0]}, "canonical span source owner")
                source.span([0, value["start"], value["end"]], "canonical AST span")
            elif tag == "SourceFileId":
                equal(value, {"tag": "SourceFileId", "items": [0]}, "source file local ID")
            elif tag in {"ExprId", "PathId", "BodyBlockId"}:
                fields(value, {"tag", "items"}, "canonical ID")
                require(type(value["items"]) is list and len(value["items"]) == 1, "ast-id", "ID wrapper")
                index = integer(value["items"][0], "canonical local ID")
                bound = len(root["expressions"]) if tag == "ExprId" else len(root["paths"]) if tag == "PathId" else block_limit
                require(bound is not None and index < bound, "ast-id", "local ID outside owning arena")
            elif tag == "Function" and "blocks" in value:
                block_limit = len(vector(value["blocks"], "function blocks"))
            if value.get("debug_non_exhaustive") is not None:
                require(tag == "SourceProvenance", "ast-schema", "unqualified omitted AST fields")
            for key, child in value.items():
                if key != "tag":
                    walk(child, block_limit)
        else:
            require(value is None or type(value) in {str, int, bool}, "ast-schema", "unsupported canonical leaf")
    walk(root)


OBSERVATION_FIELDS = set("schema binding result executed parse_attempts diagnostics json_diagnostic_bytes_base64 human_diagnostic_bytes_base64 token_inventory nodes_admitted recognized_initial recognized_final recognition_transitions events field_pub_scans field_current_token_reads reserve_attempts reserve_trace ast syntax_flavor path_segment_count_after observation_complete observer_limits".split())
BINDING_FIELDS = set("contract_decoded_sha256 package_freeze_sha256 case_id source_sha256 source_bytes display_path candidate_source_manifest_sha256 observer_source_sha256 binary_sha256 profile actual_runtime_os actual_runtime_architecture actual_pointer_width rust_compiler_target mode seam execution_id source_generation mode_execution_index".split())


def check_observation(case, observation, identities, apply_expected=True):
    fields(observation, OBSERVATION_FIELDS, "observation")
    equal(observation["schema"], SCHEMA, "observation schema")
    fields(observation["binding"], BINDING_FIELDS, "binding")
    binding = observation["binding"]
    expected_binding = {"contract_decoded_sha256": CONTRACT_SHA, "package_freeze_sha256": FREEZE_SHA,
                        "case_id": case["id"], "source_sha256": case["source"]["sha256"],
                        "source_bytes": case["source"]["bytes"], "display_path": case["source"]["path"],
                        "seam": case.get("seam", {}), **PLATFORM}
    for key, value in expected_binding.items():
        equal(binding[key], value, "binding " + key)
    require(binding["profile"] in PROFILES, "identity", "unknown profile")
    profile = identities[binding["profile"]]
    for key in ("candidate_source_manifest_sha256", "observer_source_sha256", "binary_sha256"):
        equal(binding[key], profile[key], "independent build binding " + key)
        require(re.fullmatch("[0-9a-f]{64}", binding[key]) is not None, "identity", "invalid SHA256")
    require(binding["mode"] in {"ProjectCandidate", "OwnedCandidate"}, "identity", "unknown parser mode")
    equal(binding["mode_execution_index"], 0 if binding["mode"] == "ProjectCandidate" else 1, "distinct real mode execution index")
    require(bool(text(binding["execution_id"], "execution id")), "identity", "empty execution id")
    integer(binding["source_generation"], "source generation", 1)
    equal(observation["executed"], True, "actual execution marker")
    equal(observation["observation_complete"], True, "complete untruncated observation")
    fields(observation["observer_limits"], {"events", "evidence_bytes", "charged_bytes"}, "observer limits")
    equal(observation["observer_limits"]["events"], 1000000, "reviewed event budget")
    equal(observation["observer_limits"]["evidence_bytes"], 134217728, "reviewed byte budget")
    charged = integer(observation["observer_limits"]["charged_bytes"], "charged evidence bytes")
    require(charged <= 134217728 and len(observation["events"]) < 1000000, "truncated", "observer resource overflow")
    require(observation["result"] in {"ok", "parse_error", "lex_error", "direct_seam_error"}, "schema", "unknown result")
    equal(observation["parse_attempts"], 0 if observation["result"] in {"lex_error", "direct_seam_error"} else 1, "actual parse attempt count")
    source = Source(case)
    inventory = source_tokens(source, case["limits"])
    equal(observation["token_inventory"], inventory, "independent source-derived lossless token inventory")
    equal(observation["result"] == "lex_error", inventory["denied_token_span"] is not None, "lex resource admission result")
    equal(observation["result"] == "direct_seam_error", case.get("seam", {}).get("direct_operation") == "path_segment", "direct operation tagging")
    diagnostics(observation, source, case["limits"]["diagnostics"])
    equal(bool(observation["diagnostics"]), observation["result"] != "ok", "success/error diagnostic presence")
    field_scans(observation, source, inventory["tokens"])
    events(observation, case, source, inventory["tokens"])
    ast_record(observation, source)
    if apply_expected:
        expected_predicates(case, observation)


def compare_relation(case, project, original):
    p, o = project["binding"], original["binding"]
    for key in BINDING_FIELDS - {"mode", "mode_execution_index"}:
        equal(p[key], o[key], "two modes one immutable input: " + key)
    equal([p["mode"], o["mode"]], ["ProjectCandidate", "OwnedCandidate"], "distinct observed modes")
    equal([p["mode_execution_index"], o["mode_execution_index"]], [0, 1], "two actual mode executions")
    equal(project["result"], original["result"], "original-mode result relation")
    for relation in case["expected"]["relation_to_original"]:
        if relation in {"complete_ast", "local_ids", "source_provenance"}:
            require(project["ast"] is not None and original["ast"] is not None, "relation", "AST relation missing AST")
            equal(project["ast"]["canonical"], original["ast"]["canonical"], "complete ordered AST/IDs/provenance relation")
            for key in ("source_generation", "file_id", "belongs_to_source", "spans_and_ids_valid"):
                equal(project["ast"][key], original["ast"][key], "AST provenance relation " + key)
        elif relation == "node_count":
            equal(project["nodes_admitted"], original["nodes_admitted"], "node count relation")
            equal(project["ast"]["node_count"], original["ast"]["node_count"], "AST node count relation")
        elif relation == "diagnostic_fields_order_count":
            equal(project["diagnostics"], original["diagnostics"], relation)
        elif relation == "json_diagnostic_bytes":
            equal([b64(x, relation) for x in project["json_diagnostic_bytes_base64"]],
                  [b64(x, relation) for x in original["json_diagnostic_bytes_base64"]], relation)
        elif relation == "human_diagnostic_bytes":
            equal(b64(project["human_diagnostic_bytes_base64"], relation), b64(original["human_diagnostic_bytes_base64"], relation), relation)
        else:
            raise Rejected("unsupported-expectation", relation)


def compare_rows(contract, observations, identities):
    cases = {c["id"]: c for c in contract["cases"]}
    roster = {(profile, case["id"], mode) for profile in PROFILES for case in cases.values()
              for mode in (["ProjectCandidate", "OwnedCandidate"] if "relation_to_original" in case["expected"] else ["ProjectCandidate"])}
    actual, executions, issues = {}, {}, []
    for row in vector(observations, "observations"):
        try:
            binding = obj(row.get("binding"), "binding")
            key = (binding.get("profile"), binding.get("case_id"), binding.get("mode"))
            require(key in roster, "roster-extra", str(key))
            require(key not in actual, "roster-duplicate", str(key))
            actual[key] = row
            check_observation(cases[key[1]], row, identities, apply_expected=key[2] == "ProjectCandidate")
            execution = (binding["execution_id"], binding["mode_execution_index"])
            require(execution not in executions, "execution-reused", str(execution))
            executions[execution] = key
        except (Rejected, KeyError, TypeError, IndexError, UnicodeError) as error:
            issues.append({"row": len(actual), "code": error.code if isinstance(error, Rejected) else "malformed-evidence", "detail": str(error)})
    for key in sorted(roster - actual.keys()):
        issues.append({"code": "roster-missing", "detail": list(key)})
    for profile, case_id, mode in sorted(roster):
        if mode != "OwnedCandidate":
            continue
        project_key = (profile, case_id, "ProjectCandidate")
        original_key = (profile, case_id, "OwnedCandidate")
        if project_key in actual and original_key in actual:
            try:
                compare_relation(cases[case_id], actual[project_key], actual[original_key])
            except (Rejected, KeyError, TypeError, IndexError) as error:
                issues.append({"case": case_id, "profile": profile, "code": error.code if isinstance(error, Rejected) else "malformed-relation", "detail": str(error)})
    return {"schema": "oxid-unit4-parser-comparison-v1", "status": "pass" if not issues else "fail",
            "expected_observations": len(roster), "observations": len(observations), "issues": issues,
            "contract_decoded_sha256": CONTRACT_SHA, "package_freeze_sha256": FREEZE_SHA}


def artifact(record):
    """Verify an explicitly supplied local artifact identity, including absolute paths."""
    fields(record, {"path", "bytes", "sha256"}, "artifact identity")
    path = Path(text(record["path"], "artifact path"))
    require(path.is_absolute(), "identity", "execution artifact path must be absolute")
    raw = path.read_bytes()
    equal(len(raw), record["bytes"], "artifact size " + path.name)
    equal(sha(raw), record["sha256"], "artifact hash " + path.name)
    return raw


def canonical(value):
    return json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(",", ":"), allow_nan=False).encode("utf-8")


def changed_leaves(before, after, path=""):
    if same(before, after):
        return []
    if type(before) is dict and type(after) is dict and before.keys() == after.keys():
        return [leaf for key in sorted(before) for leaf in changed_leaves(before[key], after[key], path + "/" + key)]
    if type(before) is list and type(after) is list and len(before) == len(after):
        return [leaf for i, (a, b) in enumerate(zip(before, after)) for leaf in changed_leaves(a, b, path + "/" + str(i))]
    return [path]


def effective_receipt():
    return {"effective_contract_identity": EFFECTIVE_CONTRACT_ID,
            "effective_contract_descriptor_sha256": AMENDMENT_DESCRIPTOR_SHA,
            "base_parser_package_freeze_sha256": FREEZE_SHA,
            "base_parser_corpus_decoded_sha256": CONTRACT_SHA,
            "ordered_amendment_sha256": [AMENDMENT_SHA],
            "effective_parser_document_canonical_sha256": EFFECTIVE_DOCUMENT_SHA}


def amendment_artifacts(authority):
    fields(authority, {"checkpoint", "descriptor", "amendment", "review"}, "effective contract authority")
    parsed = {}
    for key, expected in {"checkpoint": AMENDMENT_CHECKPOINT_SHA, "descriptor": AMENDMENT_DESCRIPTOR_SHA,
                          "amendment": AMENDMENT_SHA, "review": AMENDMENT_REVIEW_SHA}.items():
        equal(authority[key]["sha256"], expected, "pinned amendment " + key)
        parsed[key] = loads(artifact(authority[key]))
    checkpoint, descriptor, amendment, review = [parsed[key] for key in ("checkpoint", "descriptor", "amendment", "review")]
    root = Path(authority["checkpoint"]["path"]).parent
    for member in checkpoint["files"]:
        verify_file(root, member)
    for key in ("descriptor", "amendment"):
        name = "effective-contract.json" if key == "descriptor" else "amendment.json"
        equal(Path(authority[key]["path"]).resolve(), (root / "artifacts" / name).resolve(), "amendment checkpoint member " + key)
    equal(checkpoint["identity"], EFFECTIVE_CONTRACT_ID, "amendment checkpoint identity")
    equal(review["status"], "PASS_SOURCE_ONLY_AMENDMENT", "source-only amendment review status")
    for field, expected in {"checkpoint_sha256": AMENDMENT_CHECKPOINT_SHA, "amendment_sha256": AMENDMENT_SHA,
                            "descriptor_sha256": AMENDMENT_DESCRIPTOR_SHA, "effective_document_canonical_sha256": EFFECTIVE_DOCUMENT_SHA,
                            "exact_changed_integer_leaves": 4, "unchanged_cases": 247,
                            "base_files_unchanged": True, "candidate_observations_used_to_derive_coordinates": 0}.items():
        equal(review[field], expected, "reviewed amendment " + field)
    for record in (checkpoint, descriptor):
        equal(record["base_parser_corpus_decoded_sha256"], CONTRACT_SHA, "amendment original decoded contract")
        equal(record["base_parser_package_freeze_sha256"], FREEZE_SHA, "amendment original freeze")
    equal(descriptor["identity"], EFFECTIVE_CONTRACT_ID, "effective descriptor identity")
    equal(descriptor["ordered_amendments"], [{"file": "amendment.json", "bytes": authority["amendment"]["bytes"], "sha256": AMENDMENT_SHA}], "one exact ordered amendment")
    equal(descriptor["receipt_binding"]["required_fields"], list(effective_receipt()), "complete effective receipt fields")
    return descriptor, amendment


def apply_coordinate_amendment(base, descriptor, amendment):
    """Apply only the reviewed source-authored four-integer delta to a copy."""
    equal(amendment["identity"], EFFECTIVE_CONTRACT_ID, "amendment identity")
    equal(amendment["base_parser_package_freeze_sha256"], FREEZE_SHA, "amendment base freeze")
    equal(amendment["base_document"]["decoded_sha256"], CONTRACT_SHA, "amendment base decoded identity")
    pointer = "/cases/246/expected/first_diagnostic_projection/primary"
    equal(amendment["case_pointer"], "/cases/246", "amendment exact case pointer")
    equal(amendment["case_id"], "public-root-module-before-reserved-function", "amendment exact case ID")
    allowed = [pointer + "/" + field for field in ("column", "end", "end_column", "start")]
    equal(amendment["exact_changed_json_pointers"], allowed, "amendment exact four pointers")
    case = base["cases"][246]
    equal(case["id"], amendment["case_id"], "amendment base case ID")
    equal(case["source"], amendment["source"], "amendment unchanged exact source")
    equal(sha(canonical(case)), amendment["old_case_canonical_sha256"], "amendment complete old case")
    diagnostic = case["expected"]["first_diagnostic_projection"]
    equal(diagnostic, amendment["old_diagnostic"], "amendment complete old diagnostic")
    equal(sha(canonical(diagnostic)), amendment["old_diagnostic_canonical_sha256"], "amendment old diagnostic hash")
    operation = amendment["operation"]
    fields(operation, {"op", "pointer", "old_value", "value"}, "single amendment operation")
    equal(operation["op"], "replace", "amendment operation")
    equal(operation["pointer"], pointer, "amendment primary pointer")
    equal(operation["old_value"], diagnostic["primary"], "amendment whole old primary")
    for field in ("column", "end", "end_column", "start"):
        integer(operation["old_value"][field], "old coordinate")
        integer(operation["value"][field], "new coordinate")
    effective = copy.deepcopy(base)
    effective_case = effective["cases"][246]
    effective_case["expected"]["first_diagnostic_projection"]["primary"] = copy.deepcopy(operation["value"])
    equal(changed_leaves(base, effective), allowed, "whole-document exact changed integer leaves")
    new_diagnostic = effective_case["expected"]["first_diagnostic_projection"]
    equal(new_diagnostic, amendment["new_diagnostic"], "amendment complete new diagnostic")
    equal(sha(canonical(new_diagnostic)), amendment["new_diagnostic_canonical_sha256"], "amendment new diagnostic hash")
    equal(sha(canonical(effective_case)), amendment["new_case_canonical_sha256"], "amendment complete new case")
    source = Source(case)
    tape = source_tokens(source, case["limits"])["tokens"]
    anchors = [token for i, token in enumerate(tape) if token["kind"] == "Pub" and previous_significant(tape, i) >= 0
               and tape[previous_significant(tape, i)]["kind"] == "Fn"]
    equal(len(anchors), 1, "unique source-declared reserved function-name Pub")
    primary = new_diagnostic["primary"]
    source.location(primary)
    equal([primary["file_id"], primary["start"], primary["end"]], anchors[0]["span"], "complete source Pub token coordinates")
    encoded = canonical(effective)
    equal(len(encoded), descriptor["effective_parser_document_canonical_bytes"], "full effective canonical bytes")
    equal(sha(encoded), descriptor["effective_parser_document_canonical_sha256"], "full effective descriptor identity")
    equal(sha(encoded), EFFECTIVE_DOCUMENT_SHA, "pinned full effective document identity")
    return effective


def admit_contract_amendment(base, contract_root, authority):
    descriptor, amendment = amendment_artifacts(authority)
    # Revalidate the complete base here so callers cannot submit an already
    # amended, truncated, stale or otherwise altered in-memory document.
    verified_base = load_contract(contract_root)
    equal(base, verified_base, "exact unchanged base contract before amendment")
    raw_gzip = (Path(contract_root) / amendment["base_document"]["name"]).read_bytes()
    raw = gzip.decompress(raw_gzip)
    for key, actual in {"gzip_bytes": len(raw_gzip), "gzip_sha256": sha(raw_gzip),
                        "decoded_bytes": len(raw), "decoded_sha256": sha(raw)}.items():
        equal(actual, amendment["base_document"][key], "amendment original document " + key)
    effective = apply_coordinate_amendment(verified_base, descriptor, amendment)
    return effective, effective_receipt()


def compare_effective_rows(effective, receipt, observations, identities):
    equal(sha(canonical(effective)), EFFECTIVE_DOCUMENT_SHA, "effective comparison document, not summary-only relabeling")
    equal(receipt, effective_receipt(), "exact effective comparison receipt")
    result = compare_rows(effective, observations, identities)
    result.update(receipt)
    result["execution_contract"] = {"contract_id": CONTRACT_ID, "contract_decoded_sha256": CONTRACT_SHA, "package_freeze_sha256": FREEZE_SHA}
    return result


def authorization(path, expected_sha):
    raw = Path(path).read_bytes()
    equal(sha(raw), expected_sha, "external approval artifact hash")
    approval = loads(raw)
    fields(approval, {"schema", "status", "contract_decoded_sha256", "package_freeze_sha256",
                      "authority_checkpoint", "observer_review", "observer_source_sha256",
                      "candidate_source_manifest_sha256", "comparator_checkpoint", "observer_source_root",
                      "collector_helper_rebind", "collector_helper_review", "effective_contract_authority"}, "comparison authorization")
    equal("collector_helper_rebind" in approval, "collector_helper_review" in approval, "paired collector helper approval artifacts")
    equal(approval["schema"], "oxid-unit4-parser-comparison-authorization-v2", "authorization schema")
    equal(approval["status"], "approved-for-comparison", "durable coordinator approval")
    equal(approval["contract_decoded_sha256"], CONTRACT_SHA, "approval contract")
    equal(approval["package_freeze_sha256"], FREEZE_SHA, "approval freeze")
    equal(approval["authority_checkpoint"]["sha256"], DURABLE_SHA, "approved durable authority identity")
    durable = loads(artifact(approval["authority_checkpoint"]))
    equal(durable["schema"], "oxid-unit4-durable-source-authority-v1", "durable authority schema")
    equal(durable["head"], DURABLE_COMMIT, "durable reviewed Git commit")
    equal(durable["active_parser_freeze_sha256"], FREEZE_SHA, "durable reviewed parser contract")
    equal(approval["observer_review"]["sha256"], OBSERVER_REVIEW_SHA, "independent observer review identity")
    review = loads(artifact(approval["observer_review"]))
    equal(review["observer_source_sha256"], REVIEWED_OBSERVER_SHA, "reviewed observer v4")
    equal(review["candidate_source_manifest_sha256"], REVIEWED_PRODUCTION_SHA, "reviewed production inputs")
    equal(approval["observer_source_sha256"], REVIEWED_OBSERVER_SHA, "approved v4 observer only")
    equal(approval["candidate_source_manifest_sha256"], REVIEWED_PRODUCTION_SHA, "approved production manifest only")
    checkpoint = loads(artifact(approval["comparator_checkpoint"]))
    equal(checkpoint["schema"], "oxid-unit4-parser-comparator-checkpoint-v1", "comparator checkpoint schema")
    root = Path(approval["comparator_checkpoint"]["path"]).parent
    require(root.resolve() == Path(__file__).resolve().parent, "identity", "executing comparator differs from approved checkpoint")
    for entry in checkpoint["files"]:
        verify_file(root, entry)
    amendment_artifacts(approval["effective_contract_authority"])
    return approval


def measured_host(host):
    fields(host, {"os", "architecture", "python_pointer_width", "uname", "python_executable", "python_version"}, "measured collector host")
    uname = host["uname"]
    fields(uname, {"system", "release", "version", "machine"}, "measured uname")
    for key in ("system", "release", "version", "machine"):
        require(bool(text(uname[key], "uname " + key)), "identity", "empty uname field")
    system = {"Linux": "linux", "Darwin": "macos", "Windows": "windows"}.get(uname["system"], uname["system"].lower())
    machine = uname["machine"].lower()
    architecture = {"amd64": "x86_64", "x86_64": "x86_64", "arm64": "aarch64", "aarch64": "aarch64"}.get(machine, machine)
    equal(host["os"], system, "measured OS normalization")
    equal(host["architecture"], architecture, "measured architecture normalization")
    equal({"actual_runtime_os": host["os"], "actual_runtime_architecture": host["architecture"],
           "actual_pointer_width": host["python_pointer_width"]},
          {key: PLATFORM[key] for key in ("actual_runtime_os", "actual_runtime_architecture", "actual_pointer_width")},
          "measured supported collector runtime tuple")
    require(Path(text(host["python_executable"], "collector Python executable")).is_absolute(), "identity", "absolute collector executable required")
    require(bool(text(host["python_version"], "collector Python version")), "identity", "collector Python version required")
    return host


def normalize_raw(raw, case, build, nonce, host_runtime):
    """Independent raw/normalized consistency check, not a candidate oracle."""
    from debug_decoder import decode
    host = measured_host(host_runtime)
    fields(raw, {"schema", "case_id", "nonce", "source_utf8", "display_path", "observations"}, "raw envelope")
    equal(raw["schema"], "oxid-unit4-parser-raw-v1", "raw schema")
    equal(raw["case_id"], case["id"], "raw case identity")
    equal(raw["nonce"], nonce, "raw execution identity")
    equal(text(raw["source_utf8"], "raw source").encode(), b64(case["source"]["base64"], "contract source"), "actual executed UTF-8 source")
    equal(raw["display_path"], case["source"]["path"], "actual executed display path")
    modes = ["ProjectCandidate"] + (["OwnedCandidate"] if "relation_to_original" in case["expected"] else [])
    equal(len(vector(raw["observations"], "raw observations")), len(modes), "raw actual mode count")
    rows = []
    for index, (mode, raw_row) in enumerate(zip(modes, raw["observations"])):
        raw_fields = (OBSERVATION_FIELDS - {"schema", "binding", "json_diagnostic_bytes_base64", "human_diagnostic_bytes_base64"}) | {
            "mode", "mode_execution_index", "source_generation", "runtime_os", "runtime_architecture", "pointer_width",
            "json_diagnostic_renderings", "human_diagnostic_rendering"}
        fields(raw_row, raw_fields, "raw mode observation")
        row = dict(raw_row)
        equal(row.pop("mode"), mode, "raw distinct parser mode")
        equal(row.pop("mode_execution_index"), index, "raw actual mode execution index")
        equal(row.pop("runtime_os"), host["os"], "raw Rust OS agrees with independently measured host")
        equal(row.pop("runtime_architecture"), host["architecture"], "raw Rust architecture agrees with independently measured host")
        pointer_width = row.pop("pointer_width")
        equal(pointer_width, host["python_pointer_width"], "raw Rust ABI agrees with independently measured host width")
        binding = {"contract_decoded_sha256": CONTRACT_SHA, "package_freeze_sha256": FREEZE_SHA,
                   "case_id": case["id"], "source_sha256": case["source"]["sha256"], "source_bytes": case["source"]["bytes"],
                   "display_path": case["source"]["path"], "candidate_source_manifest_sha256": build["candidate_source_manifest_sha256"],
                   "observer_source_sha256": build["observer_source_sha256"], "binary_sha256": build["binary"]["sha256"],
                   "profile": build["profile"], "actual_runtime_os": host["os"],
                   "actual_runtime_architecture": host["architecture"], "actual_pointer_width": pointer_width,
                   "rust_compiler_target": build["target"], "mode": mode, "seam": case.get("seam", {}), "execution_id": nonce,
                   "source_generation": row.pop("source_generation"), "mode_execution_index": index}
        row["json_diagnostic_bytes_base64"] = [base64.b64encode(text(s, "raw JSON diagnostic").encode()).decode()
                                                for s in row.pop("json_diagnostic_renderings")]
        row["human_diagnostic_bytes_base64"] = base64.b64encode(text(row.pop("human_diagnostic_rendering"), "raw human diagnostic").encode()).decode()
        if row["ast"] is not None:
            fields(row["ast"], {"canonical_debug", "syntax_flavor", "belongs_to_source", "source_generation", "file_id", "node_count", "spans_and_ids_valid"}, "raw AST")
            row["ast"] = dict(row["ast"])
            row["ast"]["canonical"] = decode(row["ast"].pop("canonical_debug"))
        rows.append({**row, "schema": SCHEMA, "binding": binding})
    return rows


def normalizer_rebind(original_helpers, new_helpers, rebind, review):
    """Verify only the separately reviewed mechanical decoder substitution."""
    old, new = original_helpers["parse_debug.py"], new_helpers["parse_debug.py"]
    equal(old["sha256"], ORIGINAL_NORMALIZER_SHA, "reviewed original normalizer endpoint")
    equal(new["sha256"], REVIEWED_NORMALIZER_SHA, "reviewed optimized normalizer endpoint")
    equal(review["old_normalizer_sha256"], old["sha256"], "review names actual original normalizer")
    equal(review["new_normalizer_sha256"], new["sha256"], "review names actual optimized normalizer")
    equal(rebind["decoder_equivalence"], review["decoder_equivalence"], "review binds exact decoder equivalence artifact")
    equivalence = loads(artifact(rebind["decoder_equivalence"]))
    equal(equivalence["schema"], "oxid-unit4-debug-decoder-equivalence-v7", "decoder equivalence schema")
    equal(equivalence["status"], "pass", "decoder equivalence status")
    equal(equivalence["control_count"], 75, "reviewed mechanical decoder controls")
    equal(equivalence["candidate_executions"], 0, "decoder optimization needs no candidate execution")
    equal(equivalence["expected_corpus_changes"], 0, "decoder optimization preserves frozen expectations")
    for key, endpoint in (("old_decoder", old), ("new_decoder", new)):
        for identity in ("bytes", "sha256"):
            equal(equivalence[key][identity], endpoint[identity], "decoder equivalence endpoint " + key + "." + identity)


def collector_helpers(approval, build_record, build, overlay, original_helpers):
    """Admit only the reviewed guard/host/equivalent-decoder collector rebind.

Compiled source/observer/binary bindings remain those of the original build.
The optional rebind changes only which exact source files may collect evidence.
"""
    if "collector_helper_rebind" not in approval:
        require("collector_helper_review" not in approval, "identity", "unpaired collector helper review")
        return original_helpers
    require("collector_helper_review" in approval, "identity", "collector rebind lacks independent review")
    rebind_identity, review_identity = approval["collector_helper_rebind"], approval["collector_helper_review"]
    equal(rebind_identity["sha256"], HELPER_REBIND_SHA, "reviewed guard/host/decoder collector rebind hash")
    equal(review_identity["sha256"], HELPER_REVIEW_SHA, "independent guard/host/decoder collector review hash")
    rebind, review = loads(artifact(rebind_identity)), loads(artifact(review_identity))
    equal(rebind["schema"], "oxid-unit4-parser-helper-only-rebind-v7", "helper rebind schema")
    equal(review["schema"], "oxid-unit4-independent-observer-helper-review-v7", "helper review schema")
    equal(review["status"], "pass-guard-host-and-equivalent-decoder-rebind", "review approves guard, host and equivalent decoder delta")
    equal(review["helper_rebind_sha256"], rebind_identity["sha256"], "review names exact rebind")
    equal(review["helper_rebind_path"], rebind_identity["path"], "review names exact rebind path")
    equal(rebind["old_observer_source_sha256"], build["observer_source_sha256"], "immutable original compiled observer identity")
    equal(review["old_observer_source_sha256"], build["observer_source_sha256"], "review original compiled identity")
    equal(rebind["historical_overlay"], build["overlay_manifest"], "helper rebind exact historical compiled overlay")
    require(any(same(record, build_record) for record in rebind["historical_build_receipts"]), "identity", "build is absent from approved helper rebind")
    equal(rebind["candidate_source_manifest_sha256"], build["candidate_source_manifest_sha256"], "helper rebind production identity")
    for flag in ("all_286_derived_input_bytes_identical", "rust_and_instrumentation_bytes_identical"):
        equal(rebind[flag], True, "reviewed rebind invariant " + flag)
    equal(rebind["receipt_rewrites"], 0, "historical receipts retained")
    equal(rebind["rebuild_claim"], False, "no fabricated rebuild")
    guarded = loads(artifact(rebind["guarded_helper_overlay"]))
    equal(guarded["files"], overlay["files"], "all actual derived build input byte identities unchanged")
    for record in guarded["files"]:
        verify_file(guarded["source"], record)
    equal(guarded["candidate_source_manifest_sha256"], build["candidate_source_manifest_sha256"], "guarded helper source identity")
    equal(guarded["observer_source_sha256"], rebind["new_observer_source_sha256"], "guarded helper identity")
    equal(review["new_observer_source_sha256"], rebind["new_observer_source_sha256"], "review new helper identity")
    helper_identity = json.dumps(guarded["observer_files"], sort_keys=True, separators=(",", ":")).encode()
    equal(sha(helper_identity), rebind["new_observer_source_sha256"], "actual guarded helper manifest hash")
    checkpoint = loads(artifact(rebind["source_checkpoint"]))
    equal(rebind["source_checkpoint"]["sha256"], review["source_checkpoint_sha256"], "reviewed helper checkpoint hash")
    equal(rebind["source_checkpoint"]["path"], review["source_checkpoint_path"], "reviewed helper checkpoint path")
    helper_root = Path(rebind["source_checkpoint"]["path"]).parent
    for record in checkpoint["files"]:
        verify_file(helper_root, record)
    equal({x["path"]: x for x in checkpoint["files"]}, {x["path"]: x for x in guarded["observer_files"]}, "collector checkpoint complete helper roster")
    new_helpers = {x["path"]: x for x in guarded["observer_files"]}
    equal(new_helpers["observer.rs"], original_helpers["observer.rs"], "helper-only rebind preserves compiled observer")
    normalizer_rebind(original_helpers, new_helpers, rebind, review)
    return new_helpers


def execution_manifest(path, contract, approval):
    manifest = read_json(path)
    equal(manifest["schema"], "oxid-unit4-parser-execution-v1", "execution manifest schema")
    equal(manifest["status"], "collected", "complete evidence collection")
    host = measured_host(manifest["host_runtime"])
    equal(manifest["contract_decoded_sha256"], CONTRACT_SHA, "run contract hash")
    equal(manifest["package_freeze_sha256"], FREEZE_SHA, "run contract freeze")
    equal(manifest["authority_checkpoint"], approval["authority_checkpoint"], "run uses durable approved authority checkpoint")
    artifact(manifest["authority_checkpoint"])
    build = loads(artifact(manifest["build_receipt"]))
    equal(build["schema"], "oxid-unit4-parser-build-v1", "build schema")
    equal(build["status"], "built", "successful build")
    equal(build["exit_code"], 0, "build exit code")
    equal(build["control"], False, "instrumented build required")
    profile = manifest["profile"]
    require(profile in PROFILES, "identity", "unsupported execution profile")
    equal(build["profile"], profile, "run profile equals build profile")
    equal(build["target"], PLATFORM["rust_compiler_target"], "compiler target")
    argv = vector(build["argv"], "build command")
    require(argv.count("--target") == 1 and argv[argv.index("--target") + 1] == PLATFORM["rust_compiler_target"],
            "identity", "explicit exact compiler target")
    equal("--release" in argv, profile == "release", "actual compiler profile flag")
    require("--no-run" in argv and "--locked" in argv, "identity", "build admission flags")
    require(re.search(r"(?m)^host: x86_64-unknown-linux-gnu$", build["rustc_version"]) is not None,
            "unsupported-platform", "rustc verbose host evidence")
    for key in ("binary", "rustc", "stdout", "stderr"):
        artifact(build[key])
    cargo_rows = [loads(line) for line in artifact(build["stdout"]).splitlines() if line]
    artifacts = [r for r in cargo_rows if r.get("reason") == "compiler-artifact" and r.get("executable") and r.get("profile", {}).get("test") is True]
    equal(len(artifacts), 1, "one actual compiled test artifact")
    equal(artifacts[0]["executable"], build["binary"]["path"], "actual cargo executable identity")
    equal(artifacts[0]["target"]["name"], "oxid", "actual cargo binary target")
    projection(artifacts[0]["profile"], {"opt_level": "0" if profile == "debug" else "3",
               "debug_assertions": profile == "debug", "overflow_checks": profile == "debug", "test": True},
               "actual emitted compiler profile")
    require(any(r.get("reason") == "build-finished" and r.get("success") is True for r in cargo_rows),
            "identity", "actual cargo build success receipt")
    review = loads(artifact(approval["observer_review"]))
    for key in ("path", "sha256"):
        equal(build["overlay_manifest"][key], review["overlay_manifest"][key], "exact independently reviewed derived overlay " + key)
    overlay = loads(artifact(build["overlay_manifest"]))
    equal(overlay["schema"], "oxid-unit4-observer-overlay-v1", "overlay schema")
    equal(overlay["base_commit"], BASE_COMMIT, "exact source base commit")
    equal(overlay["control"], False, "observational overlay")
    for key in ("candidate_source_manifest_sha256", "observer_source_sha256"):
        equal(build[key], overlay[key], "build to overlay identity " + key)
        equal(build[key], approval[key], "approved identity " + key)
    overlay_root = Path(overlay["source"])
    equal(build["cwd"], str(overlay_root), "actual build working source tree")
    equal(artifacts[0]["manifest_path"], str(overlay_root / "Cargo.toml"), "actual Cargo package manifest")
    equal(artifacts[0]["target"]["src_path"], str(overlay_root / "src/cli.rs"), "actual Cargo source entrypoint")
    require(len({f["path"] for f in overlay["files"]}) == len(overlay["files"]), "identity", "duplicate overlay files")
    for entry in overlay["files"]:
        verify_file(overlay_root, entry)
    production = (overlay_root / "candidate-source-manifest.json").read_bytes()
    equal(sha(production), build["candidate_source_manifest_sha256"], "original production manifest hash")
    equal(loads(production)["commit"], BASE_COMMIT, "original production commit")
    observer_files = overlay["observer_files"]
    raw_observer_manifest = json.dumps(observer_files, sort_keys=True, separators=(",", ":")).encode()
    equal(sha(raw_observer_manifest), build["observer_source_sha256"], "observer manifest hash")
    for entry in observer_files:
        verify_file(approval["observer_source_root"], entry)
    observer_names = {f["path"]: f for f in observer_files}
    collector_names = collector_helpers(approval, manifest["build_receipt"], build, overlay, observer_names)
    for key, filename in (("driver", "run.py"), ("normalizer", "parse_debug.py")):
        artifact(manifest[key])
        equal(manifest[key]["sha256"], collector_names[filename]["sha256"], "reviewed collection helper " + key)
    cases = {c["id"]: c for c in contract["cases"]}
    expected_ids = [c["id"] for c in contract["cases"]]
    equal(manifest["requested_case_ids"], expected_ids, "exact ordered frozen case roster")
    equal(manifest["case_count"], len(expected_ids), "manifest case count")
    receipts = vector(manifest["case_receipts"], "case receipts")
    equal([r["case_id"] for r in receipts], expected_ids, "exact execution receipt roster")
    independent_rows, nonce_set = [], set()
    for receipt in receipts:
        case = cases[receipt["case_id"]]
        equal(receipt["status"], "executed", "successful real case execution")
        equal(receipt["exit_code"], 0, "case execution exit")
        nonce = text(receipt["execution_id"], "execution nonce")
        require(re.fullmatch("[0-9a-f]{32}", nonce) is not None and nonce not in nonce_set, "execution-reused", "invalid/reused execution nonce")
        nonce_set.add(nonce)
        equal(receipt["argv"], [build["binary"]["path"], "frontend::parser::unit4_observer::observe_request", "--exact", "--ignored", "--nocapture", "--test-threads=1"], "exact actual observer command")
        stdout = artifact(receipt["stdout"]).decode()
        artifact(receipt["stderr"])
        require(stdout.count("UNIT4_EXECUTED " + nonce) == 1 and re.search(r"test result: ok\. 1 passed; 0 failed; 0 ignored;", stdout),
                "zero-execution", "test process lacks exact actual execution witness")
        request = loads(artifact(receipt["request"]))
        equal(request["case_id"], case["id"], "request case")
        equal(artifact(request["source"]), b64(case["source"]["base64"], "case source"), "request exact source")
        equal(request["display_path"], case["source"]["path"], "request path")
        equal(request["limits"], case["limits"], "request limits")
        equal(request["seam"], case.get("seam", {}), "request exact seam")
        equal(request["original_mode_requested"], "relation_to_original" in case["expected"], "requested original-mode execution")
        env = request["environment"]
        seam = case.get("seam", {})
        require(env["UNIT4_NONCE"] == nonce and env["UNIT4_CASE_ID"] == case["id"], "identity", "request environment execution identity")
        equal(env["UNIT4_NODE_LIMIT"], str(seam.get("node_limit", case["limits"]["nodes"])), "actual requested node limit")
        equal(env["UNIT4_TOKEN_LIMIT"], str(case["limits"]["non_eof_tokens"]), "actual requested token limit")
        equal(env["UNIT4_RESERVE_FAIL_AT"], str(seam.get("reserve_fail_at") or 0), "actual reserve seam")
        equal(env["UNIT4_CONTROL"], "0", "actual instrumentation selection")
        equal(env["UNIT4_SOURCE"], request["source"]["path"], "actual source request path")
        equal(env["UNIT4_DISPLAY_PATH"], case["source"]["path"], "actual display path")
        equal(env["UNIT4_ORIGINAL"], str(int("relation_to_original" in case["expected"])), "actual two-mode selection")
        equal(env["UNIT4_REJECT_KIND"], seam.get("reject_node_admission", {}).get("kind", ""), "actual targeted gate")
        equal(env["UNIT4_REJECT_OCCURRENCE"], str(seam.get("reject_node_admission", {}).get("occurrence", 0)), "actual targeted occurrence")
        equal(env["UNIT4_DIRECT"], str(int(seam.get("direct_operation") == "path_segment")), "actual direct seam")
        equal(env["UNIT4_SEGMENT_START"], str(seam.get("segment_span", [0, 0])[0]), "actual direct span start")
        equal(env["UNIT4_SEGMENT_END"], str(seam.get("segment_span", [0, 0])[1]), "actual direct span end")
        equal(env["UNIT4_RAW_OUTPUT"], receipt["raw"]["path"], "actual raw output path")
        raw = loads(artifact(receipt["raw"]))
        rows = normalize_raw(raw, case, build, nonce, host)
        equal(receipt["observations"], len(rows), "receipt real mode count")
        independent_rows.extend(rows)
    equal(manifest["observation_count"], len(independent_rows), "manifest actual observation count")
    collected = artifact(manifest["observations"])
    require(collected.endswith(b"\n"), "schema", "JSONL must have final newline")
    parsed_rows = [loads(line) for line in collected.splitlines()]
    equal(parsed_rows, independent_rows, "normalized observations preserve every actual raw field")
    identities = {key: build[key] for key in ("candidate_source_manifest_sha256", "observer_source_sha256")}
    identities["binary_sha256"] = build["binary"]["sha256"]
    return profile, parsed_rows, identities


def inventory(contract):
    counts = collections.Counter(key for c in contract["cases"] for key in c["expected"])
    unsupported = sorted(set(counts) - EXPECTED_FIELDS)
    token_failures = []
    for case in contract["cases"]:
        try:
            source_tokens(Source(case), case["limits"])
        except Rejected as error:
            token_failures.append({"case_id": case["id"], "error": str(error)})
    return {"schema": "oxid-unit4-parser-comparator-inventory-v1", "contract_decoded_sha256": CONTRACT_SHA,
            "package_freeze_sha256": FREEZE_SHA, "case_count": len(contract["cases"]),
            "required_profiles": PROFILES, "project_rows_per_profile": len(contract["cases"]),
            "original_rows_per_profile": sum("relation_to_original" in c["expected"] for c in contract["cases"]),
            "predicates": dict(sorted(counts.items())), "unsupported_expected_fields": unsupported,
            "unsupported_source_lexemes": token_failures,
            "candidate_comparisons": 0, "candidate_compiler_invocations": 0,
            "qualification_status": "source-only; independent review and approved execution evidence pending"}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--contract-dir", type=Path, required=True)
    parser.add_argument("--inventory", action="store_true")
    parser.add_argument("--authorization", type=Path)
    parser.add_argument("--authorization-sha256")
    parser.add_argument("--execution-manifest", action="append", type=Path, default=[])
    parser.add_argument("--output", type=Path)
    args = parser.parse_args()
    try:
        contract = load_contract(args.contract_dir)
        if args.inventory:
            require(not args.execution_manifest, "authorization", "inventory must not read candidate observations")
            result = inventory(contract)
            status = 1 if result["unsupported_expected_fields"] or result["unsupported_source_lexemes"] else 0
        else:
            require(args.authorization is not None and args.authorization_sha256 is not None,
                    "authorization", "exact durable approved comparison authorization required")
            approved = authorization(args.authorization, args.authorization_sha256)
            effective, effective_identity = admit_contract_amendment(contract, args.contract_dir, approved["effective_contract_authority"])
            require(len(args.execution_manifest) == 2, "roster", "two complete profile execution manifests required")
            all_rows, identities = [], {}
            for path in args.execution_manifest:
                profile, rows, identity = execution_manifest(path, contract, approved)
                require(profile not in identities, "roster", "duplicate execution profile")
                identities[profile] = identity; all_rows.extend(rows)
            equal(sorted(identities), sorted(PROFILES), "exact profile roster")
            require(identities["debug"]["binary_sha256"] != identities["release"]["binary_sha256"], "identity", "same binary reused as debug/release")
            result = compare_effective_rows(effective, effective_identity, all_rows, identities)
            result["authorization_sha256"] = args.authorization_sha256
            status = 0 if result["status"] == "pass" else 1
    except (Rejected, OSError, ValueError, KeyError, TypeError, IndexError) as error:
        result = {"schema": "oxid-unit4-parser-comparison-v1", "status": "fail",
                  "issues": [{"code": error.code if isinstance(error, Rejected) else "unavailable-evidence", "detail": str(error)}]}
        status = 1
    output = json.dumps(result, sort_keys=True, indent=2) + "\n"
    if args.output:
        args.output.write_text(output)
    sys.stdout.write(output)
    return status


if __name__ == "__main__":
    raise SystemExit(main())
