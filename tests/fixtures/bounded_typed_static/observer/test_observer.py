#!/usr/bin/env python3
"""Focused fact/schema and public diagnostic parity checks, not candidate qualification."""
import argparse
import hashlib
import json
from pathlib import Path
import sys
import unittest

from observe import observe, public_check, run


OPTIONS = None
RECORDS = []


class ObserverTests(unittest.TestCase):
    def observe(self, source):
        data = source.encode("ascii")
        self.assertLessEqual(len(data), 128)
        evidence = OPTIONS.evidence / f"case-{len(RECORDS):03d}"
        RECORDS.append({"source": source, "evidence": str(evidence)})
        value = observe(OPTIONS.observer, OPTIONS.canonical, data, evidence)
        RECORDS[-1]["status"] = value["status"]
        self.assertEqual(value["schema"], "canonical-static-observation-1")
        if value["status"] in ("ok", "diagnostic") and value.get("phase") not in ("parse", "lex"):
            if value.get("route") == "public_owned":
                records = [json.loads(line) for line in (evidence / "public.stdout").read_text().splitlines()]
                diagnostics = records[:-1]
            else:
                diagnostics, _ = public_check(OPTIONS.canonical, evidence)
            if value["status"] == "diagnostic":
                self.assertTrue(diagnostics)
                self.assertEqual(value["diagnostic"], diagnostics[0])
                self.assertNotIn("typed_hir", value)
            else:
                self.assertEqual(diagnostics, [])
                for function in value["typed_hir"]["functions"]:
                    for block in function["blocks"]:
                        flow = block["flow"]
                        expected = "FlowSummary { fallthrough: %s, returns: %s, breaks: %s, continues: %s }" % tuple(
                            str(flow[name]).lower() for name in ("fallthrough", "returns", "breaks", "continues"))
                        self.assertEqual(flow["debug"], expected)
                        self.assertEqual(flow["mask"], sum(bit for name, bit in (
                            ("fallthrough", 1), ("returns", 2), ("breaks", 4), ("continues", 8)) if flow[name]))
        return value

    def success(self, source):
        value = self.observe(source)
        self.assertEqual(value["status"], "ok", value)
        return value["typed_hir"]["functions"]

    def test_library_semantics_and_namespace_restart(self):
        self.assertEqual(self.success(""), [])
        self.assertEqual(self.success(" " * 128), [])
        functions = self.success("fn f(x:i32)->i32{return g(x);}fn g(y:i32)->i32{return f(y);}")
        self.assertEqual([f["id"] for f in functions], [0, 1])
        for index, function in enumerate(functions):
            self.assertEqual([local["id"] for local in function["locals"]], [0])
            self.assertEqual([expr["id"] for expr in function["expressions"]], [0, 1])
            self.assertEqual(function["expressions"][0]["local"], 0)
            self.assertEqual(function["expressions"][1]["target"], 1 - index)
            self.assertEqual(function["blocks"][function["body"]]["flow"]["mask"], 2)
        self.success("fn main(x:i32)->i32{return main(x);}")
        self.success("fn f()->i32{return 1/0;}")
        self.success("fn f()->i32{return 2147483647+1;}")

    def test_public_function_route_parity(self):
        for source in ["pub fn f()->i32{return 1/0;}", "pub fn main(x:i32)->i32{return main(x);}",
                       "pub fn f(x:bool)->(){while x{if x{break;}else{continue;}}return;}"]:
            value = self.observe(source)
            self.assertEqual(value["status"], "ok")
            self.assertEqual(value["route"], "project_scalar")
        for source, code in [("pub fn f()->i32{return true;}", "E0300"),
                             ("pub fn f()->(){missing;return;}", "E0200"),
                             ("pub fn f()->(){return;}fn f()->(){return;}", "E0201")]:
            value = self.observe(source)
            self.assertEqual(value["status"], "diagnostic")
            self.assertEqual(value["diagnostic"]["code"], code)

    def test_project_file_identity_and_source_bytes(self):
        source = b"pub fn f()->(){return;}"
        for name, content in [("missing", None), ("different", b"pub fn g()->(){return;}")]:
            evidence = OPTIONS.evidence / ("project-file-" + name)
            evidence.mkdir()
            if content is not None:
                (evidence / "stdin.ox").write_bytes(content)
            result = run([str(OPTIONS.observer), "--project-source"], evidence, "observer", source)
            self.assertEqual(result.returncode, 70)
            self.assertEqual(result.stdout, b"")

    def test_local_declarations_literals_types_and_references(self):
        function = self.success("fn f(x:i32)->i32{let mut y:i32=-2147483648;y=x;return y;}")[0]
        self.assertEqual(function["name"], "f")
        self.assertEqual(function["signature"]["params"], ["i32"])
        self.assertEqual(function["signature"]["result"], "i32")
        self.assertEqual([(x["id"], x["name"], x["mutable"], x["annotation"], x["ty"])
                          for x in function["locals"]], [(0, "x", False, "i32", "i32"), (1, "y", True, "i32", "i32")])
        self.assertEqual(function["expressions"][0]["value"], -2147483648)
        self.assertEqual([x["local"] for x in function["expressions"][1:]], [0, 1])
        statements = function["blocks"][0]["body"]
        self.assertEqual([(x["kind"], x.get("local")) for x in statements], [("Let", 1), ("Assign", 1), ("Return", None)])
        for literal, value in [("0000000000000", 0), ("-0000000", 0), ("2147483647", 2147483647)]:
            self.assertEqual(self.success(f"fn f()->i32{{return {literal};}}")[0]["expressions"][0]["value"], value)
        unit = self.success("fn f(u:())->(){u;();return;}")[0]
        self.assertEqual([x["ty"] for x in unit["expressions"]], ["()", "()"])
        siblings = self.success("fn f(x:bool)->(){if x{let y=0;}else{let y=true;}return;}")[0]
        self.assertEqual([(x["name"], x["ty"]) for x in siblings["locals"]], [("x", "bool"), ("y", "i32"), ("y", "bool")])

    def test_actual_flow_flags_and_nearest_loop_targets(self):
        function = self.success("fn f(a:bool)->(){while a{if a{return;}if a{break;}if a{continue;}}return;}")[0]
        self.assertEqual(function["blocks"][0]["flow"]["mask"], 2)
        self.assertEqual(function["blocks"][1]["flow"]["mask"], 15)
        self.assertEqual([b["flow"]["mask"] for b in function["blocks"][2:]], [2, 4, 8])
        function = self.success("fn f(x:bool)->(){while x{if x{break;}else{continue;}}return;}")[0]
        self.assertEqual(function["blocks"][1]["flow"]["mask"], 12)
        self.assertEqual(function["blocks"][2]["body"][0]["target"], 1)
        self.assertEqual(function["blocks"][3]["body"][0]["target"], 1)
        function = self.success("fn f()->(){while true{while false{continue;}break;}return;}")[0]
        self.assertEqual(function["blocks"][1]["body"][1]["target"], 1)
        self.assertEqual(function["blocks"][2]["body"][0]["target"], 2)

    def test_expression_variant_surface(self):
        function = self.success("fn f(a:i32,b:bool)->bool{return (!(a<0)&&b)||(a==1);}")[0]
        kinds = {x["kind"] for x in function["expressions"]}
        self.assertTrue({"Local", "I32", "Comparison", "Group", "Not", "Logical"} <= kinds)
        self.assertEqual(function["expressions"][-1]["ty"], "bool")
        function = self.success("fn f(a:i32)->i32{return -(a+2*3-4/2%2);}")[0]
        self.assertEqual({x["op"] for x in function["expressions"] if x["kind"] == "Arithmetic"},
                         {"Add", "Multiply", "Subtract", "Divide", "Remainder"})
        self.assertEqual(function["expressions"][-1]["kind"], "Negate")

    def test_all_fourteen_diagnostic_templates_and_ordering(self):
        cases = [
            ("pub fn f(x:T)->(){return;}", "E0202", "resolve", "unknown type `T`"),
            ("fn f(x:i32)->(){let x=missing;return;}", "E0200", "resolve", "unknown local `missing`"),
            ("fn f()->(){absent(missing);return;}", "E0200", "resolve", "unknown direct function `absent`"),
            ("fn f(x:i32)->(){let x=0;return;}", "E0201", "resolve", "duplicate binding; shadowing is unavailable in typed-preview"),
            ("fn f()->(){let x:T=missing;return;}", "E0202", "resolve", "unknown type `T`"),
            ("fn f()->i32{return -2147483649;}", "E0203", "resolve", "decimal literal is outside the i32 range [-2147483648, 2147483647]"),
            ("fn f()->(){break;return;}", "E0204", "resolve", "`break` requires an enclosing while in the same function"),
            ("fn f()->(){continue;return;}", "E0204", "resolve", "`continue` requires an enclosing while in the same function"),
            ("fn f()->i32{return true;}", "E0300", "type", "type mismatch: expected i32, found bool"),
            ("fn f()->bool{return ()==();}", "E0300", "type", "equality requires i32 or bool operands, found ()"),
            ("fn f()->(){return;}fn g()->(){f(1);return;}", "E0301", "type", "wrong argument count: expected 0, found 1"),
            ("fn f()->(){}", "E0302", "type", "function requires an explicit terminal return"),
            ("fn f()->(){return;1;}", "E0303", "type", "statement after terminal return is unavailable in typed-preview"),
            ("fn f()->(){while true{break;1;}return;}", "E0303", "type", "statement after terminal control transfer is unavailable in typed-preview"),
            ("fn f()->(){let x=0;x=1;return;}", "E0304", "type", "assignment requires a mutable local"),
            ("fn a()->i32{return true;}fn b()->(){missing;return;}", "E0200", "resolve", "unknown local `missing`"),
            ("fn f()->(){absent=missing;return;}", "E0200", "resolve", "unknown local `absent`"),
            ("fn f()->(){return;missing;}", "E0200", "resolve", "unknown local `missing`"),
            ("fn f()->(){let x=0;x=true;return;}", "E0304", "type", "assignment requires a mutable local"),
        ]
        for source, code, stage, message in cases:
            with self.subTest(source=source):
                value = self.observe(source)
                self.assertEqual(value["status"], "diagnostic")
                diagnostic = value["diagnostic"]
                self.assertEqual((diagnostic["code"], diagnostic["stage"], diagnostic["message"]), (code, stage, message))
                self.assertEqual(diagnostic["notes"], [])

    def test_public_unknown_type_schedule_and_display_limit(self):
        cases = [
            ("fn f(x:T)->(){return;}fn f()->(){return;}", "E0201", "duplicate binding; shadowing is unavailable in typed-preview"),
            ("fn f()->(){if true{let x:A=0;}let y:B=0;return;}", "E0202", "unknown type `B`"),
            ("fn f(x:T)->U{return;}", "E0202", "unknown type `T`"),
        ]
        for source, code, message in cases:
            value = self.observe(source)
            self.assertEqual(value["route"], "public_owned")
            self.assertEqual((value["diagnostic"]["code"], value["diagnostic"]["message"]), (code, message))
        for length in (64, 65):
            value = self.observe("fn f(x:" + "A" * length + ")->(){return;}")
            self.assertEqual(value["route"], "public_owned")
            self.assertEqual(value["diagnostic"]["code"], "E0202")
            shown = "A" * 64 if length == 64 else "A" * 61 + "..."
            self.assertEqual(value["diagnostic"]["message"], f"unknown type `{shown}`")
            self.assertEqual(value["diagnostic"]["primary"]["end"] - value["diagnostic"]["primary"]["start"], length)

    def test_parse_lexical_and_outside_domain_are_distinct(self):
        self.assertEqual(self.observe("/*")["status"], "lexical_diagnostic")
        self.assertEqual(self.observe("@")["phase"], "parse")
        self.assertEqual(self.observe("fn f(")["phase"], "parse")
        self.assertEqual(self.observe("struct S{x:i32}")["status"], "outside_subset")
        for index, data in enumerate((b" " * 129, "é".encode())):
            evidence = OPTIONS.evidence / f"transport-{index}"
            evidence.mkdir()
            result = run([str(OPTIONS.observer)], evidence, "observer", data)
            self.assertEqual(result.returncode, 2)
            self.assertEqual(result.stdout, b"")


def main():
    global OPTIONS
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--observer", type=Path, required=True)
    parser.add_argument("--canonical", type=Path, required=True)
    parser.add_argument("--evidence", type=Path, required=True)
    OPTIONS = parser.parse_args()
    OPTIONS.observer = OPTIONS.observer.resolve()
    OPTIONS.canonical = OPTIONS.canonical.resolve()
    OPTIONS.evidence = OPTIONS.evidence.resolve()
    OPTIONS.evidence.mkdir(parents=True, exist_ok=False)
    result = unittest.TextTestRunner(verbosity=2).run(unittest.defaultTestLoader.loadTestsFromTestCase(ObserverTests))
    (OPTIONS.evidence / "summary.json").write_text(json.dumps({
        "success": result.wasSuccessful(), "tests": result.testsRun,
        "failures": len(result.failures), "errors": len(result.errors),
        "observer_sha256": hashlib.sha256(OPTIONS.observer.read_bytes()).hexdigest(),
        "canonical_sha256": hashlib.sha256(OPTIONS.canonical.read_bytes()).hexdigest(),
        "cases": RECORDS,
    }, indent=2) + "\n")
    return 0 if result.wasSuccessful() else 1


if __name__ == "__main__":
    sys.exit(main())
