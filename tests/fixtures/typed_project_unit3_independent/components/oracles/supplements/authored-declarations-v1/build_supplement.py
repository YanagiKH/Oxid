#!/usr/bin/env python3
"""Independent lexical declaration supplement over already frozen source bytes.

No compiler, candidate checkout, observed raw snapshot or candidate result is an
input. This bounded scanner understands only top-level declarations in the
frozen corpus; function bodies are skipped with balanced braces. It rejects
unsupported declaration/field syntax instead of inventing expected metadata.
"""
from pathlib import Path
import gzip,hashlib,json,re

HERE=Path(__file__).resolve().parent
PACKAGE=HERE.parents[1]
ORIGINAL_SHA='b89c8c5b00b13de9573513ba0f96d28fc1046f68c55d32d4ac1aad03d15c24ba'

def digest(data):return hashlib.sha256(data).hexdigest()
def write(name,value):(HERE/name).write_text(json.dumps(value,indent=2,ensure_ascii=False)+'\n')
def original_unchanged():
    path=PACKAGE/'pre-execution-manifest.json';assert digest(path.read_bytes())==ORIGINAL_SHA
    manifest=json.loads(path.read_text())
    for row in manifest['files']:
        data=(PACKAGE/row['path']).read_bytes()
        assert len(data)==row['bytes'] and digest(data)==row['sha256'],('original frozen file changed',row['path'])
    return len(manifest['files'])

LEX=re.compile(rb'//[^\n]*|/\*.*?\*/|[A-Za-z_][A-Za-z_0-9]*|[0-9]+|[^\s]',re.S)
IDENT=re.compile(rb'[A-Za-z_][A-Za-z_0-9]*\Z')
def scan(data):
    toks=[(m.group(),m.start(),m.end()) for m in LEX.finditer(data) if not m.group().startswith((b'//',b'/*'))]
    i=0;modules=[];functions=[];records=[]
    def take(value=None):
        nonlocal i
        assert i<len(toks),('unexpected eof',value)
        t=toks[i];i+=1
        if value is not None:assert t[0]==value,('unexpected token',t,value)
        return t
    def peek():return toks[i][0] if i<len(toks) else None
    def name():
        token=take();assert IDENT.fullmatch(token[0]),token
        return token[0].decode(),[token[1],token[2]]
    while i<len(toks):
        public=peek()==b'pub'
        if public:take(b'pub')
        kind=take()[0]
        if kind==b'mod':
            n,s=name();take(b';');modules.append(dict(name=n,span=s,public=public))
        elif kind==b'use':
            assert not public,'public imports are outside the frozen grammar'
            while take()[0]!=b';':pass
        elif kind==b'struct':
            n,s=name();take(b'{');fields=[]
            while peek()!=b'}':
                fp=peek()==b'pub'
                if fp:take(b'pub')
                fn,fs=name();take(b':');ty=take()[0]
                if ty==b'(':take(b')');ty=b'()'
                assert ty in (b'i32',b'bool',b'()'),('unsupported field type',ty)
                fields.append(dict(name=fn,span=fs,type=ty.decode(),public=fp))
                if peek()==b',':take(b',')
                else:assert peek()==b'}'
            take(b'}');records.append(dict(name=n,span=s,public=public,fields=fields))
        elif kind==b'fn':
            n,s=name();take(b'(')
            # Parameter/result tokens do not allocate declaration IDs. The
            # first body brace is unambiguous in this bounded source grammar.
            while take()[0]!=b'{':pass
            depth=1
            while depth:
                token=take()[0]
                if token==b'{':depth+=1
                elif token==b'}':depth-=1
            functions.append(dict(name=n,span=s,public=public))
        else:raise AssertionError(('unsupported top-level syntax',kind))
    return dict(modules=modules,functions=functions,records=records)

