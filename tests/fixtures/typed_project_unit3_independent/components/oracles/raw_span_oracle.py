#!/usr/bin/env python3
"""Independent exhaustive span-occurrence inventory for an observed raw snapshot.

Canonical observer shape follows the frozen raw structs. Enums are objects with
`tag` and named payload fields; scalar instructions nest at `scalar`, Return
payloads at `value`, Load payload at `place`, Copy payload at `operand`. Every
stored span is [file,start,end]. This walker never resolves source or decides raw
ownership. It proves only structural visit/count and source association parity.
"""
from collections import Counter
import re

def inventory(raw,route):
    assert route in ('scalar','owned')
    rows=[];decl=[]
    def emit(path,span,owner,category):
        assert isinstance(span,list) and len(span)==3 and all(type(x)==int for x in span),path
        rows.append(dict(path=path,span=span,owner=owner,category=category))
    def operand(obj,path,owner,category):emit(path+'.span',obj['span'],owner,category)
    def scalar(statement,path,owner):
        tag=statement['tag'];emit(path+'.span',statement['span'],owner,'scalar.Statement.'+tag+'.span')
        if tag=='Assign':
            v=statement['value'];p=path+'.value';t=v['tag']
            if t=='Load':operand(v['place'],p+'.place',owner,'scalar.Rvalue.Load.place.span')
            elif t=='NotBool':
                operand(v['operand'],p+'.operand',owner,'scalar.Rvalue.NotBool.operand.span')
                emit(p+'.operator_span',v['operator_span'],owner,'scalar.Rvalue.NotBool.operator_span')
            elif t=='Copy':operand(v['operand'],p+'.operand',owner,'scalar.Rvalue.Copy.operand.span')
            elif t in ('CompareScalar','CheckedI32'):
                for side in ('left','right'):operand(v[side],p+'.'+side,owner,'scalar.Rvalue.'+t+'.'+side+'.span')
                emit(p+'.operator_span',v['operator_span'],owner,'scalar.Rvalue.'+t+'.operator_span')
            else:assert t in ('Bool','I32','Unit'),('unknown Rvalue',t)
        elif tag in ('Initialize','Store'):
            operand(statement['place'],path+'.place',owner,'scalar.Statement.'+tag+'.place.span')
            operand(statement['value'],path+'.value',owner,'scalar.Statement.'+tag+'.value.span')
            if tag=='Store':emit(path+'.operator_span',statement['operator_span'],owner,'scalar.Statement.Store.operator_span')
        else:raise AssertionError(('unknown scalar Statement',tag))
    def metadata(obj,path,owner):
        if obj['diagnostic_origins'] is not None:
            for key in ('primary','cause'):
                emit(path+'.diagnostic_origins.'+key,obj['diagnostic_origins'][key],owner,'owned.DiagnosticOrigins.'+key)
    if route=='owned':
        for ri,record in enumerate(raw['records']):
            assert record['id']==ri
            p=f'records[{ri}]';owner={'record':ri}
            decl.append(dict(path=p,id=ri,kind='record',span=record['span']))
            emit(p+'.span',record['span'],owner,'owned.RawRecordDecl.span')
            for fi,field in enumerate(record['fields']):
                assert field['id']==[ri,fi]
                p=f'records[{ri}].fields[{fi}]'
                decl.append(dict(path=p,id=[ri,fi],kind='field',span=field['span']))
                emit(p+'.span',field['span'],owner,'owned.RawFieldDecl.span')
    for fi,function in enumerate(raw['functions']):
        assert function['id']==fi
        p=f'functions[{fi}]';owner={'function':fi}
        decl.append(dict(path=p,id=fi,kind='function',span=function['span']))
        emit(p+'.span',function['span'],owner,route+'.Function.span')
        tables=['locals','places']+(['owners','references','calls','loans'] if route=='owned' else [])
        for table in tables:
            for i,item in enumerate(function[table]):emit(p+f'.{table}[{i}].span',item['span'],owner,route+'.'+table+'.span')
        for bi,block in enumerate(function['blocks']):
            bp=p+f'.blocks[{bi}]';emit(bp+'.span',block['span'],owner,route+'.Block.span')
            merge=block['merge']
            if merge is not None:
                emit(bp+'.merge.span',merge['span'],owner,route+'.BoolMerge.span')
                emit(bp+'.merge.operator_span',merge['operator_span'],owner,route+'.BoolMerge.operator_span')
                assert len(merge['incoming'])==2
                for i,item in enumerate(merge['incoming']):operand(item['value'],bp+f'.merge.incoming[{i}].value',owner,route+f'.BoolMerge.incoming[{i}].value.span')
            for si,statement in enumerate(block['statements']):
                sp=bp+f'.statements[{si}]'
                if route=='scalar':scalar(statement,sp,owner);continue
                emit(sp+'.span',statement['span'],owner,'owned.Statement.span');metadata(statement,sp,owner)
                instruction=statement['kind'];tag=instruction['tag'];ip=sp+'.kind'
                if tag=='Scalar':scalar(instruction['scalar'],ip+'.scalar',owner)
                elif tag=='Construct':
                    for i,field in enumerate(instruction['fields']):operand(field['value'],ip+f'.fields[{i}].value',owner,'owned.Construct.fields.value.span')
                elif tag in ('WriteField','PrepareScalar'):operand(instruction['value'],ip+'.value',owner,'owned.'+tag+'.value.span')
                else:assert tag in ('StorageLive','StorageEnd','MoveInitialize','Replace','Discard','ReadField','OpenCall','PrepareOwned','PrepareBorrow'),('unknown OwnedInstruction',tag)
            term=block['terminator']
            if term is None:continue
            tp=bp+'.terminator';emit(tp+'.span',term['span'],owner,route+'.Terminator.span')
            if route=='owned':metadata(term,tp,owner)
            kind=term['kind'];tag=kind['tag'];kp=tp+'.kind'
            if tag=='Branch':operand(kind['condition'],kp+'.condition',owner,route+'.Branch.condition.span')
            elif tag in ('Return','ReturnScalar'):operand(kind['value'],kp+'.value',owner,route+'.'+tag+'.value.span')
            elif tag=='Call' and route=='scalar':
                for i,arg in enumerate(kind['args']):operand(arg,kp+f'.args[{i}]',owner,'scalar.Call.args.span')
            elif route=='scalar':assert tag=='Goto',('unknown scalar Terminator',tag)
            else:assert tag in ('Goto','Invoke','ReturnOwned'),('unknown OwnedTerminator',tag)
    assert len(rows)==len({x['path'] for x in rows}), 'independent inventory duplicated a stored field'
    return dict(declarations=decl,spans=rows,D=len(decl),Sspan=len(rows),Vbind=len(decl)+len(rows))

