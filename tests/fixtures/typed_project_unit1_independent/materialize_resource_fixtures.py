#!/usr/bin/env python3
"""Exact counted resource fixtures, independent of compiler implementation."""
if not __debug__:
    raise SystemExit('Qualification refuses optimized Python because it can remove verification guards')
import hashlib,json,os
from pathlib import Path
ROOT=Path(__file__).resolve().parent
BASE=ROOT/'resource-fixtures';BASE.mkdir(exist_ok=True)
rows=[]
def put(d,p,data):
    p=d/p;p.parent.mkdir(parents=True,exist_ok=True);p.write_bytes(data)
def record(name,expected):
    d=BASE/name
    files=[]
    for p in sorted(d.rglob('*')):
        if p.is_file():
            data=p.read_bytes();files.append({'path':str(p.relative_to(d)),'bytes':len(data),'sha256':hashlib.sha256(data).hexdigest()})
    rows.append({'id':name,'entry':'app.ox','expected':expected,'files':files})
def comments(length):
    chunks=[]
    while length:
        n=min(length,65536)
        assert n>=4
        chunks.append(b'/*'+b' '*(n-4)+b'*/');length-=n
    return b''.join(chunks)
for extra in [0,1]:
    name=f'bytes_{"over" if extra else "exact"}';d=BASE/name;d.mkdir(exist_ok=True)
    put(d,'app.ox',b'mod a;\n');put(d,'a.ox',comments(1048576-7)+b' '*extra)
    record(name,{'result':'error' if extra else 'ok','aggregate_source_bytes':1048576+extra,'code':'E0400' if extra else None,'stage':'source' if extra else None,'primary':[0,4,5] if extra else None})
for extra in [0,1]:
    name=f'tokens_{"over" if extra else "exact"}';d=BASE/name;d.mkdir(exist_ok=True)
    put(d,'app.ox',b'mod a;\n');put(d,'a.ox',b'/**/'*(99995+extra))
    record(name,{'result':'error' if extra else 'ok','tokens':100000+extra,'nodes':1,'code':'E0400' if extra else None,'stage':'lex' if extra else None,'primary':[1,399980,399984] if extra else None})
for children in [255,256]:
    name=f'modules_{"over" if children==256 else "exact"}';d=BASE/name;d.mkdir(exist_ok=True)
    put(d,'app.ox',''.join(f'mod m{i:03};\n' for i in range(children)).encode())
    for i in range(children):put(d,f'm{i:03}.ox',b'')
    record(name,{'result':'error' if children==256 else 'ok','modules':children+1,'code':'E0400' if children==256 else None,'stage':'source-project' if children==256 else None,'primary':[0,2554,2558] if children==256 else None})
for depth in [32,33]:
    name=f'depth_{"over" if depth==33 else "exact"}';d=BASE/name;d.mkdir(exist_ok=True)
    put(d,'app.ox',b'mod n;\n')
    for i in range(1,depth+1):put(d,'/'.join(['n']*(i-1)+['n.ox']),b'mod n;\n' if i<depth else b'')
    record(name,{'result':'error' if depth==33 else 'ok','depth':depth,'code':'E0400' if depth==33 else None,'stage':'source-project' if depth==33 else None,'primary':[32,4,5] if depth==33 else None})
for extra in [0,1]:
    name=f'entries_{"over" if extra else "exact"}';d=BASE/name;d.mkdir(exist_ok=True)
    put(d,'app.ox',''.join(f'mod m{i:03};\n' for i in range(200)).encode())
    for i in range(200):put(d,f'm{i:03}.ox',b'')
    for i in range(299+extra):put(d,f'j{i:04}xxx',b'')
    entries=os.listdir(d);units=sum(len(os.fsencode(n)) for n in entries)
    assert len(entries)==500+extra
    record(name,{'result':'error' if extra else 'ok','probes':200,'directory_entries':len(entries),'scan_entries':200*len(entries),'scan_name_units':200*units,'code':'E0400' if extra else None,'stage':'source-project' if extra else None,'message':'module directory scan budget exceeded' if extra else None,'primary':[0,1994,1998] if extra else None})
for extra in [0,1]:
    name=f'name_units_{"over" if extra else "exact"}';d=BASE/name;d.mkdir(exist_ok=True)
    put(d,'app.ox',''.join(f'mod m{i:03};\n' for i in range(128)).encode())
    for i in range(128):put(d,f'm{i:03}.ox',b'')
    for i in range(650):put(d,f'u{i:03}'+('_'*196),b'')
    put(d,'u650'+('_'*(166+extra)),b'')
    entries=os.listdir(d);units=sum(len(os.fsencode(n)) for n in entries)
    assert len(entries)==780 and units==131072+extra,(len(entries),units)
    record(name,{'result':'error' if extra else 'ok','probes':128,'directory_entries':780,'scan_entries':99840,'directory_name_units':units,'scan_name_units':128*units,'code':'E0400' if extra else None,'stage':'source-project' if extra else None,'message':'module directory scan budget exceeded' if extra else None,'primary':[0,1274,1278] if extra else None})
(ROOT/'resource-fixture-manifest.json').write_text(json.dumps({'contract_sha256':'25faad72c1c05370e80c43c8ef890b99ebd608baa39f1431521387d5ca44e0aa','fixtures':rows},indent=2)+'\n')
print('Prepared',len(rows),'independent actual-limit resource fixtures')
for r in rows:print(r['id'],r['expected'])
