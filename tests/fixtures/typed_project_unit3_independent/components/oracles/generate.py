#!/usr/bin/env python3
"""Freeze Unit3 source/request/expectations without invoking Oxid or LLVM.

Only this package and declared frozen Unit2/design inputs are read. No candidate
checkout, observer output, actual result or production resolver is an input.
"""
from __future__ import annotations
import contextlib, copy, gzip, hashlib, io, itertools, json, re, runpy, sys
from collections import Counter
from pathlib import Path

sys.dont_write_bytecode = True
HERE = Path(__file__).resolve().parent
BASE = HERE.parent
REPO = BASE / 'oxid-typed-index-ci-fix'
DESIGN = BASE / 'typed-project-unit3-design'
REVIEW = BASE / 'typed-project-unit3-independent-review'
sys.path.insert(0, str(HERE / 'vendor'))
import owned_source_model as m

def sha(data): return hashlib.sha256(data).hexdigest()
def dump(path, obj): path.write_text(json.dumps(obj, indent=2, ensure_ascii=False) + '\n')
def walk(n):
    yield n
    for v in n.data.values():
        if isinstance(v,m.Node): yield from walk(v)
        elif isinstance(v,tuple):
            for x in v:
                if isinstance(x,m.Node): yield from walk(x)
                elif isinstance(x,tuple):
                    for y in x:
                        if isinstance(y,m.Node): yield from walk(y)

def scalar_schedule(checked, dynamic):
    """Scalar contract: root 1+slots, call 1+arity+callee slots, other op 1.

    S from the independent tagged model is parameters + lexical scalar bindings
    + one result slot per source expression + implicit unit return expressions.
    It equals scalar locals+places. Owned A/C/X charges are deliberately unused.
    """
    assert not checked.program.records
    frames={f.name:m.TemplateScheduler(checked).frames[f.name]['S'] for f in checked.program.functions}
    items=[]; spent=0
    def add(op,cost,origin,event):
        nonlocal spent
        items.append(dict(index=len(items),operation=op,cost=cost,start_fuel=spent,
                          end_fuel=spent+cost,origin=origin,semantic_event=event)); spent+=cost
    if dynamic['events']:
        add('Root',1+frames['main'],f"function.{checked.function_index['main']}.name",-1)
    for e in dynamic['events']:
        kind=e['kind']
        if kind in ('call-open','prepare-scalar'): continue
        if kind=='call-invoke':
            fn=checked.functions[e['callee']]
            add('Call',1+len(fn.parameters)+frames[fn.name],e['origin'],e['index'])
        elif kind in ('return-scalar','return-unit'):
            add('Scalar' if kind=='return-unit' else 'Return',1,e['origin'],e['index'])
        elif kind in ('scalar-expression','let-scalar','store-scalar','branch','goto','lazy-branch','lazy-goto','lazy-merge'):
            op={'branch':'Branch','goto':'Goto','lazy-branch':'Branch','lazy-goto':'Goto','lazy-merge':'BoolMerge'}.get(kind,'Scalar')
            add(op,1,e['origin'],e['index'])
        else: raise ValueError(('SCALAR_MODEL_INCOMPLETE',kind))
    return dict(total_fuel=spent,items=items,frames=frames,complete_execution=dynamic['failure'] is None,
                authority='Independent scalar contract scheduler; never owned TemplateScheduler X')

def patch_fragment(source,start,end,edits):
    edits=sorted(edits,key=lambda e:(e[0],e[1]))
    out=[];p=start
    for a,b,value in edits:
        assert start<=a<=b<=end and a>=p,(start,end,edits)
        out += [source[p:a],value];p=b
    out.append(source[p:end])
    def point(pos,edge):
        shift=0
        for a,b,value in edits:
            if a==b:
                if pos>=a: shift+=len(value)
            elif pos>=b: shift+=len(value)-(b-a)
            elif pos==a: pass
            elif a<pos<b: raise ValueError(('origin inside replaced token',pos,a,b))
        return pos-start+shift
    return ''.join(out),point

