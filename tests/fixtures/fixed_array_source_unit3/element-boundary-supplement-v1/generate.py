#!/usr/bin/env python3
"""Six source-only first-excess boundary expectations; never invokes Oxid."""
import argparse
import hashlib
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parent
BASE_MANIFEST = "45e01b88abfd91a1a30eb80cfa65c033a1b2986f365d05caa44836c8051231bf"


def encode(value):
    return (json.dumps(value, sort_keys=True, indent=2) + "\n").encode()


def sha(value):
    return hashlib.sha256(value).hexdigest()


def products():
    prefix = "fn main() -> i32 { let a = [" + ",".join(["0"] * 1024) + ","
    specs = [
        ("after-max-comma-eof", "", "E0100", "expected a bool, i32 or unit expression", "missing expression"),
        ("after-max-comma-nonstarter-semi", ";", "E0100", "expected a bool, i32 or unit expression", "nonstarter, existing missing-expression class"),
        ("after-max-comma-nonstarter-plus", "+", "E0101", "unsupported typed-preview construct `+`", "nonstarter, existing excluded-expression class"),
        ("after-max-comma-starter-not", "!", "E0400", "array element limit exceeded (1024)", "possible unary expression starter"),
        ("after-max-comma-starter-borrow", "&", "E0400", "array element limit exceeded (1024)", "possible borrowed-expression prefix even though bare borrow would later be excluded"),
        ("after-max-comma-starter-array", "[", "E0400", "array element limit exceeded (1024)", "possible array-literal starter"),
    ]
    outputs, cases = {}, []
    for ident, token, code, message, reason in specs:
        source = (prefix + token).encode()
        name = "fixtures/" + ident + ".ox"
        outputs[name] = source
        start, end = len(prefix.encode()), len(source)
        primary = dict(file=0, path="main.ox", start=start, end=end, text=token,
                       line=1, column=start+1, end_line=1, end_column=end+1)
        assert source[start:end].decode() == token
        cases.append(dict(id=ident, source_path=name, source_sha256=sha(source), source_bytes=len(source),
                          status="PROSPECTIVE_NOT_EXECUTED", first_observation_slice="3A",
                          expected=dict(code=code, stage="parse", message=message, primary=primary, secondary=[], notes=[]),
                          reason=reason, admitted_previous_elements=1024,
                          excess_element_parsed=False, excess_element_reserved=False))
    outputs["cases.json"] = encode(dict(schema="oxid-array-first-excess-boundary-v1", base_manifest_sha256=BASE_MANIFEST,
        authority="Parent adopted first-actual-excess interpretation before observations, 2026-10-04",
        scope="Supplement only; all89 corrected contracts-v2 cases and source bytes stay unchanged",
        accepted_closing_bracket_reference="contracts-v2: literal-length-max-trailing-comma",
        rule="After an admitted element and comma, ] closes the array. Otherwise classify whether a possible expression begins. A real first-excess starter fails E0400 before parsing or reservation; EOF/nonstarter keeps the ordinary missing/excluded expression error.",
        cases=cases, candidate_compiler_runs=0))
    return outputs


if __name__ == "__main__":
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("command", choices=["generate", "verify"])
    args = ap.parse_args()
    result = products()
    for name, payload in result.items():
        path = ROOT / name
        if args.command == "verify":
            assert path.read_bytes() == payload, name
        else:
            path.parent.mkdir(parents=True, exist_ok=True)
            if path.exists():
                assert path.read_bytes() == payload, "refuse changed artifact " + name
            path.write_bytes(payload)
    manifest = {name: dict(bytes=len(payload), sha256=sha(payload)) for name, payload in sorted(result.items())}
    if args.command == "verify":
        assert json.loads((ROOT / "generated-manifest.json").read_text()) == manifest
        print("SOURCE_ONLY_BOUNDARY_CONSISTENCY_PASS; six expectations; zero candidate runs")
    else:
        (ROOT / "generated-manifest.json").write_bytes(encode(manifest))
        print("GENERATED_SIX_PROSPECTIVE_BOUNDARY_CASES_ONLY")
