#!/usr/bin/env python3
"""Bounded positive and deliberately misordered synthetic LLVM audit controls.

No production input is modified. Each case binds a fresh module and catalog,
verifies that LLVM accepts it, and preserves its first audit report.
"""
import argparse
import copy
import json
from pathlib import Path
import re
import subprocess
import owned_source_native_audit as a


def write(path, content):
    with path.open('x') as f:
        f.write(content)


def main():
    p=argparse.ArgumentParser()
    p.add_argument('catalog',type=Path)
    p.add_argument('output',type=Path)
    p.add_argument('--llvm-bin',required=True)
    args=p.parse_args()
    base=args.catalog.resolve().parent
    c=json.loads(args.catalog.read_text())
    # Copy binding paths into fresh catalogs, retaining original input hashes.
    for key in ('source','raw_witness','model'):
        if key+'_path' in c:
            q=Path(c[key+'_path']);c[key+'_path']=str(q if q.is_absolute() else base/q)
    _, data=a.read_bound(c,'llvm',base)
    text=data.decode()
    fs,constants=a.parse_module(text)
    raw=c.get('raw_view')
    if raw is None:
        _,data=a.read_bound(c,'raw_witness',base);r=json.loads(data);raw=r.get('raw_view',r)
    model=a.RawModel(raw)
    rows=c['stores']
    selected=next(r for r in rows if r['raw_operation']['kind']=='statement' and r['store_text'].startswith('store ptr %o'))
    fid=selected['function'];f=fs[f'__oxid_owned_fn_{fid}']
    bid=selected['raw_block'];ops=model.block_ops(fid,bid)
    opkey=(fid,bid,'statement',selected['raw_operation']['index'])
    index=[k for k,op in ops].index(opkey)
    start=f.block_lines[f'b{bid}']
    ends=[f.block_lines[f"b{x['id']}"] for x in model.functions[fid]['blocks'] if f.block_lines[f"b{x['id']}"]>start]
    end=min(ends,default=f.end)
    gs=[g for g in a.discover_guards(f,constants) if g['kind']=='fuel' and start<g['node']<end]
    guard=gs[index];next_guard=gs[index+1]
    original=list(enumerate(text.splitlines(keepends=True),1))
    cases=[]
    for name,target in [('hoisted-borrow-store',guard['loaded']),('delayed-borrow-store',next_guard['debit']+1)]:
        lines=[x for x in original if x[0]!=selected['llvm_line']]
        at=next(i for i,v in enumerate(lines) if v[0]==target)
        lines.insert(at,original[selected['llvm_line']-1])
        cases.append((name,lines,False,'STORE_BEFORE_ADMISSION' if name.startswith('hoisted') else 'STORE_DELAYED_PAST_NEXT_OPERATION'))
    lines=copy.deepcopy(original)
    branch=lines[guard['node']-1][1]
    ok=re.fullmatch(r'\s*br i1 '+a.SSA+', label %'+a.LABEL+', label %('+a.LABEL+r')\s*',branch)[1]
    lines[guard['node']-1]=(guard['node'],f'  br label %{ok}\n')
    cases.append(('removed-borrow-admission-branch',lines,False,'wrong number of operation fuel guards'))
    overflow_row=next(r for r in rows if r['raw_operation']['kind']=='statement' and model.operations[(r['function'],r['raw_block'],'statement',r['raw_operation']['index'])]['overflow_span'] is not None)
    of=fs[f"__oxid_owned_fn_{overflow_row['function']}"]
    store_node=overflow_row['llvm_line']
    og=max((g for g in a.discover_guards(of,constants) if g['kind']=='overflow' and g['node']<store_node),key=lambda g:g['node'])
    _,stored,_=a.destination(overflow_row['store_text'])
    wide_node,wide=of.defs[stored]
    value=re.fullmatch(r'zext i32 ('+a.SSA+r') to i64',wide)[1]
    value_node,value_definition=of.defs[value]
    a.require(value_definition==f'extractvalue {{ i32, i1 }} {og["aggregate"]}, 0','control does not select checked result extraction')
    moving={store_node,wide_node,value_node}
    lines=[x for x in original if x[0] not in moving]
    at=next(i for i,v in enumerate(lines) if v[0]==og['node'])
    lines[at:at]=[x for x in original if x[0] in moving]
    cases.append(('hoisted-overflow-result-store',lines,False,'STORE_BEFORE_OVERFLOW_SUCCESS'))
    names=set()
    for func in fs.values():
        names.update(func.defs)
        names.update(re.findall(a.SSA,func.header))
    mapping={name:f'%independent_value_{i}' for i,name in enumerate(sorted(names))}
    def rename(match):
        return mapping.get(match[0],match[0])
    lines=[(n,re.sub(a.SSA,rename,line)) for n,line in original]
    cases.append(('renamed-all-ssa-values',lines,True,None))
    args.output.mkdir(parents=True,exist_ok=False)
    reports=[]
    for name,lines,expect_pass,expected_error in cases:
        module=''.join(line for _,line in lines)
        mp=(args.output/(name+'.ll')).resolve()
        write(mp,module)
        cc=copy.deepcopy(c)
        cc.pop('probe_llvm_path',None);cc.pop('probe_llvm_sha256',None)
        cc['llvm_path']=str(mp);cc['llvm_sha256']=a.sha(module.encode())
        cc['control']={'kind':'synthetic-llvm-only','name':name,'base_catalog_sha256':a.sha(args.catalog.read_bytes()),'base_llvm_sha256':a.sha(text.encode())}
        positions={old:n for n,(old,_) in enumerate(lines,1)}
        functions,_=a.parse_module(module)
        for row in cc['stores']:
            row['llvm_line']=positions[row['llvm_line']]
            func=functions[f"__oxid_owned_fn_{row['function']}"]
            inst=func.instructions[row['llvm_line']]
            row['store_text']=inst['text'];row['llvm_block']=inst['block']
        cp=args.output/(name+'.catalog.json')
        write(cp,json.dumps(cc,indent=2)+'\n')
        verification=subprocess.run([str(Path(args.llvm_bin)/'llvm-as'),str(mp),'-o','/dev/null'],capture_output=True,text=True)
        report={'case':name,'catalog_sha256':a.sha(cp.read_bytes()),'llvm_sha256':a.sha(mp.read_bytes()),'llvm_as_status':verification.returncode,'llvm_as_stderr':verification.stderr,'expected_audit_pass':expect_pass}
        try:
            result=a.audit(cp,args.llvm_bin)
            report.update(audit_status='pass',counts=result['counts'])
        except a.AuditFailure as e:
            report.update(audit_status='fail',first_rejection=str(e))
        write(args.output/(name+'.first-report.json'),json.dumps(report,indent=2)+'\n')
        a.require(verification.returncode==0,f'{name}: synthetic module is not LLVM-valid')
        a.require((report['audit_status']=='pass')==expect_pass,f'{name}: unexpected audit result')
        if expected_error:
            a.require(expected_error in report['first_rejection'],f'{name}: wrong rejection reason')
        reports.append(report)
    summary={'status':'pass','controls':reports,'auditor_sha256':a.sha(Path(a.__file__).read_bytes()),'test_sha256':a.sha(Path(__file__).read_bytes())}
    write(args.output/'summary.json',json.dumps(summary,indent=2)+'\n')
    print(json.dumps(summary,indent=2))


if __name__=='__main__':
    main()
