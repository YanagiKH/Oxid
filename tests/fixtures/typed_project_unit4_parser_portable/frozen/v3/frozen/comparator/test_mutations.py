#!/usr/bin/env python3
"""Synthetic controls only. No real observation files or compiler invocation."""
import base64
import copy
import json
from pathlib import Path
import unittest

import comparator as c
from debug_decoder import decode, DecodeError

CONTRACT = Path(__file__).parent.parent / "new-contracts/parser"
HOST = {"os": "linux", "architecture": "x86_64", "python_pointer_width": 64, "uname": {"system": "Linux", "release": "synthetic", "version": "synthetic", "machine": "x86_64"}, "python_executable": "/synthetic/python3", "python_version": "synthetic Python 3"}
IDENTITIES = {p: {"candidate_source_manifest_sha256": "a"*64, "observer_source_sha256": "b"*64,
                  "binary_sha256": ("c" if p == "debug" else "d")*64} for p in c.PROFILES}


def encoded(value):
    return base64.b64encode(value).decode()


def make(case, profile="debug", mode="ProjectCandidate"):
    """Explicit artificial envelope; it is never emitted as candidate evidence."""
    source = c.Source(case)
    tokens = c.source_tokens(source, case["limits"])
    binding = {"contract_decoded_sha256": c.CONTRACT_SHA, "package_freeze_sha256": c.FREEZE_SHA,
               "case_id": case["id"], "source_sha256": case["source"]["sha256"], "source_bytes": len(source.data),
               "display_path": source.path, **IDENTITIES[profile], "profile": profile, **c.PLATFORM,
               "mode": mode, "seam": case.get("seam", {}), "execution_id": "synthetic:"+case["id"]+profile,
               "source_generation": 1, "mode_execution_index": 0 if mode == "ProjectCandidate" else 1}
    def span(s):
        return {"tag": "Span", "file": {"tag": "SourceFileId", "items": [0]}, "start": s[1], "end": s[2]}
    program = {"tag": "Program", "tokens": [{"tag": "Token", "kind": {"tag": t["kind"]}, "span": span(t["span"])} for t in tokens["tokens"]],
               **{k: [] for k in "functions expressions records items modules paths path_segments imports".split()},
               "source": {"tag": "SourceProvenance", "text_len": len(source.data), "file": {"tag": "SourceFileId", "items": [0]}, "debug_non_exhaustive": True},
               "project_syntax": False}
    return {"schema": c.SCHEMA, "binding": binding, "result": "ok", "executed": True, "parse_attempts": 1,
            "diagnostics": [], "json_diagnostic_bytes_base64": [], "human_diagnostic_bytes_base64": "",
            "token_inventory": tokens, "nodes_admitted": 0, "recognized_initial": False, "recognized_final": False,
            "recognition_transitions": [], "events": [], "field_pub_scans": [], "field_current_token_reads": [],
            "reserve_attempts": 0, "reserve_trace": [], "ast": {"canonical": program, "syntax_flavor": "OriginalSingleFile",
            "belongs_to_source": True, "source_generation": 1, "file_id": 0, "node_count": 0, "spans_and_ids_valid": True},
            "syntax_flavor": "OriginalSingleFile", "path_segment_count_after": None, "observation_complete": True,
            "observer_limits": {"events": 1000000, "evidence_bytes": 134217728, "charged_bytes": 0}}


def event(row, kind, production, cursor, detail=None, context="grammar"):
    token = row["token_inventory"]["tokens"][cursor]
    result = {"seq": len(row["events"]), "kind": kind, "production": production, "cursor": cursor,
              "token_kind": token["kind"], "span": token["span"], "cursor_token_kind": token["kind"],
              "cursor_span": token["span"], "context": context, "detail": detail}
    row["events"].append(result)
    if kind == "recognize":
        row["recognition_transitions"].append(copy.deepcopy(result))
    return result


def error(row, case, code="E0100", message="synthetic diagnostic", start=0, end=0):
    source = c.Source(case)
    row.update(result="parse_error", ast=None, syntax_flavor=None)
    diagnostic = {"schema_version": 1, "edition": "typed-preview", "kind": "diagnostic", "severity": "error",
                  "code": code, "stage": "parse", "message": message, "primary": {"file_id": 0, "path": source.path,
                  "start": start, "end": end, "line": source.positions[start][0], "column": source.positions[start][1],
                  "end_line": source.positions[end][0], "end_column": source.positions[end][1]}, "secondary": [], "notes": []}
    row["diagnostics"] = [diagnostic]
    row["json_diagnostic_bytes_base64"] = [encoded(json.dumps(diagnostic, separators=(",", ":")).encode())]
    row["human_diagnostic_bytes_base64"] = encoded(b"synthetic diagnostic\n")


