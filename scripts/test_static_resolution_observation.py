#!/usr/bin/env python3
"""Authored wire/structure controls, not candidate or canonical semantic oracles."""
import copy
import unittest

import parser_ast_observation as syntax
import static_resolution_observation as observation
from test_parser_ast_observation import Fixture, token


def column(values):
    values = list(values) + [0] * (129 - len(values))
    return bytes((value >> (plane * 8)) & 255 for plane in range(4) for value in values)


def success(fixture, facts, semantic=()):
    if isinstance(facts, dict):
        facts = [facts.get(index, 0) for index in range(1, len(fixture.rows) + 1)]
    return fixture.wire() + b"STF1" + bytes([2, len(fixture.rows)]) + bytes(10) + column(facts) + column(semantic)


def failure(fixture, kind, at, secondary=None):
    start, end = at["start"], at["end"]
    second = [secondary["start"], secondary["end"], 1] if secondary else [0, 0, 0]
    return fixture.wire() + b"STF1" + bytes([1, len(fixture.rows), kind, start, end, *second, 0, 0, 0, 0])


class AuthoredFixture(Fixture):
    """A token tape written out explicitly, with an independently authored tree."""

    def __init__(self, pieces):
        self.source, self.tokens, self.labels, self.rows = b"", [], {}, []
        for label, kind, text in pieces:
            start = len(self.source)
            self.source += text
            self.tokens.append({"id": syntax.KINDS.index(kind) + 1, "kind": kind,
                                "file_id": 0, "start": start, "end": len(self.source)})
            self.labels[label] = len(self.tokens)
        self.tokens.append({"id": 47, "kind": "Eof", "file_id": 0,
                            "start": len(self.source), "end": len(self.source)})

    def wire(self):
        raw = super().wire()
        return raw[:9] + bytes([1 if self.rows else 0]) + raw[10:]

    def between(self, first, last):
        return syntax.span(self.at(first)["start"], self.at(last)["end"])


def rich_fixture():
    pieces = [
        ("fn", "Fn", "fn"), ("s1", "Trivia", " "), ("f", "Ident", "f"),
        ("lp", "LParen", "("), ("x", "Ident", "x"), ("pc", "Colon", ":"),
        ("pt", "Ident", "i32"), ("rp", "RParen", ")"), ("arrow", "Arrow", "->"),
        ("rt", "Ident", "i32"), ("open", "LBrace", "{"),
        ("let", "Let", "let"), ("s2", "Trivia", " "), ("mut", "Mut", "mut"),
        ("s3", "Trivia", " "), ("y", "Ident", "y"), ("lc", "Colon", ":"),
        ("lt", "Ident", "i32"), ("eq1", "Equal", "="), ("xr", "Ident", "x"),
        ("plus", "Plus", "+"), ("one", "Number", "1"), ("ls", "Semi", ";"),
        ("ya", "Ident", "y"), ("eq2", "Equal", "="), ("go", "LParen", "("),
        ("yr", "Ident", "y"), ("gc", "RParen", ")"), ("as", "Semi", ";"),
        ("while", "While", "while"), ("s4", "Trivia", " "), ("true", "True", "true"),
        ("wo", "LBrace", "{"), ("if", "If", "if"), ("s5", "Trivia", " "),
        ("false", "False", "false"), ("to", "LBrace", "{"), ("break", "Break", "break"),
        ("bs", "Semi", ";"), ("tc", "RBrace", "}"), ("else", "Else", "else"),
        ("eo", "LBrace", "{"), ("continue", "Continue", "continue"), ("cs", "Semi", ";"),
        ("ec", "RBrace", "}"), ("wc", "RBrace", "}"), ("return", "Return", "return"),
        ("s6", "Trivia", " "), ("call", "Ident", "f"), ("co", "LParen", "("),
        ("arg", "Ident", "y"), ("cc", "RParen", ")"), ("rs", "Semi", ";"),
        ("close", "RBrace", "}"),
    ]
    f = AuthoredFixture([token(*piece) for piece in pieces])
    f.add(1, "f", "f", b=2, c=4, d=5)  # 1
    f.add(2, "x", "x", a=3)
    f.add(3, "pt", "pt", a=f.labels["pt"])
    f.add(3, "rt", "rt", a=f.labels["rt"])
    f.add(5, "open", "close", a=f.labels["close"], b=6)
    f.add(7, "let", "ls", a=f.labels["y"], b=7, c=8, next=11)
    f.add(3, "lt", "lt", a=f.labels["lt"])
    f.add(24, "xr", "one", a=9, b=10, c=f.labels["plus"], d=2)
    f.name("xr")
    f.add(15, "one", "one", a=f.labels["one"], d=1)
    f.add(8, "ya", "as", a=f.labels["ya"], b=f.labels["eq2"], c=12, next=14)
    f.add(21, "go", "gc", a=13, d=2)
    f.name("yr")
    f.add(14, "while", "wc", a=15, b=16, next=23)
    f.add(17, "true", "true", d=1)
    f.add(5, "wo", "wc", a=f.labels["wc"], b=17)
    f.add(13, "if", "ec", a=18, b=19, c=21)
    f.add(16, "false", "false", d=1)
    f.add(5, "to", "tc", a=f.labels["tc"], b=20)
    f.add(11, "break", "bs")
    f.add(5, "eo", "ec", a=f.labels["ec"], b=22)
    f.add(12, "continue", "cs")
    f.add(10, "return", "rs", a=24)
    f.add(20, "call", "cc", a=f.labels["call"], b=25, d=2)
    f.name("arg")
    facts = {1: 1, 2: 1, 3: 2, 4: 2, 6: 2, 7: 2, 9: 2, 10: 1,
             11: 6, 13: 6, 20: 16, 22: 16, 24: 1, 25: 6}
    return f, facts