def check(raw,route,files,expected_declarations,actual_visits,actual_counts):
    """Require raw structure, source origins and audit observations to agree.

    actual_visits entries are {kind: declaration|span, path: exact structural
    path}. Counts are separate count-pass and validation-pass records. Equality
    is multiplicity-sensitive; a missing and duplicate occurrence cannot cancel.
    """
    inv=inventory(raw,route);bykind={}
    for d in expected_declarations:
        kind=d['kind'];bykind[(kind,d['id'])]=d['span']
        for f in d.get('fields',[]):bykind[('field',tuple(f['id']))]=f['span']
    assert len(bykind)==inv['D']
    fnfiles={};recfiles={}
    for d in inv['declarations']:
        identity=tuple(d['id']) if isinstance(d['id'],list) else d['id']
        assert d['span']==bykind[(d['kind'],identity)],('declaration origin mismatch',d)
        if d['kind']=='function':fnfiles[d['id']]=d['span'][0]
        elif d['kind']=='record':recfiles[d['id']]=d['span'][0]
    for row in inv['spans']:
        file,start,end=row['span'];assert file in files
        data=files[file];assert 0<=start<=end<=len(data)
        data[:start].decode('utf-8');data[:end].decode('utf-8')
        own=row['owner'];wanted=fnfiles[own['function']] if 'function' in own else recfiles[own['record']]
        assert file==wanted,('wrong containing file',row)
    wanted=Counter([('declaration',d['path']) for d in inv['declarations']]+[('span',d['path']) for d in inv['spans']])
    actual=Counter((v['kind'],v['path']) for v in actual_visits)
    assert actual==wanted,dict(missing=list((wanted-actual).elements()),duplicate_or_extra=list((actual-wanted).elements()))
    for phase in ('count','validate'):
        assert actual_counts[phase]=={k:inv[k] for k in ('D','Sspan','Vbind')},('count mismatch',phase)
    return {k:inv[k] for k in ('D','Sspan','Vbind')}