def projectify(program,homes,record_homes,spelling='absolute',source_names=None,private_fields=False,
               explicit_files=None,prefix='',newline='\n'):
    """Module model over explicit declarations. Does not parse compiler output.

    Homes map unique body labels to symbolic modules. Private fields are used
    only by their declaring module in valid inputs. Direct imports bind original
    declarations atomically; the import table contains no allocated IDs.
    """
    source_names=source_names or {}
    modules=explicit_files or ['root','left','right']
    fid={name:i for i,name in enumerate(modules)}
    paths={name:('main.ox' if name=='root' else name+'.ox') for name in modules}
    rendered=m.render(program); src=rendered.source
    files={name:prefix for name in modules}
    for name in modules[1:]:files['root']+='mod '+name+';\n'
    imports={name:{} for name in modules}; replacements={}; declaration_rows=[]
    function_ids={}; record_ids={}
    for module in modules:
        for fn in program.functions:
            if homes[fn.name]==module:function_ids[fn.name]=len(function_ids)
        for record in program.records:
            if record_homes[record.name]==module:record_ids[record.name]=len(record_ids)
    def absolute(module,name):return 'crate::'+('' if module=='root' else module+'::')+name
    def reference(requester,kind,label):
        home=(homes if kind=='function' else record_homes)[label]
        name=source_names.get(label,label)
        if home==requester:return name
        if kind=='record' or spelling=='absolute':return absolute(home,name)
        alias=label+'_import'
        imports[requester][alias]=(home,name,kind,label)
        return alias
    # Edits are independently tied to tagged source tokens and declared targets.
    for i,fn in enumerate(program.functions):
        module=homes[fn.name];key=f'function.{i}';a,b=rendered.origins[key]
        edits=[]
        def token(k,value):
            x,y=rendered.origins[k]
            if src[x:y]!=value:edits.append((x,y,value))
        token(key+'.name',source_names.get(fn.name,fn.name))
        for j,(_,ty) in enumerate(fn.parameters):
            rec=m.ref_type(ty)[1] if m.ref_type(ty) else ty
            if rec not in m.SCALARS:
                token(key+f'.parameter.{j}.type'+('.referent' if m.ref_type(ty) else ''),reference(module,'record',rec))
        if fn.result not in m.SCALARS:token(key+'.result',reference(module,'record',fn.result))
        for node in walk(fn.body):
            if node.tag=='call':token(node.key+'.callee',reference(module,'function',node.data['function']))
            if node.tag=='literal':token(node.key+'.record',reference(module,'record',node.data['record']))
            if node.tag=='let' and node.data['annotation'] and node.data['annotation'] not in m.SCALARS:
                token(node.key+'.annotation',reference(module,'record',node.data['annotation']))
        replacements[key]=edits
    for module in modules:
        for alias,(home,name,kind,label) in sorted(imports[module].items()):
            files[module]+='use '+absolute(home,name)+' as '+alias+';\n'
    origins={};bijection=[]
    for module in modules:
        decls=[('record',i,r) for i,r in enumerate(program.records) if record_homes[r.name]==module]
        decls += [('function',i,f) for i,f in enumerate(program.functions) if homes[f.name]==module]
        for kind,index,decl in decls:
            key=f'{kind}.{index}';start,end=rendered.origins[key]
            edits=list(replacements.get(key,[]))
            if kind=='record':
                x,y=rendered.origins[key+'.name'];edits.append((x,y,source_names.get(decl.name,decl.name)))
                if not private_fields:
                    for j,_ in enumerate(decl.fields):
                        x,_=rendered.origins[f'{key}.field.{j}.name'];edits.append((x,x,'pub '))
            text,translate=patch_fragment(src,start,end,edits)
            lead='pub ' if module!='root' or kind=='record' else ''
            offset=len(files[module])+len(lead)
            files[module]+=lead+text+'\n'
            for k,(a,b) in rendered.origins.items():
                if start<=a<=b<=end:
                    assert k not in origins
                    origins[k]=[fid[module],offset+translate(a,'start'),offset+translate(b,'end')]
            actual_name=source_names.get(decl.name,decl.name)
            row=dict(kind=kind,symbol={'module':'crate' if module=='root' else 'crate::'+module,'name':actual_name},
                     model_label=decl.name,id=(function_ids if kind=='function' else record_ids)[decl.name],
                     file=fid[module],name_origin_key=key+'.name')
            if kind=='function':row.update(parameters=list(decl.parameters),result=decl.result)
            else:row['fields']=[dict(id=[record_ids[decl.name],j],name=n,type=t,origin_key=f'{key}.field.{j}.name') for j,(n,t) in enumerate(decl.fields)]
            declaration_rows.append(row)
            bijection.append(dict(model_declaration=key,model_span=[start,end],file=fid[module],source_start=offset,
                                  original_token_edits=[dict(start=a,end=b,text=value) for a,b,value in edits],public_prefix=bool(lead)))
    # Convert character offsets to UTF-8 byte offsets after newline translation.
    converted={}
    for module,text in files.items():
        converted[paths[module]]=text.replace('\n',newline)
    for k,(file,a,b) in list(origins.items()):
        original=files[modules[file]]
        origins[k]=[file,len(original[:a].replace('\n',newline).encode()),len(original[:b].replace('\n',newline).encode())]
    for row in declaration_rows:
        row['span']=origins[row['name_origin_key']]
        for field in row.get('fields',[]):field['span']=origins[field['origin_key']]
    return dict(files=converted,origins=origins,model_render=rendered,declarations=declaration_rows,
                function_ids=function_ids,record_ids=record_ids,entry=function_ids.get('main'),
                imports=[dict(requester=fid[mod],alias=alias,target_kind=kind,target_id=(function_ids if kind=='function' else record_ids)[label],
                              symbol={'module':'crate' if home=='root' else 'crate::'+home,'name':name})
                         for mod in modules for alias,(home,name,kind,label) in sorted(imports[mod].items())],
                bijection=bijection,modules=[dict(id=fid[x],file=fid[x],path=paths[x],symbol='crate' if x=='root' else 'crate::'+x) for x in modules])

