#!/usr/bin/env python3
"""Authored OPA1 controls, independent of candidate/compiler execution.

These fixtures prescribe token records, rows and expected syntax directly. They
are decoder checks, not a lexer oracle or candidate/canonical parity evidence.
"""
import copy
import unittest

import parser_ast_observation as observation


def token(label, kind, spelling):
    return label, kind, spelling.encode("ascii")


class Fixture:
    """An explicitly authored single-return function and its logical row table."""

    def __init__(self, expression_tokens, *, statements=False):
        pieces = [token("fn", "Fn", "fn"), token("space", "Trivia", " "),
                  token("function", "Ident", "f"), token("params", "LParen", "("),
                  token("params_end", "RParen", ")"), token("arrow", "Arrow", "->"),
                  token("type", "LParen", "("), token("type_end", "RParen", ")"),
                  token("block", "LBrace", "{")]
        if not statements:
            pieces += [token("return", "Return", "return"), token("value_space", "Trivia", " ")]
        pieces += expression_tokens
        if not statements:
            pieces.append(token("semi", "Semi", ";"))
        pieces.append(token("end", "RBrace", "}"))
        self.source, self.tokens, self.labels = b"", [], {}
        for label, kind, spelling in pieces:
            start = len(self.source)
            self.source += spelling
            self.tokens.append({"id": observation.KINDS.index(kind) + 1, "kind": kind,
                                "file_id": 0, "start": start, "end": len(self.source)})
            self.labels[label] = len(self.tokens)
        self.tokens.append({"id": 47, "kind": "Eof", "file_id": 0,
                            "start": len(self.source), "end": len(self.source)})
        self.rows = []
        self.add(1, "function", "function", c=2, d=3)
        self.add(4, "type", "type_end")
        self.add(5, "block", "end", a=self.labels["end"], b=4)
        if statements:
            self.add(9, expression_tokens[0][0], expression_tokens[-1][0])
        else:
            self.add(10, "return", "semi")

    def at(self, label):
        t = self.tokens[self.labels[label] - 1]
        return observation.span(t["start"], t["end"])

    def add(self, kind, first, last, **fields):
        self.rows.append({"kind": kind, "start": self.at(first)["start"],
                          "end": self.at(last)["end"], "a": 0, "b": 0,
                          "c": 0, "d": 0, "next": 0, **fields})
        return len(self.rows)

    def name(self, label):
        return self.add(19, label, label, a=self.labels[label], d=1)

    def select(self, root):
        self.rows[3]["a"] = root
        return self

    def wire(self):
        result = bytearray(b"OPA1" + bytes([0, 0, 0, 0, len(self.rows), 1, len(self.source)]))
        columns = [[], [], []]
        for r in self.rows:
            columns[0].append(r["kind"] + (r["start"] << 6) + (r["end"] << 14) + (r["next"] << 22))
            columns[1].append(r["a"] + (r["b"] << 8))
            columns[2].append(r["c"] + (r["d"] << 8))
        for column in columns:
            column += [0] * (129 - len(column))
            for plane in range(4):
                result.extend((value >> (8 * plane)) & 255 for value in column)
        return bytes(result)

    def decode(self):
        return observation.decode(self.wire(), self.source, self.tokens)


def simple_binary(kind=24, token_kind="Plus", spelling="+"):
    f = Fixture([token("a", "Ident", "a"), token("op", token_kind, spelling),
                 token("b", "Ident", "b")])
    # Parent-before-child physical order deliberately differs from canonical IDs.
    root = f.add(kind, "a", "b", a=6, b=7, c=f.labels["op"], d=2)
    f.name("a")
    f.name("b")
    return f.select(root)


