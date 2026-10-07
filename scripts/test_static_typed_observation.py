#!/usr/bin/env python3
"""Authored static wire/structure controls, not a host semantic oracle."""
import copy
import unittest

import parser_ast_observation as syntax
import static_typed_observation as observation
from test_parser_ast_observation import Fixture, token
from test_static_resolution_observation import AuthoredFixture, column, rich_fixture, two_functions


def success(fixture, facts, semantic):
    def cells(values):
        return [values.get(ref, 0) for ref in range(1, len(fixture.rows) + 1)] if isinstance(values, dict) else values
    return (fixture.wire() + b"STF1" + bytes([0, len(fixture.rows)]) + bytes(10)
            + column(cells(facts)) + column(cells(semantic)))


def failure(fixture, kind, at, *, secondary=None, label=0, expected=0, actual=0, nargs=0, argc=0):
    secondary = secondary or {"start": 0, "end": 0}
    return fixture.wire() + b"STF1" + bytes([
        1, len(fixture.rows), kind, at["start"], at["end"], secondary["start"], secondary["end"],
        label, expected, actual, nargs, argc,
    ])


def rich_semantic():
    return {1: 2, 2: 2, 5: 2, 6: 2, 8: 2, 9: 2, 10: 2, 12: 2, 13: 2,
            15: 1, 16: 12, 18: 1, 19: 4, 21: 8, 24: 2, 25: 2}


def flow(mask, f, r, b, c):
    return {"mask": mask, "fallthrough": f, "returns": r, "breaks": b, "continues": c,
            "debug": "FlowSummary { fallthrough: %s, returns: %s, breaks: %s, continues: %s }" %
                     tuple(str(value).lower() for value in (f, r, b, c))}