def remap_spans(obj,rendered,origins):
    # Existing correspondence facts carry byte pairs. Every such pair must map
    # uniquely to one source origin, even for same-spelled names in two files.
    pairs={}
    for k,pair in rendered.origins.items():
        if k in origins:
            old=pairs.setdefault(tuple(pair),origins[k]);assert old==origins[k]
    def visit(value,key=''):
        if isinstance(value,dict):return {k:visit(v,k) for k,v in value.items()}
        if isinstance(value,list):
            if len(value)==2 and all(type(x)==int for x in value) and (key=='span' or key.endswith('_span') or key in ('primary','charge','cause')):
                return pairs[tuple(value)]
            return [visit(x,key) for x in value]
        return value
    return visit(obj)

def module_correspondence(program,project,facts):
    """Independent flat-module permission/path model for this finite corpus.

    Original declaration identities were authored before source generation. This
    separately checks the real source token at every call/constructor against
    the explicit original/import bindings. It invokes no production resolver.
    """
    rows=project['declarations'];functions={r['model_label']:r for r in rows if r['kind']=='function'}
    records={r['model_label']:r for r in rows if r['kind']=='record'}
    filebytes=[s.encode() for s in project['files'].values()]
    def text(span):f,a,b=span;return filebytes[f][a:b].decode()
    def public(row,kind):
        f,a,b=row['span'];return re.search(rb'pub\s+'+kind.encode()+rb'\s+$',filebytes[f][:a]) is not None
    def visible(target,caller):
        home=target['symbol']['module'];requester=caller['symbol']['module']
        return public(target,'fn' if target['kind']=='function' else 'struct') or requester==home or requester.startswith(home+'::')
    def resolve_token(token,kind,caller):
        declarations=functions if kind=='function' else records
        if token.startswith('crate::'):
            bits=token.split('::');module='::'.join(bits[:-1]);name=bits[-1]
            candidates=[r for r in declarations.values() if r['symbol']=={'module':module,'name':name}]
        else:
            candidates=[r for r in declarations.values() if r['symbol']=={'module':caller['symbol']['module'],'name':token}]
            for imp in project['imports']:
                if imp['requester']==caller['file'] and imp['alias']==token:
                    targets=imp.get('targets',[(imp.get('target_kind'),imp.get('target_id'))])
                    for k,i in targets:
                        if k==kind:candidates += [r for r in declarations.values() if r['id']==i]
        assert len(candidates)==1,('independent source binding is not unique',kind,token,caller,candidates)
        selected=candidates[0];assert visible(selected,caller),('inaccessible authored positive',token)
        return selected
    observations=[]
    for call in facts['calls']:
        caller=functions[call['function']];token=text(project['origins'][call['identity']+'.callee'])
        selected=resolve_token(token,'function',caller)
        assert selected['id']==call['global_target']
        observations.append(dict(kind='call',span=project['origins'][call['identity']+'.callee'],target=selected['id']))
    def field_permission(record_index,field_index,caller):
        record=records[program.records[record_index].name];field=record['fields'][field_index]
        f,a,b=field['span'];is_pub=re.search(rb'pub\s+$',filebytes[f][:a]) is not None
        home=record['symbol']['module'];requester=caller['symbol']['module']
        assert is_pub or requester==home or requester.startswith(home+'::'),('inaccessible positive field',record,caller)
    for projection in facts['projections']:
        r=int(projection['record_identity'].split('.')[1]);field=int(projection['field_identity'].split('.')[-1])
        field_permission(r,field,functions[projection['function']])
    for store in facts['stores']:
        if store['kind']=='write':
            projection=store['projection'];r=int(projection['record_identity'].split('.')[1]);f=int(projection['field_identity'].split('.')[-1])
            field_permission(r,f,functions[store['function']])
    for literal in facts['literals']:
        caller=functions[literal['function']];record=resolve_token(text(literal['record_span']),'record',caller)
        assert record['id']==literal['global_record_id']
        ri=int(literal['record_identity'].split('.')[1])
        for i,_ in enumerate(program.records[ri].fields):field_permission(ri,i,caller)
    return dict(domain='flat module trees explicitly declared by the finite corpus; direct originals/imports only',
                successful_call_bindings=len(observations),call_bindings=observations,
                checked_projection_count=len(facts['projections']),checked_constructor_count=len(facts['literals']),
                module_boundaries_add_runtime_operations=False)

