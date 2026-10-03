#!/usr/bin/env python3
"""Independent raw scalar wire-format checks; no candidate input."""
import sys
if sys.flags.optimize:raise RuntimeError('Python assertions required')
import copy,json
from pathlib import Path
from actual_trace import physical_scalar

def main():
    positives=[('i32',{'tag':'I32','items':[-2147483648]},[0]*4,[0,0,0,128]),
               ('i32',{'tag':'I32','items':[2147483647]},[0]*4,[255,255,255,127]),
               ('bool',{'tag':'Bool','items':[False]},[0],[0]),('bool',{'tag':'Bool','items':[True]},[0],[1]),
               ('()',{'tag':'Unit'},[0],[0])]
    for args in positives:physical_scalar(*args)
    bad=[('short-i32',('i32',{'tag':'I32','items':[0]},[0]*4,[0]*3)),
         ('long-i32',('i32',{'tag':'I32','items':[0]},[0]*4,[0]*5)),
         ('bool-two',('bool',{'tag':'Bool','items':[True]},[0],[2])),
         ('wide-bool',('bool',{'tag':'Bool','items':[True]},[0],[1,0])),
         ('wrong-scalar-type',('bool',{'tag':'I32','items':[1]},[0],[1])),
         ('unit-one',('()',{'tag':'Unit'},[0],[1])),
         ('empty-unit',('()',{'tag':'Unit'},[0],[])),
         ('out-of-range-byte',('bool',{'tag':'Bool','items':[True]},[0],[256])),
         ('bool-as-byte',('bool',{'tag':'Bool','items':[True]},[0],[True]))]
    rows=[]
    for id,args in bad:
        try:physical_scalar(*args)
        except Exception as e:rows.append(dict(id=id,status='REJECTED',reason=repr(e)))
        else:raise AssertionError(id)
    report=dict(schema='unit3-independent-physical-byte-sensitivity-v1',positive_cases=len(positives),rejected=len(rows),mutants=rows,candidate_inputs=0,compiler_invocations=0)
    Path(__file__).with_name('physical-byte-sensitivity.json').write_text(json.dumps(report,indent=2)+'\n');print('physical format mutants rejected',len(rows))
if __name__=='__main__':main()
