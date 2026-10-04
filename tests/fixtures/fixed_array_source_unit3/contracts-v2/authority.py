#!/usr/bin/env python3
"""Source-only fixture constructor. Never imports, builds, or runs Oxid.

Expected facts are literals and bounded construction facts, not a parser model.
Byte coordinates are computed from those source facts; compiler observations are
not inputs. Run `python3 authority.py generate OUT` or `python3 authority.py verify`.
"""
import argparse
import hashlib
import json
from pathlib import Path

HERE = Path(__file__).resolve().parent
STATUS = "FROZEN_EXPECTATION_NOT_EXECUTED"


def digest(b):
    return hashlib.sha256(b).hexdigest()


def encoded(obj):
    return (json.dumps(obj, ensure_ascii=False, indent=2, sort_keys=True) + "\n").encode()


def location(source, byte):
    prefix = source.encode()[:byte].decode()
    return [prefix.count("\n") + 1, len(prefix.rsplit("\n", 1)[-1]) + 1]


def span(source, needle, occurrence=0, *, file=0, start_in=0, width=None):
    """Find an explicit source anchor; no compiler or original offsets consulted."""
    if needle == "<EOF>":
        lo = hi = len(source.encode())
    else:
        raw, target = source.encode(), needle.encode()
        starts = []
        at = raw.find(target)
        while at >= 0:
            starts.append(at)
            at = raw.find(target, at + 1)
        if occurrence >= len(starts):
            raise ValueError((needle, occurrence, source))
        lo = starts[occurrence] + start_in
        hi = lo + (len(target) - start_in if width is None else width)
    text = source.encode()[lo:hi].decode()
    return dict(file=file, start=lo, end=hi, text=text,
                line=location(source, lo)[0], column=location(source, lo)[1],
                end_line=location(source, hi)[0], end_column=location(source, hi)[1])


def array(element, length):
    return dict(tag="fixed_array", element=element, length=length)


def case(case_id, source, family, expected, *, phase="3B", files=None):
    sources = files if files is not None else {"main.ox": source}
    return dict(id=case_id, family=family, first_observation_slice=phase,
                status=STATUS, files=[dict(file=i, path=p, bytes=len(s.encode()),
                sha256=digest(s.encode()), source=s) for i, (p, s) in enumerate(sources.items())],
                expected=expected)


def diag(case_id, source, code, stage, anchor, reason, *, conflicts=1, occurrence=0,
         start_in=0, width=None, secondary=None, phase=None):
    d = dict(code=code, stage=stage,
             primary=span(source, anchor, occurrence, start_in=start_in, width=width),
             secondary=[] if secondary is None else secondary)
    return case(case_id, source, "diagnostic", dict(diagnostics=[d], reason=reason,
                competing_invalid_conditions=conflicts), phase=phase or ("3A" if stage in ("parse", "lex") else "3B"))


def preserved():
    original = json.loads((HERE / "inputs/source-controls-v1.json").read_text())
    # Disambiguating anchors are semantic source snippets, independently selected.
    anchors = {
        "excluded-nested-array-type": ("[[i32", 1, 1),
        "excluded-grouped-base": ("(a)[", 3, 1),
        "excluded-repeated-index": ("a[0][", 4, 1),
        "excluded-arbitrary-method": ("a.other(", 7, 1),
        "excluded-length-argument": ("a.len(1", 6, 1),
        "missing-length-call-close": ("a.len(;", 6, 1),
        "excluded-reference-element": ("[&i32", 1, 1),
        "excluded-record-element": ("[R;", 1, 1),
        "excluded-array-record-field": ("[i32; 1]", 0, 1),
        "excluded-repeat-syntax": ("[1;2]", 2, 1),
        "excluded-named-array-length": ("; N]", 2, 1),
        "missing-array-index": ("a[]", 2, 1),
        "missing-array-type-separator": ("i32 2]", 4, 1),
        "missing-array-element-separator": ("[1 2]", 3, 1),
    }
    out = []
    for old in original["diagnostic_cases"]:
        src, p = old["source"], old["expected"]["primary"]
        assert digest(src.encode()) == old["sha256"], old["id"]
        anchor, offset, width = anchors.get(old["id"], (p["text"], 0, len(p["text"].encode())))
        computed = span(src, anchor, start_in=offset, width=width)
        assert {k: computed[k] for k in ("file", "start", "end", "text")} == p, old["id"]
        expected = dict(diagnostics=[dict(code=old["expected"]["code"],
                        stage=old["expected"]["stage"], primary=computed)], reason=old["reason"],
                        original_identity=dict(source_sha256=old["sha256"],
                        authority_sha256=digest((HERE / "inputs/source-controls-v1.json").read_bytes())),
                        original_fields="code, stage, primary only; wording/secondary appendix is separate")
        out.append(case(old["id"], src, "adopted-diagnostic", expected,
                        phase="3A" if old["expected"]["stage"] == "parse" else "3B"))
    for old in original["semantic_cases"]:
        assert digest(old["source"].encode()) == old["sha256"]
        out.append(case(old["id"], old["source"], "deferred-effects-counterexample",
                        dict(semantic_sequence=old["expected_semantic_sequence"],
                             fuel_note=old["fuel_note"], execution="NOT_RUN; full schedule belongs to Unit3C",
                             final_target=span(old["source"], "a[touch(&mut a)]"),
                             original_source_sha256=old["sha256"]), phase="3C"))
    return out