def finish_case(identifier,family,program,project,axes=None,expected_result=None,native=False):
    rendered=project['model_render']
    checked=m.NamesTypes(program).check()
    static=m.static_check(checked,rendered)
    assert static['accepted'],(identifier,static)
    dynamic=m.Machine(checked).run()
    assert dynamic['failure'] is None,(identifier,dynamic['failure'])
    route='owned' if program.records else 'scalar'
    schedule=m.TemplateScheduler(checked).schedule(dynamic) if route=='owned' else scalar_schedule(checked,dynamic)
    if expected_result is not None:assert dynamic['result']==expected_result,(identifier,dynamic['result'],expected_result)
    origins=project['origins']
    for e in dynamic['events']:assert e['origin'] in origins,(identifier,e)
    for e in schedule['items']:assert e['origin'] in origins,(identifier,e)
    facts=remap_spans(m.correspondence_facts(checked,rendered),rendered,origins)
    for call in facts['calls']:
        call['global_target']=project['function_ids'][call['callee']]
        call['global_caller']=project['function_ids'][call['function']]
    for rec in facts['records']:
        rec['global_id']=project['record_ids'][rec['name']]
    for group in ('projections','literals'):
        for fact in facts[group]:
            idx=int(fact['record_identity'].split('.')[1]);fact['global_record_id']=project['record_ids'][program.records[idx].name]
    module_model=module_correspondence(program,project,facts)
    budget_cases=[]
    if route=='owned':
        selected={schedule['total_fuel'],schedule['total_fuel']-1}
        # One first and one final representative per prescribed charge class;
        # nested Invoke is explicitly included, no arbitrary prefix sampling.
        classes=('WriteField','Invoke','ReturnScalar','ReturnOwned','StorageEnd','PrepareBorrow')
        for operation in classes:
            candidates=[x for x in schedule['items'] if x['operation']==operation]
            if candidates:
                for item in (candidates[0],candidates[-1]):selected.update([item['end_fuel']-1,item['end_fuel']])
        nested_calls={x['identity'] for x in facts['calls'] if x['parent'] is not None}
        for item in schedule['items']:
            if item['operation']=='Invoke' and item['origin'] in nested_calls:
                selected.update([item['end_fuel']-1,item['end_fuel']]);break
        for budget in sorted(selected):
            result=m.TemplateScheduler.at_budget(schedule,dynamic,budget)
            result['paid_operation_count']=sum(x['end_fuel']<=budget for x in schedule['items'])
            denied=next((x for x in schedule['items'] if x['end_fuel']>budget),None)
            result['complete_semantic_event_prefix_length']=max(0,denied['semantic_event']) if denied else len(dynamic['events'])
            if result.get('failure'):result['failure']['span']=origins[result['failure']['origin']]
            budget_cases.append(dict(budget=budget,expected=result))
    else:
        for budget in (schedule['total_fuel']-1,schedule['total_fuel']):
            denied=next((x for x in schedule['items'] if x['end_fuel']>budget),None)
            budget_cases.append(dict(budget=budget,expected=dict(result=None if denied else dynamic['result'],
                failure=(dict(code='E0601',operation=denied['operation'],origin=denied['origin'],span=origins[denied['origin']]) if denied else None))))
    expected=dict(id=identifier,family=family,axes=axes or {},evidence_kind='MODEL_ONLY_FROZEN_BEFORE_CANDIDATE_EXECUTION',
                  route=route,entry=project['entry'],function_count=len(program.functions),record_count=len(program.records),
                  modules=project['modules'],declarations=project['declarations'],imports=project['imports'],
                  origins=origins,source_to_model_bijection=project['bijection'],correspondence=facts,
                  independent_module_model=module_model,
                  expected=dict(check='accept',result=dynamic['result'],result_type=dynamic['result_type']),
                  dynamic=dynamic,schedule=schedule,budget_cases=budget_cases,
                  native_required=native,source_authority='Independent tagged source with explicit module/declaration/field identity mapping',
                  static_state_counts={n:{k:v for k,v in data.items() if k in ('states','points')} for n,data in static['functions'].items()})
    if native:
        value=dynamic['result'];printed=('true' if value else 'false') if type(value)==bool else '()' if value is None else str(value)
        expected['native_expected']=dict(compile='success',exit_status=0,stdout=printed+'\n',stderr='',opt_level=0,
            platform='Linux x86_64',llvm_version='19.1.7',run_in_fresh_directory_with_sources_unavailable=True)
    return expected,project['files']

def scalar_family(placement,spelling,ordinal,result):
    b=m.Builder();caller='root' if placement=='root_child' else 'left';target='left' if placement=='root_child' else 'right' if placement=='sibling' else 'root'
    if result=='i32':
        leaf=m.Function('target',(('n','i32'),),'i32',b.block(b.let('i',b.i(0),True),b.let('sum',b.i(0),True),
            b.loop(b.op(b.v('i'),'<',b.v('n')),b.block(b.store('i',b.op(b.v('i'),'+',b.i(1))),
            b.iff(b.op(b.v('i'),'==',b.i(2)),b.block(b.cont())),b.store('sum',b.op(b.v('sum'),'+',b.v('i'))),
            b.iff(b.op(b.v('sum'),'>',b.i(6)),b.block(b.brk())))),b.ret(b.v('sum'))))
        body=b.block(b.ret(b.op(b.call('target',b.i(4)),'+',b.i(2))));want=10
    else:
        leaf=m.Function('target',(('flag','bool'),),'bool',b.block(b.ret(b.op(b.v('flag'),'||',b.b(True)))))
        body=b.block(b.ret(b.op(b.call('target',b.b(False)),'||',b.call('trap'))));want=True
    main=m.Function('main',(),result,body if caller=='root' else b.block(b.ret(b.call('bridge'))))
    sentinel=m.Function('sentinel',leaf.parameters,result,b.block(b.ret(b.i(91) if result=='i32' else b.b(False))))
    functions=[main];homes={'main':'root','target':target,'sentinel':'right' if target!='right' else 'left'}
    if caller!='root':functions.append(m.Function('bridge',(),result,body));homes['bridge']=caller
    functions+=[leaf,sentinel]
    if result=='bool':
        functions.append(m.Function('trap',(),'bool',b.block(b.ret(b.op(b.op(b.i(2147483647),'+',b.i(1)),'>',b.i(0))))));homes['trap']=target
    # Root helper ordinal control is independent of child same-spelling sentinel.
    if ordinal:
        helper=m.Function('root_prefix',(),result,b.block(b.ret(b.i(97) if result=='i32' else b.b(False))))
        functions.insert(0,helper);homes['root_prefix']='root'
    # child->ancestor has root target after main; root-main ordinal is still 0/1.
    program=m.Program((),tuple(functions));names={'target':'helper','sentinel':'helper'}
    project=projectify(program,homes,{},spelling,names)
    assert project['entry']==ordinal
    return program,project,want

