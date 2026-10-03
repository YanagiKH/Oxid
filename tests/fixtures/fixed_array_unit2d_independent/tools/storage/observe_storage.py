#!/usr/bin/env python3
"""Reviewer-owned LLVM *text* observation seam; never invokes a compiler.

Accept only an exact source hash and exact operation bodies chosen after freeze.
Observation helpers initialize/read byte storage, never implement array semantics.
This tool provides no claim that generated LLVM has been verified or executed.
"""
from __future__ import annotations

import argparse
import copy
import hashlib
import json
from pathlib import Path
import re

VERSION = "oxid-native-physical-observer-v2"
PREFIX = "__oxid_physical_"
NAMES = re.compile(r"[A-Za-z_][A-Za-z0-9_]*\Z")
FUNCTION = re.compile(r"^define[^\n]* @([A-Za-z_][A-Za-z0-9_]*).*\{\n", re.M)
ALLOCA = re.compile(r"  %owners = alloca \[(\d+) x i8\], align 4\n\Z")
OWNER = re.compile(r"^  %o(\d+) = getelementptr i8, ptr %owners, i64 (\d+)\n", re.M)
KINDS = {"construct_local", "construct_temporary", "move_initialize", "replace_moved",
         "replace_available", "prepare_owned", "incoming_owned", "return_owned",
         "self_move_temporary", "self_replace"}


def require(condition, message):
    if not condition:
        raise ValueError(message)


def sha(text):
    return hashlib.sha256(text.encode("utf-8")).hexdigest()


def exact(text, needle, label):
    require(needle and text.count(needle) == 1, f"{label}: expected one exact occurrence")
    return text.index(needle)


def functions(source):
    result = {}
    for start in FUNCTION.finditer(source):
        end = source.find("\n}\n", start.end())
        require(end >= 0, "unterminated function")
        name = start.group(1)
        require(name not in result, "duplicate function name")
        result[name] = (start.start(), end + 3)
    return result


def function_text(source, inventory, name):
    require(name in inventory, f"missing function {name}")
    start, end = inventory[name]
    return source[start:end], start


def extent(element, length):
    require(element in {"i32", "bool", "unit"}, "unknown scalar element")
    require(type(length) is int and 0 <= length <= 1024, "length outside frozen array envelope")
    return max(1, length) * (4 if element == "i32" else 1)


def failure_message(oid, what, expected):
    return f"physical storage check failed: {oid} {what} expected {expected}\n"


HELPERS = '''
; Reviewer-only closed byte-storage helpers. Not emitted production semantics.
define internal void @__oxid_physical_poison(ptr %p, i64 %n) noinline {
entry:
  %left = getelementptr i8, ptr %p, i64 -1
  %right = getelementptr i8, ptr %p, i64 %n
  store volatile i8 150, ptr %left, align 1
  store volatile i8 105, ptr %right, align 1
  br label %loop
loop:
  %i = phi i64 [ 0, %entry ], [ %next, %loop ]
  %cell = getelementptr i8, ptr %p, i64 %i
  store volatile i8 165, ptr %cell, align 1
  %next = add i64 %i, 1
  %more = icmp ult i64 %next, %n
  br i1 %more, label %loop, label %done
done:
  ret void
}
define internal void @__oxid_physical_expect(ptr %p, i64 %offset, i8 %expected, ptr %message, i64 %message_len) noinline {
entry:
  %cell = getelementptr i8, ptr %p, i64 %offset
  %actual = load volatile i8, ptr %cell, align 1
  %equal = icmp eq i8 %actual, %expected
  br i1 %equal, label %pass, label %fail
pass:
  ret void
fail:
  call void @__oxid_overflow(ptr %message, i64 %message_len)
  unreachable
}
'''

