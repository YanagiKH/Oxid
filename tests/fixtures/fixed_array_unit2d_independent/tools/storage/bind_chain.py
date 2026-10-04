#!/usr/bin/env python3
"""Bind frozen reviewer chain/self-chain semantics to a nominated exact LLVM file.

This is a text inventory, not an array interpreter or an LLVM executor. Raw fixture
instruction ordinals and owner identities below come from the frozen reviewer raw
fixtures, never from producer expected output. It fails if emitter spelling differs.
"""
import argparse
from pathlib import Path
import re

import observe_storage as observer


def operation_body(function, prefix):
    lines = function.splitlines(keepends=True)
    selected = [i for i, line in enumerate(lines) if "%"+prefix+"_" in line]
    observer.require(selected, "missing emitted operation " + prefix)
    observer.require(selected == list(range(selected[0], selected[-1]+1)),
                     "noncontiguous operation " + prefix)
    return "".join(lines[selected[0]:selected[-1]+1])


def bind(source, expected_sha, element, length, self_chain, case_id):
    observer.require(observer.sha(source) == expected_sha, "nominated frozen source SHA mismatch")
    size = observer.extent(element, length)
    count = 7 if self_chain else 6
    stride = ((size+3)//4)*4+4
    offsets = {f"%o{i}": (i//2)*stride+(stride-4 if i%2 else 0) for i in range(2*count)}
    inventory = observer.functions(source)
    caller, _ = observer.function_text(source,inventory,"__oxid_owned_fn_0")
    callee, _ = observer.function_text(source,inventory,"__oxid_owned_fn_1")
    calls = re.findall(r"^  call void @__oxid_owned_fn_1\([^\n]+\)\n",caller,re.M)
    observer.require(len(calls)==1,"reviewer chain must have one owned relay call")
    call = calls[0]
    observer.require(call in {"  call void @__oxid_owned_fn_1(ptr %o10, ptr %o8)\n",
                               "  call void @__oxid_owned_fn_1(ptr %fuel, ptr %o10, ptr %o8)\n"},
                     "owned relay bindings differ from frozen reviewer fixture")
    fn0, fn1 = "__oxid_owned_fn_0", "__oxid_owned_fn_1"
    specs = [
        ("construct_local", "construct_local", fn0, "%o0", None, "f0_b0_i17"),
        ("move_initialize", "move_initialize", fn0, "%o4", "%o0", "f0_b0_i19"),
        ("construct_temporary_1", "construct_temporary", fn0, "%o2", None, "f0_b0_i21"),
        ("replace_moved", "replace_moved", fn0, "%o0", "%o2", "f0_b0_i22"),
        ("construct_temporary_2", "construct_temporary", fn0, "%o6", None, "f0_b0_i24"),
        ("replace_available", "replace_available", fn0, "%o0", "%o6", "f0_b0_i25"),
    ]
    if self_chain:
        specs += [("self_move_temporary","self_move_temporary",fn0,"%o12","%o0","f0_b0_i29"),
                  ("self_replace","self_replace",fn0,"%o0","%o12","f0_b0_i30")]
    specs += [("prepare_owned","prepare_owned",fn0,"%o8","%o0",f"f0_b0_i{32 if self_chain else 27}"),
              ("incoming_owned","incoming_owned",fn1,"%o0","%arg0","f1_param0"),
              ("return_owned","return_owned",fn1,"%result","%o0","f1_b0_term")]
    observations = []
    for oid, kind, function, destination, input_, prefix in specs:
        text = caller if function==fn0 else callee
        if input_ is None and length==0:
            typ = "i32" if element=="i32" else "i8"
            operation = f"  store {typ} 0, ptr {destination}, align 1\n"
            observer.exact(text,operation,oid)
        else:
            operation = operation_body(text,prefix)
        storage_owner = "%o10" if destination=="%result" else destination
        dest_offset = offsets[storage_owner] if function==fn0 or destination=="%result" else 0
        ranges = [[offsets[input_],offsets[input_]+size]] if function==fn0 and input_ else []
        row = dict(id=oid,kind=kind,function=function,destination=destination,extent=size,
                   owner_extent=[dest_offset,dest_offset+size],preserve_ranges=ranges,
                   operation=operation,operation_sha256=observer.sha(operation),
                   frozen_instruction_prefix=prefix)
        if length > 0 and element in {"i32","bool"}:
            stage = (0 if oid in {"construct_local","move_initialize"} else
                     1 if oid in {"construct_temporary_1","replace_moved"} else 2)
            row["expected_scalar"] = [-91,-54,-17][stage] if element=="i32" else [True,False,True][stage]
            row["expectation_authority"] = "frozen reviewer chain value(ty,j), staged ownership sequence"
        if destination=="%result":
            row["external_storage"] = dict(function=fn0,owner="%o10",call=call)
            row["cross_frame_source"] = dict(function=fn1,owner="%o0",independent_allocation=True)
        if kind=="incoming_owned":
            row["cross_frame_source"] = dict(function=fn0,owner="%o8",call=call,
                                              independent_allocation=True)
        observations.append(row)
    manifest = dict(schema=observer.VERSION,source_sha256=expected_sha,case_id=case_id,
                    fixture="self_replacement_chain" if self_chain else "chain_fixture",
                    element=element,length=length,
                    provenance="frozen reviewer raw fixture emitted by nominated production source",
                    arenas=[dict(function=fn0,alloca=f"  %owners = alloca [{count*stride} x i8], align 4\n",owner_offsets=offsets),
                            dict(function=fn1,alloca=f"  %owners = alloca [{((size+3)//4)*4} x i8], align 4\n",owner_offsets={"%o0":0})],
                    observations=observations)
    # Validate all exact bindings and reconstruction before writing any output.
    observer.instrument(source,manifest)
    return manifest


def choose_mutant(source, manifest, observation_id):
    items = [x for x in manifest["observations"] if x["id"]==observation_id]
    observer.require(len(items)==1,"unknown reviewer-selected mutant observation")
    lines = items[0]["operation"].splitlines(keepends=True)
    changes = [{"from":line,"to":re.sub(r"\bi32\b","i8",line)}
               for line in lines if re.search(r"\b(?:load|store) i32\b",line)]
    spec = dict(id=observation_id+"_one_byte",source_sha256=observer.sha(source),
                observation_id=observation_id,changes=changes)
    observer.narrow_mutant(source,manifest,spec)
    return spec


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("source",type=Path)
    parser.add_argument("output",type=Path)
    parser.add_argument("--sha256",required=True)
    parser.add_argument("--element",choices=["i32","bool","unit"],required=True)
    parser.add_argument("--length",type=int,required=True)
    parser.add_argument("--case",required=True)
    parser.add_argument("--self-chain",action="store_true")
    parser.add_argument("--mutant",action="append",default=[],help="exact reviewer-selected observation ID")
    args=parser.parse_args()
    source=args.source.read_bytes().decode("utf-8")
    manifest=bind(source,args.sha256,args.element,args.length,args.self_chain,args.case)
    mutants=[choose_mutant(source,manifest,oid) for oid in args.mutant]
    receipt=observer.write_bundle(source,manifest,args.output,mutants)
    print(f"Prepared {len(receipt['cases'])} expected-only cases in {args.output}/harness.tsv; no LLVM tools run")


if __name__=="__main__":
    main()