def owned_family(placement,spelling,record_home,template):
    b=m.Builder();caller='root' if placement=='root_child' else 'left';target='left' if placement=='root_child' else 'right' if placement=='sibling' else 'root'
    home='root' if record_home=='root' else 'state'
    functions=[];homes={};record=m.Record('Cell',(('value','i32'),))
    def fn(name,params,result,body,module=target):
        f=m.Function(name,params,result,body);functions.append(f);homes[name]=module;return f
    if template=='shared_nested':
        fn('target',(('p','&Cell'),('n','i32')),'i32',b.block(b.ret(b.op(b.f('p','value'),'+',b.v('n')))))
        fn('read',(('p','&Cell'),),'i32',b.block(b.ret(b.f('p','value'))))
        body=b.block(b.let('x',b.lit('Cell',('value',b.i(4)))),b.ret(b.call('target',b.borrow('x'),b.call('read',b.borrow('x')))));want=8
    elif template=='exclusive_forward':
        fn('bump',(('p','&mut Cell'),),'()',b.block(b.write('p','value',b.op(b.f('p','value'),'+',b.i(1))),b.ret()))
        fn('target',(('p','&mut Cell'),),'i32',b.block(b.discard(b.call('bump',b.borrow('p',True,True))),
            b.discard(b.call('bump',b.borrow('p',True,True))),b.ret(b.f('p','value'))))
        body=b.block(b.let('x',b.lit('Cell',('value',b.i(4))),True),b.discard(b.call('target',b.borrow('x',True))),b.ret(b.call('target',b.borrow('x',True))));want=8
    else:
        fn('make',(('n','i32'),),'Cell',b.block(b.ret(b.lit('Cell',('value',b.v('n'))))))
        fn('target',(('p','Cell'),),'Cell',b.block(b.ret(b.v('p'))))
        fn('finish',(('p','Cell'),),'i32',b.block(b.ret(b.f('p','value'))))
        if template=='owned_staging':
            body=b.block(b.discard(b.call('make',b.i(90))),b.let('x',b.call('make',b.i(4))),
                b.let('y',b.call('target',b.call('target',b.v('x')))),b.ret(b.call('finish',b.v('y'))));want=4
        else:
            body=b.block(b.let('total',b.i(0),True),b.let('i',b.i(0),True),
                b.loop(b.op(b.v('i'),'<',b.i(3)),b.block(b.let('a',b.call('make',b.v('i'))),b.let('z',b.call('target',b.v('a'))),
                    b.store('i',b.op(b.v('i'),'+',b.i(1))),b.iff(b.op(b.v('i'),'==',b.i(1)),b.block(b.cont())),
                    b.iff(b.op(b.v('i'),'==',b.i(3)),b.block(b.brk())),
                    b.store('total',b.op(b.v('total'),'+',b.call('finish',b.v('z')))))),b.ret(b.v('total')));want=1
    core=m.Function('main' if caller=='root' else 'bridge',(),'i32',body);functions.insert(0,core);homes[core.name]=caller
    if caller!='root':functions.insert(0,m.Function('main',(),'i32',b.block(b.ret(b.call('bridge')))));homes['main']='root'
    # Same-spelled target in a different module with identical signature and a
    # distinct scalar/payload effect is a correspondence, not raw-shape control.
    twin=next(f for f in functions if f.name=='target')
    if template=='shared_nested':twin_body=b.block(b.ret(b.i(91)))
    elif template=='exclusive_forward':twin_body=b.block(b.ret(b.i(91)))
    else:twin_body=b.block(b.ret(b.lit('Cell',('value',b.i(91)))))
    functions.append(m.Function('sentinel',twin.parameters,twin.result,twin_body));homes['sentinel']='right' if target!='right' else 'left'
    program=m.Program((record,),tuple(functions))
    project=projectify(program,homes,{'Cell':home},spelling,{'target':'helper','sentinel':'helper'},explicit_files=['root','left','right']+(['state'] if home=='state' else []))
    return program,project,want

def section_files(title):
    text=(DESIGN/'pilots.md').read_text().split('## '+title,1)[1].split('\n## ',1)[0]
    return dict(re.findall(r'`([^`]+\.ox)`:\s*```text\n(.*?)```',text,re.S))