# Only appended for nonempty bool. Zero/unit and empty-i32 LLVM stay byte-exact v1.
BOOL_HELPERS = '''
; Reviewer-only bool storage checks: observe i1, never padding bits.
define internal void @__oxid_physical_poison_bool(ptr %p, i64 %n, i8 %poison) noinline {
entry:
  %left = getelementptr i8, ptr %p, i64 -1
  %right = getelementptr i8, ptr %p, i64 %n
  store volatile i8 150, ptr %left, align 1
  store volatile i8 105, ptr %right, align 1
  br label %loop
loop:
  %i = phi i64 [ 0, %entry ], [ %next, %loop ]
  %cell = getelementptr i8, ptr %p, i64 %i
  store volatile i8 %poison, ptr %cell, align 1
  %next = add i64 %i, 1
  %more = icmp ult i64 %next, %n
  br i1 %more, label %loop, label %done
done:
  ret void
}
define internal void @__oxid_physical_expect_bool(ptr %p, i64 %offset, i1 %expected, ptr %message, i64 %message_len) noinline {
entry:
  %cell = getelementptr i8, ptr %p, i64 %offset
  %actual = load volatile i1, ptr %cell, align 1
  %equal = icmp eq i1 %actual, %expected
  br i1 %equal, label %pass, label %fail
pass:
  ret void
fail:
  call void @__oxid_overflow(ptr %message, i64 %message_len)
  unreachable
}
'''

BYTE_PATTERN_HELPER = '''
; Reviewer-only byte comparison against a fixed repeating four-byte pattern.
define internal void @__oxid_physical_expect_pattern(ptr %p, i64 %n, i8 %p0, i8 %p1, i8 %p2, i8 %p3, ptr %message, i64 %message_len) noinline {
entry:
  br label %loop
loop:
  %i = phi i64 [ 0, %entry ], [ %next, %advance ]
  %cell = getelementptr i8, ptr %p, i64 %i
  %actual = load volatile i8, ptr %cell, align 1
  %ordinal = urem i64 %i, 4
  %is0 = icmp eq i64 %ordinal, 0
  %is1 = icmp eq i64 %ordinal, 1
  %is2 = icmp eq i64 %ordinal, 2
  %tail = select i1 %is2, i8 %p2, i8 %p3
  %rest = select i1 %is1, i8 %p1, i8 %tail
  %expected = select i1 %is0, i8 %p0, i8 %rest
  %equal = icmp eq i8 %actual, %expected
  br i1 %equal, label %advance, label %fail
advance:
  %next = add i64 %i, 1
  %more = icmp ult i64 %next, %n
  br i1 %more, label %loop, label %done
fail:
  call void @__oxid_overflow(ptr %message, i64 %message_len)
  unreachable
done:
  ret void
}
'''

BOOL_LOOP_HELPER = '''
; Reviewer-only loop observes each bool's i1, never high padding bits.
define internal void @__oxid_physical_expect_bool_loop(ptr %p, i64 %n, i1 %expected, ptr %message, i64 %message_len) noinline {
entry:
  br label %loop
loop:
  %i = phi i64 [ 0, %entry ], [ %next, %advance ]
  %cell = getelementptr i8, ptr %p, i64 %i
  %actual = load volatile i1, ptr %cell, align 1
  %equal = icmp eq i1 %actual, %expected
  br i1 %equal, label %advance, label %fail
advance:
  %next = add i64 %i, 1
  %more = icmp ult i64 %next, %n
  br i1 %more, label %loop, label %done
fail:
  call void @__oxid_overflow(ptr %message, i64 %message_len)
  unreachable
done:
  ret void
}
'''


def emit_patch(source, patches):
    """Retain sufficient exact bytes to reverse each accepted text edit."""
    cursor, output, receipt = 0, "", []
    for patch in sorted(patches, key=lambda p: (p["start"], p["end"])):
        start, end = patch["start"], patch["end"]
        require(cursor <= start <= end <= len(source), "overlapping/reversed patches")
        output += source[cursor:start]
        item = dict(patch, original=source[start:end], output_start=len(output))
        output += patch["replacement"]
        item["output_end"] = len(output)
        item["original_sha256"] = sha(item["original"])
        item["replacement_sha256"] = sha(item["replacement"])
        receipt.append(item)
        cursor = end
    output += source[cursor:]
    require(restore(output, receipt) == source, "round-trip source reconstruction failed")
    return output, receipt


