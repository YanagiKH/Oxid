#!/usr/bin/env python3
"""Frozen comparator negative controls; actual observations are mutated only in memory."""
import copy,json,pathlib,sys
from compare import compare_case,Rejected
contracts,obs,out=map(pathlib.Path,sys.argv[1:])
cases={c['id']:c for c in json.loads((contracts/'cases.json').read_text())['cases']}
wording={c['id']:c for c in json.loads((contracts/'diagnostic-wording-v1.json').read_text())['cases']}
results=[]
def run(name,cid,change):
 rows=[json.loads(x)for x in (obs/(cid+'.jsonl')).read_text().splitlines()]
 compare_case(cases[cid],rows,contracts,wording)
 change(rows)
 try:compare_case(cases[cid],rows,contracts,wording)
 except Exception as e:results.append({'id':name,'rejected':True,'reason':str(e)})
 else:results.append({'id':name,'rejected':False})
def row(rs,kind,**k):return next(r for r in rs if r['row']==kind and all(r.get(a)==b for a,b in k.items()))
a='grouped-complete-access-and-index'
run('wrong-valid-file', 'cross-file-array-identity',lambda rs:row(rs,'query',query_kind='value')['origin'].__setitem__(0,1))
run('swapped-elements',a,lambda rs:row(rs,'expression',kind='ArrayLiteral')['elements'].reverse())
run('duplicated-element',a,lambda rs:row(rs,'expression',kind='ArrayLiteral')['elements'].__setitem__(1,row(rs,'expression',kind='ArrayLiteral')['elements'][0]))
run('missing-element',a,lambda rs:row(rs,'expression',kind='ArrayLiteral')['elements'].pop())
run('wrong-group-child',a,lambda rs:row(rs,'expression',kind='Group').__setitem__('child',0))
run('lost-store-target-role',a,lambda rs:row(rs,'expression',kind='IndexRead',role='store-target-wrapper').__setitem__('role','value'))
run('index-root-as-scalar',a,lambda rs:row(rs,'expression',kind='IndexRead').__setitem__('kind','Number'))
run('unknown-expression-tag',a,lambda rs:row(rs,'expression').__setitem__('kind','FutureArray'))
run('wrong-token-tag',a,lambda rs:next(r for r in rs if r['row']=='token' and r['kind']=='Unsupported').__setitem__('kind','LeftBracket'))
run('query-reference-kind','child-route-shared-parameter',lambda rs:next(r for r in rs if r['row']=='query' and r['value']['tag']=='reference')['value'].__setitem__('kind','exclusive'))
run('parameter-query-site','child-route-parameter',lambda rs:next(r for r in rs if r['row']=='query' and r['query_kind']=='parameter').__setitem__('site','annotation'))
run('structural-type-length',a,lambda rs:next(r for r in rs if r['row']=='query' and r['value']['tag']=='fixed_array')['value'].__setitem__('length',3))
run('incomplete-footer',a,lambda rs:rs[-1].__setitem__('complete',False))
run('source-bytes',a,lambda rs:row(rs,'source').__setitem__('text',row(rs,'source')['text']+' '))
run('source-file-id-duplicate','cross-file-array-identity',lambda rs:next(r for r in rs if r['row']=='source' and r['file']==1).__setitem__('file',0))
run('unit-literal-origin-is-type-unit','scalar-unit-length-1',lambda rs:row(rs,'expression',kind='Unit').__setitem__('origin',[0,27,29]))
run('source-allocation-identity-duplicate','cross-file-array-identity',lambda rs:next(r for r in rs if r['row']=='source' and r['file']==1).__setitem__('identity',row(rs,'source',file=0)['identity']))
run('source-allocation-identity-zero',a,lambda rs:row(rs,'source').__setitem__('identity',0))
report={'schema':'oxid-unit3a-comparator-sensitivity-v1','controls':results,'pass':sum(r['rejected']for r in results),'total':len(results)}
out.write_text(json.dumps(report,indent=2)+'\n');print(json.dumps({'pass':report['pass'],'total':len(results)}));sys.exit(report['pass']!=report['total'])