def bind_authored_files(program,files,homes,record_homes,source_names=None,callee_aliases=None):
    """Check a full declaration/token bijection to exact reviewed pilot bytes.

    This accepts only declared function-label/callee renames and pub tokens.
    All other tokens, including Groups and operators, must be identical.
    """
    source_names=source_names or {};callee_aliases=callee_aliases or {}
    project=projectify(program,homes,record_homes,source_names=source_names,explicit_files=['root']+[p[:-3] for p in files if p!='main.ox'])
    rendered=m.render(program);actual={};token_re=re.compile(r'[A-Za-z_][A-Za-z_0-9]*|[0-9]+|\S')
    for file,(path,source) in enumerate(files.items()):
        for match in re.finditer(r'(?m)^(?:pub )?(fn|struct) ([A-Za-z_][A-Za-z0-9_]*)\b',source):
            start=match.start();brace=source.index('{',match.end());depth=0
            for end in range(brace,len(source)):
                depth+=(source[end]=='{')-(source[end]=='}')
                if depth==0:break
            symbol=('root' if path=='main.ox' else path[:-3],match.group(2))
            assert symbol not in actual
            actual[symbol]=(file,source,start,end+1)
    boundaries={};bijection=[]
    for kind,decls,home_map in [('record',program.records,record_homes),('function',program.functions,homes)]:
        for i,decl in enumerate(decls):
            key=f'{kind}.{i}';a,b=rendered.origins[key]
            symbol=(home_map[decl.name],source_names.get(decl.name,decl.name));file,source,x,y=actual[symbol]
            mt=list(token_re.finditer(rendered.source[a:b]));pt=[t for t in token_re.finditer(source[x:y]) if t.group()!='pub']
            assert len(mt)==len(pt),(symbol,len(mt),len(pt))
            replacements={decl.name:source_names.get(decl.name,decl.name),**callee_aliases.get(decl.name,{})}
            for left,right in zip(mt,pt):
                assert replacements.get(left.group(),left.group())==right.group(),(symbol,left.group(),right.group())
                boundaries[a+left.start()]=(file,len(source[:x+right.start()].encode()))
                boundaries[a+left.end()]=(file,len(source[:x+right.end()].encode()))
            bijection.append(dict(model_declaration=key,model_label=decl.name,symbol=list(symbol),file=file,
                                  exact_tokens_equal_after_declared_renames=True,renames=replacements))
    origins={}
    for key,(a,b) in rendered.origins.items():
        assert a in boundaries and b in boundaries,(key,a,b)
        fa,aa=boundaries[a];fb,bb=boundaries[b];assert fa==fb
        origins[key]=[fa,aa,bb]
    project.update(files=files,origins=origins,bijection=bijection)
    for row in project['declarations']:
        row['span']=origins[row['name_origin_key']]
        for field in row.get('fields',[]):field['span']=origins[field['origin_key']]
    # Imports are source-declared separately; exact call targets still come from
    # the body symbol mapping, never from the candidate resolver.
    project['imports']=[]
    for file,(path,source) in enumerate(files.items()):
        for match in re.finditer(r'use crate::(?:(\w+)::)?(\w+)(?: as (\w+))?;',source):
            module,name,alias=match.groups();module=module or 'root';alias=alias or name
            targets=[]
            for kind,ds,hm,ids in [('function',program.functions,homes,project['function_ids']),('record',program.records,record_homes,project['record_ids'])]:
                targets += [(kind,ids[d.name]) for d in ds if (hm[d.name],source_names.get(d.name,d.name))==(module,name)]
            assert targets,(path,match.group())
            project['imports'].append(dict(requester=file,alias=alias,targets=targets))
    return project

def scalar_pilot():
    b=m.Builder()
    helper=m.Function('helper',(),'i32',b.block(b.ret(b.i(99))))
    main=m.Function('main',(),'i32',b.block(b.ret(b.op(b.call('sum',b.i(4)),'+',b.call('choose',b.b(False),b.i(7),b.i(2))))))
    choose=m.Function('choose',(('flag','bool'),('first','i32'),('second','i32')),'i32',b.block(b.iff(b.v('flag'),b.block(b.ret(b.v('first'))),b.block(b.ret(b.v('second'))))))
    inc=m.Function('inc',(('value','i32'),),'i32',b.block(b.ret(b.op(b.v('value'),'+',b.i(1)))))
    child=m.Function('child_main',(('value','i32'),),'i32',b.block(b.ret(b.op(b.v('value'),'+',b.i(1000)))))
    total=m.Function('sum',(('n','i32'),),'i32',b.block(b.let('i',b.i(0),True),b.let('total',b.i(0),True),
        b.loop(b.op(b.v('i'),'<',b.v('n')),b.block(b.store('i',b.call('inc',b.v('i'))),
            b.iff(b.op(b.v('i'),'==',b.i(2)),b.block(b.cont())),b.store('total',b.op(b.v('total'),'+',b.v('i'))),
            b.iff(b.op(b.v('total'),'>',b.i(6)),b.block(b.brk())))),b.ret(b.v('total'))))
    program=m.Program((),(helper,main,choose,inc,child,total))
    files=section_files('Scalar split: expected i32 10')
    # Qualified path is an exact declared multi-token substitution for choose.
    # Temporarily remove that qualifier only for correspondence checking, then
    # derive origin relocation by exact byte insert positions below.
    plain={p:s.replace('crate::maths::choose','choose') for p,s in files.items()}
    homes={'helper':'root','main':'root','choose':'maths','inc':'maths','child_main':'maths','sum':'loops'}
    project=bind_authored_files(program,plain,homes,{}, {'child_main':'main'})
    insertion=plain['main.ox'].index('choose(false')
    for key,span in project['origins'].items():
        if span[0]==0:
            if span[1]>insertion:span[1]+=len('crate::maths::')
            if span[2]>insertion:span[2]+=len('crate::maths::')
    project['files']=files
    project['bijection'].append(dict(file=0,insert_at=insertion,text='crate::maths::',kind='declared-qualified-callee-prefix',target='crate::maths::choose'))
    return program,project