def restore(output, patches):
    restored = output
    for patch in reversed(patches):
        a, b = patch["output_start"], patch["output_end"]
        require(restored[a:b] == patch["replacement"], "observation text differs from receipt")
        require(sha(patch["original"]) == patch["original_sha256"], "original receipt hash mismatch")
        restored = restored[:a] + patch["original"] + restored[b:]
    return restored


def instrument(source, manifest):
    require(manifest["schema"] == VERSION, "unknown manifest schema")
    require(sha(source) == manifest["source_sha256"], "source hash mismatch")
    require(PREFIX not in source, "reviewer helper prefix already present")
    require("declare void @__oxid_overflow(ptr, i64) noreturn\n" in source,
            "expected existing fatal ABI declaration")
    require(source.endswith("\n"), "source must end with newline")
    element, length = manifest["element"], manifest["length"]
    size = extent(element, length)
    payload = length == 0 or element == "unit"
    bool_payload = element == "bool" and length > 0
    compact = size > 16
    inventory = functions(source)
    arenas, patches, bodies, ids, globals_ = {}, [], [], set(), []
    for spec in manifest["arenas"]:
        name = spec["function"]
        require(name not in arenas, "duplicate arena")
        text, base = function_text(source, inventory, name)
        allocation = spec["alloca"]
        match = ALLOCA.fullmatch(allocation)
        require(match, "only canonical owner alloca may be rebased")
        arena_size = int(match.group(1))
        require(arena_size > 0 and arena_size % 4 == 0, "invalid owner allocation extent")
        at = exact(text, allocation, name + " alloca") + base
        offsets = {f"%o{m.group(1)}": int(m.group(2)) for m in OWNER.finditer(text)}
        require(offsets == spec["owner_offsets"], f"{name}: owner offsets differ from frozen manifest")
        require(all(0 <= offset < arena_size for offset in offsets.values()), "owner outside arena")
        replacement = (f"  %{PREFIX}arena = alloca [{arena_size + 8} x i8], align 4\n"
                       f"  %owners = getelementptr i8, ptr %{PREFIX}arena, i64 4\n")
        patches.append(dict(kind="arena_rebase", id=name, start=at, end=at+len(allocation),
                            replacement=replacement, original_extent=arena_size,
                            physical_extent=arena_size+8, rebase=4,
                            preserved_owner_offsets=offsets))
        arenas[name] = (arena_size, offsets)
    require(arenas, "no guarded arenas")
    for number, obs in enumerate(manifest["observations"]):
        oid, kind, name = obs["id"], obs["kind"], obs["function"]
        require(NAMES.fullmatch(oid) and oid not in ids, "invalid/duplicate observation ID")
        ids.add(oid)
        require(kind in KINDS, "unrecognized frozen destination path")
        text, base = function_text(source, inventory, name)
        operation = obs["operation"]
        require(operation.endswith("\n") and operation.startswith("  "), "operation must contain complete instruction lines")
        require(not re.search(r"^\s*(ret|br|switch|unreachable|invoke|indirectbr|resume|callbr)\b", operation, re.M),
                "observation cannot span a terminator")
        require(not re.search(r"^[^ ;].*:\s*$", operation, re.M), "observation cannot span labels")
        require(sha(operation) == obs["operation_sha256"], "frozen operation hash mismatch")
        at = exact(text, operation, oid + " operation") + base
        destination = obs["destination"]
        require(re.fullmatch(r"%(?:o\d+|result)", destination), "unsupported destination pointer")
        storage = obs.get("external_storage")
        if destination == "%result":
            require(kind == "return_owned" and storage, "result needs caller arena binding")
            storage_function, storage_owner = storage["function"], storage["owner"]
            caller, _ = function_text(source, inventory, storage_function)
            call = storage["call"]
            exact(caller, call, oid + " caller binding")
            require(re.search(r"\bcall void @" + re.escape(name) + r"\(", call), "binding is not this callee")
            require(re.search(r"\(ptr (?:%fuel, ptr )?" + re.escape(storage_owner) + r"(?:,|\))", call),
                    "result is not first non-fuel call argument")
        else:
            require(not storage, "local owner must use local arena proof")
            storage_function, storage_owner = name, destination
        require(storage_function in arenas, "destination arena not rebased")
        arena_size, offsets = arenas[storage_function]
        require(storage_owner in offsets, "destination owner missing from arena")
        owner_offset = offsets[storage_owner]
        require(owner_offset + size <= arena_size, "payload exceeds owner arena")
        # The reviewer must bind the exact owner extent, including a zero sentinel.
        require(obs["extent"] == size, "destination extent differs from scalar layout")
        occupied = obs["owner_extent"]
        require(occupied == [owner_offset, owner_offset + size], "owner extent binding mismatch")
        # Source guard overlap would poison data read by the unchanged operation.
        for preserved in obs["preserve_ranges"]:
            require(len(preserved) == 2 and preserved[0] <= preserved[1], "invalid preserved range")
            require(owner_offset + size + 1 <= preserved[0] or preserved[1] <= owner_offset - 1,
                    "poison/guard extent overlaps source or other preserved storage")
        poison = 165
        if bool_payload:
            scalar = obs["expected_scalar"]
            require(type(scalar) is bool, "nonempty bool needs an exact boolean expectation")
            poison = 164 if scalar else 165
            require((poison & 1) != int(scalar), "bool poison low bit must oppose expected")
            poison_call = f"  call void @{PREFIX}poison_bool(ptr {destination}, i64 {size}, i8 {poison})\n"
        else:
            poison_call = f"  call void @{PREFIX}poison(ptr {destination}, i64 {size})\n"
        before = (f"  ; {PREFIX}begin {oid}\n"
                  + poison_call +
                  f"  ; {PREFIX}production_begin {oid}\n")
        after = f"  ; {PREFIX}production_end {oid}\n"
        checks = [(-1, 150, "left_guard", "i8"), (size, 105, "right_guard", "i8")]
        if payload:
            policy = "all_zero"
            checks += [(i, 0, f"payload_{i}", "i8") for i in range(size)]
        elif bool_payload:
            policy = "per_cell_i1_only"
            checks += [(i,int(obs["expected_scalar"]),f"bool_cell_{i}","i1") for i in range(length)]
        else:
            scalar = obs["expected_scalar"]
            require(type(scalar) is int and -(2**31) <= scalar < 2**31,
                    "nonempty i32 needs an exact signed-i32 expectation")
            require('target triple = "x86_64-unknown-linux-gnu"' in source,
                    "i32 byte expectations are qualified only for the pinned little-endian host")
            policy = "repeated_i32_little_endian_bytes"
            cell = scalar.to_bytes(4,"little",signed=True)
            checks += [(i,cell[i % 4],f"payload_{i}","i8") for i in range(size)]
        for offset, expected, what, check_ty in checks[:2] if compact else checks:
            message = failure_message(oid, what, expected)
            encoded = message.encode("ascii")
            symbol = f"@{PREFIX}message_{number}_{what}"
            escaped = "".join(f"\\{byte:02X}" for byte in encoded)
            globals_.append(f'{symbol} = private constant [{len(encoded)} x i8] c"{escaped}"\n')
            checker = "expect_bool" if check_ty == "i1" else "expect"
            after += (f"  call void @{PREFIX}{checker}(ptr {destination}, i64 {offset}, {check_ty} {expected}, "
                      f"ptr {symbol}, i64 {len(encoded)})\n")
        if compact:
            expected = int(obs["expected_scalar"]) if not payload else 0
            message = failure_message(oid,"all_i1_cells" if bool_payload else "payload_pattern",expected)
            encoded = message.encode("ascii")
            symbol = f"@{PREFIX}message_{number}_payload_loop"
            escaped = "".join(f"\\{byte:02X}" for byte in encoded)
            globals_.append(f'{symbol} = private constant [{len(encoded)} x i8] c"{escaped}"\n')
            if bool_payload:
                after += (f"  call void @{PREFIX}expect_bool_loop(ptr {destination}, i64 {size}, i1 {expected}, "
                          f"ptr {symbol}, i64 {len(encoded)})\n")
            else:
                pattern = bytes(4) if payload else cell
                pattern_args = ", ".join(f"i8 {b}" for b in pattern)
                after += (f"  call void @{PREFIX}expect_pattern(ptr {destination}, i64 {size}, {pattern_args}, "
                          f"ptr {symbol}, i64 {len(encoded)})\n")
        after += f"  ; {PREFIX}end {oid}\n"
        replacement = before + operation + after
        require(replacement[len(before):len(before)+len(operation)] == operation, "production operation edited")
        patches.append(dict(kind="observation", id=oid, start=at, end=at+len(operation),
                            replacement=replacement, production_relative_start=len(before),
                            production_relative_end=len(before)+len(operation)))
        bodies.append(dict(id=oid, kind=kind, function=name, destination=destination,
                           storage_function=storage_function, storage_owner=storage_owner,
                           owner_offset=owner_offset, extent=size, before_sha256=sha(operation),
                           after_sha256=sha(operation), byte_equal=True, poison=poison,
                           guard_bytes=[150,105], expected_payload=policy,
                           expected_scalar=obs.get("expected_scalar"),
                           expected_i32_cell_bytes=list(cell) if not payload and not bool_payload else None,
                           bool_padding_bits_observed=False,
                           initialized_observed_offsets=[v[0] for v in checks],
                           payload_check_count=size,
                           payload_byte_checks=0 if bool_payload else size,
                           payload_i1_checks=length if bool_payload else 0,
                           zero_payload_byte_checks=size if payload else 0,
                           i32_payload_byte_checks=size if not payload and not bool_payload else 0))
        bodies[-1]["check_implementation"] = "compact_loop" if compact else "per_byte_or_i1_calls"
        bodies[-1]["static_check_call_count"] = 3 if compact else len(checks)
        bodies[-1]["runtime_payload_interval"] = [0,size]
    require(bodies, "no observations")
    helpers = HELPERS + (BOOL_HELPERS if bool_payload else "")
    if compact:
        helpers += BOOL_LOOP_HELPER if bool_payload else BYTE_PATTERN_HELPER
    patches.append(dict(kind="closed_helpers", id="helpers", start=len(source), end=len(source),
                        replacement="\n"+"".join(globals_)+helpers))
    output, patch_receipts = emit_patch(source, patches)
    receipt = dict(schema=VERSION, status="text-prepared-not-compiled-or-executed",
                   source_sha256=sha(source), output_sha256=sha(output), fixture=manifest["fixture"],
                   element=element, length=length, observations=bodies, patches=patch_receipts,
                   reconstructed_source_sha256=sha(restore(output, patch_receipts)),
                   operation_bodies_byte_equal=True,
                   all_observed_bytes_initialized_before_loads=True,
                   original_owner_offsets_preserved=True,
                   source_native_semantics_claim=False)
    verify_receipt(source, output, receipt)
    return output, receipt