class MutationControls(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.contract = c.load_contract(CONTRACT)
        cls.cases = {case["id"]: case for case in cls.contract["cases"]}

    def reject(self, operation, code=None):
        with self.assertRaises(c.Rejected) as caught:
            operation()
        if code:
            self.assertEqual(caught.exception.code, code)
        return caught.exception

    def empty_pair(self):
        case = copy.deepcopy(self.cases["ledger-empty"])
        case["expected"]["relation_to_original"] = ["complete_ast", "local_ids", "node_count", "source_provenance"]
        rows = [make(case, profile, mode) for profile in c.PROFILES for mode in ("ProjectCandidate", "OwnedCandidate")]
        return {"cases": [case]}, rows

    def test_synthetic_positive_complete_roster(self):
        contract, rows = self.empty_pair()
        self.assertEqual(c.compare_rows(contract, rows, IDENTITIES)["status"], "pass")

    def test_missing_extra_duplicate_zero_rosters(self):
        contract, rows = self.empty_pair()
        for mutated in (rows[:-1], rows+[rows[0]], [], rows+[dict(rows[0], binding=dict(rows[0]["binding"], case_id="invented"))]):
            with self.subTest(rows=len(mutated)):
                self.assertEqual(c.compare_rows(contract, mutated, IDENTITIES)["status"], "fail")

    def test_stale_identity_profile_host_source_and_execution(self):
        case = self.cases["ledger-empty"]
        for field, value in {"contract_decoded_sha256": "0"*64, "package_freeze_sha256": "0"*64,
                             "candidate_source_manifest_sha256": "0"*64, "observer_source_sha256": "0"*64,
                             "binary_sha256": "0"*64, "source_sha256": "0"*64, "source_bytes": 7,
                             "display_path": "stale.ox", "profile": "release", "actual_runtime_os": "windows",
                             "actual_runtime_architecture": "aarch64", "actual_pointer_width": 32,
                             "rust_compiler_target": "aarch64-unknown-linux-gnu", "mode_execution_index": 1,
                             "execution_id": "", "source_generation": 0}.items():
            row = make(case); row["binding"][field] = value
            with self.subTest(field=field):
                self.reject(lambda: c.check_observation(case, row, IDENTITIES))

    def test_zero_unexecuted_incomplete_and_boolean_integer(self):
        case = self.cases["ledger-empty"]
        for key, value in {"executed": False, "parse_attempts": 0, "observation_complete": False, "nodes_admitted": False}.items():
            row = make(case); row[key] = value
            with self.subTest(key=key):
                self.reject(lambda: c.check_observation(case, row, IDENTITIES))

    def test_missing_and_unknown_signal_fail(self):
        case = self.cases["ledger-empty"]
        row = make(case); del row["events"]
        self.reject(lambda: c.check_observation(case, row, IDENTITIES), "missing-signal")
        row = make(case); row["made_up_success"] = True
        self.reject(lambda: c.check_observation(case, row, IDENTITIES), "unknown-signal")

    def test_unknown_expectation_is_not_skipped(self):
        case = copy.deepcopy(self.cases["ledger-empty"])
        case["expected"]["unknown_expectation"] = True
        self.reject(lambda: c.check_observation(case, make(case), IDENTITIES), "unsupported-expectation")

    def test_stale_actual_ast_provenance_and_ids(self):
        case = self.cases["ledger-empty"]
        row = make(case); row["ast"]["source_generation"] = 2
        self.reject(lambda: c.check_observation(case, row, IDENTITIES))
        row = make(case); row["ast"]["canonical"]["source"]["text_len"] = 1
        self.reject(lambda: c.check_observation(case, row, IDENTITIES))
        row = make(case); row["ast"]["canonical"]["items"] = [{"tag": "Function", "items": [0]}]
        self.reject(lambda: c.check_observation(case, row, IDENTITIES), "ast-id")

    def test_typed_ast_arenas_and_nested_field_types(self):
        case = self.cases["ledger-empty"]
        for arena in ("functions", "records", "expressions", "modules", "imports", "paths", "path_segments"):
            row = make(case); row["ast"]["canonical"][arena] = [True]
            with self.subTest(arena=arena):
                self.reject(lambda: c.check_observation(case, row, IDENTITIES))
        row = make(case); root = row["ast"]["canonical"]
        span = {"tag": "Span", "file": {"tag": "SourceFileId", "items": [0]}, "start": 0, "end": 0}
        root["records"] = [{"tag": "StructDecl", "public": None, "name": span, "fields": [], "span": span, "end": span}]
        root["items"] = [{"tag": "Struct", "items": [0]}]
        c.typed_ast(root)
        for field, bad in {"public": False, "name": 0, "fields": False, "span": [], "end": None}.items():
            mutant = copy.deepcopy(root); mutant["records"][0][field] = bad
            with self.subTest(nested_field=field): self.reject(lambda: c.typed_ast(mutant))
        mutant = copy.deepcopy(root); mutant["records"][0]["fields"] = [False]
        self.reject(lambda: c.typed_ast(mutant))
        mutant = copy.deepcopy(root); mutant["expressions"] = [{"tag": "Expr", "kind": {"tag": "Number", "digits": span, "negative": 0}, "span": span}]
        self.reject(lambda: c.typed_ast(mutant), "ast-schema")

    def test_relation_cannot_reuse_mode_or_stale_generation(self):
        contract, rows = self.empty_pair(); case = contract["cases"][0]
        self.reject(lambda: c.compare_relation(case, rows[0], rows[0]))
        altered = copy.deepcopy(rows[1]); altered["binding"]["source_generation"] = 2
        self.reject(lambda: c.compare_relation(case, rows[0], altered))

    def node_order(self):
        case = self.cases["node-order-pub-fn"]; row = make(case)
        error(row, case, "E0400", "syntax node limit exceeded")
        pub = next(i for i,t in enumerate(row["token_inventory"]["tokens"]) if t["kind"] == "Pub")
        fn = c.next_significant(row["token_inventory"]["tokens"], pub)
        event(row, "consume", "token", pub)
        event(row, "recognize", "top_public", fn, {"before": False, "after": True})
        event(row, "node_attempt", "function", fn, 0)
        event(row, "node_reject", "function", fn, 0)
        row["recognized_final"] = True
        return case, row

    def test_positive_required_event_order(self):
        case, row = self.node_order()
        c.check_observation(case, row, IDENTITIES)

    def first_admission(self, case_id):
        """Source-only synthetic traces for the six frozen admission grammars."""
        case = self.cases[case_id]; row = make(case)
        error(row, case, "E0400", "syntax node limit exceeded")
        significant = [(i, t["kind"]) for i, t in enumerate(row["token_inventory"]["tokens"])
                       if t["kind"] not in {"Trivia", "Eof"}]
        first, kind = significant[0]
        if case_id == "node-order-field":
            record, name, opening, cursor = [i for i, _ in significant[:4]]
            event(row, "node_attempt", "record", record, 0)
            event(row, "node_admit", "record", record, 1)
            for token in (record, name, opening): event(row, "consume", "token", token)
            production, nodes = "field", 1
        else:
            cursor, nodes = first, 0
            if kind == "Pub":
                next_cursor, kind = significant[1]
                if kind in {"Fn", "Struct"}:
                    cursor = next_cursor
                    event(row, "consume", "token", first)
                    event(row, "recognize", "top_public", cursor, {"before": False, "after": True})
                    row["recognized_final"] = True
            production = {"Mod": "module", "Use": "import", "Fn": "function", "Struct": "record"}[kind]
        event(row, "node_attempt", production, cursor, nodes)
        event(row, "node_reject", production, cursor, nodes)
        row["nodes_admitted"] = nodes
        return case, row

    def test_first_admission_all_six_source_grammars(self):
        ids = [case["id"] for case in self.contract["cases"] if "required_order" in case["expected"]]
        self.assertEqual(len(ids), 6)
        for case_id in ids:
            with self.subTest(case_id=case_id):
                case, row = self.first_admission(case_id)
                c.check_observation(case, row, IDENTITIES)

    def test_first_admission_allows_legal_later_recovery_retry(self):
        for case_id in ("node-order-pub-fn", "node-order-pub-struct", "node-order-field"):
            case, row = self.first_admission(case_id)
            final = row["events"][-1]; cursor, production = final["cursor"], final["production"]
            event(row, "recover_enter", "recovery", cursor, context="recovery")
            event(row, "recover_inspect", "recovery", cursor, context="recovery")
            event(row, "recover_exit", "recovery", cursor, context="recovery")
            event(row, "node_attempt", production, cursor, row["nodes_admitted"])
            event(row, "node_reject", production, cursor, row["nodes_admitted"])
            with self.subTest(case_id=case_id):
                c.check_observation(case, row, IDENTITIES)
                # Later retries still obey unchanged global no-append semantics.
                event(row, "append", "functions", cursor, 1)
                self.reject(lambda: c.check_observation(case, row, IDENTITIES), "event-order")

    def test_first_admission_bad_first_good_later_cannot_pass(self):
        for case_id in ("node-order-pub-fn", "node-order-pub-struct", "node-order-field"):
            case, good = self.first_admission(case_id)
            for bad_kind in ("node_admit", "consume"):
                row = copy.deepcopy(good)
                row["events"][-1]["kind"] = bad_kind
                row["events"].extend(copy.deepcopy(good["events"]))
                with self.subTest(case_id=case_id, bad_kind=bad_kind):
                    self.reject(lambda: c.required_first_admission(case, row, case["expected"]["required_order"]))

    def test_first_admission_unrelated_earlier_good_cannot_satisfy_target(self):
        for case_id in ("node-order-pub-fn", "node-order-field"):
            case, row = self.first_admission(case_id)
            unrelated = copy.deepcopy(row["events"])
            for e in unrelated:
                if e["kind"] in {"node_attempt", "node_reject"}: e["production"] = "import"
            row["events"] = unrelated + row["events"]
            with self.subTest(case_id=case_id):
                self.reject(lambda: c.required_first_admission(case, row, case["expected"]["required_order"]))
        case, row = self.first_admission("node-order-field")
        # The first field gate at a wrong source cursor cannot be skipped in
        # favor of a later gate with the right cursor.
        good = copy.deepcopy(row["events"][-2:])
        for e in row["events"][-2:]: e["cursor"] += 2
        row["events"].extend(good)
        self.reject(lambda: c.required_first_admission(case, row, case["expected"]["required_order"]))

    def test_first_admission_missing_duplicate_reordered_semantics_reject(self):
        for case_id in ("node-order-pub-fn", "node-order-pub-struct", "node-order-field", "node-order-mod"):
            case, good = self.first_admission(case_id)
            for index in range(len(good["events"])):
                for mutation in ("missing", "duplicate", "reordered"):
                    row = copy.deepcopy(good)
                    if mutation == "missing": del row["events"][index]
                    elif mutation == "duplicate": row["events"].insert(index, copy.deepcopy(row["events"][index]))
                    elif index + 1 < len(row["events"]):
                        row["events"][index], row["events"][index+1] = row["events"][index+1], row["events"][index]
                    else: continue
                    with self.subTest(case_id=case_id, index=index, mutation=mutation):
                        # A duplicate decision after the first boundary belongs
                        # to the unchanged global node ledger, which must reject
                        # a decision without its own attempt. Other mutations
                        # must also fail the bounded predicate in isolation.
                        if not (mutation == "duplicate" and index == len(good["events"])-1):
                            self.reject(lambda: c.required_first_admission(case, row, case["expected"]["required_order"]))
                        for seq, e in enumerate(row["events"]): e["seq"] = seq
                        row["recognition_transitions"] = [copy.deepcopy(e) for e in row["events"] if e["kind"] == "recognize"]
                        self.reject(lambda: c.check_observation(case, row, IDENTITIES))

    def test_first_admission_extra_meaningful_events_reject(self):
        for case_id in ("node-order-pub-fn", "node-order-field", "node-order-use"):
            case, good = self.first_admission(case_id)
            for kind in ("recognize", "recognize_already_true", "node_attempt", "node_admit", "node_reject",
                         "reserve", "append", "path_count_checked", "field_scan_enter", "consume"):
                row = copy.deepcopy(good)
                extra = copy.deepcopy(row["events"][-2]); extra.update(kind=kind, production="unrelated")
                row["events"].insert(len(row["events"])-2, extra)
                with self.subTest(case_id=case_id, kind=kind):
                    self.reject(lambda: c.required_first_admission(case, row, case["expected"]["required_order"]))

    def test_module_recognition_cannot_reuse_a_prior_declaration_admission(self):
        case = copy.deepcopy(self.cases["ledger-mod"])
        data = b"mod a; mod b;"
        case["source"] = {"base64": encoded(data), "bytes": len(data), "sha256": c.sha(data), "path": "synthetic.ox"}
        row = make(case); tape = row["token_inventory"]["tokens"]
        modules = [i for i,t in enumerate(tape) if t["kind"] == "Mod"]
        event(row, "node_attempt", "module", modules[0], 0)
        event(row, "node_admit", "module", modules[0], 1)
        event(row, "consume", "token", modules[1])
        current = c.next_significant(tape, modules[1])
        trigger = event(row, "recognize", "module_keyword", current, {"before": False, "after": True})
        self.reject(lambda: c.recognition_trigger(trigger, row["events"][:-1], tape, [], c.Source(case)), "event-order")

    def test_ordinary_budget_top_public_recognition_must_precede_own_node(self):
        case = self.cases["ledger-public-empty-fn"]; row = make(case); tape = row["token_inventory"]["tokens"]
        pub = next(i for i,t in enumerate(tape) if t["kind"] == "Pub"); fn = c.next_significant(tape, pub)
        event(row, "node_attempt", "function", fn, 0)
        event(row, "node_admit", "function", fn, 1)
        event(row, "consume", "token", pub)
        trigger = event(row, "recognize", "top_public", fn, {"before": False, "after": True})
        row.update(nodes_admitted=1, recognized_final=True)
        self.reject(lambda: c.recognition_trigger(trigger, row["events"][:-1], tape, [], c.Source(case)), "event-order")
        self.reject(lambda: c.events(row, case, c.Source(case), tape), "event-order")

    def test_recovery_consumed_pub_before_fn_does_not_require_recognition(self):
        case = self.cases["ledger-public-empty-fn"]
        for mode in ("ProjectCandidate", "OwnedCandidate"):
            row = make(case, mode=mode); tape = row["token_inventory"]["tokens"]
            pub = next(i for i,t in enumerate(tape) if t["kind"] == "Pub"); fn = c.next_significant(tape, pub)
            event(row, "recover_enter", "recovery", pub, context="recovery")
            event(row, "consume", "token", pub, context="recovery")
            event(row, "recover_exit", "recovery", fn, context="recovery")
            event(row, "node_attempt", "function", fn, 0)
            event(row, "node_admit", "function", fn, 1)
            row.update(result="parse_error", nodes_admitted=1)
            with self.subTest(mode=mode):
                c.events(row, case, c.Source(case), tape)
            trigger = event(row, "recognize", "top_public", fn, {"before": False, "after": True})
            self.reject(lambda: c.recognition_trigger(trigger, row["events"][:-1], tape, [], c.Source(case)))

    def test_reordered_events_rejected_even_with_renumbered_sequence(self):
        case, row = self.node_order()
        row["events"][0], row["events"][1] = row["events"][1], row["events"][0]
        for i,e in enumerate(row["events"]): e["seq"] = i
        row["recognition_transitions"] = [copy.deepcopy(row["events"][0])]
        self.reject(lambda: c.check_observation(case, row, IDENTITIES), "recognition-trigger")

    def test_recovery_cannot_recognize_and_recognition_cannot_reset(self):
        case, row = self.node_order()
        row["recognized_final"] = False
        self.reject(lambda: c.check_observation(case, row, IDENTITIES))
        case, row = self.node_order()
        row["events"][1]["context"] = "recovery"
        row["recognition_transitions"][0]["context"] = "recovery"
        self.reject(lambda: c.check_observation(case, row, IDENTITIES))

    def test_no_append_after_rejected_node(self):
        case, row = self.node_order()
        event(row, "append", "functions", row["events"][-1]["cursor"], 1)
        self.reject(lambda: c.check_observation(case, row, IDENTITIES), "event-order")

    def reserve_event(self, row, cursor, success):
        detail = {"kind": "absolute path segments", "length": 1, "element_bytes": 24, "success": success}
        event(row, "reserve", detail["kind"], cursor, detail)
        row["reserve_trace"].append(copy.deepcopy(detail))
        row["reserve_attempts"] += 1

    def test_failed_and_successful_reserves_after_denied_nodes_reject(self):
        for success in (True, False):
            case, row = self.node_order()
            self.reserve_event(row, row["events"][-1]["cursor"], success)
            with self.subTest(success=success):
                self.reject(lambda: c.check_observation(case, row, IDENTITIES), "event-order")

    def test_first_failed_reserve_and_later_admission_barriers(self):
        case = copy.deepcopy(self.cases["ledger-empty"])
        data = b"crate::f"
        case["source"] = {"base64": encoded(data), "bytes": len(data), "sha256": c.sha(data), "path": "synthetic.ox"}
        case["limits"]["nodes"] = 100
        row = make(case)
        error(row, case, "E0400", "synthetic reserve failure")
        event(row, "path_count_checked", "path_segment", 0, 0)
        event(row, "node_attempt", "path_segment", 0, 0)
        event(row, "node_admit", "path_segment", 0, 1)
        row["nodes_admitted"] = 1
        self.reserve_event(row, 0, False)
        # A real admitted segment is permitted to make its first failing reserve.
        c.check_observation(case, row, IDENTITIES, apply_expected=False)
        for success in (True, False):
            repeated = copy.deepcopy(row)
            self.reserve_event(repeated, 0, success)
            with self.subTest(repeated_success=success):
                self.reject(lambda: c.check_observation(case, repeated, IDENTITIES, apply_expected=False), "event-order")
            later = copy.deepcopy(row)
            cursor = next(i for i, token in enumerate(later["token_inventory"]["tokens"]) if token["kind"] == "Ident")
            event(later, "recover_enter", "recovery", 0, context="recovery")
            event(later, "recover_exit", "recovery", cursor, context="recovery")
            event(later, "path_count_checked", "path_segment", cursor, 0)
            event(later, "node_attempt", "path_segment", cursor, 1)
            event(later, "node_admit", "path_segment", cursor, 2)
            later["nodes_admitted"] = 2
            self.reserve_event(later, cursor, success)
            with self.subTest(later_success=success):
                c.check_observation(case, later, IDENTITIES, apply_expected=False)

    def test_reserve_failure_barrier_allows_later_module_admission(self):
        case = copy.deepcopy(self.cases["ledger-empty"])
        data = b"mod a; mod b;"
        case["source"] = {"base64": encoded(data), "bytes": len(data), "sha256": c.sha(data), "path": "synthetic.ox"}
        case["seam"] = {"reserve_fail_at": 1}
        case["limits"]["nodes"] = 100
        row = make(case); error(row, case, "E0400", "synthetic first module reserve failure")
        tape = row["token_inventory"]["tokens"]
        modules = [i for i, token in enumerate(tape) if token["kind"] == "Mod"]
        for number, cursor in enumerate(modules):
            if number:
                event(row, "recover_enter", "recovery", cursor, context="recovery")
                event(row, "recover_exit", "recovery", cursor, context="recovery")
            event(row, "node_attempt", "module", cursor, number)
            event(row, "node_admit", "module", cursor, number+1)
            event(row, "consume", "token", cursor)
            name = c.next_significant(tape, cursor)
            event(row, "recognize" if number == 0 else "recognize_already_true", "module_keyword", name,
                  {"before": bool(number), "after": True})
            event(row, "consume", "token", name)
            semi = c.next_significant(tape, name)
            event(row, "consume", "token", semi)
            after = c.next_significant(tape, semi)
            self.reserve_event(row, after, number > 0)
        row.update(nodes_admitted=2, recognized_final=True)
        c.check_observation(case, row, IDENTITIES, apply_expected=False)

    def test_targeted_node_budget_lowers_at_exact_occurrence_and_stays_lowered(self):
        case = copy.deepcopy(self.cases["ledger-empty"])
        data = b"mod a; mod b; fn f()->(){}"
        case["source"] = {"base64": encoded(data), "bytes": len(data), "sha256": c.sha(data), "path": "synthetic.ox"}
        case["seam"] = {"reject_node_admission": {"kind": "module", "occurrence": 2}}
        case["limits"]["nodes"] = 100
        row = make(case); error(row, case)
        tape = row["token_inventory"]["tokens"]
        cursors = [i for i, token in enumerate(tape) if token["kind"] in {"Mod", "Fn"}]
        event(row, "node_attempt", "module", cursors[0], 0)
        event(row, "node_admit", "module", cursors[0], 1)
        event(row, "node_attempt", "module", cursors[1], 1)
        event(row, "node_reject", "module", cursors[1], 1)
        event(row, "recover_enter", "recovery", cursors[1], context="recovery")
        event(row, "recover_exit", "recovery", cursors[2], context="recovery")
        event(row, "node_attempt", "function", cursors[2], 1)
        event(row, "node_reject", "function", cursors[2], 1)
        row["nodes_admitted"] = 1
        # This is an isolated real-event ledger control: the second module gate
        # exhausts the budget permanently, including for another production.
        c.events(row, case, c.Source(case), tape)
        for decision in (1, 3, 7):
            bad = copy.deepcopy(row)
            if decision == 1:
                bad["events"][decision].update(kind="node_reject", detail=0)
            else:
                bad["events"][decision].update(kind="node_admit", detail=2)
            with self.subTest(decision=decision):
                self.reject(lambda: c.events(bad, case, c.Source(case), tape), "node-ledger")

    def amendment_authority(self):
        root = CONTRACT.parent / "parser-v1-location-amendment-v1"
        review = CONTRACT.parent.parent / "parser-amendment-independent-review/review.json"
        paths = {"checkpoint": root / "AMENDMENT-CHECKPOINT-v1.json", "descriptor": root / "artifacts/effective-contract.json",
                 "amendment": root / "artifacts/amendment.json", "review": review}
        return {key: {"path": str(path), "bytes": path.stat().st_size, "sha256": c.sha(path.read_bytes())} for key, path in paths.items()}

    def test_exact_reviewed_amendment_preserves_base_and_execution_identity(self):
        base_bytes = c.canonical(self.contract)
        effective, receipt = c.admit_contract_amendment(self.contract, CONTRACT, self.amendment_authority())
        self.assertEqual(c.canonical(self.contract), base_bytes)
        self.assertEqual(len(c.changed_leaves(self.contract, effective)), 4)
        self.assertEqual(c.sha(c.canonical(effective)), c.EFFECTIVE_DOCUMENT_SHA)
        self.assertEqual(receipt, c.effective_receipt())
        case = next(case for case in effective["cases"] if case["id"] == "ledger-empty")
        row = make(case)
        c.check_observation(case, row, IDENTITIES)
        row["binding"]["contract_decoded_sha256"] = c.EFFECTIVE_DOCUMENT_SHA
        self.reject(lambda: c.check_observation(case, row, IDENTITIES))

    def test_amendment_requires_exact_complete_reviewed_artifacts(self):
        authority = self.amendment_authority()
        c.amendment_artifacts(authority)
        for key in authority:
            missing = copy.deepcopy(authority); del missing[key]
            with self.subTest(missing=key): self.reject(lambda: c.amendment_artifacts(missing))
            stale = copy.deepcopy(authority); stale[key]["sha256"] = "0"*64
            with self.subTest(stale=key): self.reject(lambda: c.amendment_artifacts(stale))
        duplicate = copy.deepcopy(authority); duplicate["additional_amendment"] = duplicate["amendment"]
        self.reject(lambda: c.amendment_artifacts(duplicate), "unknown-signal")
        stale = copy.deepcopy(authority)
        stale["descriptor"]["sha256"] = "8d3bf7cdb027e1c9589275787dafbd8c6b89439521f51201dcc47571cd014c72"
        self.reject(lambda: c.amendment_artifacts(stale))
        self.reject(lambda: c.amendment_artifacts(c.effective_receipt()), "missing-signal")

    def test_amendment_rejects_wrong_base_source_and_already_applied(self):
        authority = self.amendment_authority()
        mutations = []
        wrong = copy.deepcopy(self.contract); wrong["cases"][0]["source"]["sha256"] = "0"*64; mutations.append(wrong)
        wrong = copy.deepcopy(self.contract); wrong["cases"][246]["source"]["path"] = "wrong.ox"; mutations.append(wrong)
        wrong = copy.deepcopy(self.contract); wrong["cases"].reverse(); mutations.append(wrong)
        effective, _ = c.admit_contract_amendment(self.contract, CONTRACT, authority); mutations.append(effective)
        for index, mutant in enumerate(mutations):
            with self.subTest(mutation=index):
                self.reject(lambda: c.admit_contract_amendment(mutant, CONTRACT, authority))

    def test_amendment_noncoordinate_wrong_old_and_typed_changes_reject(self):
        descriptor, amendment = c.amendment_artifacts(self.amendment_authority())
        for mutation in ("path", "message", "boolean", "float", "old_primary", "old_case_hash", "additional_pointer", "duplicate_pointer"):
            bad = copy.deepcopy(amendment)
            if mutation == "path": bad["operation"]["value"]["path"] = "wrong.ox"
            elif mutation == "message": bad["new_diagnostic"]["message"] = "invented"
            elif mutation == "boolean": bad["operation"]["value"]["column"] = True
            elif mutation == "float": bad["operation"]["value"]["column"] = 15.0
            elif mutation == "old_primary": bad["operation"]["old_value"]["start"] += 1
            elif mutation == "old_case_hash": bad["old_case_canonical_sha256"] = "0"*64
            elif mutation == "additional_pointer": bad["exact_changed_json_pointers"].append("/cases/0/expected/result")
            else: bad["exact_changed_json_pointers"].append(bad["exact_changed_json_pointers"][0])
            with self.subTest(mutation=mutation):
                self.reject(lambda: c.apply_coordinate_amendment(self.contract, descriptor, bad))
        wrong_descriptor = copy.deepcopy(descriptor); wrong_descriptor["effective_parser_document_canonical_sha256"] = "0"*64
        self.reject(lambda: c.apply_coordinate_amendment(self.contract, wrong_descriptor, amendment))

    def test_effective_comparison_cannot_relabel_base_or_summary_only(self):
        from unittest.mock import patch
        effective, receipt = c.admit_contract_amendment(self.contract, CONTRACT, self.amendment_authority())
        self.reject(lambda: c.compare_effective_rows(self.contract, receipt, [], IDENTITIES))
        self.reject(lambda: c.compare_effective_rows(receipt, receipt, [], IDENTITIES))
        forged = dict(receipt, effective_contract_identity="stale")
        self.reject(lambda: c.compare_effective_rows(effective, forged, [], IDENTITIES))
        with patch.object(c, "compare_rows", return_value={"status": "synthetic-spy"}) as compared:
            result = c.compare_effective_rows(effective, receipt, [], IDENTITIES)
            self.assertIs(compared.call_args.args[0], effective)
            self.assertEqual({key: result[key] for key in receipt}, receipt)
            self.assertEqual(result["execution_contract"]["contract_decoded_sha256"], c.CONTRACT_SHA)

    def test_new_authorization_requires_schema_and_amendment_authority(self):
        import tempfile
        keys = "status contract_decoded_sha256 package_freeze_sha256 authority_checkpoint observer_review observer_source_sha256 candidate_source_manifest_sha256 comparator_checkpoint observer_source_root collector_helper_rebind collector_helper_review effective_contract_authority".split()
        approval = dict.fromkeys(keys, None)
        approval["schema"] = "oxid-unit4-parser-comparison-authorization-v1"
        with tempfile.TemporaryDirectory() as folder:
            path = Path(folder) / "authorization.json"
            for missing in (False, True):
                value = copy.deepcopy(approval)
                if missing: del value["effective_contract_authority"]
                path.write_text(json.dumps(value))
                self.reject(lambda: c.authorization(path, c.sha(path.read_bytes())))

    def scan(self):
        case = self.cases["field-prefix-eof"]; row = make(case)
        error(row, case)
        event(row, "node_attempt", "field", 6, 0)
        event(row, "node_admit", "field", 6, 1)
        event(row, "field_scan_enter", "field", 6)
        event(row, "field_scan_exit", "field", 6)
        row["nodes_admitted"] = 1
        row["field_pub_scans"] = [dict(case["expected"]["field_pub_scans_exact"][0], start_seq=2, end_seq=3,
                                      source_bytes_read=0, namespace_work_units=0, reserve_calls=0, append_calls=0)]
        return case, row

    def test_positive_field_eof_scan(self):
        case, row = self.scan()
        c.check_observation(case, row, IDENTITIES)

    def test_successful_field_scan_must_precede_pub_consume_and_recognition(self):
        case = self.cases["field-prefix-ident"]; row = make(case); tape = row["token_inventory"]["tokens"]
        event(row, "node_attempt", "field", 6, 0); event(row, "node_admit", "field", 6, 1)
        event(row, "field_scan_enter", "field", 6); event(row, "field_scan_exit", "field", 6)
        event(row, "consume", "token", 6)
        trigger = event(row, "recognize", "field_public", 8, {"before": False, "after": True})
        scan = dict(case["expected"]["field_pub_scans_exact"][0], start_seq=2, end_seq=3,
                    source_bytes_read=0, namespace_work_units=0, reserve_calls=0, append_calls=0)
        c.recognition_trigger(trigger, row["events"][:-1], tape, [scan], c.Source(case))
        scan["end_seq"] = trigger["seq"] + 1
        self.reject(lambda: c.recognition_trigger(trigger, row["events"][:-1], tape, [scan], c.Source(case)), "event-order")

    def test_backward_cursor_events_rejected_independently(self):
        case, row = self.node_order()
        event(row, "consume", "token", 0)
        error = self.reject(lambda: c.events(row, case, c.Source(case), row["token_inventory"]["tokens"]), "event-order")
        self.assertEqual(error.detail, "parser cursor moved backward within one mode")

    def test_forged_scans_skipped_eof_repeated_initiator_and_work(self):
        for key, value in {"inspected_tokens": [6], "eof_charge_initiator": None, "terminal_kind": "Ident",
                           "result_is_ident": True, "end_seq": 2, "source_bytes_read": 1,
                           "namespace_work_units": 1, "reserve_calls": 1, "append_calls": 1}.items():
            case, row = self.scan(); row["field_pub_scans"][0][key] = value
            with self.subTest(field=key):
                self.reject(lambda: c.check_observation(case, row, IDENTITIES))
        case, row = self.scan(); row["field_pub_scans"] *= 2
        self.reject(lambda: c.check_observation(case, row, IDENTITIES), "field-scan")

    def test_token_tape_mutations_and_denied_admission(self):
        case = self.cases["ledger-empty"]; row = make(case)
        row["token_inventory"]["tokens"].append(copy.deepcopy(row["token_inventory"]["tokens"][0]))
        self.reject(lambda: c.check_observation(case, row, IDENTITIES))
        for name in ["trivia-token-count-100001", "whitespace-token-bytes-65537"]:
            case = self.cases[name]; inventory = c.source_tokens(c.Source(case), case["limits"])
            self.assertIsNotNone(inventory["denied_token_span"])
            self.assertEqual(inventory["eof_tokens"], 0)

    def test_unicode_scalar_crlf_origins_and_invalid_boundary(self):
        source = c.Source(self.cases["denial-pub-use-unicode-crlf"])
        loc = copy.deepcopy(self.cases["denial-pub-use-unicode-crlf"]["expected"]["first_diagnostic_projection"]["primary"])
        source.location(loc)
        loc["column"] += 1
        self.reject(lambda: source.location(loc))
        middle = next(i for i in range(len(source.data)) if i not in source.positions)
        self.reject(lambda: source.span([0, middle, middle]), "origin")

    def test_literal_diagnostic_bytes_and_full_vector(self):
        case = self.cases["unit1-double_colon_return"]
        row = {"diagnostics": copy.deepcopy(case["expected"]["diagnostics_exact"]), "recognized_final": False,
               "result": "parse_error", "human_diagnostic_bytes_base64": case["expected"]["human_diagnostics_base64_exact"]}
        c.expected_predicates(case, row)
        row["human_diagnostic_bytes_base64"] = encoded(c.b64(row["human_diagnostic_bytes_base64"], "literal") + b"\n")
        self.reject(lambda: c.expected_predicates(case, row))
        row["human_diagnostic_bytes_base64"] = case["expected"]["human_diagnostics_base64_exact"]
        row["diagnostics"][0]["invented_field"] = 1
        self.reject(lambda: c.expected_predicates(case, row))

    def test_projection_is_bounded_but_global_origins_are_not(self):
        case, row = self.node_order()
        row["diagnostics"][0]["notes"].append("allowed unprescribed note")
        row["json_diagnostic_bytes_base64"] = [encoded(json.dumps(row["diagnostics"][0]).encode())]
        c.check_observation(case, row, IDENTITIES)
        row["diagnostics"][0]["primary"]["end_column"] += 1
        row["json_diagnostic_bytes_base64"] = [encoded(json.dumps(row["diagnostics"][0]).encode())]
        self.reject(lambda: c.check_observation(case, row, IDENTITIES))

    def test_direct_overflow_requires_real_count_events(self):
        case = self.cases["q-count-overflow"]; row = make(case)
        error(row, case, "E0400", "project syntax count overflow", 0, 5)
        row.update(result="direct_seam_error", parse_attempts=0, recognized_final=None, path_segment_count_after="usize::MAX")
        event(row, "path_count_checked", "path_segment", 0, 2**64-1)
        event(row, "path_count_overflow", "path_segment", 0, 2**64-1)
        c.check_observation(case, row, IDENTITIES)
        row["events"][0]["detail"] -= 1
        self.reject(lambda: c.check_observation(case, row, IDENTITIES), "path-ledger" if False else "mismatch")

    def test_diagnostic_json_byte_binding(self):
        case, row = self.node_order()
        row["json_diagnostic_bytes_base64"] = [encoded(b"{}")]
        self.reject(lambda: c.check_observation(case, row, IDENTITIES))

    def test_duplicate_json_keys_nonfinite_and_debug_decoder(self):
        self.reject(lambda: c.loads('{"x":1,"x":2}'), "duplicate-json-key")
        self.reject(lambda: c.loads('{"x":NaN}'), "schema")
        self.assertEqual(decode('SourceProvenance { text_len: 0, file: SourceFileId(0), .. }'),
                         {"tag": "SourceProvenance", "text_len": 0, "file": {"tag": "SourceFileId", "items": [0]}, "debug_non_exhaustive": True})
        self.assertEqual(decode('Thing([Some(ExprId(2)), None, true, "x\\n\\u{3b1}"])')["items"][0][3], "x\nα")
        for invalid in ('Thing { x: 1, x: 2 }', 'Thing { .. }', 'Thing() trailing'):
            with self.assertRaises(DecodeError): decode(invalid)

    def test_all_contract_source_lexemes_and_expected_predicate_inventory(self):
        inventory = c.inventory(self.contract)
        self.assertEqual(inventory["unsupported_expected_fields"], [])
        self.assertEqual(inventory["unsupported_source_lexemes"], [])
        self.assertEqual(len(inventory["predicates"]), 22)
        self.assertEqual(inventory["original_rows_per_profile"], 71)

    def test_every_scalar_ledger_and_exact_predicate_mutation(self):
        expected = {"result": "parse_error", "recognized_final": False, "parse_attempts": 1,
                    "recognition_transitions_exact": [], "recognition_transitions_count": 0,
                    "diagnostic_count_exact": 1, "nodes_exact": 4, "non_eof_tokens": 9, "eof_tokens": 1,
                    "reserve_attempts_exact": 1, "reserve_trace_exact": [{"kind": "absolute paths", "length": 1, "element_bytes": 40, "success": False}],
                    "path_segment_count_after": "usize::MAX", "field_pub_lookahead_calls": 0,
                    "field_scans_disjoint": True, "field_lookahead_inspections_bound": "non_eof_tokens"}
        row = {"result": "parse_error", "recognized_final": False, "parse_attempts": 1, "recognition_transitions": [],
               "diagnostics": [{}], "nodes_admitted": 4, "token_inventory": {"non_eof_tokens": 9, "eof_tokens": 1},
               "reserve_attempts": 1, "reserve_trace": copy.deepcopy(expected["reserve_trace_exact"]),
               "path_segment_count_after": "usize::MAX", "field_pub_scans": []}
        c.expected_predicates({"expected": expected}, row)
        for key in ("reserve_attempts", "nodes_admitted", "path_segment_count_after", "recognized_final"):
            mutant = copy.deepcopy(row); mutant[key] = None
            self.reject(lambda: c.expected_predicates({"expected": expected}, mutant))
        mutant = copy.deepcopy(row); mutant["reserve_trace"][0]["success"] = True
        self.reject(lambda: c.expected_predicates({"expected": expected}, mutant))

    def test_raw_transport_source_mode_and_opaque_ast_binding(self):
        case = self.cases["ledger-empty"]
        row = make(case)
        raw_row = {k: copy.deepcopy(v) for k,v in row.items() if k not in {"schema", "binding", "json_diagnostic_bytes_base64", "human_diagnostic_bytes_base64"}}
        raw_row.update(mode="ProjectCandidate", mode_execution_index=0, source_generation=1, runtime_os="linux",
                       runtime_architecture="x86_64", pointer_width=64, json_diagnostic_renderings=[], human_diagnostic_rendering="")
        raw_row["ast"].pop("canonical")
        raw_row["ast"]["canonical_debug"] = "Program { tokens: [Token { kind: Eof, span: Span { file: SourceFileId(0), start: 0, end: 0 } }], functions: [], expressions: [], records: [], items: [], modules: [], paths: [], path_segments: [], imports: [], source: SourceProvenance { text_len: 0, file: SourceFileId(0), .. }, project_syntax: false }"
        envelope = {"schema": "oxid-unit4-parser-raw-v1", "case_id": case["id"], "nonce": row["binding"]["execution_id"],
                    "source_utf8": "", "display_path": case["source"]["path"], "observations": [raw_row]}
        build = {**IDENTITIES["debug"], "profile": "debug", "target": "x86_64-unknown-linux-gnu", "binary": {"sha256": "c"*64}}
        actual = c.normalize_raw(envelope, case, build, envelope["nonce"], HOST)
        self.assertEqual(actual, [row])
        for key, value in {"source_utf8": "x", "display_path": "stale.ox", "case_id": "stale", "nonce": "stale"}.items():
            mutant = copy.deepcopy(envelope); mutant[key] = value
            self.reject(lambda: c.normalize_raw(mutant, case, build, envelope["nonce"], HOST))
        mutant = copy.deepcopy(envelope); mutant["observations"] *= 2
        self.reject(lambda: c.normalize_raw(mutant, case, build, envelope["nonce"], HOST))
        for key, value in {"binding": row["binding"], "schema": c.SCHEMA, "unknown_raw_alias": True}.items():
            mutant = copy.deepcopy(envelope); mutant["observations"][0][key] = value
            with self.subTest(raw_reserved_key=key):
                self.reject(lambda: c.normalize_raw(mutant, case, build, envelope["nonce"], HOST))
        for key, value in {"runtime_os": "macos", "runtime_architecture": "aarch64", "pointer_width": 32}.items():
            mutant = copy.deepcopy(envelope); mutant["observations"][0][key] = value
            with self.subTest(raw_host_disagreement=key):
                self.reject(lambda: c.normalize_raw(mutant, case, build, envelope["nonce"], HOST))
        mutant = copy.deepcopy(envelope); mutant["observations"][0]["ast"]["canonical"] = row["ast"]["canonical"]
        self.reject(lambda: c.normalize_raw(mutant, case, build, envelope["nonce"], HOST))

    def test_file_identity_is_not_self_asserted(self):
        import tempfile
        with tempfile.TemporaryDirectory() as folder:
            path = Path(folder)/"binary"; path.write_bytes(b"synthetic executable identity")
            record = {"path": str(path), "bytes": path.stat().st_size, "sha256": c.sha(path.read_bytes())}
            self.assertEqual(c.artifact(record), path.read_bytes())
            path.write_bytes(b"stale executable")
            self.reject(lambda: c.artifact(record))

    def test_authority_checkpoint_hash_is_required_before_reading_evidence(self):
        import tempfile
        with tempfile.TemporaryDirectory() as folder:
            path = Path(folder)/"approval.json"; path.write_text("{}")
            self.reject(lambda: c.authorization(path, "0"*64))

    def test_unreviewed_or_unpaired_collector_rebind_rejected(self):
        original = {"run.py": {"sha256": "original"}}
        self.assertIs(c.collector_helpers({}, {}, {}, {}, original), original)
        self.reject(lambda: c.collector_helpers({"collector_helper_review": {}}, {}, {}, {}, original), "identity")
        self.reject(lambda: c.collector_helpers({"collector_helper_rebind": {}}, {}, {}, {}, original), "identity")
        self.reject(lambda: c.collector_helpers({"collector_helper_rebind": {"sha256": "0"*64}, "collector_helper_review": {"sha256": c.HELPER_REVIEW_SHA}}, {}, {}, {}, original))
        stale_v6 = {"collector_helper_rebind": {"sha256": "008b9bfa7f745a2aee67a399a3452fc9fc8f8e15b1961f985241f11b0acfa7b4"},
                    "collector_helper_review": {"sha256": "03d144546ad638abc4d0c668d6718fc710fd8ce0fa5855342d711fe9e3e7f13e"}}
        caught = self.reject(lambda: c.collector_helpers(stale_v6, {}, {}, {}, original))
        self.assertEqual(caught.detail, "reviewed guard/host/decoder collector rebind hash")

    def test_actual_measured_host_and_target_label_disagreement(self):
        c.measured_host(HOST)
        host = copy.deepcopy(HOST); host["uname"]["machine"] = "AMD64"
        c.measured_host(host)
        for field, value in {"os": "windows", "architecture": "aarch64", "python_pointer_width": 32}.items():
            host = copy.deepcopy(HOST); host[field] = value
            with self.subTest(host_field=field): self.reject(lambda: c.measured_host(host))
        for field, value in {"system": "Darwin", "machine": "aarch64"}.items():
            host = copy.deepcopy(HOST); host["uname"][field] = value
            with self.subTest(measurement=field): self.reject(lambda: c.measured_host(host))
        host = copy.deepcopy(HOST); host["python_pointer_width"] = 64.0
        self.reject(lambda: c.measured_host(host))
        host = copy.deepcopy(HOST); del host["uname"]
        self.reject(lambda: c.measured_host(host), "missing-signal")

    def test_optimized_normalizer_rebind_requires_reviewed_endpoints_and_report(self):
        import tempfile
        with tempfile.TemporaryDirectory() as folder:
            path = Path(folder)/"equivalence.json"
            original = {"parse_debug.py": {"path": "parse_debug.py", "bytes": 4120, "sha256": c.ORIGINAL_NORMALIZER_SHA}}
            optimized = {"parse_debug.py": {"path": "parse_debug.py", "bytes": 4192, "sha256": c.REVIEWED_NORMALIZER_SHA}}
            report = {"schema": "oxid-unit4-debug-decoder-equivalence-v7", "status": "pass", "control_count": 75,
                      "candidate_executions": 0, "expected_corpus_changes": 0,
                      "old_decoder": dict(original["parse_debug.py"], path="/synthetic/old/parse_debug.py"),
                      "new_decoder": dict(optimized["parse_debug.py"], path="/synthetic/new/parse_debug.py")}
            path.write_text(json.dumps(report)); identity = {"path": str(path), "bytes": path.stat().st_size, "sha256": c.sha(path.read_bytes())}
            rebind = {"decoder_equivalence": identity}
            review = {"old_normalizer_sha256": c.ORIGINAL_NORMALIZER_SHA, "new_normalizer_sha256": c.REVIEWED_NORMALIZER_SHA,
                      "decoder_equivalence": identity}
            c.normalizer_rebind(original, optimized, rebind, review)
            for stale in (c.ORIGINAL_NORMALIZER_SHA, "0"*64):
                wrong = copy.deepcopy(optimized); wrong["parse_debug.py"]["sha256"] = stale
                caught = self.reject(lambda: c.normalizer_rebind(original, wrong, rebind, review))
                self.assertEqual(caught.detail, "reviewed optimized normalizer endpoint")
            wrong_old = copy.deepcopy(original); wrong_old["parse_debug.py"]["sha256"] = "0"*64
            self.reject(lambda: c.normalizer_rebind(wrong_old, optimized, rebind, review))
            for endpoint in ("old_normalizer_sha256", "new_normalizer_sha256"):
                wrong_review = copy.deepcopy(review); wrong_review[endpoint] = "0"*64
                self.reject(lambda: c.normalizer_rebind(original, optimized, rebind, wrong_review))
            wrong_rebind = copy.deepcopy(rebind); wrong_rebind["decoder_equivalence"]["sha256"] = "0"*64
            caught = self.reject(lambda: c.normalizer_rebind(original, optimized, wrong_rebind, review))
            self.assertEqual(caught.detail, "review binds exact decoder equivalence artifact")
            path.write_text(json.dumps(dict(report, status="stale")))
            self.reject(lambda: c.normalizer_rebind(original, optimized, rebind, review))
            for key, value in (("status", "fail"), ("control_count", 74), ("candidate_executions", 1), ("expected_corpus_changes", 1)):
                changed = dict(report); changed[key] = value; path.write_text(json.dumps(changed))
                changed_identity = {"path": str(path), "bytes": path.stat().st_size, "sha256": c.sha(path.read_bytes())}
                self.reject(lambda: c.normalizer_rebind(original, optimized, {"decoder_equivalence": changed_identity}, dict(review, decoder_equivalence=changed_identity)))

    def test_complete_synthetic_disk_receipt_chain_and_identity_mutations(self):
        import tempfile
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            def save(name, value):
                path = root/name; path.parent.mkdir(parents=True, exist_ok=True)
                path.write_bytes(value if isinstance(value, bytes) else json.dumps(value, sort_keys=True).encode())
                return {"path": str(path), "bytes": path.stat().st_size, "sha256": c.sha(path.read_bytes())}
            def relative(record):
                return dict(record, path=Path(record["path"]).name)
            case = self.cases["ledger-empty"]; row = make(case)
            nonce = "1"*32; row["binding"]["execution_id"] = nonce
            production = save("overlay/candidate-source-manifest.json", {"schema": "oxid-unit4-candidate-source-manifest-v1", "commit": c.BASE_COMMIT, "files": []})
            helper1 = save("observer/run.py", b"synthetic driver"); helper2 = save("observer/parse_debug.py", b"synthetic normalizer")
            helpers = [relative(helper1), relative(helper2)]
            observer_sha = c.sha(json.dumps(helpers, sort_keys=True, separators=(",", ":")).encode())
            overlay = save("overlay/overlay-manifest.json", {"schema": "oxid-unit4-observer-overlay-v1", "base_commit": c.BASE_COMMIT,
                "control": False, "source": str(root/"overlay"), "candidate_source_manifest_sha256": production["sha256"],
                "observer_source_sha256": observer_sha, "files": [relative(production)], "observer_files": helpers})
            binary = save("binary", b"synthetic binary"); rustc = save("rustc", b"synthetic rustc")
            cargo_artifact = {"reason": "compiler-artifact", "executable": binary["path"],
                              "profile": {"test": True, "opt_level": "0", "debug_assertions": True, "overflow_checks": True},
                              "target": {"name": "oxid", "src_path": str(root/"overlay/src/cli.rs")}, "manifest_path": str(root/"overlay/Cargo.toml")}
            cargo = save("cargo.jsonl", (json.dumps(cargo_artifact)+"\n"+json.dumps({"reason": "build-finished", "success": True})+"\n").encode())
            empty = save("empty", b"")
            build = {"schema": "oxid-unit4-parser-build-v1", "status": "built", "exit_code": 0, "control": False,
                "profile": "debug", "cwd": str(root/"overlay"), "target": "x86_64-unknown-linux-gnu", "argv": ["cargo", "test", "--no-run", "--locked", "--target", "x86_64-unknown-linux-gnu"],
                "rustc_version": "rustc synthetic\nhost: x86_64-unknown-linux-gnu\n", "binary": binary, "rustc": rustc, "stdout": cargo,
                "stderr": empty, "overlay_manifest": overlay, "candidate_source_manifest_sha256": production["sha256"], "observer_source_sha256": observer_sha}
            checkpoint = save("checkpoint", b"explicitly synthetic test authority")
            approval = {"authority_checkpoint": checkpoint, "candidate_source_manifest_sha256": production["sha256"],
                        "observer_source_sha256": observer_sha, "observer_source_root": str(root/"observer"),
                        "observer_review": save("review.json", {"overlay_manifest": {"path": overlay["path"], "sha256": overlay["sha256"]}})}
            source = save("source.ox", b"")
            raw_path = root/"raw.json"
            env = {"UNIT4_NONCE": nonce, "UNIT4_CASE_ID": case["id"], "UNIT4_NODE_LIMIT": "100000", "UNIT4_TOKEN_LIMIT": "100000",
                   "UNIT4_RESERVE_FAIL_AT": "0", "UNIT4_CONTROL": "0", "UNIT4_SOURCE": source["path"], "UNIT4_DISPLAY_PATH": case["source"]["path"],
                   "UNIT4_ORIGINAL": "0", "UNIT4_REJECT_KIND": "", "UNIT4_REJECT_OCCURRENCE": "0", "UNIT4_DIRECT": "0",
                   "UNIT4_SEGMENT_START": "0", "UNIT4_SEGMENT_END": "0", "UNIT4_RAW_OUTPUT": str(raw_path)}
            request = save("request.json", {"case_id": case["id"], "source": source, "display_path": case["source"]["path"], "limits": case["limits"], "seam": {}, "original_mode_requested": False, "environment": env})
            raw_row = {k: copy.deepcopy(v) for k,v in row.items() if k not in {"schema", "binding", "json_diagnostic_bytes_base64", "human_diagnostic_bytes_base64"}}
            raw_row.update(mode="ProjectCandidate", mode_execution_index=0, source_generation=1, runtime_os="linux", runtime_architecture="x86_64", pointer_width=64, json_diagnostic_renderings=[], human_diagnostic_rendering="")
            raw_row["ast"].pop("canonical")
            raw_row["ast"]["canonical_debug"] = "Program { tokens: [Token { kind: Eof, span: Span { file: SourceFileId(0), start: 0, end: 0 } }], functions: [], expressions: [], records: [], items: [], modules: [], paths: [], path_segments: [], imports: [], source: SourceProvenance { text_len: 0, file: SourceFileId(0), .. }, project_syntax: false }"
            raw = {"schema": "oxid-unit4-parser-raw-v1", "case_id": case["id"], "nonce": nonce, "source_utf8": "", "display_path": case["source"]["path"], "observations": [raw_row]}
            normalized = c.normalize_raw(raw, case, build, nonce, HOST)
            receipt = {"case_id": case["id"], "execution_id": nonce, "status": "executed", "exit_code": 0,
                       "argv": [binary["path"], "frontend::parser::unit4_observer::observe_request", "--exact", "--ignored", "--nocapture", "--test-threads=1"],
                       "stdout": save("stdout", ("UNIT4_EXECUTED "+nonce+"\ntest result: ok. 1 passed; 0 failed; 0 ignored;\n").encode()),
                       "stderr": empty, "request": request, "raw": save("raw.json", raw), "observations": 1}
            manifest = {"schema": "oxid-unit4-parser-execution-v1", "status": "collected", "contract_decoded_sha256": c.CONTRACT_SHA,
                        "package_freeze_sha256": c.FREEZE_SHA, "host_runtime": HOST, "authority_checkpoint": checkpoint, "build_receipt": save("build.json", build),
                        "profile": "debug", "driver": helper1, "normalizer": helper2, "requested_case_ids": [case["id"]],
                        "case_count": 1, "case_receipts": [receipt], "observation_count": 1,
                        "observations": save("observations.jsonl", (json.dumps(normalized[0])+"\n").encode())}
            manifest_path = save("manifest.json", manifest)["path"]
            profile, observed, _ = c.execution_manifest(manifest_path, {"cases": [case]}, approval)
            self.assertEqual((profile, observed), ("debug", normalized))
            for field, value in {"status": "running", "requested_case_ids": [], "case_count": 0, "profile": "release", "observation_count": 0}.items():
                changed = copy.deepcopy(manifest); changed[field] = value
                save("manifest.json", changed)
                with self.subTest(field=field):
                    self.reject(lambda: c.execution_manifest(manifest_path, {"cases": [case]}, approval))
            changed = copy.deepcopy(manifest); changed["case_receipts"][0]["stdout"] = save("zero-stdout", b"test result: ok. 0 passed; 0 failed; 0 ignored;\n")
            save("manifest.json", changed)
            self.reject(lambda: c.execution_manifest(manifest_path, {"cases": [case]}, approval), "zero-execution")
            changed = copy.deepcopy(manifest); changed["driver"] = save("foreign-driver", b"wrong helper")
            save("manifest.json", changed)
            self.reject(lambda: c.execution_manifest(manifest_path, {"cases": [case]}, approval))
            # Rehash the tampered overlay and every outward receipt while
            # retaining the approved original source/helper identities.
            altered_overlay = c.loads(c.artifact(overlay)); altered_overlay["files"].append(relative(save("overlay/extra.rs", b"altered compiler code")))
            changed_build = copy.deepcopy(build); changed_build["overlay_manifest"] = save("altered-overlay.json", altered_overlay)
            changed = copy.deepcopy(manifest); changed["build_receipt"] = save("altered-build.json", changed_build)
            save("manifest.json", changed)
            self.reject(lambda: c.execution_manifest(manifest_path, {"cases": [case]}, approval))
            original_overlay = c.loads(c.artifact(overlay))
            changed_build = copy.deepcopy(build)
            changed_build["overlay_manifest"] = save("overlay/overlay-manifest.json", altered_overlay)
            changed = copy.deepcopy(manifest); changed["build_receipt"] = save("altered-build.json", changed_build)
            save("manifest.json", changed)
            caught = self.reject(lambda: c.execution_manifest(manifest_path, {"cases": [case]}, approval))
            self.assertEqual(caught.detail, "exact independently reviewed derived overlay sha256")
            self.assertEqual(save("overlay/overlay-manifest.json", original_overlay), overlay)
            # Wrong actual emitted profile/source data must fail even with
            # fresh hashes and convincing command/profile labels.
            for field, value in [("opt_level", "3"), ("debug_assertions", False), ("overflow_checks", False),
                                 ("manifest_path", str(root/"foreign/Cargo.toml")), ("src_path", str(root/"foreign/main.rs")), ("cwd", str(root/"foreign"))]:
                altered_artifact, changed_build = copy.deepcopy(cargo_artifact), copy.deepcopy(build)
                if field in {"opt_level", "debug_assertions", "overflow_checks"}: altered_artifact["profile"][field] = value
                elif field == "src_path": altered_artifact["target"][field] = value
                elif field == "manifest_path": altered_artifact[field] = value
                else: changed_build[field] = value
                changed_build["stdout"] = save("altered-cargo.jsonl", (json.dumps(altered_artifact)+"\n"+json.dumps({"reason":"build-finished","success":True})+"\n").encode())
                changed = copy.deepcopy(manifest); changed["build_receipt"] = save("altered-build.json", changed_build)
                save("manifest.json", changed)
                with self.subTest(actual_identity=field):
                    self.reject(lambda: c.execution_manifest(manifest_path, {"cases": [case]}, approval))


if __name__ == "__main__":
    unittest.main(verbosity=2)