def two_functions():
    f = AuthoredFixture([token(*piece) for piece in [
        ("fn", "Fn", "fn"), ("s", "Trivia", " "), ("f", "Ident", "f"),
        ("lp", "LParen", "("), ("rp", "RParen", ")"), ("a", "Arrow", "->"),
        ("t1", "LParen", "("), ("t2", "RParen", ")"), ("o1", "LBrace", "{"),
        ("r1", "Return", "return"), ("s1", "Trivia", " "), ("n1", "Number", "1"),
        ("semi1", "Semi", ";"), ("c1", "RBrace", "}"),
        ("fn2", "Fn", "fn"), ("s2", "Trivia", " "), ("g", "Ident", "g"),
        ("lp2", "LParen", "("), ("rp2", "RParen", ")"), ("a2", "Arrow", "->"),
        ("t3", "LParen", "("), ("t4", "RParen", ")"), ("o2", "LBrace", "{"),
        ("r2", "Return", "return"), ("s3", "Trivia", " "), ("n2", "Number", "2"),
        ("semi2", "Semi", ";"), ("c2", "RBrace", "}"),
    ]])
    f.add(1, "f", "f", c=2, d=3, next=6)
    f.add(4, "t1", "t2")
    f.add(5, "o1", "c1", a=f.labels["c1"], b=4)
    f.add(10, "r1", "semi1", a=5)
    f.add(15, "n1", "n1", a=f.labels["n1"], d=1)
    f.add(1, "g", "g", c=7, d=8)
    f.add(4, "t3", "t4")
    f.add(5, "o2", "c2", a=f.labels["c2"], b=9)
    f.add(10, "r2", "semi2", a=10)
    f.add(15, "n2", "n2", a=f.labels["n2"], d=1)
    return f, {1: 1, 2: 3, 5: 1, 6: 2, 7: 3, 10: 2}