class ProjectionExpressionTests(unittest.TestCase):
    def reject(self, fixture, pattern=None):
        if pattern:
            with self.assertRaisesRegex(observation.ObservationError, pattern):
                fixture.decode()
        else:
            with self.assertRaises(observation.ObservationError):
                fixture.decode()

    def test_every_binary_operator_has_exact_fields_and_postorder_ids(self):
        cases = [
            (24, "Plus", "+", "Arithmetic", "Add"),
            (25, "Minus", "-", "Arithmetic", "Subtract"),
            (26, "Star", "*", "Arithmetic", "Multiply"),
            (27, "Slash", "/", "Arithmetic", "Divide"),
            (28, "Percent", "%", "Arithmetic", "Remainder"),
            (29, "EqualEqual", "==", "Comparison", "Equal"),
            (30, "NotEqual", "!=", "Comparison", "NotEqual"),
            (31, "Less", "<", "Comparison", "Less"),
            (32, "LessEqual", "<=", "Comparison", "LessEqual"),
            (33, "Greater", ">", "Comparison", "Greater"),
            (34, "GreaterEqual", ">=", "Comparison", "GreaterEqual"),
            (35, "AndAnd", "&&", "Logical", "And"),
            (36, "OrOr", "||", "Logical", "Or"),
        ]
        for kind, token_kind, spelling, family, op in cases:
            with self.subTest(op=op):
                f = simple_binary(kind, token_kind, spelling)
                got = f.decode()["ast"]
                self.assertEqual(got["expressions"], [
                    {"id": 0, "kind": "Name", "name": f.at("a"), "span": f.at("a")},
                    {"id": 1, "kind": "Name", "name": f.at("b"), "span": f.at("b")},
                    {"id": 2, "kind": family, "op": op, "left": 0, "right": 1,
                     "operator_span": f.at("op"),
                     "span": observation.span(f.at("a")["start"], f.at("b")["end"])},
                ])
                self.assertEqual(got["functions"][0]["blocks"][0]["body"][0]["value"], 2)
                for field, value in [("d", 1), ("d", 3), ("c", f.labels["a"]),
                                     ("a", 0), ("a", 5), ("b", 6), ("next", 6),
                                     ("start", f.at("op")["start"]),
                                     ("end", f.at("op")["end"])]:
                    bad = copy.deepcopy(f)
                    bad.rows[4][field] = value
                    self.reject(bad)

    def test_group_prefixes_signed_literal_and_trivia(self):
        f = Fixture([token("not", "Not", "!"), token("negate", "Minus", "-"),
                     token("open", "LParen", "("), token("sign", "Minus", "-"),
                     token("comment", "Trivia", " /*x*/ "), token("digits", "Number", "007"),
                     token("close", "RParen", ")")])
        f.add(23, "not", "close", a=6, b=f.labels["not"], d=4)
        f.add(22, "negate", "close", a=7, b=f.labels["negate"], d=3)
        f.add(21, "open", "close", a=8, d=2)
        f.add(15, "sign", "digits", a=f.labels["digits"], b=1, d=1)
        f.select(5)
        got = f.decode()["ast"]["expressions"]
        expected = [
            {"id": 0, "kind": "Number", "digits": f.at("digits"), "negative": True,
             "span": observation.span(f.at("sign")["start"], f.at("digits")["end"])},
            {"id": 1, "kind": "Group", "operand": 0,
             "span": observation.span(f.at("open")["start"], f.at("close")["end"])},
            {"id": 2, "kind": "Negate", "operand": 1, "operator_span": f.at("negate"),
             "span": observation.span(f.at("negate")["start"], f.at("close")["end"])},
            {"id": 3, "kind": "Not", "operand": 2, "operator_span": f.at("not"),
             "span": observation.span(f.at("not")["start"], f.at("close")["end"])},
        ]
        self.assertEqual(got, expected)
        for row in (4, 5, 6):
            for field, value in [("c", 1), ("next", 8), ("d", 0), ("d", 64),
                                 ("a", row + 1), ("a", 128), ("start", 0), ("end", 0)]:
                with self.subTest(row=row, field=field, value=value):
                    bad = copy.deepcopy(f)
                    bad.rows[row][field] = value
                    self.reject(bad)
        for row in (4, 5, 6):
            bad = copy.deepcopy(f)
            bad.rows[row]["b"] = f.labels["digits"]
            self.reject(bad)

    def test_negate_positive_number_is_not_a_canonical_signed_literal(self):
        f = Fixture([token("minus", "Minus", "-"), token("digit", "Number", "1")])
        f.add(22, "minus", "digit", a=6, b=f.labels["minus"], d=2)
        f.add(15, "digit", "digit", a=f.labels["digit"], d=1)
        self.reject(f.select(5), "signed Number")

    def test_negate_negative_number_is_admitted(self):
        f = Fixture([token("negate", "Minus", "-"), token("sign", "Minus", "-"),
                     token("digits", "Number", "1")])
        f.add(22, "negate", "digits", a=6, b=f.labels["negate"], d=2)
        f.add(15, "sign", "digits", a=f.labels["digits"], b=1, d=1)
        result = f.select(5).decode()["ast"]["expressions"]
        self.assertTrue(result[0]["negative"])
        self.assertEqual(result[1]["kind"], "Negate")

    def test_unparenthesized_precedence_associativity_and_comparison_chains(self):
        cases = [
            (26, "Star", "*", 24, "Plus", "+", True),
            (24, "Plus", "+", 36, "OrOr", "||", False),
            (25, "Minus", "-", 25, "Minus", "-", False),
            (31, "Less", "<", 31, "Less", "<", True),
            (31, "Less", "<", 31, "Less", "<", False),
            (35, "AndAnd", "&&", 35, "AndAnd", "&&", False),
        ]
        for outer, outer_token, outer_spelling, inner, inner_token, inner_spelling, left_inner in cases:
            with self.subTest(outer=outer, inner=inner, left_inner=left_inner):
                ops = [(inner_token, inner_spelling), (outer_token, outer_spelling)] if left_inner else [
                    (outer_token, outer_spelling), (inner_token, inner_spelling)]
                f = Fixture([token("a", "Ident", "a"), token("op1", *ops[0]),
                             token("b", "Ident", "b"), token("op2", *ops[1]), token("c", "Ident", "c")])
                a, b, c = f.name("a"), f.name("b"), f.name("c")
                child = f.add(inner, "a" if left_inner else "b", "b" if left_inner else "c",
                              a=a if left_inner else b, b=b if left_inner else c,
                              c=f.labels["op1" if left_inner else "op2"], d=2)
                root = f.add(outer, "a", "c", a=child if left_inner else a,
                             b=c if left_inner else child, c=f.labels["op2" if left_inner else "op1"], d=3)
                self.reject(f.select(root), "precedence or associativity")

    def test_parenthesized_comparison_and_left_associativity(self):
        f = Fixture([token("open", "LParen", "("), token("a", "Ident", "a"),
                     token("lt", "Less", "<"), token("b", "Ident", "b"),
                     token("close", "RParen", ")"), token("eq", "EqualEqual", "=="),
                     token("c", "Ident", "c")])
        a, b, c = f.name("a"), f.name("b"), f.name("c")
        compare = f.add(31, "a", "b", a=a, b=b, c=f.labels["lt"], d=2)
        group = f.add(21, "open", "close", a=compare, d=3)
        root = f.add(29, "open", "c", a=group, b=c, c=f.labels["eq"], d=4)
        self.assertEqual([e["kind"] for e in f.select(root).decode()["ast"]["expressions"]],
                         ["Name", "Name", "Comparison", "Group", "Name", "Comparison"])
        f = Fixture([token("a", "Ident", "a"), token("op1", "Minus", "-"),
                     token("b", "Ident", "b"), token("op2", "Minus", "-"), token("c", "Ident", "c")])
        a, b, c = f.name("a"), f.name("b"), f.name("c")
        left = f.add(25, "a", "b", a=a, b=b, c=f.labels["op1"], d=2)
        root = f.add(25, "a", "c", a=left, b=c, c=f.labels["op2"], d=3)
        result = f.select(root).decode()["ast"]["expressions"]
        self.assertEqual((result[-1]["left"], result[-1]["right"]), (2, 3))

    def test_unary_operand_cannot_implicitly_group_a_binary(self):
        f = Fixture([token("not", "Not", "!"), token("a", "Ident", "a"),
                     token("op", "Plus", "+"), token("b", "Ident", "b")])
        a, b = f.name("a"), f.name("b")
        binary = f.add(24, "a", "b", a=a, b=b, c=f.labels["op"], d=2)
        root = f.add(23, "not", "b", a=binary, b=f.labels["not"], d=3)
        self.reject(f.select(root), "precedence or associativity")

    def test_height_64_boundary_is_recomputed(self):
        for count in (63, 64):
            f = Fixture([token(str(i), "Not", "!") for i in range(count)] + [token("a", "Ident", "a")])
            root = f.name("a")
            for i in reversed(range(count)):
                root = f.add(23, str(i), "a", a=root, b=f.labels[str(i)], d=count - i + 1)
            f.select(root)
            if count == 63:
                self.assertEqual(len(f.decode()["ast"]["expressions"]), 64)
            else:
                self.reject(f, "computed expression height")

    def test_missing_group_close_cannot_be_silently_omitted(self):
        f = Fixture([token("open", "LParen", "("), token("a", "Ident", "a")])
        child = f.name("a")
        root = f.add(21, "open", "a", a=child, d=2)
        self.reject(f.select(root), "expected RParen")

    def test_call_zero_arguments_has_exact_fields(self):
        f = Fixture([token("callee", "Ident", "g"), token("open", "LParen", "("),
                     token("close", "RParen", ")")])
        root = f.add(20, "callee", "close", a=f.labels["callee"], d=1)
        f.select(root)
        self.assertEqual(f.decode()["ast"]["expressions"], [
            {"id": 0, "kind": "Call", "callee": f.at("callee"), "args": [],
             "span": observation.span(f.at("callee")["start"], f.at("close")["end"])},
        ])
        for field, value in [("a", f.labels["function"]), ("b", root), ("c", 1),
                             ("d", 0), ("d", 2), ("next", root),
                             ("start", f.at("open")["start"]), ("end", f.at("open")["end"])]:
            with self.subTest(field=field, value=value):
                bad = copy.deepcopy(f)
                bad.rows[root - 1][field] = value
                self.reject(bad)

    def test_nested_calls_and_binary_arguments_have_source_order_postorder_ids(self):
        f = Fixture([token("g", "Ident", "g"), token("g_open", "LParen", "("),
                     token("a", "Ident", "a"), token("comma1", "Comma", ","),
                     token("h", "Ident", "h"), token("h_open", "LParen", "("),
                     token("b", "Ident", "b"), token("h_close", "RParen", ")"),
                     token("comma2", "Comma", ","), token("c", "Ident", "c"),
                     token("plus", "Plus", "+"), token("d", "Ident", "d"),
                     token("g_close", "RParen", ")")])
        f.add(20, "g", "g_close", a=f.labels["g"], b=6, d=3)
        f.add(19, "a", "a", a=f.labels["a"], d=1, next=7)
        f.add(20, "h", "h_close", a=f.labels["h"], b=8, d=2, next=9)
        f.name("b")
        f.add(24, "c", "d", a=10, b=11, c=f.labels["plus"], d=2)
        f.name("c")
        f.name("d")
        f.select(5)
        self.assertEqual(f.decode()["ast"]["expressions"], [
            {"id": 0, "kind": "Name", "name": f.at("a"), "span": f.at("a")},
            {"id": 1, "kind": "Name", "name": f.at("b"), "span": f.at("b")},
            {"id": 2, "kind": "Call", "callee": f.at("h"), "args": [1],
             "span": observation.span(f.at("h")["start"], f.at("h_close")["end"])},
            {"id": 3, "kind": "Name", "name": f.at("c"), "span": f.at("c")},
            {"id": 4, "kind": "Name", "name": f.at("d"), "span": f.at("d")},
            {"id": 5, "kind": "Arithmetic", "op": "Add", "left": 3, "right": 4,
             "operator_span": f.at("plus"),
             "span": observation.span(f.at("c")["start"], f.at("d")["end"])},
            {"id": 6, "kind": "Call", "callee": f.at("g"), "args": [0, 2, 5],
             "span": observation.span(f.at("g")["start"], f.at("g_close")["end"])},
        ])
        for row, field, value in [(4, "b", 7), (4, "b", 8), (4, "d", 2),
                                  (6, "c", 8), (6, "d", 1), (5, "next", 0),
                                  (5, "next", 6), (6, "next", 6), (7, "next", 9),
                                  (8, "next", 6), (9, "next", 11), (4, "a", f.labels["h"])]:
            with self.subTest(row=row, field=field, value=value):
                bad = copy.deepcopy(f)
                bad.rows[row][field] = value
                self.reject(bad)

    def test_group_and_prefix_argument_roots_own_their_next_links(self):
        f = Fixture([token("g", "Ident", "g"), token("g_open", "LParen", "("),
                     token("open", "LParen", "("), token("a", "Ident", "a"),
                     token("close", "RParen", ")"), token("comma1", "Comma", ","),
                     token("minus", "Minus", "-"), token("b", "Ident", "b"),
                     token("comma2", "Comma", ","), token("not", "Not", "!"),
                     token("c", "Ident", "c"), token("g_close", "RParen", ")")])
        f.add(20, "g", "g_close", a=f.labels["g"], b=6, d=3)
        f.add(21, "open", "close", a=9, d=2, next=7)
        f.add(22, "minus", "b", a=10, b=f.labels["minus"], d=2, next=8)
        f.add(23, "not", "c", a=11, b=f.labels["not"], d=2)
        f.name("a")
        f.name("b")
        f.name("c")
        f.select(5)
        exprs = f.decode()["ast"]["expressions"]
        self.assertEqual(exprs[-1]["args"], [1, 3, 5])
        self.assertEqual([e["kind"] for e in exprs], ["Name", "Group", "Name", "Negate", "Name", "Not", "Call"])
        for row in (8, 9, 10):
            bad = copy.deepcopy(f)
            bad.rows[row]["next"] = 6
            self.reject(bad, "non-list row retained a next link")

    def test_call_delimiters_and_argument_lists_cannot_omit_tokens(self):
        for spelling in (",", "a,", "a", "a b"):
            with self.subTest(spelling=spelling):
                pieces = [token("g", "Ident", "g"), token("open", "LParen", "(")]
                if spelling.startswith("a"):
                    pieces.append(token("a", "Ident", "a"))
                if spelling.endswith(","):
                    pieces.append(token("comma", "Comma", ","))
                if spelling == "a b":
                    pieces += [token("space_args", "Trivia", " "), token("b", "Ident", "b")]
                if spelling != "a":
                    pieces.append(token("close", "RParen", ")"))
                f = Fixture(pieces)
                child = f.name("a") if spelling.startswith("a") else 0
                if spelling == "a b":
                    f.rows[child - 1]["next"] = f.name("b")
                root = f.add(20, "g", "a" if spelling == "a" else "close",
                             a=f.labels["g"], b=child, d=2 if child else 1)
                self.reject(f.select(root))

    def test_call_cannot_share_a_previous_argument_or_omit_callee_row_ownership(self):
        f = Fixture([token("g", "Ident", "g"), token("open", "LParen", "("),
                     token("a1", "Ident", "a"), token("comma", "Comma", ","),
                     token("a2", "Ident", "a"), token("close", "RParen", ")")])
        argument = f.name("a1")
        f.rows[argument - 1]["next"] = argument
        root = f.add(20, "g", "close", a=f.labels["g"], b=argument, d=2)
        self.reject(f.select(root), "cyclic or shared")
        f = Fixture([token("g", "Ident", "g"), token("open", "LParen", "("), token("close", "RParen", ")")])
        root = f.add(20, "g", "close", a=f.labels["g"], d=1)
        f.name("g")  # A Call must not also allocate an unused Name for its callee.
        self.reject(f.select(root), "orphan active rows")

    def test_call_height_includes_maximum_argument_and_empty_nested_call(self):
        for count in (62, 63):
            pieces = [token("g", "Ident", "g"), token("g_open", "LParen", "(")]
            pieces += [token(str(i), "Not", "!") for i in range(count)]
            pieces += [token("h", "Ident", "h"), token("h_open", "LParen", "("),
                       token("h_close", "RParen", ")"), token("g_close", "RParen", ")")]
            f = Fixture(pieces)
            child = f.add(20, "h", "h_close", a=f.labels["h"], d=1)
            for i in reversed(range(count)):
                child = f.add(23, str(i), "h_close", a=child, b=f.labels[str(i)], d=count - i + 1)
            root = f.add(20, "g", "g_close", a=f.labels["g"], b=child, d=count + 2)
            f.select(root)
            if count == 62:
                self.assertEqual(len(f.decode()["ast"]["expressions"]), 64)
            else:
                self.reject(f, "computed expression height")

    def test_let_mutability_optional_types_and_complete_statement_fields(self):
        for mutable in (False, True):
            for annotation in (None, "wat", "()"):
                with self.subTest(mutable=mutable, annotation=annotation):
                    pieces = [token("let", "Let", "let"), token("let_space", "Trivia", " ")]
                    if mutable:
                        pieces += [token("mut", "Mut", "mut"), token("mut_space", "Trivia", " ")]
                    pieces.append(token("binding", "Ident", "x"))
                    if annotation:
                        pieces.append(token("colon", "Colon", ":"))
                        pieces += ([token("annotation", "Ident", "wat")] if annotation == "wat" else [
                            token("annotation", "LParen", "("), token("annotation_end", "RParen", ")")])
                    pieces += [token("equal", "Equal", "="), token("number", "Number", "1"), token("semi", "Semi", ";")]
                    f = Fixture(pieces, statements=True)
                    ty, expected_type = 0, None
                    if annotation == "wat":
                        ty = f.add(3, "annotation", "annotation", a=f.labels["annotation"])
                        expected_type = {"kind": "TypeName", "name": f.at("annotation"), "span": f.at("annotation")}
                    elif annotation:
                        ty = f.add(4, "annotation", "annotation_end")
                        expected_type = {"kind": "TypeUnit", "span": observation.span(
                            f.at("annotation")["start"], f.at("annotation_end")["end"])}
                    value = f.add(15, "number", "number", a=f.labels["number"], d=1)
                    f.rows[3].update(kind=7 if mutable else 6, a=f.labels["binding"], b=ty, c=value)
                    expected = {"kind": "Let", "mutable": mutable, "name": f.at("binding"),
                                "annotation": expected_type, "init": 0,
                                "span": observation.span(f.at("let")["start"], f.at("semi")["end"])}
                    self.assertEqual(f.decode()["ast"]["functions"][0]["blocks"][0]["body"], [expected])
                    for field, bad_value in [("a", f.labels["function"]), ("b", 4), ("c", 0),
                                             ("c", 4), ("d", 1), ("next", 4),
                                             ("kind", 6 if mutable else 7),
                                             ("start", f.at("binding")["start"]),
                                             ("end", f.at("number")["end"])]:
                        bad = copy.deepcopy(f)
                        bad.rows[3][field] = bad_value
                        self.reject(bad)
                    if ty:
                        for field, bad_value in [("next", 4), ("b", 1), ("d", 1), ("start", 0)]:
                            bad = copy.deepcopy(f)
                            bad.rows[ty - 1][field] = bad_value
                            self.reject(bad)
                        bad = copy.deepcopy(f)
                        bad.rows[3]["b"] = 0
                        self.reject(bad)

    def test_assignment_exact_target_operator_value_and_no_semantic_resolution(self):
        f = Fixture([token("target", "Ident", "unknown"), token("space_before", "Trivia", " /*x*/ "),
                     token("equal", "Equal", "="), token("space_after", "Trivia", " "),
                     token("value", "True", "true"), token("semi", "Semi", ";")], statements=True)
        value = f.add(17, "value", "value", d=1)
        f.rows[3].update(kind=8, a=f.labels["target"], b=f.labels["equal"], c=value)
        got = f.decode()["ast"]
        self.assertEqual(got["functions"][0]["blocks"][0]["body"], [
            {"kind": "Assign", "name": f.at("target"), "operator_span": f.at("equal"), "value": 0,
             "span": observation.span(f.at("target")["start"], f.at("semi")["end"])},
        ])
        self.assertEqual(got["expressions"], [{"id": 0, "kind": "Bool", "value": True, "span": f.at("value")}])
        for field, bad_value in [("a", 0), ("a", f.labels["function"]), ("b", 0),
                                 ("b", f.labels["target"]), ("c", 0), ("c", 4),
                                 ("d", 1), ("next", 4), ("start", f.at("equal")["start"]),
                                 ("end", f.at("equal")["end"])]:
            with self.subTest(field=field, value=bad_value):
                bad = copy.deepcopy(f)
                bad.rows[3][field] = bad_value
                self.reject(bad)
        bad = copy.deepcopy(f)
        bad.name("target")
        self.reject(bad, "orphan active rows")

    def test_bindings_and_assignments_keep_global_expression_postorder_across_statements(self):
        f = Fixture([token("let", "Let", "let"), token("space", "Trivia", " "),
                     token("binding", "Ident", "x"), token("let_equal", "Equal", "="),
                     token("a", "Ident", "a"), token("plus", "Plus", "+"), token("two", "Number", "2"),
                     token("let_semi", "Semi", ";"), token("target", "Ident", "x"),
                     token("assign_equal", "Equal", "="), token("g", "Ident", "g"),
                     token("open", "LParen", "("), token("argument", "Ident", "x"),
                     token("close", "RParen", ")"), token("assign_semi", "Semi", ";"),
                     token("return", "Return", "return"), token("return_space", "Trivia", " "),
                     token("returned", "Ident", "x"), token("return_semi", "Semi", ";")], statements=True)
        a = f.name("a")
        number = f.add(15, "two", "two", a=f.labels["two"], d=1)
        binary = f.add(24, "a", "two", a=a, b=number, c=f.labels["plus"], d=2)
        assign = f.add(8, "target", "assign_semi", a=f.labels["target"], b=f.labels["assign_equal"], c=9, next=11)
        f.add(20, "g", "close", a=f.labels["g"], b=10, d=2)
        f.name("argument")
        f.add(10, "return", "return_semi", a=12)
        f.name("returned")
        f.rows[3].update(kind=6, a=f.labels["binding"], c=binary, end=f.at("let_semi")["end"], next=assign)
        got = f.decode()["ast"]
        self.assertEqual([e["kind"] for e in got["expressions"]], ["Name", "Number", "Arithmetic", "Name", "Call", "Name"])
        self.assertEqual(got["expressions"][4]["args"], [3])
        statements = got["functions"][0]["blocks"][0]["body"]
        self.assertEqual([(s["kind"], s.get("init", s.get("value"))) for s in statements],
                         [("Let", 2), ("Assign", 4), ("Return", 5)])
        for row, field, bad_value in [(3, "next", 0), (3, "next", 11), (7, "c", binary),
                                      (7, "next", 4), (10, "a", binary), (6, "next", assign)]:
            bad = copy.deepcopy(f)
            bad.rows[row][field] = bad_value
            self.reject(bad)

    def test_binding_delimiters_and_initializer_cannot_be_omitted(self):
        for missing in ("equal", "semi", "number"):
            pieces = [token("let", "Let", "let"), token("space", "Trivia", " "), token("binding", "Ident", "x")]
            pieces += [piece for piece in [token("equal", "Equal", "="), token("number", "Number", "1"),
                                           token("semi", "Semi", ";")] if piece[0] != missing]
            f = Fixture(pieces, statements=True)
            value = f.add(15, "number", "number", a=f.labels["number"], d=1) if missing != "number" else 0
            f.rows[3].update(kind=6, a=f.labels["binding"], c=value)
            self.reject(f)

    def test_expression_diagnostics_preserve_complete_first_diagnostic(self):
        for detail, source, start, end, message in [
            (22, b"fn f()->(){(a;}", 13, 14, "grouping requires `)`"),
            (23, b"fn f()->(){a<b<c;}", 14, 15,
             "comparison operators cannot be chained; use parentheses"),
            (24, b"fn f()->(){g(a;}", 14, 15, "call requires `)`"),
            (25, b"fn f()->(){let =1;}", 15, 16, "expected binding name"),
            (26, b"fn f()->(){let x;}", 16, 17, "binding requires an initializer"),
        ]:
            with self.subTest(detail=detail):
                raw = b"OPA1" + bytes([1, detail, start, end, 0, 0, len(source)])
                got = observation.decode(raw, source)
                self.assertEqual(got, {
                    "status": "diagnostic", "projection": "first_parser_diagnostic",
                    "diagnostic": {"schema_version": 1, "edition": "typed-preview", "kind": "diagnostic",
                                   "severity": "error", "code": "E0100", "stage": "parse", "message": message,
                                   "primary": {"file_id": 0, "start": start, "end": end, "path": "stdin.ox",
                                               "line": 1, "column": start + 1, "end_line": 1, "end_column": end + 1},
                                   "secondary": [], "notes": []},
                })


if __name__ == "__main__":
    unittest.main()
