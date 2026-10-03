#!/usr/bin/env python3
"""Reproduce one parser source-coordinate amendment without compiler access."""
import argparse
import base64
import copy
import gzip
import hashlib
import json
from pathlib import Path
import re

FREEZE = "7a2ec4fdf43bf94f3348a04251a9c04b77610b0e6bdd7318ce62dc2c5a5c4027"
GZIP = "ce2b6f72448def28b0cfaecfab2b8cc8b0f7c5a3bf340511e040dd28421839bb"
DECODED = "b19819e2e4af627dfe3877ef7753fe237aa7830b16d2a83197fae0ed02010cbc"
NAME = "parser-contract-new-v1.json.gz"
IDENTITY = "oxid-unit4-parser-v1-location-amendment-v1"
CANON = "UTF-8 json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(',', ':'), allow_nan=False), without trailing newline"
SOURCES = {
    "public-root-original-reserved-function": b"fn pub()->i32{return 1;}",
    "public-root-module-before-reserved-function": b"mod child; fn pub()->i32{return 1;}",
    "public-root-module-before-signature-error": b"mod child;\nfn main()::i32 {}",
}


def sha(data):
    return hashlib.sha256(data).hexdigest()


def canonical(value):
    return json.dumps(value, ensure_ascii=False, sort_keys=True,
                      separators=(",", ":"), allow_nan=False).encode("utf-8")


def encoded(value):
    return (json.dumps(value, ensure_ascii=False, sort_keys=True,
                       indent=2, allow_nan=False) + "\n").encode("utf-8")


def need(condition, message):
    if not condition:
        raise ValueError(message)


def position(source, offset):
    prefix = source[:offset].decode("utf-8", errors="strict")
    return prefix.count("\n") + 1, len(prefix.rsplit("\n", 1)[-1]) + 1


def location(source, start, end):
    line, column = position(source, start)
    end_line, end_column = position(source, end)
    return dict(file_id=0, path="activation.ox", start=start, end=end, line=line,
                column=column, end_line=end_line, end_column=end_column)


def differences(before, after, path=""):
    if type(before) is not type(after):
        return [path]
    if isinstance(before, dict):
        need(before.keys() == after.keys(), "unexpected object key change")
        return [p for key in before for p in differences(before[key], after[key], path + "/" + key)]
    if isinstance(before, list):
        need(len(before) == len(after), "unexpected list length change")
        return [p for i, item in enumerate(before) for p in differences(item, after[i], path + "/" + str(i))]
    return [] if before == after else [path]