class ResolutionProjectionTests(unittest.TestCase):
    def decode(self, fixture, raw):
        return observation.decode(raw, fixture.source, fixture.tokens)

    def reject(self, fixture, raw, pattern=None):
        with self.assertRaisesRegex(observation.ObservationError, pattern or "."):
            self.decode(fixture, raw)

    def test_complete_hir_structure_and_pending_typing(self):
        f, facts = rich_fixture()
        actual = self.decode(f, success(f, facts))
        self.assertEqual({k: actual[k] for k in ("schema", "status", "phase", "typing", "route")},
                         {"schema": "canonical-resolution-observation-1", "status": "resolved",
                          "phase": "resolve", "typing": "pending", "route": "scalar"})
        self.assertEqual(actual["ast"], f.decode()["ast"])
        expression = lambda id, kind, first, last, **fields: {
            "id": id, "kind": kind, "span": f.between(first, last), **fields}
        statement = lambda kind, first, last, **fields: {
            "kind": kind, "span": f.between(first, last), **fields}
        block = lambda id, first, last, body: {
            "id": id, "span": f.between(first, last), "end": f.at(last), "body": body}
        expected = {"functions": [{
            "id": 0, "name": "f", "signature": {"span": f.at("f"), "params": ["i32"], "result": "i32"},
            "body": 0, "end": f.at("close"),
            "locals": [
                {"id": 0, "name": "x", "mutable": False, "span": f.at("x"), "annotation": "i32"},
                {"id": 1, "name": "y", "mutable": True, "span": f.at("y"), "annotation": "i32"},
            ],
            "expressions": [
                expression(0, "Local", "xr", "xr", local=0),
                expression(1, "I32", "one", "one", value=1),
                expression(2, "Arithmetic", "xr", "one", op="Add", left=0, right=1, operator_span=f.at("plus")),
                expression(3, "Local", "yr", "yr", local=1),
                expression(4, "Group", "go", "gc", operand=3),
                expression(5, "Bool", "true", "true", value=True),
                expression(6, "Bool", "false", "false", value=False),
                expression(7, "Local", "arg", "arg", local=1),
                expression(8, "Call", "call", "cc", target=0, args=[7]),
            ],
            "blocks": [
                block(0, "open", "close", [
                    statement("Let", "let", "ls", local=1, init=2),
                    statement("Assign", "ya", "as", local=1, target_span=f.at("ya"), operator_span=f.at("eq2"), value=4),
                    statement("While", "while", "wc", loop_id=1, condition=5, body=1),
                    statement("Return", "return", "rs", value=8),
                ]),
                block(1, "wo", "wc", [statement("If", "if", "ec", condition=6, then_block=2, else_block=3)]),
                block(2, "to", "tc", [statement("Break", "break", "bs", target=1)]),
                block(3, "eo", "ec", [statement("Continue", "continue", "cs", target=1)]),
            ],
        }]}
        self.assertEqual(actual["resolved_hir"], expected)
        self.assertNotIn("typed_hir", actual)

    def test_candidate_values_and_primitive_codes_are_copied_not_inferred(self):
        f, facts = rich_fixture()
        facts[10], facts[3], facts[4], facts[7] = -(1 << 31), 1, 3, 1
        result = self.decode(f, success(f, facts))["resolved_hir"]["functions"][0]
        self.assertEqual(result["expressions"][1]["value"], -(1 << 31))
        self.assertEqual(result["signature"]["params"], ["bool"])
        self.assertEqual(result["signature"]["result"], "()")
        self.assertEqual(result["locals"][1]["annotation"], "bool")
        # A wrong, but structurally valid, supplied binding must survive for the
        # canonical comparator to reject; projection must not fix it by lookup.
        facts[13] = 2
        self.assertEqual(self.decode(f, success(f, facts))["resolved_hir"]["functions"][0]
                         ["expressions"][3]["local"], 0)

    def test_signed_zero_min_max_and_literal_trivia_are_exact(self):
        f = Fixture([token("minus", "Minus", "-"), token("trivia", "Trivia", " /*x*/ "),
                     token("digits", "Number", "0000")])
        f.add(15, "minus", "digits", a=f.labels["digits"], b=1, d=1)
        f.select(5)
        for value in (0, -(1 << 31), (1 << 31) - 1, -1):
            with self.subTest(value=value):
                got = self.decode(f, success(f, {1: 1, 2: 3, 5: value}))["resolved_hir"]["functions"][0]
                self.assertEqual(got["expressions"], [{"id": 0, "kind": "I32", "value": value,
                    "span": syntax.span(f.at("minus")["start"], f.at("digits")["end"])}])

    def test_function_expression_and_block_ids_restart(self):
        f, facts = two_functions()
        result = self.decode(f, success(f, facts))["resolved_hir"]["functions"]
        self.assertEqual([function["id"] for function in result], [0, 1])
        for index, function in enumerate(result):
            self.assertEqual(function["expressions"][0]["id"], 0)
            self.assertEqual(function["expressions"][0]["value"], index + 1)
            self.assertEqual(function["blocks"][0]["id"], 0)
            self.assertEqual(function["blocks"][0]["body"][0]["value"], 0)

    def test_empty_program(self):
        f = AuthoredFixture([])
        self.assertEqual(self.decode(f, success(f, {}))["resolved_hir"], {"functions": []})

    def test_same_kind_local_reference_cannot_cross_function_ownership(self):
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
        functions = self.decode(f, success(f, facts))["resolved_hir"]["functions"]
        self.assertEqual([function["locals"][0]["id"] for function in functions], [0, 0])
        self.reject(f, success(f, {**facts, 14: 2}), "same function")
        self.reject(f, success(f, {**facts, 7: 9}), "same function")

    def test_public_function_preserves_project_route_and_ast_visibility(self):
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
        result = self.decode(f, success(f, {1: 1, 2: 3}))
        self.assertEqual(result["route"], "project_scalar")
        self.assertEqual(result["ast"]["functions"][0]["public"], f.at("pub"))
        self.assertIsNone(result["resolved_hir"]["functions"][0]["blocks"][0]["body"][0]["value"])

    def test_wire_framing_row_roles_and_all_zero_semantic_column(self):
        f, facts = rich_fixture()
        raw = success(f, facts)
        for offset, byte in [(1559, ord("X")), (1563, 0), (1563, 3), (1564, 0),
                             *[(i, 1) for i in range(1565, 1575)], (2091, 1), (2606, 1), (1703, 1)]:
            with self.subTest(offset=offset):
                bad = bytearray(raw)
                bad[offset] = byte
                self.reject(f, bytes(bad))
        for bad in (raw[:1559], raw[:-1], raw + b"x"):
            self.reject(f, bad)
        for ref, bad_value in [(1, 0), (2, 2), (3, 0), (3, -1), (4, 4), (6, 1),
                               (5, 1), (8, 1), (15, 1), (23, 1),
                               (9, 0), (9, 1), (9, 128), (11, 5), (24, 6), (20, 5), (22, 19)]:
            with self.subTest(ref=ref, bad_value=bad_value):
                self.reject(f, success(f, {**facts, ref: bad_value}))
        # Synthetic tag-2 probe-shaped payloads have no resolver row roles.
        self.reject(f, success(f, [index * 11 for index in range(129)]))

    def test_full_opa1_and_independent_tokens_are_still_required(self):
        f, facts = rich_fixture()
        raw = success(f, facts)
        for tokens in (None, [], f.tokens[:-1], [{**f.tokens[0], "end": 1}, *f.tokens[1:]]):
            with self.assertRaises(observation.ObservationError):
                observation.decode(raw, f.source, tokens)
        bad = copy.deepcopy(f)
        bad.rows[7]["b"] = 9
        self.reject(bad, success(bad, facts))
        with self.assertRaises(observation.ObservationError):
            observation.decode(raw, f.source[:-1], f.tokens)


