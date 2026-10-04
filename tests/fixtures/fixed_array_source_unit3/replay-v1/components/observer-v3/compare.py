#!/usr/bin/env python3
"""Independent source-role and frozen-authority comparator. Never writes expectations."""
import argparse, hashlib, json, pathlib, re

REVIEWED_SOURCE = "aace15bc61711ce9f7731c435f5618eb9b6a1118da4f226e9172b4b053891624"

class Rejected(Exception): pass
def check(ok, message):
    if not ok: raise Rejected(message)
def digest(data): return hashlib.sha256(data).hexdigest()
def origin_key(x):
    if isinstance(x,list): return tuple(x)
    return (x['file'],x['start'],x['end'])
def tokens(data):
    # Independent lexical role check, not a language parser or compiler oracle.
    text=data.decode('utf8'); result=[]; pos=0
    pat=re.compile(r'\s+|//[^\n]*|/\*.*?\*/|"(?:\\.|[^"\\])*"|[A-Za-z_][A-Za-z_0-9]*|[0-9]+|[^\s]',re.S)
    for m in pat.finditer(text):
        check(m.start()==pos,'role tokenizer gap'); pos=m.end()
        v=m.group();
        if not v.isspace() and not v.startswith('//') and not v.startswith('/*'):
            result.append((len(text[:m.start()].encode()),len(text[:m.end()].encode()),v))
    check(pos==len(text),'role tokenizer tail')
    return result

def validate_artifacts(directory,case_ids):
    ids=set(case_ids);check(len(ids)==len(case_ids),'duplicate requested case IDs')
    members=list(directory.iterdir());check(all(p.is_file() and not p.is_symlink() for p in members),'non-file observation artifact')
    allowed={cid+'.'+suffix for cid in ids for suffix in ('jsonl','error')}
    check({p.name for p in members}<=allowed,'unknown observation artifact')
    for cid in ids:
        count=sum((directory/(cid+'.'+suffix)).exists() for suffix in ('jsonl','error'))
        check(count==1,'case must have exactly one complete or error artifact: '+cid)
    check(len(members)==len(ids),'observation artifact count')