def build(base_root):
    freeze_bytes = (base_root / "package-freeze.json").read_bytes()
    need(sha(freeze_bytes) == FREEZE, "unapproved base package freeze")
    freeze = json.loads(freeze_bytes)
    for row in freeze["files"]:
        data = (base_root / row["path"]).read_bytes()
        need(len(data) == row["bytes"] and sha(data) == row["sha256"], "frozen member changed: " + row["path"])
    packed = (base_root / NAME).read_bytes()
    need(len(packed) == 45324 and sha(packed) == GZIP, "unapproved compressed corpus")
    decoded = gzip.decompress(packed)
    need(len(decoded) == 2113466 and sha(decoded) == DECODED, "unapproved decoded corpus")
    before = json.loads(decoded)
    need(before["case_count"] == len(before["cases"]) == 248, "base roster changed")
    selected = [(i, c) for i, c in enumerate(before["cases"]) if c["category"] == "public-root-first-error"]
    need([c["id"] for _, c in selected] == list(SOURCES), "first-error root inventory changed")
    audits = []
    for index, case in selected:
        source = base64.b64decode(case["source"]["base64"], validate=True)
        need(source == SOURCES[case["id"]], "root source bytes changed")
        need((len(source), sha(source), case["source"]["path"]) == (case["source"]["bytes"], case["source"]["sha256"], "activation.ox"), "source binding mismatch")
        if case["id"] == "public-root-module-before-signature-error":
            need(source.count(b"()::") == 1, "signature anchor not unique")
            start = source.index(b"()::") + 2
            end, token = start + 1, b":"
            derivation = "First colon immediately after the parameter list; the preserved projection identifies one colon, not the adjacent pair."
        else:
            matches = list(re.finditer(rb"\bpub\b", source))
            need(len(matches) == 1, "reserved function-name token not unique")
            start, end = matches[0].span()
            token = b"pub"
            derivation = "Unique complete reserved function-name token pub, identified by ASCII word boundaries."
        computed = location(source, start, end)
        old = case["expected"]["first_diagnostic_projection"]["primary"]
        need(source[start:end] == token, "target token mismatch")
        audits.append(dict(case_id=case["id"], case_index=index, source=case["source"],
            source_utf8=source.decode("utf-8"), target_utf8=token.decode("utf-8"), derivation=derivation,
            original_primary=old, original_slice_utf8=source[old["start"]:old["end"]].decode("utf-8"),
            computed_primary=computed, coordinate_change_required=old != computed,
            byte_index=[dict(offset=i, hex=f"{b:02x}") for i, b in enumerate(source)]))
    need([a["case_id"] for a in audits if a["coordinate_change_required"]] == ["public-root-module-before-reserved-function"], "unexpected coordinate finding")
    target = audits[1]
    index = target["case_index"]
    need(index == 246, "target index changed")
    case = before["cases"][index]
    old_diagnostic = case["expected"]["first_diagnostic_projection"]
    need(old_diagnostic["primary"] == location(SOURCES[case["id"]], 13, 16), "old primary changed")
    need((old_diagnostic["code"], old_diagnostic["stage"], old_diagnostic["message"]) == ("E0101", "parse", "unsupported typed-preview construct `pub`"), "diagnostic semantics changed")
    after = copy.deepcopy(before)
    new_case = after["cases"][index]
    new_diagnostic = new_case["expected"]["first_diagnostic_projection"]
    new_diagnostic["primary"] = target["computed_primary"]
    pointer = f"/cases/{index}/expected/first_diagnostic_projection/primary"
    changes = sorted(differences(before, after))
    need(changes == sorted(pointer + "/" + key for key in ["start", "end", "column", "end_column"]), "non-coordinate change detected")
    amendment = dict(schema_version=1, identity=IDENTITY, canonicalization=CANON,
        base_parser_package_freeze_sha256=FREEZE,
        base_document=dict(name=NAME, gzip_bytes=len(packed), gzip_sha256=GZIP, decoded_bytes=len(decoded), decoded_sha256=DECODED),
        case_id=case["id"], case_pointer=f"/cases/{index}", source=case["source"],
        old_case_canonical_sha256=sha(canonical(case)), old_diagnostic_canonical_sha256=sha(canonical(old_diagnostic)),
        old_diagnostic=old_diagnostic,
        operation=dict(op="replace", pointer=pointer, old_value=old_diagnostic["primary"], value=target["computed_primary"]),
        new_case_canonical_sha256=sha(canonical(new_case)), new_diagnostic_canonical_sha256=sha(canonical(new_diagnostic)),
        new_diagnostic=new_diagnostic, exact_changed_json_pointers=changes,
        reason="The complete reserved function-name token pub occupies UTF-8 bytes [14,17), line 1 scalar columns [15,18); the prior [13,16) range selected a space followed by pu.",
        scope="Only start, end, column and end_column change. All source, message, code, stage, recognition, roster, limits, provenance and other expectation values remain unchanged.")
    amendment_bytes = encoded(amendment)
    effective = dict(schema_version=1, identity=IDENTITY, canonicalization=CANON,
        base_parser_package_freeze_sha256=FREEZE, base_parser_corpus_decoded_sha256=DECODED,
        ordered_amendments=[dict(file="amendment.json", bytes=len(amendment_bytes), sha256=sha(amendment_bytes))],
        effective_parser_document_canonical_bytes=len(canonical(after)),
        effective_parser_document_canonical_sha256=sha(canonical(after)),
        admission_rule="Independently pin this descriptor and amendment bytes. Verify the base package freeze, every frozen member, original gzip and decoded corpus, exact case ID/index/source, old whole-case and diagnostic hashes, complete old diagnostic and old primary. Replace only the specified primary in memory once. Require complete new diagnostic, new hashes, exact four changed integer leaves and full effective canonical-document hash. Reject stale or additional amendments and repeated application.",
        companion_rule="All base package and transported authority bindings remain in force. The original gzip, decoded bytes, generator, validator and freeze remain unchanged. Historical contract_id metadata inside the base document does not identify the effective authority.",
        receipt_binding=dict(required_fields=["effective_contract_identity", "effective_contract_descriptor_sha256", "base_parser_package_freeze_sha256", "base_parser_corpus_decoded_sha256", "ordered_amendment_sha256", "effective_parser_document_canonical_sha256"],
            descriptor_hash_rule="SHA256 of exact effective-contract.json bytes including the final newline.",
            fresh_comparison_rule="Every new comparison must bind the distinct effective identity and all required hashes in addition to the existing observer/build/platform/profile/roster bindings. Prior base-contract observations retain their original bytes and identities and do not qualify this amended contract."))
    proof = dict(schema_version=1, base_parser_package_freeze_sha256=FREEZE, base_parser_corpus_decoded_sha256=DECODED,
        method="Strict source bytes and independent unique lexical anchors, then UTF-8 prefix decoding and scalar counts. No compiler invocation, candidate-code read or candidate-output input.",
        cases=audits, exact_changed_json_pointers=changes, unchanged_case_count=247,
        unchanged_top_level_keys=[key for key in before if key != "cases"],
        prior_audit_scope=dict(independent_audit="new-contract-review/audit_parser_source_only.py:42-56",
            validator="parser/validate_source_contract.py:36-40,64-66",
            verified="The 533-location audit verified file ownership, in-range byte offsets, valid UTF-8 boundary decoding and consistent line/scalar-column coordinates computed from the supplied offsets. The validator made the same coordinate-consistency check. Separate semantic token checks existed for Q35 and diagnostic-cap rows.",
            not_verified="Neither location walk independently located the intended offending token for these three public-root-first-error rows. The wrong [13,16) offset and its internally consistent columns therefore passed. The count 533 measures coordinate consistency checks, not 533 independently established offending-token origins.",
            current_closure="This proof locates the unique pub token or first post-parameter colon directly in each of the three exact frozen sources before comparing its coordinates."),
        compiler_invocations=0, candidate_result_files_read=0)
    return {"amendment.json": amendment_bytes, "effective-contract.json": encoded(effective), "source-coordinate-proof.json": encoded(proof)}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--base-root", type=Path, required=True)
    parser.add_argument("--output-dir", type=Path, required=True, help="Exclusive fresh output directory")
    args = parser.parse_args()
    artifacts = build(args.base_root)
    args.output_dir.mkdir(parents=False, exist_ok=False)
    for name, data in artifacts.items():
        with (args.output_dir / name).open("xb") as stream:
            stream.write(data)
    print(json.dumps({name: dict(bytes=len(data), sha256=sha(data)) for name, data in artifacts.items()}, sort_keys=True))


if __name__ == "__main__":
    main()
