"""Independent reader for the deliberately small native LLVM dialect.
No emitter label/type/plan helpers are imported. Inputs are emitted text only.
"""
import re
from dataclasses import dataclass

@dataclass
class CFG:
    name: str
    blocks: dict
    edges: dict
    @classmethod
    def parse(cls,module,name):
        active=False; label=None; blocks={}
        for raw in module.splitlines():
            if raw.startswith('define ') and '@'+name+'(' in raw:
                assert not active; active=True; continue
            if not active: continue
            if raw=='}': break
            line=raw.strip()
            if not line or line.startswith(';'): continue
            if line.endswith(':'):
                label=line[:-1]; assert label not in blocks; blocks[label]=[]
            else:
                assert label is not None,(name,line)
                blocks[label].append(line)
        assert active and blocks,(name,'missing')
        edges={}
        for block,lines in blocks.items():
            assert lines,(name,block,'empty')
            end=lines[-1]
            if end.startswith('br '): edges[block]=re.findall(r'label %([a-zA-Z0-9_.$-]+)',end)
            else:
                assert end.startswith('ret ') or end=='unreachable',(name,block,end)
                edges[block]=[]
            assert all(dest in blocks for dest in edges[block])
        return cls(name,blocks,edges)
    def reachable(self,start='entry',without=None):
        todo=[start]; seen=set()
        while todo:
            block=todo.pop()
            if block==without or block in seen: continue
            seen.add(block); todo.extend(self.edges[block])
        return seen
    def location(self,line):
        found=[(b,i) for b,lines in self.blocks.items() for i,x in enumerate(lines) if x==line]
        assert len(found)==1,(self.name,line,found)
        return found[0]
    def dominates(self,first,second):
        a,ai=self.location(first); b,bi=self.location(second)
        return (ai<=bi if a==b else b in self.reachable() and b not in self.reachable(without=a))
    def success_protects(self,success,failure,line):
        block,_=self.location(line)
        return (block in self.reachable() and block in self.reachable(success)
                and block not in self.reachable(without=success)
                and block not in self.reachable(failure)
                and self.blocks[failure][-1]=='unreachable'
                and any('@__oxid_overflow(' in s for s in self.blocks[failure]))
    def phi_predecessors(self):
        observed=[]
        for block,lines in self.blocks.items():
            actual={source for source,nexts in self.edges.items() if block in nexts}
            for index,line in enumerate(lines):
                if '= phi ptr ' not in line: continue
                entries=re.findall(r'\[\s*(%[^, ]+)\s*,\s*%([^\] ]+)\s*\]',line)
                named=[label for _,label in entries]
                assert set(named)==actual,(self.name,block,line,actual)
                assert len(named)==len(set(named)),(self.name,'duplicate phi predecessor')
                destination=line.split(' = ',1)[0]
                flat=[x for body in self.blocks.values() for x in body]
                loads=[x for x in flat if 'load i64, ptr '+destination+',' in x]
                assert len(loads)==1,(self.name,block,'selected pointer load',loads)
                assert self.dominates(line,loads[0]),(self.name,block,'phi must dominate selected load')
                inputs={slot for slot,_ in entries}
                between=flat[flat.index(line):flat.index(loads[0])]
                eager=[x for x in between if any('load i64, ptr '+slot+',' in x for slot in inputs)]
                assert not eager,(self.name,block,'eager incoming slot load',eager)
                observed.append(dict(block=block,actual_predecessors=sorted(actual),entries=entries,load=loads[0]))
        return observed

def verify_bounds(module,function,prefix,length,guarded=False):
    cfg=CFG.parse(module,function)
    lines=[s for ss in cfg.blocks.values() for s in ss]
    nonnegative=[s for s in lines if '= icmp sge i32 ' in s and prefix in s]
    upper=[s for s in lines if '= icmp slt i32 ' in s and prefix in s]
    assert len(nonnegative)==len(upper)==1,(prefix,nonnegative,upper)
    low=nonnegative[0]; high=upper[0]
    low_input=low.split('icmp sge i32 ',1)[1].split(',')[0]
    high_input=high.split('icmp slt i32 ',1)[1].split(',')[0]
    assert low_input==high_input and low.endswith(', 0') and high.endswith(', '+str(length)),(prefix,low,high)
    low_ssa=low.split(' = ',1)[0]; high_ssa=high.split(' = ',1)[0]
    joined=[s for s in lines if '= and i1 ' in s and low_ssa in s and high_ssa in s]
    assert len(joined)==1,(prefix,joined)
    condition=joined[0].split(' = ',1)[0]
    branches=[s for s in lines if s.startswith('br i1 '+condition+',')]
    assert len(branches)==1
    success,failure=re.findall(r'label %([a-zA-Z0-9_.$-]+)',branches[0])
    assert cfg.dominates(low,joined[0]) and cfg.dominates(high,joined[0]) and cfg.dominates(joined[0],branches[0])
    protected=[]
    for line in lines:
        if prefix not in line: continue
        if any(marker in line for marker in [' = sext i32 ',' = zext i32 ',' = mul i64 ',' = getelementptr ',' = load ptr, ']) or (('load ' in line or line.startswith('store ')) and '_ptr' in line):
            assert cfg.success_protects(success,failure,line),(prefix,success,failure,line)
            protected.append(line)
    assert any('getelementptr' in s for s in protected),(prefix,'no protected element address')
    assert any('load ' in s or s.startswith('store ') for s in protected),(prefix,'no protected element access')
    if guarded:
        match=re.fullmatch(r'(f\d+_b\d+)_i(\d+)',prefix); assert match
        guard=match[1]+'_g'+str(int(match[2])+1)
        for line in [low,high,joined[0],branches[0],*protected]:
            assert cfg.success_protects(guard+'_ok',guard+'_error',line),(prefix,'fuel dominance',line)
    return dict(function=function,prefix=prefix,length=length,guarded=guarded,success=success,failure=failure,protected=protected)