class TypedProjectionTests(unittest.TestCase):
    def decode(self, fixture, raw):
        return observation.decode(raw, fixture.source, fixture.tokens)

    def reject(self, fixture, raw, pattern=None):
        with self.assertRaisesRegex(observation.ObservationError, pattern or "."):
            self.decode(fixture, raw)

    def test_complete_hir_has_all_copied_types_and_flows(self):
        f, facts = rich_fixture()
        actual = self.decode(f, success(f, facts, rich_semantic()))
        self.assertEqual({k: actual[k] for k in ("schema", "status", "route")},
                         {"schema": "canonical-static-observation-1", "status": "ok", "route": "scalar"})
        self.assertEqual(set(actual), {"schema", "status", "route", "ast", "typed_hir"})
        self.assertEqual(actual["ast"], f.decode()["ast"])
        expression = lambda id, kind, first, last, ty="i32", **fields: {
            "id": id, "kind": kind, "span": f.between(first, last), "ty": ty, **fields}
        statement = lambda kind, first, last, **fields: {"kind": kind, "span": f.between(first, last), **fields}
        block = lambda id, first, last, exits, body: {
            "id": id, "span": f.between(first, last), "end": f.at(last), "flow": exits, "body": body}
        expected = {"functions": [{
            "id": 0, "name": "f", "signature": {"span": f.at("f"), "params": ["i32"], "result": "i32"},
            "body": 0, "end": f.at("close"),
            "locals": [
                {"id": 0, "name": "x", "mutable": False, "span": f.at("x"), "annotation": "i32", "ty": "i32"},
                {"id": 1, "name": "y", "mutable": True, "span": f.at("y"), "annotation": "i32", "ty": "i32"},
            ],
            "expressions": [
                expression(0, "Local", "xr", "xr", local=0),
                expression(1, "I32", "one", "one", value=1),
                expression(2, "Arithmetic", "xr", "one", op="Add", left=0, right=1, operator_span=f.at("plus")),
                expression(3, "Local", "yr", "yr", local=1),
                expression(4, "Group", "go", "gc", operand=3),
                expression(5, "Bool", "true", "true", ty="bool", value=True),
                expression(6, "Bool", "false", "false", ty="bool", value=False),
                expression(7, "Local", "arg", "arg", local=1),
                expression(8, "Call", "call", "cc", target=0, args=[7]),
            ],
            "blocks": [
                block(0, "open", "close", flow(2, False, True, False, False), [
                    statement("Let", "let", "ls", local=1, init=2),
                    statement("Assign", "ya", "as", local=1, target_span=f.at("ya"), operator_span=f.at("eq2"), value=4),
                    statement("While", "while", "wc", loop_id=1, condition=5, body=1),
                    statement("Return", "return", "rs", value=8),
                ]),
                block(1, "wo", "wc", flow(12, False, False, True, True), [
                    statement("If", "if", "ec", condition=6, then_block=2, else_block=3)]),
                block(2, "to", "tc", flow(4, False, False, True, False), [statement("Break", "break", "bs", target=1)]),
                block(3, "eo", "ec", flow(8, False, False, False, True), [statement("Continue", "continue", "cs", target=1)]),
            ],
        }]}
        self.assertEqual(actual["typed_hir"], expected)

    def test_supplied_type_binding_literal_and_flow_are_not_recomputed(self):
        f, facts = rich_fixture()
        semantic = rich_semantic()
        facts.update({3: 1, 4: 3, 7: 1, 10: -(1 << 31), 13: 2})
        semantic.update({1: 3, 2: 1, 6: 1, 9: 1, 13: 1, 24: 3, 25: 1, 8: 3, 12: 1, 16: 15})
        got = self.decode(f, success(f, facts, semantic))["typed_hir"]["functions"][0]
        self.assertEqual(got["signature"], {"span": f.at("f"), "params": ["bool"], "result": "()"})
        self.assertEqual(got["locals"][1]["ty"], "bool")
        self.assertEqual(got["expressions"][1]["value"], -(1 << 31))
        self.assertEqual(got["expressions"][2]["ty"], "()")
        self.assertEqual(got["expressions"][3]["local"], 0)
        self.assertEqual(got["blocks"][1]["flow"], flow(15, True, True, True, True))
        # These deliberately wrong semantics must reach the canonical comparator
        # unchanged. Agreement checks cannot turn this adapter into a checker.

    def test_all_nonzero_flow_masks_copy_the_four_observed_bits(self):
        f, facts = rich_fixture()
        views = [
            (1, True, False, False, False), (2, False, True, False, False), (3, True, True, False, False),
            (4, False, False, True, False), (5, True, False, True, False), (6, False, True, True, False),
            (7, True, True, True, False), (8, False, False, False, True), (9, True, False, False, True),
            (10, False, True, False, True), (11, True, True, False, True), (12, False, False, True, True),
            (13, True, False, True, True), (14, False, True, True, True), (15, True, True, True, True),
        ]
        for mask, *bits in views:
            with self.subTest(mask=mask):
                got = self.decode(f, success(f, facts, {**rich_semantic(), 16: mask}))
                self.assertEqual(got["typed_hir"]["functions"][0]["blocks"][1]["flow"], flow(mask, *bits))

    def test_signed_values_and_per_function_ids(self):
        f, facts = two_functions()
        semantic = {1: 3, 3: 2, 5: 2, 6: 3, 8: 2, 10: 2}
        for first, second in ((0, -1), (-(1 << 31), (1 << 31) - 1)):
            got = self.decode(f, success(f, {**facts, 5: first, 10: second}, semantic))["typed_hir"]["functions"]
            self.assertEqual([fn["id"] for fn in got], [0, 1])
            self.assertEqual([fn["expressions"][0]["id"] for fn in got], [0, 0])
            self.assertEqual([fn["expressions"][0]["value"] for fn in got], [first, second])
            self.assertEqual([fn["blocks"][0]["id"] for fn in got], [0, 0])

    def test_empty_and_public_project_routes(self):
        f = AuthoredFixture([])
        self.assertEqual(self.decode(f, success(f, {}, {})), {
            "schema": "canonical-static-observation-1", "status": "ok", "route": "scalar",
            "ast": {"tokens": f.tokens, "items": [], "functions": [], "expressions": []},
            "typed_hir": {"functions": []},
        })
        f = AuthoredFixture([token(*piece) for piece in [
            ("pub", "Pub", "pub"), ("s", "Trivia", " "), ("fn", "Fn", "fn"),
            ("s2", "Trivia", " "), ("f", "Ident", "f"), ("lp", "LParen", "("),
            ("rp", "RParen", ")"), ("arrow", "Arrow", "->"), ("t1", "LParen", "("),
            ("t2", "RParen", ")"), ("open", "LBrace", "{"), ("ret", "Return", "return"),
            ("semi", "Semi", ";"), ("close", "RBrace", "}"),
        ]])
        f.add(1, "f", "f", a=f.labels["pub"], c=2, d=3)
        f.add(4, "t1", "t2")
        f.add(5, "open", "close", a=f.labels["close"], b=4)
        f.add(10, "ret", "semi")
        got = self.decode(f, success(f, {1: 1, 2: 3}, {1: 3, 3: 2}))
        self.assertEqual(got["route"], "project_scalar")
        self.assertEqual(got["ast"]["functions"][0]["public"], f.at("pub"))
        self.assertIsNone(got["typed_hir"]["functions"][0]["blocks"][0]["body"][0]["value"])

    def test_framing_all_roles_inactive_and_sentinel_cells(self):
        f, facts = rich_fixture()
        semantic = rich_semantic()
        raw = success(f, facts, semantic)
        for offset, byte in [(1559, ord("X")), (1563, 2), (1563, 3), (1564, 0),
                             *[(i, 1) for i in range(1565, 1575)], (1703, 1), (2219, 1), (2606, 1),
                             (1575 + len(f.rows), 1), (2091 + len(f.rows), 1)]:
            with self.subTest(offset=offset):
                bad = bytearray(raw)
                bad[offset] = byte
                self.reject(f, bytes(bad))
        for bad in (raw[:1559], raw[:1575], raw[:-1], raw + b"x"):
            self.reject(f, bad)
        for ref, bad in [(1, 0), (2, -1), (6, 4), (8, 0), (10, 4), (12, -1), (15, 0), (24, 0),
                         (3, 1), (4, 1), (7, 1), (11, 1), (14, 1), (17, 1), (20, 1), (22, 1), (23, 1),
                         (5, 1), (5, 3), (16, 0), (16, -1), (19, 16)]:
            with self.subTest(ref=ref, bad=bad):
                self.reject(f, success(f, facts, {**semantic, ref: bad}))
        for ref, bad in [(1, 0), (2, 2), (3, 0), (4, 4), (6, 1), (5, 1), (8, 1), (15, 1),
                         (9, 0), (9, 1), (11, 5), (24, 6), (20, 5), (22, 19)]:
            with self.subTest(resolution_ref=ref, bad=bad):
                self.reject(f, success(f, {**facts, ref: bad}, semantic))
        for ref in (1, 2, 6, 9, 24):
            self.reject(f, success(f, facts, {**semantic, ref: 3}), "facts disagree")

    def test_missing_tokens_and_invalid_ast_still_reject(self):
        f, facts = rich_fixture()
        for tokens in (None, [], f.tokens[:-1]):
            with self.assertRaises(observation.ObservationError):
                observation.decode(success(f, facts, rich_semantic()), f.source, tokens)
        bad = copy.deepcopy(f)
        bad.rows[7]["b"] = 9
        self.reject(bad, success(bad, facts, rich_semantic()))

    def test_local_references_cannot_cross_function_ownership(self):
        pieces = []
        for prefix, function, local in (("a", "f", "x"), ("b", "g", "y")):
            pieces += [token(prefix + label, kind, text) for label, kind, text in [
                ("fn", "Fn", "fn"), ("s", "Trivia", " "), ("f", "Ident", function),
                ("lp", "LParen", "("), ("p", "Ident", local), ("colon", "Colon", ":"),
                ("pt", "Ident", "i32"), ("rp", "RParen", ")"), ("arrow", "Arrow", "->"),
                ("rt", "Ident", "i32"), ("open", "LBrace", "{"), ("ret", "Return", "return"),
                ("s2", "Trivia", " "), ("name", "Ident", local), ("semi", "Semi", ";"), ("close", "RBrace", "}"),
            ]]
        f = AuthoredFixture(pieces)
        for prefix, base in (("a", 0), ("b", 7)):
            f.add(1, prefix + "f", prefix + "f", b=base + 2, c=base + 4, d=base + 5, next=8 if base == 0 else 0)
            f.add(2, prefix + "p", prefix + "p", a=base + 3)
            f.add(3, prefix + "pt", prefix + "pt", a=f.labels[prefix + "pt"])
            f.add(3, prefix + "rt", prefix + "rt", a=f.labels[prefix + "rt"])
            f.add(5, prefix + "open", prefix + "close", a=f.labels[prefix + "close"], b=base + 6)
            f.add(10, prefix + "ret", prefix + "semi", a=base + 7)
            f.name(prefix + "name")
        facts = {1: 1, 2: 1, 3: 2, 4: 2, 7: 2, 8: 2, 9: 1, 10: 2, 11: 2, 14: 9}
        semantic = {1: 2, 2: 2, 5: 2, 7: 2, 8: 2, 9: 2, 12: 2, 14: 2}
        got = self.decode(f, success(f, facts, semantic))["typed_hir"]["functions"]
        self.assertEqual([function["locals"][0]["id"] for function in got], [0, 0])
        self.reject(f, success(f, {**facts, 7: 9}, semantic), "same function")
        self.reject(f, success(f, {**facts, 14: 2}, semantic), "same function")


