#!/usr/bin/env python3
"""Prospective diagnostic rendering appendix; no compiler invocation."""
import argparse
import json
from pathlib import Path
import authority as a

HERE = Path(__file__).resolve().parent


def message(case, diagnostic):
    ident, code, primary = case["id"], diagnostic["code"], diagnostic["primary"]
    exact = {
        "missing-array-element-separator": "array literal requires `,` or `]`",
        "missing-array-index": "array index requires an expression",
        "missing-array-type-separator": "array type requires `;` after its element type",
        "missing-length-call-close": "call requires `)`",
        "missing-length-close-at-eof": "call requires `)`",
        "overlength-token": "array length limit exceeded (1024)",
        "literal-length-one-over": "array element limit exceeded (1024)",
        "type-zero-token-one-over": "token resource limit exceeded",
        "parameters-one-over": "parameter limit exceeded",
        "call-arguments-one-over": "argument limit exceeded",
        "empty-nonzero-annotation": "type mismatch: expected [i32; 1], found [i32; 0]",
        "literal-nested-nonempty-is-nonscalar": "array elements must have scalar bool, i32 or () type",
    }
    if ident in exact:
        return exact[ident]
    if code == "E0101":
        return "unsupported typed-preview construct `" + primary["text"] + "`"
    if code == "E0200":
        role = "local" if ident in ("read-unknown-base-first", "write-resolve-base-before-both-operands") else "direct function"
        return "unknown " + role + " `" + primary["text"] + "`"
    if code == "E0203":
        return "decimal literal is outside the i32 range [-2147483648, 2147483647]"
    if code == "E0305":
        return "array access requires an array binding"
    if code == "E0304":
        return "operation requires a mutable owned binding"
    if code == "E0300" and "empty" in ident:
        return "empty array literal requires an explicit array annotation on its local initializer"
    if code == "E0300":
        return "type mismatch: expected i32, found bool"
    raise ValueError((ident, code))


def json_span(sp, path):
    return dict(file_id=sp["file"], path=path, **{k: sp[k] for k in
        ("start", "end", "line", "column", "end_line", "end_column")})


def renders(d, paths):
    p = d["primary"]
    obj = dict(schema_version=1, edition="typed-preview", kind="diagnostic", severity="error",
               code=d["code"], stage=d["stage"], message=d["message"],
               primary=json_span(p, paths[p["file"]]),
               secondary=[dict(span=json_span(s["span"], paths[s["span"]["file"]]), message=s["message"]) for s in d["secondary"]],
               notes=d["notes"])
    human = f'error[{d["code"]}] ({d["stage"]}): {d["message"]}\n'
    human += f'  --> {paths[p["file"]]}:{p["line"]}:{p["column"]}\n'
    for sec in d["secondary"]:
        sp = sec["span"]
        human += f'  ::: {paths[sp["file"]]}:{sp["line"]}:{sp["column"]}: {sec["message"]}\n'
    json_line = json.dumps(obj, ensure_ascii=False, separators=(",", ":")) + "\n"
    return dict(human=human, human_sha256=a.digest(human.encode()), json_line=json_line,
                json_line_sha256=a.digest(json_line.encode()), json_object=obj)


def build():
    products = a.build()
    cases = json.loads(products["cases.json"])["cases"]
    out = []
    for case in cases:
        for original in case["expected"].get("diagnostics", []):
            d = dict(original)
            d.update(message=message(case, d), secondary=[], notes=[])
            src = products["fixtures/" + case["id"] + "/main.ox"].decode()
            if d["code"] == "E0305":
                d["secondary"].append(dict(span=a.span(src, "s =", width=1), message="binding declared here"))
            if case["id"] == "write-owner-mutability-last":
                d["secondary"].append(dict(span=a.span(src, "a =", width=1), message="immutable binding declared here"))
            if case["id"] == "empty-nonzero-annotation":
                d["secondary"].append(dict(span=a.span(src, "a:", width=1), message="binding declared here"))
            out.append(dict(id=case["id"], first_observation_slice=case["first_observation_slice"],
                source_sha256=case["files"][0]["sha256"], diagnostic=d,
                rendered=renders(d, [f["path"] for f in case["files"]])))
    # Check new wording agrees with the independently proposed appendix for all original34.
    proposed = json.loads((HERE / "input-audit/wording-appendix-draft.json").read_text())
    by_id = {c["id"]: c for c in out}
    for row in proposed["cases"]:
        actual = by_id[row["id"]]["diagnostic"]
        draft = row["proposed_additional_expectation"]
        assert actual["message"] == draft["message"], row["id"]
        assert actual["notes"] == draft["notes"], row["id"]
        assert [dict(**{k:s["span"][k] for k in ("file", "start", "end", "text")}, message=s["message"]) for s in actual["secondary"]] == draft["secondary"], row["id"]
    supplemental = []
    for control in proposed["supplemental_controls"]:
        entry = dict(control)
        if "source" in entry:
            src = entry["source"]
            expected = dict(entry["expected"])
            primary = a.span(src, expected["primary"]["text"])
            assert {k: primary[k] for k in ("file", "start", "end", "text")} == expected["primary"], entry["id"]
            expected["primary"] = primary
            entry.update(source_sha256=a.digest(src.encode()), source_bytes=len(src.encode()),
                         expected=expected, rendered=renders(expected, ["main.ox"]))
        supplemental.append(entry)
    return a.encoded(dict(schema="oxid-array-diagnostic-wording-v1", status="NEW_PROSPECTIVE_APPENDIX_REQUIRES_REVIEW",
        scope="Exact new message/secondary/notes/rendering addition, separate from unchanged original34 authority. No diagnostic was observed from candidate compilation.",
        display_path_policy="Fixture-relative main.ox / child.ox; comparison must bind relocation without changing source file identity",
        array_type_format="[bool; N], [i32; N], [(); N]; canonical decimal N", cases=out,
        supplemental_controls=supplemental, supplemental_control_constraints=proposed["supplemental_control_constraints"]))


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("command", choices=["generate", "verify"])
    args = parser.parse_args()
    target = HERE / "diagnostic-wording-v1.json"
    payload = build()
    if args.command == "verify":
        assert target.read_bytes() == payload
        print("DIAGNOSTIC_APPENDIX_SOURCE_CONSISTENCY_PASS; candidate runs=0")
    else:
        if target.exists() and target.read_bytes() != payload:
            raise SystemExit("refusing to replace different existing diagnostic appendix")
        target.write_bytes(payload)
        print("GENERATED_PROSPECTIVE_DIAGNOSTIC_APPENDIX_ONLY")
