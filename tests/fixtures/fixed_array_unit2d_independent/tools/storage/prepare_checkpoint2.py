#!/usr/bin/env python3
"""Prepare the specifically parent-nominated checkpoint2 modules, without LLVM."""
import hashlib
import json
import argparse
from pathlib import Path

import bind_chain
import observe_storage as observer

ROOT = Path(__file__).resolve().parent.parent
INPUT = ROOT.parent / "checkpoint2" / "native"
OUTPUT = ROOT / "checkpoint2-observations-v2"
COMMIT = "3882ccd174e361ef1a1dcfa030e12740cb139d93"


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--input",type=Path,default=INPUT,help="directory of nominated uninstrumented reviewer chain LLVM")
    parser.add_argument("--output",type=Path,default=OUTPUT,help="new evidence directory; must not exist")
    parser.add_argument("--source-commit",default=COMMIT,help="exact source identity recorded with input hashes")
    args=parser.parse_args()
    input_dir,output,commit=args.input,args.output,args.source_commit
    frozen = []
    for self_chain, lengths in [(False,[0,1,4,1024]),(True,[0,4])]:
        for element in ["i32","bool","unit"]:
            for length in lengths:
                name=f"{'selfchain' if self_chain else 'chain'}-{element}-n{length}"
                path=input_dir/(name+".ll")
                data=path.read_bytes()
                source=data.decode("utf-8")
                frozen.append(dict(path=str(path),name=name,sha256=hashlib.sha256(data).hexdigest(),
                                   byte_length=len(data),element=element,length=length,
                                   self_chain=self_chain,source=source))
    output.mkdir(exist_ok=False)
    observer.write_json(output/"input-binding.json",dict(source_commit=commit,
                         authority="parent-nominated immutable reviewer raw fixture emission",
                         inputs=[{k:v for k,v in item.items() if k!="source"} for item in frozen]))
    rows, receipts, coverage = [], [], []
    for item in frozen:
        # Recheck files after capturing all hashes and immediately before derivation.
        source=Path(item["path"]).read_bytes().decode("utf-8")
        observer.require(observer.sha(source)==item["sha256"],"frozen input changed while preparing")
        manifest=bind_chain.bind(source,item["sha256"],item["element"],item["length"],
                                 item["self_chain"],item["name"].replace("-","_"))
        manifest["source_commit"]=commit
        if item["element"]=="i32" and item["length"]==0:
            selected=(["self_move_temporary","self_replace"] if item["self_chain"] else
                      [obs["id"] for obs in manifest["observations"]])
        else:
            selected=[]
        specs=[bind_chain.choose_mutant(source,manifest,oid) for oid in selected]
        receipt=observer.write_bundle(source,manifest,output/item["name"],specs)
        for case in receipt["cases"]:
            row=dict(case)
            row["module"]=item["name"]+"/"+case["module"]
            row["bundle_receipt"]=item["name"]+"/harness-receipt.json"
            row["insertion_receipt"]=item["name"]+"/"+case["insertion_receipt"]
            rows.append(row)
        coverage.append(dict(input=item["name"],source_sha256=item["sha256"],
                             observations=len(manifest["observations"]),
                             payload_policy="all_zero" if item["length"]==0 or item["element"]=="unit" else
                                            "per_cell_i1_only" if item["element"]=="bool" else "all_i32_payload_bytes",
                             extent=observer.extent(item["element"],item["length"]),
                             mutant_observations=selected))
    header=["case","module","status","stdout_hex","stderr_hex"]
    tsv="\t".join(header)+"\n"+"".join("\t".join(str(row[k]) for k in header)+"\n" for row in rows)
    (output/"harness.tsv").write_text(tsv)
    observer.write_json(output/"harness-receipt.json",dict(schema=observer.VERSION,
                          source_commit=commit,status="expected-results-only-not-executed",
                          source_files="input-binding.json",cases=rows,coverage=coverage,
                          positive_cases=sum(r["status"]==0 for r in rows),
                          mutant_cases=sum(r["status"]==1 for r in rows),
                          all_original_modules_preserved=True,
                          all_operation_bodies_byte_equal=True,
                          all_insertions_reconstruct_exact_input=True,
                          all_modules_acyclic=True,llvm_tools_invoked=False))
    hashes=[]
    for path in sorted(output.rglob("*")):
        if path.is_file():
            hashes.append(f"{hashlib.sha256(path.read_bytes()).hexdigest()}  {path.relative_to(output)}\n")
    (output/"SHA256SUMS").write_text("".join(hashes))
    print(json.dumps(dict(output=str(output),positive_cases=sum(r["status"]==0 for r in rows),
                          mutant_cases=sum(r["status"]==1 for r in rows),
                          observation_sites=sum(c["observations"] for c in coverage),
                          harness_sha256=hashlib.sha256(tsv.encode()).hexdigest())))


if __name__=="__main__":
    main()