def compare_case(case,rows,contract_dir,wording):
    check(rows and rows[0]['row']=='header','missing header')
    check(rows[0]['schema']=='oxid-array-source-observation-v1','wrong schema')
    check(rows[0]['reviewed_source_archive']==REVIEWED_SOURCE,'source build binding')
    check(rows[-1].get('complete') is True and rows[-1]['row']=='complete','incomplete observer')
    check(rows[-1]['rows_before_footer']==len(rows)-1,'row count mismatch')
    check(len(rows)<=rows[0]['row_limit']<=200000,'row limit')
    check(sum(r['row']=='header'for r in rows)==1 and sum(r['row']=='complete'for r in rows)==1,'duplicate envelope row')
    source_rows=[r for r in rows if r['row']=='source']
    check(all(isinstance(r.get('identity'),int)and r['identity']>0 for r in source_rows),'invalid source-allocation identity')
    check(len({r['identity']for r in source_rows})==len(source_rows),'duplicate source-allocation identity')
    check(len(source_rows)==len(case['files']),'source count')
    sources={}; file_tokens={}
    for ref in case['files']:
        matches=[r for r in source_rows if r['file']==ref['file']]
        check(len(matches)==1,'duplicate/missing source id')
        row=matches[0]; data=row['text'].encode('utf8')
        check(row['path']==ref['path'],'display path mismatch')
        check(len(data)==ref['bytes'] and digest(data)==ref['sha256'],'source binding mismatch')
        expected=(contract_dir/'fixtures'/case['id']/ref['path']).read_bytes()
        check(data==expected,'source bytes mismatch')
        check(pathlib.Path(row['original_path']).name==ref['path'],'nonbijective display mapping')
        sources[ref['file']]=data; file_tokens[ref['file']]=tokens(data)
    def span(x):
        fid,start,end=origin_key(x);check(fid in sources,'unknown file')
        b=sources[fid];check(0<=start<=end<=len(b),'out-of-range span')
        try: before=b[:start].decode(); between=b[start:end].decode(); upto=b[:end].decode()
        except UnicodeDecodeError: raise Rejected('non-UTF8 boundary')
        return {'file':fid,'start':start,'end':end,'line':before.count('\n')+1,'column':len(before.rsplit('\n',1)[-1])+1,'end_line':upto.count('\n')+1,'end_column':len(upto.rsplit('\n',1)[-1])+1,'text':between}
    def exact_origin(observed,expected): check(span(observed)==expected,'source origin mismatch')
    allowed_rows={'header','source','diagnostic','complete','expression','statement','query','selector','token','function','equality','summary','reservation','type_syntax'}
    check(all(r['row'] in allowed_rows for r in rows),'unknown observation row')
    check(rows[0]['scope']==('single' if len(case['files'])==1 else 'project'),'scope mismatch')
    expected=case['expected']
    diagnostics=[r for r in rows if r['row']=='diagnostic']
    if 'diagnostics' in expected:
        check(rows[-1]['phase']=='diagnostic','wrong failure phase')
        check(len(diagnostics)==len(expected['diagnostics']),'diagnostic count/order')
        wc=wording[case['id']]
        for index,(r,e) in enumerate(zip(diagnostics,expected['diagnostics'])):
            d=r['diagnostic']; check(d['code']==e['code'] and d['stage']==e['stage'],'code/stage mismatch')
            p=d['primary']; exact_origin([p['file_id'],p['start'],p['end']],e['primary'])
            check(index==0,'unreviewed multiple wording diagnostics')
            check(d==wc['rendered']['json_object'],'full diagnostic object mismatch')
            check(r['human']==wc['rendered']['human'],'human bytes mismatch')
            check(r['json_line']==wc['rendered']['json_line'],'JSON bytes mismatch')
        check(not any(r['row'] in ('expression','summary','query') for r in rows),'partial AST escaped failure')
        return {'diagnostics':len(diagnostics),'phase':'lex-parse'}
    check(not diagnostics,'unexpected diagnostic')
    check(rows[-1]['phase']=='source-types','missing source-type completion')
    sums=[r for r in rows if r['row']=='summary'];check(len(sums)==1,'summary count'); summary=sums[0]
    for fid,data in sources.items():
        tape=[r for r in rows if r['row']=='token' and r['span'][0]==fid];cursor=0
        check(tape and tape[-1]['kind']=='Eof','lossless tape EOF')
        for r in tape:
            o=span(r['span']);check(o['start']==cursor,'lossless tape continuity');cursor=o['end']
            if o['text']in('[',']'):check(r['kind']=='Unsupported','public bracket token changed')
        check(cursor==len(data),'lossless tape source coverage')
    expr_rows=[r for r in rows if r['row']=='expression']; expr={}
    for r in expr_rows:
        check(r['kind'] in {'ArrayLiteral','IndexRead','ArrayLength','Group','FieldRead','Call','Bool','Unit','Number','Name','Not','Logical','Comparison','Arithmetic','StructLiteral'},'unknown expression kind')
        key=(r['file'],r['id']);check(key not in expr,'duplicate expression id');expr[key]=r;span(r['origin']);check(r['origin'][0]==r['file'],'expression wrong file')
    for fid in sources: check(sorted(i for(f,i)in expr if f==fid)==list(range(sum(1 for(f,i)in expr if f==fid))),'noncontiguous expression IDs')
    statements=[r for r in rows if r['row']=='statement']; target_ids={(r['file'],r['target']) for r in statements if r['kind']=='IndexAssign'}
    def child(r,id):
        key=(r['file'],id);check(key in expr,'invalid child edge');check(id<r['id'],'nonpostorder child edge');return expr[key]
    def lex_within(org):
        f,a,b=origin_key(org);return [(l,h,v) for l,h,v in file_tokens[f] if a<=l and h<=b]
    def same_tokens(org,expected_tokens):check([v for _,_,v in lex_within(org)]==expected_tokens,'source role token mismatch')
    for r in expr_rows:
        org=span(r['origin']); ts=lex_within(r['origin']);kind=r['kind'];check(ts or org['start']==org['end'],'empty expression role')
        if kind=='ArrayLiteral':
            check(ts[0][2]=='[' and ts[-1][2]==']','literal role boundary')
            ids=r['elements'];check(len(ids)==r['element_count']<=1024,'literal element count')
            check(len(ids)==len(set(ids)),'duplicate literal child')
            cursor=ts[0][1]
            for j,i in enumerate(ids):
                c=child(r,i); cs=span(c['origin']);check(cs['file']==org['file'] and cursor<=cs['start']<cs['end']<=ts[-1][0],'literal child containment/order')
                between=[v for l,h,v in ts if cursor<=l and h<=cs['start']]
                check(between==([] if j==0 else [',']),'literal child separator role');cursor=cs['end']
            tail=[v for l,h,v in ts if cursor<=l and h<=ts[-1][0]]
            check(tail in ([],[',']) if ids else tail==[],'literal tail role')
        elif kind=='Group':
            c=child(r,r['child']); cs=span(c['origin']);check(ts[0][2]=='(' and ts[-1][2]==')','group boundary role')
            check([v for l,h,v in ts if l<cs['start']]==['('] and [v for l,h,v in ts if h>cs['end']]==[')'],'group child role')
        elif kind=='IndexRead':
            base=span(r['base']);cs=span(child(r,r['index'])['origin']);check(re.fullmatch(r'[A-Za-z_][A-Za-z_0-9]*',base['text']) is not None,'base is not name')
            check(org['start']==base['start'] and org['file']==base['file']==cs['file'],'index base origin')
            check([v for l,h,v in ts if l<cs['start']]==[base['text'],'['] and [v for l,h,v in ts if h>cs['end']]==[']'],'index child role')
            check(r['role']==('store-target-wrapper' if (r['file'],r['id'])in target_ids else 'value'),'index assignment role')
        elif kind=='ArrayLength':
            base=span(r['base']);check(org['start']==base['start'] and org['file']==base['file'],'length base origin')
            check(re.fullmatch(r'[A-Za-z_][A-Za-z_0-9]*',base['text']) is not None,'length base name');same_tokens(r['origin'],[base['text'],'.','len','(',')'])
        elif kind=='Unit':same_tokens(r['origin'],['(',')'])
        elif kind=='Bool':check(org['text']in('true','false'),'boolean role')
        elif kind=='Number':check(re.fullmatch(r'-?\s*[0-9]+',org['text']) is not None,'number role')
    for r in statements:
        check(r['kind']=='IndexAssign','unexpected statement kind');fid=r['file'];check((fid,r['target'])in expr and (fid,r['value'])in expr,'statement root edge')
        t=expr[fid,r['target']];v=expr[fid,r['value']];check(t['kind']=='IndexRead','target direct kind')
        ts=span(t['origin']);vs=span(v['origin']);op=span(r['operator']);ss=span(r['origin'])
        check(ss['file']==ts['file']==op['file']==vs['file']==fid,'store origin file')
        check(ss['start']==ts['start']<ts['end']<=op['start']<op['end']<=vs['start']<vs['end']<=ss['end'],'store operand lexical roles')
        check(op['text']=='=','store operator role')
        tail=[x for x in lex_within(r['origin']) if x[0]>=vs['end']];check([x[2]for x in tail]==[';'],'store terminator role')
    def expr_projection(r):
        p={'kind':r['kind'],'origin':span(r['origin'])}
        for k in ('base','field'): 
            if k in r:p[k]=span(r[k])
        for k in ('index','child'):
            if k in r:p[k]=span(child(r,r[k])['origin'])
        if 'elements'in r:p.update(elements=[span(child(r,i)['origin'])for i in r['elements']],element_count=r['element_count'])
        if 'role'in r:p['role']=r['role']
        return p
    if 'ast'in expected:
        kinds={x['kind']for x in expected['ast']};actual=[expr_projection(r)for r in expr_rows if r['kind']in kinds]
        check(len(actual)==len(expected['ast']),'complete requested AST kind count')
        for e in expected['ast']:
            matches=[a for a in actual if a['kind']==e['kind'] and a['origin']==e['origin']];check(len(matches)==1,'missing/duplicate requested AST occurrence')
            for k,v in e.items():check(matches[0].get(k)==v,'AST field '+k)
    if 'statements'in expected:
        check(len(statements)==len(expected['statements']),'statement count')
        for r,e in zip(statements,expected['statements']):
            p={'kind':r['kind'],'origin':span(r['origin']),'operator':span(r['operator']),'target':span(expr[r['file'],r['target']]['origin']),'value':span(expr[r['file'],r['value']]['origin'])}
            check(p==e,'statement exact projection')
    type_rows=[r for r in rows if r['row']=='type_syntax']
    check(len({origin_key(r['origin'])for r in type_rows})==len(type_rows),'duplicate type syntax use')
    for r in type_rows:
        ts=[v for _,_,v in lex_within(r['origin'])];kind=r['kind']
        check(kind in ('Array','ArrayReference'),'unknown type syntax kind')
        if kind=='ArrayReference':
            prefix=['&','mut']if r['mutable']else['&'];check(ts[:len(prefix)]==prefix,'reference source role');ts=ts[len(prefix):]
        else:check(not r['mutable'],'array value carries mutability')
        element=['(',')']if r['element']=='unit'else[r['element']]
        check(r['element']in('unit','bool','i32'),'unknown scalar element')
        prefix=['[']+element+[';'];check(ts[:len(prefix)]==prefix and ts[-1:]==[']']and len(ts)==len(prefix)+2,'array type source role')
        length=ts[len(prefix)];check(re.fullmatch('[0-9]+',length)is not None,'length spelling role')
        check(0<=r['length']<=1024 and (length.lstrip('0')or'0')==str(r['length']),'syntax descriptor/source mismatch')
    queries=[r for r in rows if r['row']=='query']; selected=[]
    check(len({(origin_key(r['origin']),r['query_kind'])for r in queries})==len(queries),'duplicate query use')
    def value(v):
        v=dict(v)
        if v['tag']=='record':v.pop('record_id',None);v['declaration']=span(v['declaration'])
        if v['tag']=='reference':v['aggregate']=value(v['aggregate'])
        return v
    for e in expected.get('type_queries',[]):
        matches=[r for r in queries if span(r['origin'])==e['origin'] and r['query_kind']==e.get('query_kind','value')];check(len(matches)==1,'type query occurrence/position')
        check(value(matches[0]['value'])==e['value'],'checked descriptor mismatch')
        if e.get('query_kind')=='parameter':check(matches[0]['site']=='parameter','parameter query position')
        selected.append(matches[0])
    if 'identity'in expected:
        check([value(r['value'])for r in selected]==expected['identity']['descriptors'],'identity descriptor order')
        eq={tuple(sorted((origin_key(r['a']),origin_key(r['b'])))):r['equal'] for r in rows if r['row']=='equality'}
        matrix=[[eq[tuple(sorted((origin_key(a['origin']),origin_key(b['origin']))))]for b in selected]for a in selected]
        check(matrix==expected['identity']['equality_matrix'],'actual type equality mismatch')
    if 'selectors'in expected:
        sel=[r['owned']for r in rows if r['row']=='selector'];check(sel==expected['selectors']['ast'],'AST selectors');check(summary['owned']==expected['selectors']['source_owner'],'SourceOwner selector')
    if 'route'in expected:check(summary['owned']==(expected['route']!='scalar'),'whole source route selection')
    if 'entry'in expected:
        entry=span(summary['entry']);check(entry['file']==expected['entry']['file'] and entry['text']==expected['entry']['name'],'original root entry')
    for k,v in expected.get('inventory',{}).items():
        if not k.startswith('later_'):check(summary[k]==v,'inventory '+k)
    check(summary['array_literal_elements']==sum(len(r.get('elements',[]))for r in expr_rows),'actual literal inventory')
    check(summary['array_store_target_wrappers']==len(target_ids),'target inventory')
    check(summary['ast_expression_count']==len(expr_rows),'expression inventory')
    check(summary['ast_payload']<=288*summary['syntax_nodes'],'288N bound')
    check(summary['validation_visits']<=11*summary['syntax_nodes']+2*summary['non_eof_tokens']+4*summary['modules'],'validation work bound')
    if 'array_node_count'in expected:check(summary['array_node_count']==expected['array_node_count'],'array count')
    if 'nominal_array_rows'in expected:check(summary['records']-summary['declared_records']==expected['nominal_array_rows'],'nominal row growth')
    if 'parameters'in expected:check(max(r['parameters']for r in rows if r['row']=='function')==expected['parameters'],'parameter count')
    if 'call_arguments'in expected:check(max(r['arguments']for r in expr_rows if r['kind']=='Call')==expected['call_arguments'],'argument count')
    return {'phase':'source-types','expressions':len(expr_rows),'queries':len(selected),'rows':len(rows)}