def new_diagnostics():
    rows = []
    def add(*args, **kwargs): rows.append(diag(*args, **kwargs))
    add("write-resolve-base-before-both-operands", "fn main() -> i32 { missing[unknown()] = absent(); return 0; }\n",
        "E0200", "resolve", "missing", "Unresolved target base wins before RHS or index resolution", conflicts=3)
    add("write-resolve-rhs-before-index", "fn main() -> i32 { let mut a = [1]; a[unknown()] = absent(); return 0; }\n",
        "E0200", "resolve", "absent", "Resolve RHS subtree before index subtree", conflicts=2)
    add("write-whole-resolution-before-rhs-typing", "fn main() -> i32 { let mut a = [1]; a[unknown()] = true + 1; return 0; }\n",
        "E0200", "resolve", "unknown", "All resolution finishes before any body typing; RHS-first is not interleaving", conflicts=2)
    add("write-both-subtree-errors-rhs-wins", "fn main() -> i32 { let s = 0; s[false + 2] = true + 1; return 0; }\n",
        "E0300", "type", "true", "RHS internal error beats index internal error, non-array base and immutability", conflicts=4)
    src = "fn first() -> i32 { let a = [1]; return a[true]; }\nfn main() -> i32 { return unknown(); }\n"
    add("whole-program-resolution-before-earlier-function-type", src, "E0200", "resolve", "unknown",
        "A later function resolution failure suppresses earlier function typing", conflicts=2)
    # Only one invalid semantic condition, separating precedence from error detection.
    add("write-one-conflict-element", "fn main() -> i32 { let mut a = [1]; a[0] = false; return 0; }\n",
        "E0300", "type", "false", "Only element identity is invalid; base, index and owner mutability are valid")
    add("write-one-conflict-index", "fn main() -> i32 { let mut a = [1]; a[true] = 2; return 0; }\n",
        "E0300", "type", "true", "Only index type is invalid")
    add("empty-scalar-context", "fn main() -> i32 { let a: i32 = ([]); return 0; }\n",
        "E0300", "type", "[]", "Scalar local annotation cannot seed empty arrays")
    add("empty-call-context-excluded", "fn take(a: [i32; 0]) -> i32 { return 0; }\nfn main() -> i32 { return take([]); }\n",
        "E0300", "type", "[]", "Callee parameter type does not flow into a call argument")
    add("empty-return-context-excluded", "fn empty() -> [i32; 0] { return []; }\nfn main() -> i32 { return 0; }\n",
        "E0300", "type", "[]", "Function result type does not seed an empty return literal")
    add("empty-reassignment-context-excluded", "fn main() -> i32 { let mut a: [i32; 0] = []; a = []; return 0; }\n",
        "E0300", "type", "[]", "Assignment target type does not seed empty RHS", occurrence=1)
    add("literal-first-heterogeneous-element", "fn main() -> i32 { let a = [1, true, ()]; return 0; }\n",
        "E0300", "type", "true", "Elements type/check left-to-right; first heterogeneity wins", conflicts=2)
    add("literal-nested-nonempty-is-nonscalar", "fn main() -> i32 { let a = [[1]]; return 0; }\n",
        "E0300", "type", "[1]", "Nested literal parses, but its owned-array element type is excluded")
    syntax = [
        ("excluded-called-base", "fn make() -> [i32; 1] { return [1]; }\nfn main() -> i32 { return make()[0]; }\n", "make()[", 6, 1),
        ("excluded-literal-base", "fn main() -> i32 { return [1][0]; }\n", "[1][", 3, 1),
        ("excluded-grouped-length-base", "fn main() -> i32 { let a = [1]; return (a).len(); }\n", "(a).", 3, 1),
        ("excluded-indexed-length-base", "fn main() -> i32 { let a = [1]; return a[0].len(); }\n", "a[0].", 4, 1),
        ("excluded-length-store-target", "fn main() -> i32 { let mut a = [1]; a.len() = 1; return 0; }\n", "a.len()", 0, 7),
        ("excluded-grouped-element-type", "fn main() -> i32 { let a: [(i32); 1] = [1]; return 0; }\n", "[(i32)", 1, 1),
        ("excluded-negative-length", "fn main() -> i32 { let a: [i32; -1] = [1]; return 0; }\n", "-1", 0, 1),
        ("excluded-suffixed-length", "fn main() -> i32 { let a: [i32; 1u8] = [1]; return 0; }\n", "1u8", 0, 3),
        ("excluded-separated-length", "fn main() -> i32 { let a: [i32; 1_0] = [1]; return 0; }\n", "1_0", 0, 3),
    ]
    for ident, src, anchor, offset, width in syntax:
        add(ident, src, "E0101", "parse", anchor, "First token proving excluded syntax, except complete invalid target",
            start_in=offset, width=width)
    src = "fn main() -> i32 { let a = [1]; return a.len("
    add("missing-length-close-at-eof", src, "E0100", "parse", "<EOF>", "Missing ')' reports zero-width EOF")
    return rows


