#!/usr/bin/env python3
"""Reviewer-owned LLVM-CFG guard audit, independent from Rust emitter/tests."""
from pathlib import Path
import re,sys,subprocess,os,json
if not __debug__:
    raise SystemExit("CFG audit requires enabled assertions; do not use python -O or PYTHONOPTIMIZE")
if len(sys.argv) != 2:
    raise SystemExit("Usage: verify_owned_native_cfg.py LLVM_EVIDENCE_DIRECTORY")
root = Path(sys.argv[1])
if not root.is_dir():
    raise SystemExit(f"CFG audit input is not an existing directory: {root}")
paths = sorted(root.glob('*.ll'))
if not any('mutant' not in path.name for path in paths):
    raise SystemExit(f"CFG audit requires at least one non-mutant .ll module: {root}")
totals=dict(modules=0,functions=0,stores=0,overflow_stores=0,error_sinks=0)
for path in paths:
    if 'mutant' in path.name: continue
    subprocess.run([os.environ['OXID_LLVM_BIN']+'/llvm-as',str(path),'-o','/dev/null'],check=True,capture_output=True)
    text=path.read_text();totals['modules']+=1
    for match in re.finditer(r'define internal .*?@(__oxid_owned_fn_\d+)\(([^\n]*)\) noinline \{\n(.*?)\n\}',text,re.S):
        fn,params,body=match.groups()
        if 'ptr %fuel' not in params: continue
        blocks={};current=None
        for line in body.splitlines():
            if re.fullmatch(r'[A-Za-z0-9_]+:',line):current=line[:-1];blocks[current]=[]
            elif line.strip():blocks[current].append(line.strip())
        edges={b:re.findall(r'label %([A-Za-z0-9_]+)',lines[-1]) for b,lines in blocks.items()}
        def reachable(start,exclude=None):
            seen=set();todo=[start]
            while todo:
                n=todo.pop()
                if n in seen or n==exclude:continue
                seen.add(n);todo.extend(edges[n])
            return seen
        reached=reachable('entry');totals['functions']+=1
        for b,lines in blocks.items():
            if b.endswith('_error'):
                assert lines[-1]=='unreachable' and any('@__oxid_overflow(' in l for l in lines),(path,fn,b)
                assert not edges[b];totals['error_sinks']+=1
            for line in lines:
                if not line.startswith('store ') or ', ptr %fuel' in line: continue
                op=re.search(r'%(f\d+_b\d+)_(i(\d+)|merge|term)_',line)
                if not op:continue # parameter copies and unnamed empty stores are checked separately by source review
                prefix,kind,index=op.group(1),op.group(2),op.group(3)
                if kind=='term':
                    guards=[int(re.fullmatch(re.escape(prefix)+r'_g(\d+)_ok',n).group(1)) for n in blocks if re.fullmatch(re.escape(prefix)+r'_g(\d+)_ok',n)]
                    guard=f'{prefix}_g{max(guards)}_ok'
                elif kind=='merge':guard=prefix+'_g0_ok'
                else:guard=f'{prefix}_g{int(index)+1}_ok'
                assert guard in blocks,(path,fn,line,guard)
                assert b in reached and b not in reachable('entry',guard),(path,fn,b,line,guard)
                assert b not in reachable(guard[:-2]+'error'),(path,fn,line)
                totals['stores']+=1
                overflow=f'{prefix}_{kind}_checked_ok'
                if overflow in blocks:
                    assert b not in reachable('entry',overflow),(path,fn,line,overflow)
                    assert b not in reachable(overflow[:-2]+'error');totals['overflow_stores']+=1
print(json.dumps(totals,indent=2))