class ResolutionDiagnosticTests(unittest.TestCase):
    def check(self, f, raw, code, message, at, secondary=None, route=None):
        got = observation.decode(raw, f.source, f.tokens)
        expected = syntax.diagnostic(f.source, code, "resolve", message, at["start"], at["end"])
        if secondary:
            expected["secondary"] = [{"span": syntax.diagnostic(f.source, code, "resolve", "",
                secondary["start"], secondary["end"])["primary"], "message": "first declared here"}]
        result = {"schema": "canonical-resolution-observation-1", "status": "diagnostic", "phase": "resolve",
                  "diagnostic": expected}
        if route:
            result["route"] = route
        self.assertEqual(got, result)
        self.assertNotIn("resolved_hir", got)

    def test_unknown_local_and_call_exact_identifier_contexts(self):
        for call in (False, True):
            pieces = [token("name", "Ident", "missing")]
            if call:
                pieces += [token("lp", "LParen", "("), token("rp", "RParen", ")")]
            f = Fixture(pieces)
            if call:
                f.add(20, "name", "rp", a=f.labels["name"], d=1)
            else:
                f.name("name")
            f.select(5)
            self.check(f, failure(f, 2 if call else 1, f.at("name")), "E0200",
                       "unknown " + ("direct function" if call else "local") + " `missing`", f.at("name"))
            if call:
                with self.assertRaises(observation.ObservationError):
                    observation.decode(failure(f, 2, syntax.span(f.at("name")["start"], f.at("rp")["end"])),
                                       f.source, f.tokens)

    def test_range_and_loop_diagnostics(self):
        f = Fixture([token("minus", "Minus", "-"), token("space", "Trivia", " "),
                     token("digits", "Number", "2147483649")])
        f.add(15, "minus", "digits", a=f.labels["digits"], b=1, d=1)
        f.select(5)
        at = syntax.span(f.at("minus")["start"], f.at("digits")["end"])
        self.check(f, failure(f, 5, at), "E0203", "decimal literal is outside the i32 range [-2147483648, 2147483647]", at)
        for word, tag, code in (("Break", 11, 6), ("Continue", 12, 7)):
            f = Fixture([token("keyword", word, word.lower()), token("semi", "Semi", ";")], statements=True)
            f.rows[3]["kind"] = tag
            at = syntax.span(f.at("keyword")["start"], f.at("semi")["end"])
            self.check(f, failure(f, code, at), "E0204", "`" + word.lower() + "` requires an enclosing while in the same function", at)
            with self.assertRaises(observation.ObservationError):
                observation.decode(failure(f, code, f.at("keyword")), f.source, f.tokens)

    def test_duplicate_binding_exact_secondary(self):
        f, _ = two_functions()
        # Same-width spelling edit changes the independently authored second name.
        pos = f.at("g")["start"]
        f.source = f.source[:pos] + b"f" + f.source[pos + 1:]
        self.check(f, failure(f, 3, f.at("g"), f.at("f")), "E0201",
                   "duplicate binding; shadowing is unavailable in typed-preview", f.at("g"), f.at("f"))
        for bad in (failure(f, 3, f.at("f"), f.at("g")), failure(f, 3, f.at("g")),
                    failure(f, 3, f.at("g"), f.at("g")), failure(f, 3, f.at("g"), f.at("n1"))):
            with self.assertRaises(observation.ObservationError):
                observation.decode(bad, f.source, f.tokens)

    def test_unknown_type_display_64_byte_rule_and_owned_route(self):
        for length in (64, 65):
            name = "T" * length
            f = AuthoredFixture([token(*piece) for piece in [
                ("fn", "Fn", "fn"), ("s", "Trivia", " "), ("f", "Ident", "f"),
                ("lp", "LParen", "("), ("rp", "RParen", ")"), ("arrow", "Arrow", "->"),
                ("ty", "Ident", name), ("open", "LBrace", "{"), ("close", "RBrace", "}"),
            ]])
            f.add(1, "f", "f", c=2, d=3)
            f.add(3, "ty", "ty", a=f.labels["ty"])
            f.add(5, "open", "close", a=f.labels["close"])
            shown = name if length == 64 else "T" * 61 + "..."
            self.check(f, failure(f, 4, f.at("ty")), "E0202", "unknown type `" + shown + "`", f.at("ty"), route="public_owned")
            with self.assertRaises(observation.ObservationError):
                observation.decode(success(f, {1: 1, 2: 2}), f.source, f.tokens)

    def test_diagnostic_unused_fields_wrong_roles_and_no_partial_facts(self):
        f = Fixture([token("name", "Ident", "missing")])
        f.select(f.name("name"))
        raw = failure(f, 1, f.at("name"))
        for offset, byte in [(1565, 0), (1565, 8), *[(i, 1) for i in range(1568, 1575)]]:
            bad = bytearray(raw)
            bad[offset] = byte
            with self.assertRaises(observation.ObservationError):
                observation.decode(bytes(bad), f.source, f.tokens)
        for bad in (raw + column([]), raw + b"x", raw[:-1], failure(f, 2, f.at("name")), failure(f, 1, f.at("function"))):
            with self.assertRaises(observation.ObservationError):
                observation.decode(bad, f.source, f.tokens)

    def test_inherited_eleven_byte_syntax_failure_has_no_suffix(self):
        source = b"return"
        tokens = [{"id": 14, "kind": "Return", "file_id": 0, "start": 0, "end": 6},
                  {"id": 47, "kind": "Eof", "file_id": 0, "start": 6, "end": 6}]
        raw = b"OPA1" + bytes([1, 1, 0, 6, 0, 0, 6])
        result = observation.decode(raw, source, tokens)
        self.assertEqual(result, {"schema": "canonical-resolution-observation-1", "status": "diagnostic",
            "phase": "parse", "diagnostic": syntax.diagnostic(source, "E0100", "parse", "expected a top-level function declaration", 0, 6)})
        with self.assertRaises(observation.ObservationError):
            observation.decode(raw + b"STF1" + bytes(12), source, tokens)

    def test_inherited_lexical_failure_needs_no_nonexistent_success_tape(self):
        for source, detail, message in ((b'"oops', 1, "unterminated string literal"),
                                        (b"/*oops", 2, "unterminated block comment")):
            raw = b"OPA1" + bytes([6, detail, 0, len(source), 0, 0, len(source)])
            self.assertEqual(observation.decode(raw, source), {
                "schema": "canonical-resolution-observation-1", "status": "lexical_diagnostic", "phase": "lex",
                "diagnostic": syntax.diagnostic(source, "E0100", "lex", message, 0, len(source))})
            with self.assertRaises(observation.ObservationError):
                observation.decode(raw + b"x", source)


if __name__ == "__main__":
    unittest.main()