def declarations(files):
    scanned={path:scan(data) for path,data in files.items()};order=[];symbols={}
    def visit(path,symbol):
        assert path in scanned and path not in symbols,('bad module file graph',path)
        symbols[path]=symbol;order.append(path)
        for mod in scanned[path]['modules']:
            # All current corpus modules are root children. This path rule
            # also handles explicitly declared nested foo/bar.ox source files.
            child=mod['name']+'.ox' if path=='main.ox' else path[:-3]+'/'+mod['name']+'.ox'
            visit(child,symbol+'::'+mod['name'])
    visit('main.ox','crate')
    assert set(order)==set(files),'unreachable advertised file'
    assert order==list(files),'advertised file order differs from independent DFS'
    rows=[];function_id=record_id=0
    for fid,path in enumerate(order):
        for f in scanned[path]['functions']:
            rows.append(dict(kind='function',id=function_id,file=fid,symbol=dict(module=symbols[path],name=f['name']),
                             span=[fid,*f['span']],public=f['public']))
            function_id+=1
        for r in scanned[path]['records']:
            fields=[dict(id=[record_id,j],name=f['name'],type=f['type'],span=[fid,*f['span']],public=f['public']) for j,f in enumerate(r['fields'])]
            rows.append(dict(kind='record',id=record_id,file=fid,symbol=dict(module=symbols[path],name=r['name']),
                             span=[fid,*r['span']],public=r['public'],fields=fields))
            record_id+=1
    fields=sum(len(r.get('fields',[])) for r in rows)
    return dict(function_count=function_id,record_count=record_id,field_count=fields,declarations=rows,
                declaration_association_count=function_id+record_id+fields,
                modules=[dict(id=i,file=i,path=p,symbol=symbols[p]) for i,p in enumerate(order)])

def main():
    unchanged=original_unchanged()
    cases=[json.loads(x) for x in gzip.open(PACKAGE/'expected.jsonl.gz','rt')]
    result=[];authored=[];existing_field_matches=0;added_fields=0
    for case in cases:
        files={row['path']:(PACKAGE/'sources'/case['id']/row['path']).read_bytes() for row in case['source_manifest']}
        for row in case['source_manifest']:
            data=files[row['path']];assert len(data)==row['bytes'] and digest(data)==row['sha256']
        d=declarations(files)
        assert (d['function_count'],d['record_count'])==(case['function_count'],case['record_count']),case['id']
        old=case['declarations']
        if isinstance(old,dict):
            authored.append(case['id']);prior=[dict(r,kind=kind) for key,kind in [('functions','function'),('records','record')] for r in old[key]]
            added_fields+=d['field_count']
        else:prior=old
        byid={(r['kind'],r['id']):r for r in d['declarations']}
        for row in prior:
            fresh=byid[(row['kind'],row['id'])];assert row['span']==fresh['span'],(case['id'],row,fresh)
            assert row.get('name',row.get('symbol',{}).get('name'))==fresh['symbol']['name']
            for field in row.get('fields',[]):
                actual=fresh['fields'][field['id'][1]]
                assert (field['id'],field['name'],field['span'])==(actual['id'],actual['name'],actual['span'])
                existing_field_matches+=1
        result.append(dict(id=case['id'],source_manifest=case['source_manifest'],**d))
    data=dict(schema='unit3-source-declaration-supplement-v1',parent_manifest_sha256=ORIGINAL_SHA,
              evidence_kind='SOURCE_ONLY_PRE_EXECUTION_SUPPLEMENT',cases=result)
    write('declarations.json',data)
    receipt=dict(schema='unit3-source-declaration-supplement-check-v1',parent_files_unchanged=unchanged,cases=len(result),
                 independently_rederived_existing_field_origins=existing_field_matches,new_authored_field_origins=added_fields,
                 authored_cases=authored,compiler_invocations=0,candidate_snapshot_inputs=0,candidate_observation_inputs=0,
                 checks=['every source hash equals original freeze','source module DFS equals frozen file order',
                         'all prior function/record IDs and name origins match independent scanning',
                         'all prior model-derived field IDs/name origins match independent scanning',
                         'all authored records now have complete field IDs/types/name origins'])
    write('source-only-selfcheck.json',receipt)
    original_unchanged()
    print(json.dumps({k:v for k,v in receipt.items() if k not in ('authored_cases','checks')}))

if __name__=='__main__':main()