def verify_receipt(source, output, receipt):
    require(sha(source) == receipt["source_sha256"], "receipt input hash mismatch")
    require(sha(output) == receipt["output_sha256"], "receipt output hash mismatch")
    require(restore(output, receipt["patches"]) == source, "receipt does not reconstruct original source")
    for patch in receipt["patches"]:
        if patch["kind"] == "observation":
            start = patch["output_start"] + patch["production_relative_start"]
            end = patch["output_start"] + patch["production_relative_end"]
            require(output[start:end] == patch["original"], "production body changed inside observation")


def narrow_mutant(source, manifest, spec):
    """Create only reviewer-selected full-i32 -> one-byte sentinel controls."""
    require(manifest["element"] == "i32" and manifest["length"] == 0, "mutant requires empty i32")
    require(sha(source) == spec["source_sha256"] == manifest["source_sha256"], "mutant source mismatch")
    require(NAMES.fullmatch(spec["id"]), "invalid mutation ID")
    selected = [o for o in manifest["observations"] if o["id"] == spec["observation_id"]]
    require(len(selected) == 1, "mutation target must be one observation")
    target = selected[0]
    changed = target["operation"]
    changes = spec["changes"]
    require(len(changes) in {1,2}, "only one store or one load/store pair allowed")
    load_vars, store_vars = [], []
    for change in changes:
        old, new = change["from"], change["to"]
        require(old.endswith("\n") and old.count("\n") == 1, "mutation must change one complete instruction")
        require(len(re.findall(r"\bi32\b",old)) == 1 and new == re.sub(r"\bi32\b", "i8", old),
                "mutation may only narrow i32 to i8")
        load = re.fullmatch(r"  (%[A-Za-z0-9_]+) = load i32, ptr %[A-Za-z0-9_]+, align 1\n", old)
        store = re.fullmatch(r"  store i32 (0|%[A-Za-z0-9_]+), ptr %[A-Za-z0-9_]+, align 1\n", old)
        require(load or store, "mutation must narrow an ordinary scalar load/store")
        if load:
            load_vars.append(load.group(1))
        if store:
            store_vars.append(store.group(1))
        exact(changed, old, "mutant instruction")
        changed = changed.replace(old, new, 1)
    require((len(changes) == 1 and not load_vars and store_vars == ["0"])
            or (len(changes) == 2 and len(load_vars) == 1 and load_vars == store_vars),
            "mutant requires zero store or type-consistent load/store pair")
    inventory = functions(source)
    body, base = function_text(source, inventory, target["function"])
    at = base + exact(body, target["operation"], "mutant operation")
    mutated = source[:at] + changed + source[at+len(target["operation"]):]
    mutated_manifest = copy.deepcopy(manifest)
    mutated_manifest["source_sha256"] = sha(mutated)
    for obs in mutated_manifest["observations"]:
        if obs["id"] == target["id"]:
            obs["operation"], obs["operation_sha256"] = changed, sha(changed)
    receipt = dict(schema=VERSION, mutation_id=spec["id"], observation_id=target["id"],
                   original_source_sha256=sha(source), mutated_source_sha256=sha(mutated),
                   original_operation=target["operation"], changed_operation=changed,
                   operation_start=at, changes=changes, expected_first_payload_mismatch=1,
                   expected_payload=[0,165,165,165],
                   scope="one selected empty-i32 destination; negative control only",
                   execution_status="not-run")
    require(mutated[:at]+target["operation"]+mutated[at+len(changed):] == source,
            "mutation is not isolated to nominated body")
    return mutated, mutated_manifest, receipt