def bounded_constructions():
    rows = []
    # Source type identity, syntax and literal widths. No implied native admission.
    for element, spelling, value in [("bool", "bool", "true"), ("i32", "i32", "7"), ("unit", "()", "()")]:
        for n in (0, 1):
            literal = "[]" if n == 0 else "[" + value + ",]"
            init = "((" + literal + "))" if n == 0 else literal
            src = f"fn main() -> i32 {{ let a: [{spelling}; {n}] = {init}; return a.len(); }}\n"
            expected = dict(parse="accept", selectors=dict(ast=[True], source_owner=True), route="owned-whole-program",
                            type_queries=[dict(origin=span(src, f"[{spelling}; {n}]"), value=array(element, n))],
                            ast=[dict(kind="ArrayLiteral", origin=span(src, literal), element_count=n,
                                      elements=[] if n == 0 else [span(src, literal, start_in=1, width=len(value.encode()))]),
                                 dict(kind="ArrayLength", origin=span(src, "a.len()"), base=span(src, "a.len()", width=1))],
                            inventory=dict(array_literal_elements=n, array_store_target_wrappers=0),
                            later_type_expectation=dict(binding=array(element, n), initializer=array(element, n),
                                                        return_expression="i32", empty_context="whole annotated local only"))
            if n == 0:
                expected["ast"].extend([dict(kind="Group", origin=span(src, "([])"), child=span(src, "[]")),
                                        dict(kind="Group", origin=span(src, "(([]))"), child=span(src, "([])"))])
            rows.append(case(f"scalar-{element}-length-{n}", src, "scalar-array-boundary", expected, phase="3A"))
    for n in (1024, 1025):
        src = "fn main() -> i32 { let a = [" + ",".join("0" for _ in range(n)) + ",]; return a.len(); }\n"
        if n == 1025:
            rows.append(diag("literal-length-one-over", src, "E0400", "parse", "0", "Reject first actual excess element before parsing/reserving it", occurrence=1024))
        else:
            literal = "[" + ",".join("0" for _ in range(n)) + ",]"
            rows.append(case("literal-length-max-trailing-comma", src, "scalar-array-boundary",
                dict(parse="accept", selectors=dict(ast=[True], source_owner=True), route="owned-whole-program",
                     ast=[dict(kind="ArrayLiteral", origin=span(src, literal), element_count=n,
                               elements=[span(src, "0", i) for i in range(n)]),
                          dict(kind="ArrayLength", origin=span(src, "a.len()"), base=span(src, "a.len()", width=1))],
                     inventory=dict(array_literal_elements=n, array_store_target_wrappers=0),
                     later_type_expectation=dict(binding=array("i32", n)),
                     downstream="No consumer run here. Straightforward lowering exceeds native 256 scalar slots; do not weaken it."), phase="3A"))
    for spelling in ("0001024", "0" * 65536):
        n = int(spelling[:32]) if len(spelling) < 100 else 0
        src = "fn inspect(a: [i32; " + spelling + "]) -> i32 { return 0; }\nfn main() -> i32 { return 0; }\n"
        rows.append(case("type-max-leading-zeros" if n else "type-zero-max-token", src, "type-length-token",
            dict(parse="accept", selectors=dict(ast=[True], source_owner=True), route="owned-whole-program",
                 type_queries=[dict(origin=span(src, "[i32; " + spelling + "]"), value=array("i32", n))],
                 inventory=dict(array_literal_elements=0, array_store_target_wrappers=0),
                 note="Length token scanning is O(source bytes), checked before narrowing; no N placeholder expressions"), phase="3A"))
    src = "fn inspect(a: [i32; " + "0" * 65537 + "]) -> i32 { return 0; }\n"
    rows.append(diag("type-zero-token-one-over", src, "E0400", "lex", "0" * 65537,
                     "Existing token-byte cap is enforced by lexer before candidate parsing"))
    for n in (256, 257):
        params = ", ".join(f"p{i}: i32" for i in range(n))
        src = f"fn helper({params}) -> i32 {{ return 0; }}\nfn main() -> i32 {{ let a = [1]; return 0; }}\n"
        if n == 257:
            rows.append(diag("parameters-one-over", src, "E0400", "parse", "p256", "Existing parameter cap remains 256"))
        else:
            rows.append(case("parameters-max", src, "call-vs-literal-cap", dict(parse="accept", parameters=n,
                selectors=dict(ast=[True], source_owner=True), route="owned-whole-program",
                downstream="Native still rejects this unused helper at 64 parameters; no consumer observation"), phase="3A"))
        src = "fn main() -> i32 { let a = [1]; return helper(" + ",".join("0" for _ in range(n)) + "); }\n"
        if n == 257:
            rows.append(diag("call-arguments-one-over", src, "E0400", "parse", "0", "Call cap is 256 even though literal cap is 1024", occurrence=256))
        else:
            rows.append(case("call-arguments-max", src, "call-vs-literal-cap", dict(parse="accept", call_arguments=n,
                selectors=dict(ast=[True], source_owner=True), route="owned-whole-program",
                later_resolution="E0200 helper intentionally undefined; grammar-only positive, not typed success"), phase="3A"))
    return rows


