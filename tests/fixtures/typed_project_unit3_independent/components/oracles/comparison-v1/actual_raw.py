"""Mechanical actual-raw joins. No expected file or source resolver is imported."""
import sys
if sys.flags.optimize:
    raise RuntimeError("Unit3 verification requires Python assertions (PYTHONOPTIMIZE=0)")


SCALARS={'I32':'i32','Bool':'bool','Unit':'()'}
def tuple_value(v):
    assert isinstance(v,dict) and set(v)=={'tag','items'} and len(v['items'])==1,v
    return v['items'][0]
def parameter_value(v):
    if v['tag']=='Scalar':return v['scalar']
    return tuple_value(v)
def value_type(v):
    t=v['tag']
    if t in SCALARS:return dict(mode='scalar',type=SCALARS[t])
    if t=='Scalar':return value_type(v['scalar'])
    if t=='Value':return value_type(tuple_value(v))
    if t=='Owned':return dict(mode='owned',record=tuple_value(v))
    if t=='Reference':return dict(mode={'Shared':'shared','Exclusive':'exclusive'}[v['kind']['tag']],record=v['record'])
    raise AssertionError(('unknown actual type',v))
def scalar(v):
    t=v['tag'];assert t in SCALARS,v
    if t=='Unit':return None
    value=v['value'] if set(v)=={'tag','value'} else tuple_value(v)
    assert type(value) is (bool if t=='Bool' else int),v
    return value
def base(v):
    assert v['tag'] in ('Owner','Parameter'),v
    return dict(space='owner' if v['tag']=='Owner' else 'reference',id=tuple_value(v))

class Raw:
    def __init__(self,raw,route):
        self.raw=raw;self.route=route;self.functions=raw['functions'];self.records=raw.get('records',[])
        assert [f['id'] for f in self.functions]==list(range(len(self.functions)))
        assert [r['id'] for r in self.records]==list(range(len(self.records)))
        self.all_operations={i:list(self.operations(i)) for i in range(len(self.functions))}

    def operations(self,fid):
        f=self.functions[fid]
        for bi,b in enumerate(f['blocks']):
            if b['merge'] is not None:yield dict(function=fid,block=bi,position='merge',operation='BoolMerge',span=b['merge']['span'],payload=b['merge'])
            for si,s in enumerate(b['statements']):
                k=s['kind'] if self.route=='owned' else s
                op=k['tag'] if self.route=='owned' else 'Scalar'
                yield dict(function=fid,block=bi,position=f'statement[{si}]',operation=op,span=s['span'],payload=k,
                           diagnostic_origins=s.get('diagnostic_origins'))
            t=b['terminator']
            if t is not None:yield dict(function=fid,block=bi,position='terminator',operation=t['kind']['tag'],span=t['span'],payload=t['kind'],diagnostic_origins=t.get('diagnostic_origins'))

    def op_at(self,fid,block,position):
        found=[o for o in self.all_operations[fid] if o['block']==block and o['position']==position]
        assert len(found)==1,('no unique actual control point',fid,block,position)
        return found[0]

    def parameter(self,fid,position):
        f=self.functions[fid]
        if self.route=='scalar':
            assert position<f['param_count'];v=f['locals'][position]
            return dict(space='scalar',id=position,span=v['span'],type=value_type(v['ty']),position=position)
        p=f['parameters'][position];slot=parameter_value(p);tag=p['tag']
        if tag=='Scalar':
            v=f['locals'][slot];ty=value_type(v['ty']);space='scalar'
        elif tag=='Owned':
            v=f['owners'][slot];ty=dict(mode='owned',record=v['record']);space='owner'
        elif tag=='Reference':
            v=f['references'][slot];ty=dict(mode={'Shared':'shared','Exclusive':'exclusive'}[v['kind']['tag']],record=v['record']);space='reference'
        else:raise AssertionError(('unknown actual parameter',p))
        return dict(space=space,id=slot,span=v['span'],type=ty,position=position)

    def signature(self,fid):
        f=self.functions[fid];n=f['param_count'] if self.route=='scalar' else len(f['parameters'])
        return dict(result=value_type(f['result']),parameters=[self.parameter(fid,i) for i in range(n)])

    def owners(self,fid):
        if self.route!='owned':return []
        return [self.owner(fid,i) for i,_ in enumerate(self.functions[fid]['owners'])]

    def owner(self,fid,oid):
        f=self.functions[fid];o=f['owners'][oid];k=o['kind'];tag=k['tag']
        row=dict(function=fid,role=tag,span=o['span'],record=o['record'])
        if tag=='Local':row['mutable']=k['mutable']
        elif tag=='Parameter':row['position']=k['position']
        elif tag in ('StagedArgument','CallResult'):
            row['call_span']=f['calls'][k['call']]['span']
            if tag=='StagedArgument':row['argument']=k['argument']
        else:assert tag=='Temporary',tag
        return row

    def bindings(self,fid):
        f=self.functions[fid];params=self.signature(fid)['parameters'];out=[]
        positions={(p['space'],p['id']):p['position'] for p in params}
        for i,l in enumerate(f['locals']):
            if l['kind']['tag'] in ('Binding','Parameter'):
                pos=positions.get(('scalar',i));out.append(dict(space='scalar',id=i,span=l['span'],type=value_type(l['ty']),mutable=False,position=pos,role='parameter' if pos is not None else 'local'))
        for i,p in enumerate(f['places']):out.append(dict(space='place',id=i,span=p['span'],type=value_type(p['ty']),mutable=True,position=None,role='local'))
        if self.route=='owned':
            for i,o in enumerate(f['owners']):
                tag=o['kind']['tag']
                if tag in ('Local','Parameter'):
                    pos=positions.get(('owner',i));out.append(dict(space='owner',id=i,span=o['span'],type=dict(mode='owned',record=o['record']),
                        mutable=o['kind'].get('mutable',False),position=pos,role='parameter' if pos is not None else 'local'))
            for i,r in enumerate(f['references']):out.append(dict(space='reference',id=i,span=r['span'],
                type=dict(mode={'Shared':'shared','Exclusive':'exclusive'}[r['kind']['tag']],record=r['record']),mutable=False,position=r['position'],role='parameter'))
        return out

    def calls(self,fid):
        f=self.functions[fid]
        if self.route=='owned':return [dict(c,id=i,function=fid) for i,c in enumerate(f['calls'])]
        return [dict(o['payload'],id=o['block'],function=fid,span=o['span']) for o in self.all_operations[fid] if o['operation']=='Call']

    def frame_counts(self,fid):
        f=self.functions[fid];s=len(f['locals'])+len(f['places'])
        if self.route=='scalar':return s
        a=sum(len(c['arguments']) for c in f['calls']);p=sum(max(1,len(self.records[o['record']]['fields'])) for o in f['owners'])
        o=len(f['owners']);r=len(f['references']);l=len(f['loans']);c=len(f['calls'])
        return dict(S=s,A=a,P=p,O=o,R=r,L=l,C=c,X=s+a+p+4*o+8*r+12*l+2*c)