class TypedDiagnosticTests(unittest.TestCase):
    def check(self, f, raw, code, message, at, *, phase="type", secondary=None, label=None, route=None):
        diagnostic = syntax.diagnostic(f.source, code, phase, message, at["start"], at["end"])
        if secondary:
            diagnostic["secondary"] = [{"span": syntax.diagnostic(f.source, code, phase, "",
                secondary["start"], secondary["end"])["primary"], "message": label}]
        expected = {"schema": "canonical-static-observation-1", "status": "diagnostic", "phase": phase,
                    "diagnostic": diagnostic}
        if route:
            expected["route"] = route
        self.assertEqual(observation.decode(raw, f.source, f.tokens), expected)

    def reject(self, f, raw):
        with self.assertRaises(observation.ObservationError):
            observation.decode(raw, f.source, f.tokens)

    def test_type_mismatch_constraint_spans_and_label_roles(self):
        f, _ = rich_fixture()
        cases = [
            (f.at("xr"), None, 0),  # arithmetic operand
            (f.at("one"), None, 0),
            (f.at("true"), None, 0),  # while condition
            (f.at("false"), None, 0),  # if condition
            (f.between("call", "cc"), None, 0),  # returned expression
            (f.at("arg"), f.at("f"), 2),  # call argument
            (f.between("xr", "one"), f.at("y"), 3),  # annotated initializer
            (f.between("go", "gc"), f.at("y"), 3),  # assignment RHS
        ]
        for at, second, label in cases:
            with self.subTest(at=at, label=label):
                raw = failure(f, 8, at, secondary=second, label=label, expected=1, actual=2)
                self.check(f, raw, "E0300", "type mismatch: expected bool, found i32", at, secondary=second,
                           label={0: None, 2: "function declared here", 3: "binding declared here"}[label])
        for at, second, label in [(f.at("arg"), None, 0), (f.at("yr"), None, 0),
                                  (f.between("go", "gc"), None, 0), (f.at("one"), f.at("y"), 3),
                                  (f.at("arg"), f.at("y"), 3), (f.at("true"), f.at("f"), 2),
                                  (f.at("arg"), f.at("y"), 2)]:
            self.reject(f, failure(f, 8, at, secondary=second, label=label, expected=1, actual=2))

    def test_absent_return_value_and_unary_operand(self):
        f = Fixture([])
        self.check(f, failure(f, 8, syntax.span(f.at("return")["start"], f.at("semi")["end"]), expected=2, actual=3),
                   "E0300", "type mismatch: expected i32, found ()",
                   syntax.span(f.at("return")["start"], f.at("semi")["end"]))
        f = Fixture([token("not", "Not", "!"), token("one", "Number", "1")])
        f.select(f.add(23, "not", "one", a=6, b=f.labels["not"], d=2))
        f.add(15, "one", "one", a=f.labels["one"], d=1)
        self.check(f, failure(f, 8, f.at("one"), expected=1, actual=2), "E0300",
                   "type mismatch: expected bool, found i32", f.at("one"))

    def test_equality_unit_requires_left_operand(self):
        f = Fixture([token("a", "LParen", "("), token("b", "RParen", ")"), token("eq", "EqualEqual", "=="),
                     token("c", "LParen", "("), token("d", "RParen", ")")])
        f.select(f.add(29, "a", "d", a=6, b=7, c=f.labels["eq"], d=2))
        f.add(18, "a", "b", d=1)
        f.add(18, "c", "d", d=1)
        left, right = syntax.span(f.at("a")["start"], f.at("b")["end"]), syntax.span(f.at("c")["start"], f.at("d")["end"])
        self.check(f, failure(f, 9, left), "E0300", "equality requires i32 or bool operands, found ()", left)
        self.reject(f, failure(f, 9, right))
        self.reject(f, failure(f, 8, left, expected=1, actual=3))
        self.check(f, failure(f, 8, right, expected=1, actual=3), "E0300",
                   "type mismatch: expected bool, found ()", right)

    def test_arity_has_structural_counts_and_named_function_secondary(self):
        f = Fixture([token("call", "Ident", "f"), token("lp", "LParen", "("), token("n", "Number", "1"),
                     token("rp", "RParen", ")")])
        f.select(f.add(20, "call", "rp", a=f.labels["call"], b=6, d=2))
        f.add(15, "n", "n", a=f.labels["n"], d=1)
        at = syntax.span(f.at("call")["start"], f.at("rp")["end"])
        raw = failure(f, 10, at, secondary=f.at("function"), label=2, nargs=0, argc=1)
        self.check(f, raw, "E0301", "wrong argument count: expected 0, found 1", at,
                   secondary=f.at("function"), label="function declared here")
        for expected, actual in ((1, 0), (0, 0), (128, 1), (0, 128)):
            self.reject(f, failure(f, 10, at, secondary=f.at("function"), label=2, nargs=expected, argc=actual))
        self.reject(f, failure(f, 10, f.at("call"), secondary=f.at("function"), label=2, nargs=0, argc=1))
        self.reject(f, failure(f, 8, f.at("n"), secondary=f.at("function"), label=2, expected=1, actual=2))

    def test_missing_return_and_whole_following_statements(self):
        f, _ = rich_fixture()
        self.check(f, failure(f, 11, f.at("close")), "E0302", "function requires an explicit terminal return", f.at("close"))
        self.reject(f, failure(f, 11, f.at("wc")))
        for kind, word in ((12, "return"), (13, "control transfer")):
            at = f.between("ya", "as")
            self.check(f, failure(f, kind, at), "E0303", "statement after terminal " + word + " is unavailable in typed-preview", at)
            self.reject(f, failure(f, kind, f.at("ya")))
            self.reject(f, failure(f, kind, f.between("let", "ls")))

    def test_immutable_assignment_target_and_parameter_secondary(self):
        f = AuthoredFixture([token(*p) for p in [
            ("fn", "Fn", "fn"), ("s", "Trivia", " "), ("f", "Ident", "f"), ("lp", "LParen", "("),
            ("p", "Ident", "x"), ("colon", "Colon", ":"), ("ty", "Ident", "i32"), ("rp", "RParen", ")"),
            ("arrow", "Arrow", "->"), ("u1", "LParen", "("), ("u2", "RParen", ")"), ("open", "LBrace", "{"),
            ("target", "Ident", "x"), ("eq", "Equal", "="), ("n", "Number", "1"), ("semi", "Semi", ";"),
            ("ret", "Return", "return"), ("rs", "Semi", ";"), ("close", "RBrace", "}"),
        ]])
        f.add(1, "f", "f", b=2, c=4, d=5)
        f.add(2, "p", "p", a=3)
        f.add(3, "ty", "ty", a=f.labels["ty"])
        f.add(4, "u1", "u2")
        f.add(5, "open", "close", a=f.labels["close"], b=6)
        f.add(8, "target", "semi", a=f.labels["target"], b=f.labels["eq"], c=7, next=8)
        f.add(15, "n", "n", a=f.labels["n"], d=1)
        f.add(10, "ret", "rs")
        self.check(f, failure(f, 14, f.at("target"), secondary=f.at("p"), label=4), "E0304",
                   "assignment requires a mutable local", f.at("target"), secondary=f.at("p"), label="immutable binding declared here")
        self.reject(f, failure(f, 14, f.between("target", "semi"), secondary=f.at("p"), label=4))
        self.reject(f, failure(f, 14, f.at("target"), secondary=f.at("f"), label=4))
        self.reject(f, failure(f, 8, f.at("n"), secondary=f.at("p"), label=3, expected=1, actual=2))

    def test_type_payload_spans_unused_fields_and_partial_facts_reject(self):
        f, _ = rich_fixture()
        raw = failure(f, 8, f.at("true"), expected=1, actual=2)
        for offset, value in [(1565, 0), (1565, 15), (1566, 128), (1567, 128),
                              (1568, 1), (1569, 1), (1570, 1), (1570, 4),
                              (1571, 0), (1571, 2), (1571, 4), (1572, 0), (1572, 4), (1573, 1), (1574, 1)]:
            bad = bytearray(raw)
            bad[offset] = value
            self.reject(f, bytes(bad))
        for bad in (raw[:-1], raw + b"x", raw + column([]), raw + column([]) + column([])):
            self.reject(f, bad)
        for kind, at in ((9, f.at("xr")), (11, f.at("close")), (12, f.between("ya", "as")),
                         (13, f.between("ya", "as"))):
            self.reject(f, failure(f, kind, at, expected=1))

    def test_resolution_unknown_name_and_owned_unknown_type(self):
        f = Fixture([token("name", "Ident", "missing")])
        f.select(f.name("name"))
        self.check(f, failure(f, 1, f.at("name")), "E0200", "unknown local `missing`", f.at("name"), phase="resolve")
        for length in (64, 65):
            f = AuthoredFixture([token(*p) for p in [
                ("fn", "Fn", "fn"), ("s", "Trivia", " "), ("f", "Ident", "f"), ("lp", "LParen", "("),
                ("rp", "RParen", ")"), ("a", "Arrow", "->"), ("t", "Ident", "T" * length),
                ("o", "LBrace", "{"), ("c", "RBrace", "}"),
            ]])
            f.add(1, "f", "f", c=2, d=3)
            f.add(3, "t", "t", a=f.labels["t"])
            f.add(5, "o", "c", a=f.labels["c"])
            name = "T" * length if length == 64 else "T" * 61 + "..."
            self.check(f, failure(f, 4, f.at("t")), "E0202", "unknown type `" + name + "`", f.at("t"),
                       phase="resolve", route="public_owned")
            self.reject(f, failure(f, 11, f.at("c")))

    def test_duplicate_before_unknown_type_keeps_public_owned_route(self):
        f = AuthoredFixture([token(*p) for p in [
            ("fn", "Fn", "fn"), ("s", "Trivia", " "), ("f", "Ident", "f"), ("lp", "LParen", "("),
            ("rp", "RParen", ")"), ("a", "Arrow", "->"), ("t", "Ident", "T"),
            ("o", "LBrace", "{"), ("c", "RBrace", "}"),
            ("fn2", "Fn", "fn"), ("s2", "Trivia", " "), ("f2", "Ident", "f"), ("lp2", "LParen", "("),
            ("rp2", "RParen", ")"), ("a2", "Arrow", "->"), ("u1", "LParen", "("), ("u2", "RParen", ")"),
            ("o2", "LBrace", "{"), ("c2", "RBrace", "}"),
        ]])
        f.add(1, "f", "f", c=2, d=3, next=4)
        f.add(3, "t", "t", a=f.labels["t"])
        f.add(5, "o", "c", a=f.labels["c"])
        f.add(1, "f2", "f2", c=5, d=6)
        f.add(4, "u1", "u2")
        f.add(5, "o2", "c2", a=f.labels["c2"])
        raw = failure(f, 3, f.at("f2"), secondary=f.at("f"), label=1)
        self.check(f, raw, "E0201", "duplicate binding; shadowing is unavailable in typed-preview", f.at("f2"),
                   phase="resolve", secondary=f.at("f"), label="first declared here", route="public_owned")
        self.reject(f, failure(f, 3, f.at("f"), secondary=f.at("f2"), label=1))

    def test_other_resolution_templates_and_contexts_are_inherited(self):
        f = Fixture([token("call", "Ident", "missing"), token("lp", "LParen", "("), token("rp", "RParen", ")")])
        f.select(f.add(20, "call", "rp", a=f.labels["call"], d=1))
        self.check(f, failure(f, 2, f.at("call")), "E0200", "unknown direct function `missing`", f.at("call"), phase="resolve")
        self.reject(f, failure(f, 2, syntax.span(f.at("call")["start"], f.at("rp")["end"])))
        f = Fixture([token("minus", "Minus", "-"), token("space", "Trivia", " /*x*/ "),
                     token("digits", "Number", "2147483649")])
        f.select(f.add(15, "minus", "digits", a=f.labels["digits"], b=1, d=1))
        at = syntax.span(f.at("minus")["start"], f.at("digits")["end"])
        self.check(f, failure(f, 5, at), "E0203", "decimal literal is outside the i32 range [-2147483648, 2147483647]", at, phase="resolve")
        self.reject(f, failure(f, 5, f.at("digits")))
        for word, row_kind, diagnostic_kind in (("break", 11, 6), ("continue", 12, 7)):
            f = Fixture([token("transfer", word.title(), word), token("semi", "Semi", ";")], statements=True)
            f.rows[3]["kind"] = row_kind
            at = syntax.span(f.at("transfer")["start"], f.at("semi")["end"])
            self.check(f, failure(f, diagnostic_kind, at), "E0204", "`" + word + "` requires an enclosing while in the same function", at, phase="resolve")
            self.reject(f, failure(f, diagnostic_kind, f.at("transfer")))

    def test_syntax_failure_passthrough_and_no_static_suffix(self):
        source = b"return"
        tokens = [{"id": 14, "kind": "Return", "file_id": 0, "start": 0, "end": 6},
                  {"id": 47, "kind": "Eof", "file_id": 0, "start": 6, "end": 6}]
        raw = b"OPA1" + bytes([1, 1, 0, 6, 0, 0, 6])
        self.assertEqual(observation.decode(raw, source, tokens), {
            "schema": "canonical-static-observation-1", "status": "diagnostic", "phase": "parse",
            "diagnostic": syntax.diagnostic(source, "E0100", "parse", "expected a top-level function declaration", 0, 6)})
        for tail in (b"x", b"STF1" + bytes(12)):
            with self.assertRaises(observation.ObservationError):
                observation.decode(raw + tail, source, tokens)
        source = b'"oops'
        raw = b"OPA1" + bytes([6, 1, 0, len(source), 0, 0, len(source)])
        self.assertEqual(observation.decode(raw, source), {
            "schema": "canonical-static-observation-1", "status": "lexical_diagnostic", "phase": "lex",
            "diagnostic": syntax.diagnostic(source, "E0100", "lex", "unterminated string literal", 0, len(source))})


if __name__ == "__main__":
    unittest.main()