def main():
    assert sha((DESIGN/'proposed-design.md').read_bytes())=='f8351b4e63d68cf11995192bad93fb2b527020e9eeab11572ca0dc15452302a8'
    assert sha((REVIEW/'review-manifest.json').read_bytes())=='6846274f8784a7e7b7c9d5e0703a56d05dcadb230f23422d14ac50770ff8c17f'
    assert (HERE/'vendor/owned_source_model.py').read_bytes()==(REPO/'scripts/owned_source_model.py').read_bytes()
    cases=[];sources={}
    def add(identifier,family,program,project,axes=None,want=None,native=False):
        exp,files=finish_case(identifier,family,program,project,axes,want,native)
        cases.append(exp);sources[identifier]=files
    for placement,spelling,ordinal,result in itertools.product(('root_child','sibling','child_ancestor'),('absolute','import'),(0,1),('i32','bool')):
        p,j,w=scalar_family(placement,spelling,ordinal,result)
        identifier=f'scalar-{placement}-{spelling}-entry{ordinal}-{result}'
        native=(placement,spelling,ordinal,result) in [('root_child','absolute',0,'i32'),('sibling','import',1,'bool'),('child_ancestor','absolute',1,'bool')]
        add(identifier,'scalar24',p,j,dict(placement=placement,spelling=spelling,entry_ordinal=ordinal,result=result),w,native)
    for placement,spelling,home,event in itertools.product(('root_child','sibling','child_ancestor'),('absolute','import'),('root','child'),('shared_nested','exclusive_forward','owned_staging','loop_cleanup')):
        p,j,w=owned_family(placement,spelling,home,event)
        identifier=f'owned-{placement}-{spelling}-{home}-{event}'
        native=(placement,spelling,home,event) in [('root_child','absolute','root','shared_nested'),('sibling','import','child','exclusive_forward'),('child_ancestor','absolute','child','owned_staging'),('root_child','import','root','loop_cleanup')]
        add(identifier,'owned48',p,j,dict(placement=placement,spelling=spelling,record_home=home,event_template=event),w,native)
    with contextlib.redirect_stdout(io.StringIO()):
        oldargv=sys.argv;sys.argv=[str(HERE/'vendor/reproduce-body-predictions.py'),str(HERE/'vendor')]
        env=runpy.run_path(sys.argv[0]);sys.argv=oldargv
    old=env['old'];assert m.render(old).source==(REPO/'fixtures/owned_source/batch.ox').read_text()
    j=bind_authored_files(old,{'main.ox':m.render(old).source},{f.name:'root' for f in old.functions},{'Batch':'root'})
    add('pilot-original-batch','pilot',old,j,want=816)
    original=m.render(old);byname={f.name:i for i,f in enumerate(old.functions)}
    def declaration(name):
        a,b=original.origins[f'function.{byname[name]}'];return original.source[a:b]+'\n'
    record=original.source[slice(*original.origins['record.0'])]
    for name,_ in old.records[0].fields:record=record.replace(name+':','pub '+name+':')
    mechanical={'main.ox':'mod state;\nmod jobs;\n'+''.join('use crate::state::'+name+';\n' for name in ('Batch','retry','done','relay','finish'))+'use crate::jobs::dispatch;\n'+declaration('main'),
                'state.ox':'pub '+record+'\n'+''.join('pub '+declaration(name) for name in ('retry','commit','done','relay','finish')),
                'jobs.ox':'use crate::state::Batch;\nuse crate::state::commit;\npub '+declaration('dispatch')}
    homes={f.name:('root' if f.name=='main' else 'jobs' if f.name=='dispatch' else 'state') for f in old.functions}
    j=bind_authored_files(old,mechanical,homes,{'Batch':'state'});add('pilot-mechanical-batch','pilot',old,j,want=816,native=True)
    opaque=env['opaque'];files=section_files('Opaque Batch split: expected i32 816, model-predicted 1,577 fuel')
    homes={f.name:('root' if f.name=='main' else 'jobs' if f.name=='dispatch' else 'state') for f in opaque.functions}
    j=bind_authored_files(opaque,files,homes,{'Batch':'state'});add('pilot-opaque-batch','pilot',opaque,j,want=816,native=True)
    p,j=scalar_pilot();add('pilot-scalar','pilot',p,j,want=10,native=True)
    # Independently authored original-source control, explicit child-main rename.
    original_files={'main.ox':m.render(p).source}
    q=bind_authored_files(p,original_files,{f.name:'root' for f in p.functions},{})
    add('control-original-scalar-pilot','original-control',p,q,want=10)
    entry=env['owned_entry'];files=section_files('Separate owned entry-binding control: expected i32 5')
    homes={f.name:('root' if f.name in ('main','helper') else 'state') for f in entry.functions}
    j=bind_authored_files(entry,files,homes,{'Counter':'state'},{'child_main':'main'},{'main':{'bump':'increment'}})
    add('control-owned-entry-id1','owned-entry-control',entry,j,want=5,native=True)
    # RFC0015 exact Counter, separately counted from the ID1 addition.
    rfc=(REPO/'rfcs/0015-bounded-typed-projects.md').read_text();section=rfc.split('### 8.1',1)[1].split('### 8.2',1)[0]
    code=re.findall(r'```(?:text|oxid)?\n(.*?)```',section,re.S)
    counter_files={'main.ox':code[0],'state.ox':code[1]}
    counter=m.Program(entry.records,tuple(f for f in entry.functions if f.name not in ('helper','child_main')))
    homes={f.name:('root' if f.name=='main' else 'state') for f in counter.functions}
    j=bind_authored_files(counter,counter_files,homes,{'Counter':'state'},callee_aliases={'main':{'bump':'increment'}})
    add('pilot-counter','pilot',counter,j,want=5,native=True)
    # Extensions append finite negative families before the freeze is emitted.
    from negative_cases import append_negatives
    append_negatives(cases,sources,m,projectify,finish_case,bind_authored_files)
    output(cases,sources)

