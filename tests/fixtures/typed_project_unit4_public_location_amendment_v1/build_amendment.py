#!/usr/bin/env python3
"""Derive one source-coordinate amendment; never invokes or imports the compiler."""
import argparse
import copy
import hashlib
import json
from pathlib import Path
import re

FREEZE = "12b40d321014719805f4864f297ba3598a3c4c5ec6f416de99a0218de68f2a85"
PUBLIC = "b4f1549baf04b8f8a5eed88bb23ee185b7f5b9e899dd0a80b712436f1ae8fe92"
PUBLIC_NAME = "public-cli-contract-replacement-v3.json"
IDENTITY = "oxid-unit4-public-v3-location-amendment-v1"
CANON = "UTF-8 json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(',', ':'), allow_nan=False), without trailing newline"
SOURCES = {
    "host-malformed-root-first": b"mod child; fn pub()->i32{return 1;}",
    "host-old-reserved-name": b"fn pub()->i32{return 1;}",
    "host-malformed-signature-first": b"mod child;\nfn main()::i32 {}",
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
    return dict(file=0, path="main.ox", start=start, end=end, line=line,
                scalar_column=column, end_line=end_line,
                end_scalar_column=end_column)


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
    freeze_bytes = (base_root / "PUBLIC-FREEZE-v3.json").read_bytes()
    need(sha(freeze_bytes) == FREEZE, "unapproved base freeze")
    freeze = json.loads(freeze_bytes)
    base_bytes = (base_root / PUBLIC_NAME).read_bytes()
    need(sha(base_bytes) == PUBLIC, "unapproved base public document")
    frozen_row = [f for f in freeze["files"] if f["path"].endswith("/" + PUBLIC_NAME)]
    need(len(frozen_row) == 1 and frozen_row[0]["sha256"] == PUBLIC and frozen_row[0]["bytes"] == len(base_bytes), "base binding mismatch")
    before = json.loads(base_bytes)
    rows = before["inherited_negative_and_route_cases"]
    first_only = [(i, c) for i, c in enumerate(rows)
                  if c.get("expected_public_projection", {}).get("kind") == "first-diagnostic-only"]
    need([c["id"] for _, c in first_only] == list(SOURCES), "first-only inventory changed")
    audits = []
    for index, case in first_only:
        source = (base_root / case["source_root"] / case["entry"]).read_bytes()
        need(source == SOURCES[case["id"]], "unexpected source bytes")
        bindings = [f for f in case["source_files"] if f["path"] == case["entry"]]
        need(len(bindings) == 1 and bindings[0]["bytes"] == len(source) and bindings[0]["sha256"] == sha(source), "source binding mismatch")
        if case["id"] == "host-malformed-signature-first":
            need(source.count(b"()::") == 1, "signature target not unique")
            start = source.index(b"()::") + 2
            end = start + 1
            token = b":"
            derivation = "First colon immediately after the function parameter list; the frozen diagnostic selects one colon token, not the adjacent pair."
        else:
            matches = list(re.finditer(rb"\bpub\b", source))
            need(len(matches) == 1, "reserved name target not unique")
            start, end = matches[0].span()
            token = b"pub"
            derivation = "Unique reserved function-name token pub; ASCII word boundaries identify the complete token."
        computed = location(source, start, end)
        original = case["expected_public_projection"]["first_diagnostic"]["location"]
        need(source[start:end] == token, "target token mismatch")
        audits.append(dict(case_id=case["id"], case_index=index,
            source_path=case["source_root"] + "/" + case["entry"],
            source_bytes=len(source), source_sha256=sha(source),
            source_utf8=source.decode("utf-8"), source_hex=source.hex(),
            target_utf8=token.decode("utf-8"), derivation=derivation,
            original_location=original, original_slice_utf8=source[original["start"]:original["end"]].decode("utf-8"),
            computed_location=computed, coordinate_change_required=original != computed,
            byte_index=[dict(offset=i, hex=f"{b:02x}") for i, b in enumerate(source)]))
    need([a["case_id"] for a in audits if a["coordinate_change_required"]] == ["host-malformed-root-first"], "unexpected coordinate finding")
    target = audits[0]
    index = target["case_index"]
    case = rows[index]
    diagnostic = case["expected_public_projection"]["first_diagnostic"]
    need(index == 13, "target index changed")
    need(diagnostic == dict(code="E0101", literal_message="unsupported typed-preview construct `pub`", location=location(SOURCES[case["id"]], 13, 16), related_locations=[], stage="parse"), "old diagnostic subtree mismatch")
    after = copy.deepcopy(before)
    after_case = after["inherited_negative_and_route_cases"][index]
    after_diagnostic = after_case["expected_public_projection"]["first_diagnostic"]
    after_diagnostic["location"] = target["computed_location"]
    pointer = f"/inherited_negative_and_route_cases/{index}/expected_public_projection/first_diagnostic/location"
    changed = sorted(differences(before, after))
    need(changed == sorted(pointer + "/" + key for key in ["start", "end", "scalar_column", "end_scalar_column"]), "non-coordinate change detected")
    amendment = dict(schema_version=1, identity=IDENTITY,
        base_public_freeze_sha256=FREEZE, base_document=dict(name=PUBLIC_NAME, bytes=len(base_bytes), sha256=PUBLIC),
        canonicalization=CANON, case_id=case["id"], case_pointer=f"/inherited_negative_and_route_cases/{index}",
        old_case_canonical_sha256=sha(canonical(case)),
        old_diagnostic_canonical_sha256=sha(canonical(diagnostic)),
        operation=dict(op="replace", pointer=pointer, old_value=diagnostic["location"], value=target["computed_location"]),
        new_case_canonical_sha256=sha(canonical(after_case)),
        new_diagnostic_canonical_sha256=sha(canonical(after_diagnostic)),
        source=dict(path=target["source_path"], bytes=target["source_bytes"], sha256=target["source_sha256"]),
        reason="The unique reserved function-name token pub occupies UTF-8 bytes [14,17), line 1 scalar columns [15,18). The prior [13,16) range selects a space followed by pu.",
        scope="Only four coordinate integers change. Source bytes, diagnostic code/stage/message, operation roster, all other cases and companion authorities remain unchanged.")
    amendment_bytes = encoded(amendment)
    effective = dict(schema_version=1, identity=IDENTITY, canonicalization=CANON,
        base_public_freeze_sha256=FREEZE, base_public_document_sha256=PUBLIC,
        ordered_amendments=[dict(file="amendment.json", bytes=len(amendment_bytes), sha256=sha(amendment_bytes))],
        effective_public_document_canonical_bytes=len(canonical(after)),
        effective_public_document_canonical_sha256=sha(canonical(after)),
        companion_authority_rule="All base PUBLIC-FREEZE-v3 file, source, generator and validator bindings remain in force. Only the public document in memory receives this exact amendment. The original on-disk public document and v3 freeze must retain their original bytes.",
        admission_rule="Pin the base freeze and amendment bytes independently. Check base raw document, exact case ID/index, old case and diagnostic subtree hashes and old location before applying once; require the stated new subtree and effective document hashes afterward. Reject unlisted changes, stale source, duplicate application or additional amendments.",
        receipt_binding=dict(required_fields=["effective_contract_identity", "effective_contract_descriptor_sha256", "base_public_freeze_sha256", "ordered_amendment_sha256", "effective_public_document_canonical_sha256"],
            descriptor_hash_rule="SHA256 of exact effective-contract.json bytes including its final newline.",
            fresh_comparison_rule="New comparisons must bind this effective identity and descriptor hash, the base freeze, the ordered amendment hash, and effective document hash. Existing v3 receipts remain unchanged and do not qualify the amended contract. A receipt naming only unmodified v3 is insufficient."))
    proof = dict(schema_version=1, method="Strict frozen-source bytes, unique lexical anchors, UTF-8 prefix decoding and scalar counting; no compiler invocation or candidate-output input.",
        base_public_freeze_sha256=FREEZE, base_public_document_sha256=PUBLIC,
        canonicalization=CANON, cases=audits, exact_changed_json_pointers=changed,
        unchanged_inherited_case_count=len(rows)-1, unchanged_literal_case_count=len(before["literal_cases"]),
        unchanged_top_level_keys=[key for key in before if key != "inherited_negative_and_route_cases"],
        original_document_canonical_sha256=sha(canonical(before)),
        effective_document_canonical_sha256=sha(canonical(after)),
        compiler_invocations=0, candidate_result_files_read=0)
    return {"amendment.json": amendment_bytes, "effective-contract.json": encoded(effective), "source-coordinate-proof.json": encoded(proof)}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--base-root", type=Path, required=True,
                        help="Approved original or verified materialized public-v3 package root")
    parser.add_argument("--output-dir", type=Path, required=True,
                        help="Exclusive fresh directory for the three generated artifacts")
    args = parser.parse_args()
    artifacts = build(args.base_root)
    args.output_dir.mkdir(parents=False, exist_ok=False)
    for name, data in artifacts.items():
        with (args.output_dir / name).open("xb") as stream:
            stream.write(data)
    print(json.dumps({name: dict(bytes=len(data), sha256=sha(data)) for name, data in artifacts.items()}, sort_keys=True))


if __name__ == "__main__":
    main()