def shape_and_resolution():
    rows = []
    src = "fn main() -> i32 { let mut a: [i32; 2] = [5, 7]; a[(0)] = (a[1]); return (a.len()); }\n"
    a = lambda text, **kw: span(src, text, **kw)
    expected = dict(parse="accept", selectors=dict(ast=[True], source_owner=True), route="owned-whole-program",
        ast=[dict(kind="ArrayLiteral", origin=a("[5, 7]"), elements=[a("5"), a("7")], element_count=2),
             dict(kind="Group", origin=a("(0)"), child=a("0")),
             dict(kind="IndexRead", origin=a("a[(0)]"), base=a("a[(0)]", width=1), index=a("(0)"), role="store-target-wrapper"),
             dict(kind="IndexRead", origin=a("a[1]"), base=a("a[1]", width=1), index=a("1]", width=1), role="value"),
             dict(kind="Group", origin=a("(a[1])"), child=a("a[1]")),
             dict(kind="ArrayLength", origin=a("a.len()"), base=a("a.len()", width=1)),
             dict(kind="Group", origin=a("(a.len())"), child=a("a.len()"))],
        statements=[dict(kind="IndexAssign", origin=a("a[(0)] = (a[1]);"), target=a("a[(0)]"),
                         operator=a("= (a[1])", width=1), value=a("(a[1])"))],
        type_queries=[dict(origin=a("[i32; 2]"), value=array("i32", 2))],
        inventory=dict(array_literal_elements=2, array_store_target_wrappers=1, ast_expression_count=11,
                       later_hir_expression_count=10),
        later_resolution=dict(binding_origin=a("a: [i32", width=1), write_base_first=a("a[(0)]", width=1),
            write_hir_postorder=[a("1]", width=1), a("a[1]"), a("(a[1])"), a("0"), a("(0)")],
            no_hir_read_for_target=True),
        later_type_expectation=dict(binding=array("i32", 2), write_root_order=[a("(a[1])"), a("(0)")],
                                     all_scalar_expressions="i32", literal=array("i32", 2)))
    rows.append(case("grouped-complete-access-and-index", src, "ast-two-root-write", expected, phase="3A"))
    src = "struct R { len: i32 }\nfn main() -> i32 { let r = R { len: 2 }; return r.len; }\n"
    rows.append(case("record-len-field-stays-field", src, "array-free-compatibility", dict(parse="accept",
         ast=[dict(kind="FieldRead", origin=span(src, "r.len"))], array_node_count=0,
         selectors=dict(ast=[True], source_owner=True), route="owned-whole-program", note="Owned selector is true because of records, not array spelling"), phase="3A"))
    return rows


