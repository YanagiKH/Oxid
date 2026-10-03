#!/usr/bin/env python3
"""Reject a dropped final commit in a cloned actual original-file1 journal."""
import sys
if sys.flags.optimize:raise RuntimeError('Python assertions required')
import json,hashlib
from compare_mutations import ROOT,HERE,load,read_artifacts,original_file_positive
def main():
    p=ROOT/'typed-project-unit3-mutations/smoke-v1/debug/source-nonzero-original-file-positive/receipt.json'
    e=json.loads(p.read_text());a=json.loads(open(e['mutant_observations']['path']).read());c=load()['control-original-scalar-pilot']
    artifacts=read_artifacts(e);original_file_positive(c,a,artifacts,{'entry':1})
    trace=a['reference']['reference-default']['trace'];trace.pop(max(i for i,r in enumerate(trace) if r['kind']=='operation_commit'))
    try:original_file_positive(c,a,artifacts,{'entry':1})
    except AssertionError as error:reason=repr(error)
    else:raise AssertionError('missing terminal commit escaped file1 positive comparator')
    report=dict(schema='unit3-original-file-terminal-sensitivity-v1',input_sha256=hashlib.sha256(p.read_bytes()).hexdigest(),original='MATCH',dropped_final_commit='REJECTED',reason=reason,actual_artifact_edits=0,expected_edits=0,compiler_invocations=0)
    (HERE/'original-file-terminal-sensitivity.json').write_text(json.dumps(report,indent=2)+'\n');print(report)
if __name__=='__main__':main()