def output(cases,sources):
    ids=[c['id'] for c in cases];assert len(ids)==len(set(ids))
    requests=[]
    for c in cases:
        files=sources[c['id']];folder=HERE/'sources'/c['id'];folder.mkdir(parents=True,exist_ok=True)
        listing=[]
        for path,source in files.items():
            payload=source.encode();(folder/path).write_bytes(payload)
            listing.append(dict(path=path,bytes=len(payload),sha256=sha(payload)))
        c['source_manifest']=listing
        # This is the only observer input. It contains no expected targets,
        # route, types, ownership states, result, events or proof tokens.
        req=dict(id=c['id'],mode='private-project-candidate',entry='main.ox',source_root='sources/'+c['id'],source_files=listing,
                 operations=['check','run'],profiles=['debug','release'])
        if c.get('budget_cases'):req['fuel_budgets']=[x['budget'] for x in c['budget_cases']]
        if c.get('native_required'):req['native']={'opt_level':0,'source_free':True}
        elif c.get('expected',{}).get('native'):req['native']={'opt_level':0,'admission_only':True}
        requests.append(req)
    (HERE/'requests.jsonl').write_text(''.join(json.dumps(r,sort_keys=True)+'\n' for r in requests))
    data=''.join(json.dumps(c,sort_keys=True,separators=(',',':'),ensure_ascii=False)+'\n' for c in cases).encode()
    with (HERE/'expected.jsonl.gz').open('wb') as out:
        with gzip.GzipFile(filename='',fileobj=out,mode='wb',mtime=0) as z:z.write(data)
    counts=Counter(c['family'] for c in cases)
    assert counts['scalar24']==24 and counts['owned48']==48
    dump(HERE/'coverage.json',dict(evidence_kind='MODEL_ONLY_PRE_EXECUTION_FREEZE',cases=len(cases),families=dict(counts),
        native_covering_subset=[c['id'] for c in cases if c.get('native_required')],
        actual_compiler_invocations=0,actual_native_executions=0,
        model_limits=dict(states_per_function=100000,dynamic_events=100000,on_exhaustion='MODEL_INCOMPLETE; fail package generation'),
        required_followup=['actual private check/reference in debug and release','passive raw-call/record/field/source-span and essential event correspondence',
        'source-free native of the selected seven family cases and five project pilots/control','exact association visitor and mutation probes','whole-program resource/privacy/driver gates']))
    inputs=[DESIGN/'proposed-design.md',DESIGN/'pilots.md',DESIGN/'body-predictions.model-only.json',REVIEW/'review-manifest.json',REVIEW/'span-inventory.md',REVIEW/'review.md',
        REPO/'rfcs/0014-owned-structs-call-borrows.md',REPO/'rfcs/0015-bounded-typed-projects.md',REPO/'scripts/owned_source_model.py',REPO/'src/frontend/oir/execute.rs',REPO/'src/frontend/oir/lower.rs',
        REPO/'src/frontend/oir/owned/source/typeck.rs',REPO/'src/frontend/oir/owned/source/diagnostic.rs',REPO/'src/frontend/oir/owned/flow.rs',REPO/'src/frontend/oir/native.rs',REPO/'src/frontend/oir/owned/native.rs',
        REPO/'fixtures/owned_source/batch.ox',BASE/'typed-project-unit2-ci-fix/final-candidate/manifest.json']
    dump(HERE/'provenance.json',dict(schema='oxid-unit3-independent-oracles-v1',source_publication_supplied='152dfbe6107253add076fe2ddc4f6167159ea610',
        source_tree_supplied='017495b7f8abcc5f475994774dc522e8d6f54f84',
        checkout_note='Corrected publication bytes are in a working tree whose local HEAD is historical 47f9e168. Input byte hashes below, not HEAD equality, bind this model.',
        inputs=[dict(path=str(p),bytes=p.stat().st_size,sha256=sha(p.read_bytes())) for p in inputs],
        no_candidate_observations_read=True,no_compiler_invoked=True,no_llvm_invoked=True))
    print(json.dumps(dict(cases=len(cases),families=dict(counts),expected_bytes=len(data),compressed_bytes=(HERE/'expected.jsonl.gz').stat().st_size)))

if __name__=='__main__':main()