def identity_and_routes():
    rows = []
    # Explicit whole-project loading: file IDs are original root then declared child.
    child_forms = {
        "annotation": "fn unused() -> i32 { let a: [i32; 0] = []; return 0; }\n",
        "result": "fn unused() -> [i32; 1] { return [1]; }\n",
        "parameter": "fn unused(a: [i32; 1]) -> i32 { return 0; }\n",
        "shared-parameter": "fn unused(a: &[i32; 1]) -> i32 { return 0; }\n",
        "exclusive-parameter": "fn unused(a: &mut [i32; 1]) -> i32 { return 0; }\n",
        "literal": "fn unused() -> i32 { [1]; return 0; }\n",
        "read-only": "fn unused(s: i32) -> i32 { return s[0]; }\n",
        "write-only": "fn unused(s: i32) -> i32 { s[0] = 1; return 0; }\n",
        "length-only": "fn unused(s: i32) -> i32 { return s.len(); }\n",
    }
    for tag, child in child_forms.items():
        root = "mod child;\nfn main() -> i32 { return 0; }\n"
        e = dict(parse="accept", selectors=dict(ast=[False, True], source_owner=True),
                 route="owned-whole-project-once", entry=dict(file=0, name="main"),
                 whole_project_includes_unused=True, fallback_on_failure=False,
                 selector_trigger=tag)
        if tag in ("parameter", "shared-parameter", "exclusive-parameter"):
            spelling = {"parameter": "[i32; 1]", "shared-parameter": "&[i32; 1]", "exclusive-parameter": "&mut [i32; 1]"}[tag]
            value = array("i32", 1) if tag == "parameter" else dict(tag="reference",
                kind="shared" if tag == "shared-parameter" else "exclusive", aggregate=array("i32", 1))
            e["type_queries"] = [dict(origin=span(child, spelling, file=1), query_kind="parameter", value=value)]
        if tag in ("read-only", "write-only", "length-only"):
            needle = {"read-only": "s[0]", "write-only": "s[0]", "length-only": "s.len()"}[tag]
            e["later_type_diagnostic"] = dict(code="E0305", stage="type", primary=span(child, needle, file=1))
        rows.append(case("child-route-" + tag, None, "whole-project-routing", e, phase="3A", files={"main.ox": root, "child.ox": child}))
    src = "// [i32; 3] a[0] a.len()\nfn main() -> i32 { /* [] */ return 0; }\n"
    rows.append(case("bracket-comments-do-not-route", src, "array-free-compatibility", dict(parse="accept",
        selectors=dict(ast=[False], source_owner=False), route="scalar", array_node_count=0), phase="3A"))
    src = 'fn main() -> i32 { "[i32; 1]"; return 0; }\n'
    rows.append(diag("bracket-string-is-not-array-syntax", src, "E0101", "parse", '"[i32; 1]"',
        "Existing unsupported string expression rejects before any AST selector; contents never select owned route"))
    root = "mod child;\nfn main() -> i32 { let a: [i32; 1] = crate::child::make(); return crate::child::take(a); }\n"
    child = "pub fn make() -> [i32; 1] { return [4]; }\npub fn take(a: [i32; 1]) -> i32 { return a[0]; }\n"
    rows.append(case("cross-file-array-identity", None, "structural-identity", dict(parse="accept",
        selectors=dict(ast=[True, True], source_owner=True), route="owned-whole-project-once", nominal_array_rows=0,
        type_queries=[dict(origin=span(root, "[i32; 1]", file=0), value=array("i32", 1)),
                      dict(origin=span(child, "[i32; 1]", 0, file=1), value=array("i32", 1)),
                      dict(origin=span(child, "[i32; 1]", 1, file=1), value=array("i32", 1))],
        later_resolution=dict(calls=[dict(use=span(root, "crate::child::make", file=0), declaration=span(child, "make", file=1)),
                                    dict(use=span(root, "crate::child::take", file=0), declaration=span(child, "take", file=1))]),
        later_type_expectation="accept; all three structural identities equal regardless of file",
        entry=dict(file=0, name="main")), phase="3A", files={"main.ox": root, "child.ox": child}))
    src = "struct R { x: i32 }\nfn probe(b: [bool; 0], i: [i32; 0], u: [(); 0], j: [i32; 1], r: R) -> i32 { return 0; }\nfn main() -> i32 { return 0; }\n"
    descriptors = [array("bool", 0), array("i32", 0), array("unit", 0), array("i32", 1), dict(tag="record", declaration=span(src, "R {", width=1))]
    rows.append(case("structural-identities-pairwise-distinct", src, "structural-identity", dict(parse="accept",
        selectors=dict(ast=[True], source_owner=True), route="owned-whole-program", nominal_array_rows=0,
        type_queries=[dict(origin=span(src, spelling, start_in=3 if spelling == "r: R" else 0), value=desc) for spelling, desc in zip(["[bool; 0]", "[i32; 0]", "[(); 0]", "[i32; 1]", "r: R"], descriptors)],
        identity=dict(descriptors=descriptors, equality_matrix=[[i == j for j in range(5)] for i in range(5)]),
        note="Zero widths and equal physical width never collapse scalar tag, array length or nominal identity"), phase="3A"))
    return rows