def main():
    ap=argparse.ArgumentParser();ap.add_argument('contracts',type=pathlib.Path);ap.add_argument('observations',type=pathlib.Path);ap.add_argument('output',type=pathlib.Path);a=ap.parse_args()
    cases=json.loads((a.contracts/'cases.json').read_text())['cases'];w=json.loads((a.contracts/'diagnostic-wording-v1.json').read_text());wording={x['id']:x for x in w['cases']}
    report={'schema':'oxid-unit3a-independent-comparison-v1','cases':[],'deferred':[x['id']for x in cases if x['first_observation_slice']!='3A']}
    selected=[x for x in cases if x['first_observation_slice']=='3A'];check(len(selected)==58,'authority applicable case count changed')
    validate_artifacts(a.observations,[x['id']for x in selected])
    for case in selected:
        result={'id':case['id']}
        try:
            path=a.observations/(case['id']+'.jsonl')
            if not path.exists():raise Rejected('observer incomplete: '+(a.observations/(case['id']+'.error')).read_text())
            data=path.read_bytes();rows=[json.loads(x)for x in data.splitlines()];check(len(data)<=rows[0]['byte_limit']<=16*1024*1024,'serialized bytes limit')
            check(rows[-1]['bytes_before_footer']==len(b'\n'.join(data.splitlines()[:-1]))+1,'byte completion count')
            result.update(compare_case(case,rows,a.contracts,wording));result['status']='PASS';result['observation_sha256']=digest(data)
        except Exception as e:result.update(status='FAIL',reason=str(e))
        report['cases'].append(result)
    report['pass']=sum(x['status']=='PASS'for x in report['cases']);report['fail']=len(selected)-report['pass'];a.output.write_text(json.dumps(report,indent=2)+'\n');print(json.dumps({'pass':report['pass'],'fail':report['fail'],'deferred':len(report['deferred'])}))
    raise SystemExit(bool(report['fail']))
if __name__=='__main__':main()