def write_json(path, value):
    path.write_text(json.dumps(value, indent=2)+"\n", encoding="utf-8")


def write_bundle(source, manifest, output_dir, mutant_specs=()):
    """Write untouched source, instrumented modules, reversibility and ELF expectations."""
    observed, receipt = instrument(source, manifest)
    case = manifest.get("case_id", f"{manifest['element']}_{manifest['length']}")
    require(NAMES.fullmatch(case), "invalid case ID")
    mutants = []
    mutation_ids = set()
    # Validate all inputs before creating any evidence output.
    for spec in mutant_specs:
        require(spec["id"] not in mutation_ids, "duplicate mutation ID")
        mutation_ids.add(spec["id"])
        mutated, mutant_manifest, mutation_receipt = narrow_mutant(source, manifest, spec)
        mutant_observed, mutant_receipt = instrument(mutated, mutant_manifest)
        mutants.append((spec, mutated, mutant_manifest, mutation_receipt, mutant_observed, mutant_receipt))
    output_dir.mkdir(parents=True, exist_ok=False)
    (output_dir / "production.ll").write_bytes(source.encode("utf-8"))
    (output_dir / "observed.ll").write_bytes(observed.encode("utf-8"))
    write_json(output_dir / "manifest.json", manifest)
    write_json(output_dir / "insertion-receipt.json", receipt)
    positive = dict(case=case+"_positive", module="observed.ll", status=0,
                    stdout_hex=f"{manifest['length']}\n".encode().hex(), stderr_hex="",
                    original_production_sha256=sha(source), observation_input_sha256=sha(source),
                    module_sha256=sha(observed), insertion_receipt="insertion-receipt.json",
                    reconstructed_source_sha256=receipt["reconstructed_source_sha256"],
                    operation_bodies_byte_equal=True, mutation=None, execution_status="not-run")
    cases = [positive]
    for spec, mutated, mm, mr, mo, ir in mutants:
        folder = output_dir / spec["id"]
        folder.mkdir()
        (folder / "uninstrumented-mutant.ll").write_bytes(mutated.encode("utf-8"))
        (folder / "observed-mutant.ll").write_bytes(mo.encode("utf-8"))
        write_json(folder / "manifest.json", mm)
        write_json(folder / "mutation-receipt.json", mr)
        write_json(folder / "insertion-receipt.json", ir)
        expected_error = failure_message(spec["observation_id"], "payload_1", 0)
        cases.append(dict(case=case+"_"+spec["id"], module=f"{spec['id']}/observed-mutant.ll",
                          status=1, stdout_hex="", stderr_hex=expected_error.encode("ascii").hex(),
                          original_production_sha256=sha(source), observation_input_sha256=sha(mutated),
                          module_sha256=sha(mo), insertion_receipt=f"{spec['id']}/insertion-receipt.json",
                          reconstructed_source_sha256=ir["reconstructed_source_sha256"],
                          operation_bodies_byte_equal=True,
                          mutation=dict(receipt=f"{spec['id']}/mutation-receipt.json", **mr),
                          execution_status="not-run"))
    header = ["case", "module", "status", "stdout_hex", "stderr_hex"]
    tsv = "\t".join(header)+"\n"
    tsv += "".join("\t".join(str(row[key]) for key in header)+"\n" for row in cases)
    (output_dir / "harness.tsv").write_text(tsv, encoding="utf-8")
    harness_receipt = dict(schema=VERSION, status="expected-results-only-not-executed",
                           original_production_sha256=sha(source), cases=cases,
                           source_free_execution_required=True, pinned_llvm="19.1.7", opt_level="O0",
                           original_source_preserved="production.ll",
                           outer_arena_rebases=[p for p in receipt["patches"] if p["kind"]=="arena_rebase"])
    write_json(output_dir / "harness-receipt.json", harness_receipt)
    return harness_receipt


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest="command", required=True)
    prepare = sub.add_parser("prepare")
    prepare.add_argument("source", type=Path)
    prepare.add_argument("manifest", type=Path)
    prepare.add_argument("output", type=Path)
    prepare.add_argument("--mutant", action="append", type=Path, default=[])
    verify = sub.add_parser("verify")
    verify.add_argument("source", type=Path)
    verify.add_argument("observed", type=Path)
    verify.add_argument("receipt", type=Path)
    args = parser.parse_args()
    source = args.source.read_bytes().decode("utf-8")
    if args.command == "verify":
        verify_receipt(source, args.observed.read_bytes().decode("utf-8"), json.loads(args.receipt.read_text()))
        print("Exact source reconstruction and unchanged operation bodies verified (text only)")
        return
    manifest = json.loads(args.manifest.read_text())
    receipt = write_bundle(source, manifest, args.output,
                           [json.loads(path.read_text()) for path in args.mutant])
    print(json.dumps(dict(output=str(args.output), cases=len(receipt["cases"]),
                          source_sha256=sha(source), status=receipt["status"])))


if __name__ == "__main__":
    main()
