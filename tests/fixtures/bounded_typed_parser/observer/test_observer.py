#!/usr/bin/env python3
"""Focused tests for the canonical observer's projection, not a new parser."""
import argparse
import hashlib
import json
from pathlib import Path
import subprocess
import sys
import unittest


OBSERVER = None
OBSERVATIONS = []


def at(start, end):
    return {"file_id": 0, "start": start, "end": end}


class ObserverTests(unittest.TestCase):
    def observe(self, source):
        data = source.encode() if isinstance(source, str) else source
        result = subprocess.run([str(OBSERVER)], input=data, capture_output=True, timeout=10)
        record = {"input_hex": data.hex(), "exit_code": result.returncode,
                  "stdout": result.stdout.decode(), "stderr": result.stderr.decode()}
        OBSERVATIONS.append(record)
        self.assertEqual(result.returncode, 0, record)
        self.assertEqual(result.stderr, b"")
        value = json.loads(result.stdout)
        if value["status"] == "ok":
            tokens = value["ast"]["tokens"]
            self.assertEqual(tokens[-1], {"id": 47, "kind": "Eof", **at(len(data), len(data))})
            cursor = 0
            for token in tokens:
                self.assertEqual(token["file_id"], 0)
                self.assertEqual(token["start"], cursor)
                self.assertLessEqual(token["start"], token["end"])
                cursor = token["end"]
        return value

    def ast(self, source):
        value = self.observe(source)
        self.assertEqual(value["status"], "ok", value)
        return value["ast"]

    def test_empty_and_inclusive_byte_bound(self):
        self.assertEqual(self.ast(""), {"tokens": [{"id": 47, "kind": "Eof", **at(0, 0)}],
                                      "items": [], "functions": [], "expressions": []})
        self.assertEqual(self.ast(" " * 128)["functions"], [])

    def test_out_of_input_domain(self):
        for data in [b" " * 129, "é".encode(), b"\xff"]:
            result = subprocess.run([str(OBSERVER)], input=data, capture_output=True, timeout=10)
            OBSERVATIONS.append({"input_hex": data.hex(), "exit_code": result.returncode,
                                 "stdout": result.stdout.decode(), "stderr": result.stderr.decode()})
            self.assertEqual(result.returncode, 2)
            self.assertEqual(result.stdout, b"")
            self.assertIn(b"at most 128 bytes", result.stderr)

    def test_complete_scalar_expression_and_function_fields(self):
        ast = self.ast("pub fn one(a:i32,b:())->i32{return -(a+-007);}")
        self.assertEqual(ast["items"], [{"kind": "Function", "id": 0}])
        self.assertEqual(ast["functions"], [{
            "id": 0, "public": at(0, 3), "name": at(7, 10),
            "params": [{"name": at(11, 12), "ty": {"kind": "TypeName", "span": at(13, 16), "name": at(13, 16)}},
                       {"name": at(17, 18), "ty": {"kind": "TypeUnit", "span": at(19, 21)}}],
            "result": {"kind": "TypeName", "span": at(24, 27), "name": at(24, 27)},
            "body": 0, "end": at(45, 46),
            "blocks": [{"id": 0, "span": at(27, 46), "end": at(45, 46),
                        "body": [{"span": at(28, 45), "kind": "Return", "value": 4}]}],
        }])
        self.assertEqual(ast["expressions"], [
            {"id": 0, "span": at(37, 38), "kind": "Name", "name": at(37, 38)},
            {"id": 1, "span": at(39, 43), "kind": "Number", "digits": at(40, 43), "negative": True},
            {"id": 2, "span": at(37, 43), "kind": "Arithmetic", "op": "Add", "left": 0, "right": 1, "operator_span": at(38, 39)},
            {"id": 3, "span": at(36, 44), "kind": "Group", "operand": 2},
            {"id": 4, "span": at(35, 44), "kind": "Negate", "operand": 3, "operator_span": at(35, 36)},
        ])

    def test_binary_operator_names_edges_and_spans(self):
        cases = [("+", "Arithmetic", "Add"), ("-", "Arithmetic", "Subtract"),
                 ("*", "Arithmetic", "Multiply"), ("/", "Arithmetic", "Divide"),
                 ("%", "Arithmetic", "Remainder"), ("==", "Comparison", "Equal"),
                 ("!=", "Comparison", "NotEqual"), ("<", "Comparison", "Less"),
                 ("<=", "Comparison", "LessEqual"), (">", "Comparison", "Greater"),
                 (">=", "Comparison", "GreaterEqual"), ("&&", "Logical", "And"),
                 ("||", "Logical", "Or")]
        for operator, kind, name in cases:
            with self.subTest(operator=operator):
                source = "fn f()->(){a" + operator + "b;}"
                expression = self.ast(source)["expressions"][-1]
                start = source.index(operator, source.index("{"))
                self.assertEqual(expression, {"id": 2, "span": at(start - 1, start + len(operator) + 1),
                                              "kind": kind, "op": name, "left": 0, "right": 1,
                                              "operator_span": at(start, start + len(operator))})

    def test_block_order_and_statement_fields(self):
        source = "fn a()->(){let mut x:i32=2;x=x+1;if !false{f(x,());}else{return;}while true{break;continue;}}"
        ast = self.ast(source)
        fn = ast["functions"][0]
        blocks = fn["blocks"]
        self.assertEqual(fn["public"], None)
        self.assertEqual(fn["body"], 0)
        self.assertEqual([block["id"] for block in blocks], [0, 1, 2, 3])
        statements = blocks[0]["body"]
        self.assertEqual([s["kind"] for s in statements], ["Let", "Assign", "If", "While"])
        self.assertEqual(set(statements[0]), {"kind", "span", "mutable", "name", "annotation", "init"})
        self.assertTrue(statements[0]["mutable"])
        self.assertEqual(statements[0]["annotation"]["kind"], "TypeName")
        self.assertEqual(set(statements[1]), {"kind", "span", "name", "operator_span", "value"})
        self.assertEqual(statements[2]["then_block"], 1)
        self.assertEqual(statements[2]["else_block"], 2)
        self.assertEqual(statements[3]["body"], 3)
        self.assertEqual(blocks[2]["body"][0]["value"], None)
        self.assertEqual([s["kind"] for s in blocks[3]["body"]], ["Break", "Continue"])
        for block in blocks:
            block_text = source[block["span"]["start"]:block["span"]["end"]]
            self.assertEqual((block_text[0], block_text[-1]), ("{", "}"))
            self.assertEqual(source[block["end"]["start"]:block["end"]["end"]], "}")
        expressions = ast["expressions"]
        call = next(e for e in expressions if e["kind"] == "Call")
        self.assertEqual(set(call), {"id", "span", "kind", "callee", "args"})
        self.assertEqual([expressions[i]["kind"] for i in call["args"]], ["Name", "Unit"])
        negate = next(e for e in expressions if e["kind"] == "Not")
        self.assertEqual(set(negate), {"id", "span", "kind", "operand", "operator_span"})
        self.assertFalse(expressions[negate["operand"]]["value"])

    def test_missing_optional_fields_are_null(self):
        ast = self.ast("fn f()->(){let x=1;if true{return;}return x;}")
        statements = ast["functions"][0]["blocks"][0]["body"]
        self.assertFalse(statements[0]["mutable"])
        self.assertIsNone(statements[0]["annotation"])
        self.assertIsNone(statements[1]["else_block"])
        self.assertIsInstance(statements[2]["value"], int)

    def test_multiple_functions_global_expression_ids_and_unknown_types(self):
        ast = self.ast("fn a()->(){return 00;}fn b(x:wat)->wat{return x;}")
        self.assertEqual(ast["items"], [{"kind": "Function", "id": 0}, {"kind": "Function", "id": 1}])
        self.assertEqual([e["id"] for e in ast["expressions"]], [0, 1])
        self.assertEqual([f["blocks"][0]["body"][0]["value"] for f in ast["functions"]], [0, 1])
        self.assertEqual(ast["functions"][1]["result"]["kind"], "TypeName")
        self.assertFalse(ast["expressions"][0]["negative"])

    def test_decimal_spelling_is_not_converted(self):
        digits = "9" * 90
        ast = self.ast("fn f()->(){return " + digits + ";}")
        number = ast["expressions"][0]
        self.assertEqual(number, {"id": 0, "kind": "Number", "span": at(18, 108),
                                  "digits": at(18, 108), "negative": False})

    def test_first_diagnostic_is_complete_renderer_object(self):
        value = self.observe("fn a()->(){return 1} fn b()->(){return 2}")
        self.assertEqual(value, {"status": "diagnostic", "projection": "first_parser_diagnostic", "diagnostic": {
            "schema_version": 1, "edition": "typed-preview", "kind": "diagnostic", "severity": "error",
            "code": "E0100", "stage": "parse", "message": "statement requires `;`",
            "primary": {"file_id": 0, "path": "stdin.ox", "start": 19, "end": 20,
                        "line": 1, "column": 20, "end_line": 1, "end_column": 21},
            "secondary": [], "notes": [],
        }})

    def test_lexical_failure_precedes_parsing_and_remains_distinct(self):
        value = self.observe("garbage /*")
        self.assertEqual(set(value), {"status", "diagnostic"})
        self.assertEqual(value["status"], "lexical_diagnostic")
        self.assertEqual(value["diagnostic"]["stage"], "lex")
        self.assertEqual(value["diagnostic"]["message"], "unterminated block comment")

    def test_canonical_depth_and_unsupported_spelling_failures(self):
        value = self.observe("fn f()->(){return " + "!" * 64 + "[;}" )
        self.assertEqual(value["status"], "diagnostic")
        self.assertEqual(value["diagnostic"]["code"], "E0400")
        value = self.observe("for")
        self.assertEqual(value["diagnostic"]["code"], "E0101")

    def test_successful_outside_subset_is_not_a_partial_ast(self):
        cases = {
            "module": "mod foo;", "import": "use std::io::read_stdin;",
            "record": "struct A{x:i32}", "enum": "enum E{A}",
            "qualified_type": "fn f(x:crate::m::A)->(){}",
            "reference_type": "fn f(x:&A)->(){}",
            "array_type": "fn f()->[i32;2]{}",
            "slice_reference_type": "fn f(x:&[i32])->(){}",
            "array_reference_type": "fn f(x:&[i32;2])->(){}",
            "field_assignment": "fn f()->(){a.x=1;}",
            "index_assignment": "fn f()->(){a[0]=1;}",
            "match": "fn f()->(){match x{E::A=>{}}}",
            "qualified_value": "fn f()->(){crate::m::f();}",
            "borrow_argument": "fn f()->(){g(&x);}",
            "record_literal": "fn f()->(){A{x:1};}",
            "array_literal": "fn f()->(){[1];}",
            "field_access": "fn f()->(){a.x;}",
            "indexing": "fn f()->(){a[0];}",
            "array_length": "fn f()->(){a.len();}",
        }
        for family, source in cases.items():
            with self.subTest(family=family):
                value = self.observe(source)
                self.assertEqual(value["status"], "outside_subset", value)
                self.assertEqual(value["projection"], "successful_ast_outside_subset")
                self.assertEqual(value["family"], family)
                self.assertEqual(set(value), {"status", "projection", "family", "span"})
        self.assertEqual(self.observe("fn f()->(){E::A;}")["family"], "qualified_value")

    def test_malformed_excluded_syntax_remains_canonical_diagnostic(self):
        value = self.observe("struct {")
        self.assertEqual(value["status"], "diagnostic")
        self.assertEqual(value["projection"], "first_parser_diagnostic")
        self.assertNotIn("family", value)


def main():
    global OBSERVER
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--observer", type=Path, required=True)
    parser.add_argument("--report", type=Path)
    args = parser.parse_args()
    OBSERVER = args.observer.resolve(strict=True)
    result = unittest.TextTestRunner(verbosity=2).run(unittest.defaultTestLoader.loadTestsFromTestCase(ObserverTests))
    if args.report:
        args.report.parent.mkdir(parents=True, exist_ok=True)
        args.report.write_text(json.dumps({"observer": str(OBSERVER),
            "observer_sha256": hashlib.sha256(OBSERVER.read_bytes()).hexdigest(),
            "test_script_sha256": hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
            "tests_run": result.testsRun, "success": result.wasSuccessful(),
            "failures": [str(test) for test, _ in result.failures],
            "errors": [str(test) for test, _ in result.errors],
            "observations": OBSERVATIONS}, indent=2) + "\n")
    return 0 if result.wasSuccessful() else 1


if __name__ == "__main__":
    sys.exit(main())
