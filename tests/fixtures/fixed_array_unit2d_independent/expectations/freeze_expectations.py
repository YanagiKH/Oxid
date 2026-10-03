#!/usr/bin/env python3
"""Specification-derived Unit2D expectations; no candidate import or execution."""
import hashlib, json
from pathlib import Path
ROOT = Path(__file__).resolve().parent
KINDS = ['bool', 'i32', 'unit']
access = []
for kind in KINDS:
    for n in range(5):
        sequence = ([bool(i%2) for i in range(n)] if kind == 'bool' else
                    [(-1 if i%2 else 1)*(31+17*i) for i in range(n)] if kind == 'i32' else [None]*n)
        for index in sorted(set([-2147483648,-1,*range(n),n,2147483647])):
            valid = 0 <= index < n
            value = (True if kind=='bool' else -2147483648 if kind=='i32' else None)
            changed = sequence.copy()
            if valid: changed[index] = value
            access.append(dict(type=kind,n=n,index=index,bounds=not valid,read=sequence[index] if valid else None,before=sequence,write=value,after=changed if valid else sequence))
assert len(access)==90
texts = [
 ('a\x00\t\r\n\\\".ox','é\r\n界\te\u0301\n🦀\x01z'),
 ('empty.ox',''),
 ('eof.ox','x\n'),
 ('long.ox','é'*257+'界'*129+'x'*311+'\r\nEND'),
]
coords=[]
for fid,(path,text) in enumerate(texts):
    encoded=text.encode(); starts=[0]; p=0
    for ch in text:
        p+=len(ch.encode()); starts.append(p)
    for start in starts:
        prefix=encoded[:start].decode()
        line=prefix.count('\n')+1
        col=len(prefix.rsplit('\n',1)[-1])+1
        coords.append(dict(file=fid,start=start,line=line,column=col))
model=dict(schema=1,authority='accepted proposal v3, sections 5-8; no candidate observations',baseline_head='140514616f1020f3abe2aaadfd70d248f4d52659',baseline_tree='d5ae187c63c98b04215f0f3e0d0ec0796e41586b',access=access,length=[dict(type=t,n=n,result=n,charge=1,requires_full_read=True) for t in KINDS for n in range(5)],coordinates=dict(sources=[dict(path=p,text=t) for p,t in texts],expected=coords),storage=[dict(type=t,n=n,extent=(4 if t=='i32' and n==0 else max(1,n*(4 if t=='i32' else 1))),canonical_full_zero=(t=='unit' or n==0)) for t in KINDS for n in (0,1,4,1024)],storage_paths=['construct-local','construct-temporary','move-initialize','replace-available','replace-moved','prepare-owned-staging','incoming-owned-parameter','return-owned-call-result','self-replacement-via-temporary'],phi_paths=['arithmetic-bounds','bounds-arithmetic','bounds-bounds','bounds-ordinary-suffix','no-split','guarded-terminator'],resource_constants=dict(max_k=2*100000+4096+1,occurrence=64,lookup=40,string_header=24,plan_bound=184*256+8*8192+48*4096+8*8192,admission_bound=(24+8+8+48)*256+24*4096,metadata_cap=32*1024*1024))
c=model['resource_constants']; c['diagnostic_bound']=(c['occurrence']+c['lookup']+c['string_header'])*c['max_k']; c['combined_bound']=c['diagnostic_bound']+c['plan_bound']+c['admission_bound']; assert c['combined_bound']==26620032
(ROOT/'expected-v1.json').write_text(json.dumps(model,ensure_ascii=False,indent=2)+'\n')
files=['freeze_expectations.py','requirements-v1.md','expected-v1.json']
(ROOT/'freeze-binding-v1.json').write_text(json.dumps({p:hashlib.sha256((ROOT/p).read_bytes()).hexdigest() for p in files},indent=2)+'\n')
print(json.dumps(dict(core_index_rows=len(access),access_executions=len(access)*2,length_executions=15,coordinate_points=len(coords),resources=c),indent=2))
