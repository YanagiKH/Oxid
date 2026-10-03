#!/usr/bin/env python3
"""Re-read all frozen preparation outputs and verify text receipts, never LLVM."""
import hashlib
import json
import argparse
from pathlib import Path
import observe_storage as observer


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument("root",nargs="?",type=Path,default=Path(__file__).resolve().parent.parent/"checkpoint2-observations-v2")
    root=parser.parse_args().root
    receipt=json.loads((root/"harness-receipt.json").read_text())
    counts=dict(cases=0,positives=0,mutants=0,positive_observation_sites=0,
                positive_zero_payload_bytes_checked=0,positive_i32_payload_bytes_checked=0,
                positive_bool_i1_cells_checked=0,positive_guard_bytes_checked=0)
    for case in receipt["cases"]:
        counts["cases"]+=1
        observed=(root/case["module"]).read_bytes().decode("utf-8")
        observer.require(observer.sha(observed)==case["module_sha256"],"frozen observed module changed")
        insertion=json.loads((root/case["insertion_receipt"]).read_text())
        bundle=(root/case["bundle_receipt"]).parent
        original=(bundle/"production.ll").read_bytes().decode("utf-8")
        observer.require(observer.sha(original)==case["original_production_sha256"],"original module changed")
        if case["mutation"] is None:
            counts["positives"]+=1
            observer.verify_receipt(original,observed,insertion)
            for site in insertion["observations"]:
                counts["positive_observation_sites"]+=1
                counts["positive_zero_payload_bytes_checked"]+=site["zero_payload_byte_checks"]
                counts["positive_i32_payload_bytes_checked"]+=site["i32_payload_byte_checks"]
                counts["positive_bool_i1_cells_checked"]+=site["payload_i1_checks"]
                counts["positive_guard_bytes_checked"]+=2
        else:
            counts["mutants"]+=1
            mutation=case["mutation"]
            spec=dict(id=mutation["mutation_id"],source_sha256=mutation["original_source_sha256"],
                      observation_id=mutation["observation_id"],changes=mutation["changes"])
            manifest=json.loads((bundle/"manifest.json").read_text())
            mutated,_,_=observer.narrow_mutant(original,manifest,spec)
            observer.verify_receipt(mutated,observed,insertion)
            stored=(root/case["module"]).parent/"uninstrumented-mutant.ll"
            observer.require(stored.read_bytes()==mutated.encode(),"mutant differs from exact narrow replay")
    summary=dict(status="text-reconstruction-passed-native-execution-pending",**counts,
                 harness_sha256=hashlib.sha256((root/"harness.tsv").read_bytes()).hexdigest(),
                 all_selected_production_bodies_unchanged=True,
                 all_original_modules_reconstructed=True,
                 all_mutants_exact_narrow_type_replays=True,
                 compiler_invoked=False,elf_executed=False)
    print(json.dumps(summary,indent=2))


if __name__=="__main__":
    main()