def build():
    cases = preserved() + new_diagnostics() + bounded_constructions() + shape_and_resolution() + identity_and_routes()
    assert len({c["id"] for c in cases}) == len(cases)
    products = {}
    for c in cases:
        for f in c["files"]:
            products["fixtures/" + c["id"] + "/" + f["path"]] = f.pop("source").encode()
    products["cases.json"] = encoded(dict(schema="oxid-fixed-array-source-contract-v1", status=STATUS,
        evidence="Only generator consistency is executed. No compiler observations or semantic passes.", cases=cases))
    return products


def verify():
    products = build()
    for path, payload in products.items():
        assert (HERE / path).read_bytes() == payload, "stale/mutated generated file: " + path
    manifest = json.loads((HERE / "generated-manifest.json").read_text())
    assert manifest == {p: dict(bytes=len(b), sha256=digest(b)) for p, b in sorted(products.items())}
    expected_files = set(products) | {"generated-manifest.json"}
    actual = {str(p.relative_to(HERE)) for p in (HERE / "fixtures").rglob("*") if p.is_file()}
    assert actual == {p for p in expected_files if p.startswith("fixtures/")}
    # Coordinate facts are rechecked independently against original bytes, not just hashes.
    data = json.loads(products["cases.json"])
    for c in data["cases"]:
        sources = [products["fixtures/" + c["id"] + "/" + f["path"]].decode() for f in c["files"]]
        def walk(value):
            if isinstance(value, dict):
                if all(k in value for k in ("file", "start", "end", "text")):
                    s = sources[value["file"]]
                    assert s.encode()[value["start"]:value["end"]].decode() == value["text"]
                    assert location(s, value["start"]) == [value["line"], value["column"]]
                    assert location(s, value["end"]) == [value["end_line"], value["end_column"]]
                for v in value.values(): walk(v)
            elif isinstance(value, list):
                for v in value: walk(v)
        walk(c["expected"])
    print(json.dumps(dict(status="SOURCE_PACKAGE_CONSISTENCY_PASS", expected_cases=len(data["cases"]),
                          generated_files=len(products), candidate_compiler_runs=0, execution_passes=0)))


def main():
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("command", choices=["generate", "verify"])
    p.add_argument("output", nargs="?")
    args = p.parse_args()
    if args.command == "verify":
        if args.output: p.error("verify accepts no output path")
        verify()
    else:
        if not args.output: p.error("generate needs an explicit output directory")
        output = Path(args.output)
        products = build()
        products["generated-manifest.json"] = encoded({p: dict(bytes=len(b), sha256=digest(b)) for p, b in sorted(products.items())})
        for path, payload in products.items():
            target = output / path
            target.parent.mkdir(parents=True, exist_ok=True)
            if target.exists() and target.read_bytes() != payload:
                raise SystemExit("refusing to replace different existing artifact: " + str(target))
            target.write_bytes(payload)
        print(json.dumps(dict(status="GENERATED_EXPECTATIONS_ONLY", files=len(products), bytes=sum(map(len, products.values())))))


if __name__ == "__main__":
    main()
