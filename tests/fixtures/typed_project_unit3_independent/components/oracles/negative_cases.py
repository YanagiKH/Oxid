"""Bounded, independently authored source negatives and their positive controls.

This file imports no compiler code and reads no observations. Static ownership
diagnostics use the already independent finite-state source model. Visibility,
entry, project route and native graph rules are separately declared authorities.
"""
import copy, re

def append_negatives(cases,sources,m,projectify,finish_case,bind_authored_files):
    from generate import remap_spans, scalar_schedule
    def register(identifier,family,program,homes,record_homes,*,positive=False,want=None,source_names=None,spelling='absolute'):
        project=projectify(program,homes,record_homes,spelling,source_names,explicit_files=['root','state','other'])
        if positive:
            c,files=finish_case(identifier,family,program,project,expected_result=want)
        else:
            rendered=project['model_render'];case=m.Case(identifier,family,program)
            exp=m.case_expectation(case)
            assert exp['expected']['status']=='reject',(identifier,exp['expected'])
            expected=remap_spans(exp['expected'],rendered,project['origins'])
            # Related labels are ordered source spans; never reinterpret them
            # through the primary file or drop duplicates.
            pairs={tuple(pair):project['origins'][k] for k,pair in rendered.origins.items()}
            def rel(obj):
                if isinstance(obj,dict):
                    for k,v in obj.items():
                        if k=='related':obj[k]=[pairs[tuple(p)] for p in v]
                        else:rel(v)
                elif isinstance(obj,list):
                    for x in obj:rel(x)
            rel(expected)
            # The inherited body model's simplified message/label presentation
            # is not the established source diagnostic adapter contract. Bind
            # that presentation here before any candidate observation is read.
            def bind_diagnostic(d):
                if not isinstance(d,dict):return
                if 'message' in d:d['model_message']=d.pop('message')
                if d.get('code')=='E0311':
                    ck=m.NamesTypes(program).check()
                    binding=ck.bindings[ck.references[d['origin']]]
                    d['related'].append(project['origins'][binding.origin])
                if identifier=='negative-arity':
                    d['code']='E0301';d['related']=[project['origins']['function.1.name']]
                elif identifier in ('negative-scalar_argument','negative-borrow_mode','negative-nominal_value','negative-nominal_reference'):
                    d['related']=[project['origins']['function.1.name']]
                for item in d.get('reachable_diagnostics',[]):bind_diagnostic(item)
            bind_diagnostic(expected)
            c=dict(id=identifier,family=family,evidence_kind='MODEL_ONLY_SOURCE_REJECTION',
                   authority='source-type-or-authoritative-ownership-stage; never raw malformed-input classification',
                   route='owned' if program.records else 'scalar',expected=expected,entry=project['entry'],
                   function_count=len(program.functions),record_count=len(program.records),declarations=project['declarations'],
                   modules=project['modules'],imports=project['imports'],origins=project['origins'],source_to_model_bijection=project['bijection'],
                   positive_control='positive-'+identifier.removeprefix('negative-'),native_required=False)
            files=project['files']
        cases.append(c);sources[identifier]=files;return c,project

    # Each ownership negative has a separately rendered valid source control.
    # Names identify the precise first failing operation, not a vague feature.
    names=['relay_reuse','double_move','exclusive_then_read','exclusive_then_shared','exclusive_then_move',
           'shared_then_nested_mutation','exclusive_alias','shared_upgrade','forwarded_overlap','join_move','continue_move']
    for kind in names:
        for bad in (False,True):
            b=m.Builder();record=m.Record('Cell',(('value','i32'),))
            fs=[];homes={}
            def fn(name,params,result,body):
                fs.append(m.Function(name,params,result,body));homes[name]='state'
            fn('read',(('p','&Cell'),),'i32',b.block(b.ret(b.f('p','value'))))
            fn('take',(('p','Cell'),),'i32',b.block(b.ret(b.f('p','value'))))
            fn('relay',(('p','Cell'),),'Cell',b.block(b.ret(b.v('p'))))
            fn('bump',(('p','&mut Cell'),),'i32',b.block(b.write('p','value',b.op(b.f('p','value'),'+',b.i(1))),b.ret(b.f('p','value'))))
            pre=[b.let('x',b.lit('Cell',('value',b.i(4))),True),b.let('y',b.lit('Cell',('value',b.i(7))),True)]
            if kind=='relay_reuse':
                body=pre+[b.let('z',b.call('relay',b.v('x'))),b.ret(b.call('read',b.borrow('x' if bad else 'z')))]
            elif kind=='double_move':
                fn('pair',(('a','Cell'),('b','Cell')),'i32',b.block(b.ret(b.op(b.f('a','value'),'+',b.f('b','value')))))
                body=pre+[b.ret(b.call('pair',b.v('x'),b.v('x' if bad else 'y')))]
            elif kind in ('exclusive_then_read','exclusive_then_shared','exclusive_then_move'):
                fn('pair',(('p','&mut Cell'),('n','i32')),'i32',b.block(b.ret(b.v('n'))))
                name='x' if bad else 'y'
                last=b.f(name,'value') if kind=='exclusive_then_read' else b.call('read',b.borrow(name)) if kind=='exclusive_then_shared' else b.call('take',b.v(name))
                body=pre+[b.ret(b.call('pair',b.borrow('x',True),last))]
            elif kind=='shared_then_nested_mutation':
                fn('pair',(('p','&Cell'),('n','i32')),'i32',b.block(b.ret(b.v('n'))))
                body=pre+[b.ret(b.call('pair',b.borrow('x'),b.call('bump',b.borrow('x' if bad else 'y',True))))]
            elif kind=='exclusive_alias':
                fn('pair',(('a','&mut Cell'),('b','&mut Cell')),'i32',b.block(b.ret(b.op(b.f('a','value'),'+',b.f('b','value')))))
                body=pre+[b.ret(b.call('pair',b.borrow('x',True),b.borrow('x' if bad else 'y',True)))]
            elif kind=='shared_upgrade':
                fn('forward',(('p','&Cell' if bad else '&mut Cell'),),'i32',b.block(b.ret(b.call('bump',b.borrow('p',True,True)))))
                body=pre+[b.ret(b.call('forward',b.borrow('x',not bad)))]
            elif kind=='forwarded_overlap':
                fn('pair',(('p','&mut Cell'),('n','i32')),'i32',b.block(b.ret(b.v('n'))))
                if bad:inner=b.block(b.ret(b.call('pair',b.borrow('p',True,True),b.f('p','value'))))
                else:inner=b.block(b.let('snapshot',b.f('p','value')),b.ret(b.call('pair',b.borrow('p',True,True),b.v('snapshot'))))
                fn('forward',(('p','&mut Cell'),),'i32',inner)
                body=pre+[b.ret(b.call('forward',b.borrow('x',True)))]
            elif kind=='join_move':
                body=pre+[b.iff(b.b(True),b.block(b.discard(b.call('take',b.v('x' if bad else 'y'))))),b.ret(b.call('read',b.borrow('x')))]
            else:
                body=pre+[b.let('i',b.i(0),True),b.loop(b.op(b.v('i'),'<',b.i(2)),b.block(
                    *([] if bad else [b.let('z',b.lit('Cell',('value',b.i(4))))]),
                    b.discard(b.call('take',b.v('x' if bad else 'z'))),b.store('i',b.op(b.v('i'),'+',b.i(1))),b.cont())),b.ret(b.i(0))]
            fs.insert(0,m.Function('main',(),'i32',b.block(*body)));homes['main']='root'
            p=m.Program((record,),tuple(fs));identifier=('negative-' if bad else 'positive-')+kind
            register(identifier,'source-negative' if bad else 'negative-control',p,homes,{'Cell':'state'},positive=not bad)

    # Cross-file signature/nominal controls. Same source-spelled Cell records
    # remain distinct symbolic model types before any checker is consulted.
    for kind in ['arity','scalar_argument','borrow_mode','nominal_value','nominal_reference','nominal_return']:
        for bad in (False,True):
            b=m.Builder();records=(m.Record('Cell',(('value','i32'),)),m.Record('OtherCell',(('value','i32'),)))
            ty='OtherCell' if bad else 'Cell';fs=[];homes={'main':'root','target':'state'}
            if kind=='arity':
                target=m.Function('target',(('n','i32'),),'i32',b.block(b.ret(b.v('n'))));body=b.block(b.ret(b.call('target',*(() if bad else (b.i(4),)))))
            elif kind=='scalar_argument':
                target=m.Function('target',(('n','i32'),),'i32',b.block(b.ret(b.v('n'))));body=b.block(b.ret(b.call('target',b.b(True) if bad else b.i(4))))
            elif kind=='borrow_mode':
                target=m.Function('target',(('p','&Cell'),),'i32',b.block(b.ret(b.f('p','value'))))
                body=b.block(b.let('x',b.lit('Cell',('value',b.i(4))),True),b.ret(b.call('target',b.borrow('x',bad))))
            elif kind=='nominal_value':
                target=m.Function('target',(('p','Cell'),),'i32',b.block(b.ret(b.f('p','value'))))
                body=b.block(b.let('x',b.lit(ty,('value',b.i(4)))),b.ret(b.call('target',b.v('x'))))
            elif kind=='nominal_reference':
                target=m.Function('target',(('p','&Cell'),),'i32',b.block(b.ret(b.f('p','value'))))
                body=b.block(b.let('x',b.lit(ty,('value',b.i(4)))),b.ret(b.call('target',b.borrow('x'))))
            else:
                target=m.Function('target',(),'Cell',b.block(b.ret(b.lit(ty,('value',b.i(4))))));body=b.block(b.let('x',b.call('target')),b.ret(b.f('x','value')))
            p=m.Program(records,(m.Function('main',(),'i32',body),target))
            register(('negative-' if bad else 'positive-')+kind,'source-negative' if bad else 'negative-control',p,homes,{'Cell':'state','OtherCell':'other'},
                     positive=not bad,source_names={'OtherCell':'Cell'})

    # Authored module privacy controls have an independent permission authority.
    # This tiny declaration scanner allocates IDs only by file/declaration order;
    # it never resolves a call or uses production parser facts.
    def loc(files,path,needle,occurrence=0,sub=None):
        text=files[path];starts=[x.start() for x in re.finditer(re.escape(needle),text)];assert len(starts)>occurrence,(path,needle)
        start=starts[occurrence]
        if sub is not None:start+=needle.index(sub);needle=sub
        return [list(files).index(path),len(text[:start].encode()),len(text[:start+len(needle)].encode())]
    def authored(identifier,family,files,expected,*,positive_control=None,native=False):
        functions=[];records=[]
        for file,(path,text) in enumerate(files.items()):
            for match in re.finditer(r'(?m)^[ \t]*(?:pub )?(fn|struct) (\w+)',text):
                out=functions if match.group(1)=='fn' else records
                out.append(dict(id=len(out),file=file,name=match.group(2),span=[file,len(text[:match.start(2)].encode()),len(text[:match.end(2)].encode())]))
        entry=next((f['id'] for f in functions if f['file']==0 and f['name']=='main'),None)
        c=dict(id=identifier,family=family,evidence_kind='INDEPENDENT_AUTHORED_PROJECT_CONTRACT',route='owned' if records else 'scalar',
               expected=expected,entry=entry,function_count=len(functions),record_count=len(records),declarations=dict(functions=functions,records=records),
               positive_control=positive_control,native_required=native)
        cases.append(c);sources[identifier]=files;return c
    def reject(files,stage,code,path,needle,secondary=(),sub=None):
        return dict(check='reject',authority='source-rejection',stage=stage,code=code,span=loc(files,path,needle,sub=sub),
                    related=[loc(files,*x) for x in secondary],span_match='exact',selection='one specified first source failure')
    factory='pub struct Cell { value: i32 }\npub fn make() -> Cell { return Cell { value: 4 }; }\n'
    for kind,statement in [('private_read','return x.value;'),('private_write','x.value = 5; return 0;'),
                           ('private_wrong_assignment','x.value = true; return 0;'),('private_rhs_precedence','x.value = true + 1; return 0;')]:
        files={'main.ox':'mod state;\nfn main() -> i32 { let mut x = crate::state::make(); '+statement+' }\n','state.ox':factory}
        if kind=='private_rhs_precedence':expected=reject(files,'type','E0300','main.ox','true')
        else:expected=reject(files,'type','E0206','main.ox','x.value',secondary=(('state.ox','value'),),sub='value')
        authored('negative-'+kind,'source-negative',files,expected,positive_control='pilot-counter')
    files={'main.ox':'mod state;\nfn main() -> i32 { let x = crate::state::Cell { value: 4 }; return 0; }\n','state.ox':factory}
    authored('negative-private_constructor','source-negative',files,reject(files,'resolve','E0206','main.ox','value',secondary=(('state.ox','value'),)),positive_control='positive-nominal_value')
    files={'main.ox':'mod state;\nfn main() -> i32 { return crate::state::secret(); }\n','state.ox':'fn secret() -> i32 { return 4; }\n'}
    authored('negative-private_callee','source-negative',files,reject(files,'resolve','E0206','main.ox','secret',secondary=(('state.ox','secret'),)),positive_control='scalar-root_child-absolute-entry0-i32')
    files={'main.ox':'mod state;\nfn main() -> i32 { return 0; }\n','state.ox':'struct Cell { value: i32 }\npub fn make() -> Cell { return Cell { value: 4 }; }\n'}
    expected=reject(files,'resolve','E0207','state.ox','-> Cell',secondary=(('state.ox','Cell'),),sub='Cell')
    authored('negative-signature_exposure','source-negative',files,expected,positive_control='pilot-counter')
    files={'main.ox':'mod state;\nuse crate::state::Dual;\nfn main() -> i32 { return 0; }\n',
           'state.ox':'struct Dual { value: i32 }\npub fn Dual() -> i32 { return 4; }\n'}
    expected=reject(files,'resolve','E0206','main.ox','Dual',secondary=(('state.ox','Dual'),))
    expected['import_commit']='none in either namespace for rejected import'
    authored('negative-atomic_paired_import','source-negative',files,expected,positive_control='positive-atomic_paired_import')
    good=copy.deepcopy(files);good['state.ox']=good['state.ox'].replace('struct Dual','pub struct Dual',1)
    authored('positive-atomic_paired_import','negative-control',good,dict(check='accept',result=0,result_type='i32',imports='Dual binds record0 and function1 atomically'))

    # Root-original identity, unused ownership and route-selection controls.
    for route in ('scalar','owned'):
        files={'main.ox':'mod state;\nuse crate::state::main;\nfn helper() -> i32 { return 7; }\n',
               'state.ox':('pub struct Cell { pub value: i32 }\n' if route=='owned' else '')+'pub fn main() -> i32 { return 91; }\n'}
        authored('negative-imported_main-'+route,'entry-negative',files,dict(check='accept',run=dict(code='E0600',stage='oir-run',span=None,related=[]),
                 native=dict(code='E0700',stage='native-admission',span=None,related=[])),positive_control='control-owned-entry-id1' if route=='owned' else 'pilot-scalar')
    c=next(x for x in cases if x['id']=='negative-relay_reuse');files=copy.deepcopy(sources[c['id']])
    original=sources[c['id']]['main.ox'];cut=original.index('fn main()')
    files['main.ox']='mod state;\nmod other;\n'
    files['other.ox']='pub '+original[cut:].replace('fn main()','fn unused()',1)
    # Move the unused failing function to the child: +4 for pub, +2 for the
    # main->unused rename, and remove the original module/import prefix.
    expected=copy.deepcopy(c['expected'])
    shifted=set()
    def shift(obj):
        if isinstance(obj,dict):
            for v in obj.values():shift(v)
        elif isinstance(obj,list):
            if len(obj)==3 and all(type(x)==int for x in obj):
                if id(obj) in shifted:return
                shifted.add(id(obj))
                if obj[0]==0:
                    obj[0]=2;obj[1]+=6-cut;obj[2]+=6-cut
            else:
                for v in obj:shift(v)
    shift(expected)
    authored('negative-unused_move_beats_missing_main','source-negative',files,expected,positive_control='negative-imported_main-owned')
    for nominal in (False,True):
        b=m.Builder();fs=(m.Function('main',(),'i32',b.block(b.ret(b.call('answer',b.i(3))))),m.Function('answer',(('n','i32'),),'i32',b.block(b.ret(b.op(b.v('n'),'+',b.i(1))))))
        p=m.Program((m.Record('Unused',()),) if nominal else (),fs)
        register('control-unused_nominal-'+str(nominal).lower(),'route-control',p,{'main':'root','answer':'state'},{'Unused':'other'} if nominal else {},positive=True,want=4)

    # Return, join, break and continue cleanup all have live and already-moved
    # lexical owners. This additional bounded positive reaches an owned return
    # from a branch while an earlier owner remains in its caller scope.
    b=m.Builder();record=m.Record('Cell',(('value','i32'),))
    yes=b.block(b.let('z',b.lit('Cell',('value',b.i(9)))),b.ret(b.v('a')))
    choose=m.Function('choose',(('n','i32'),),'Cell',b.block(
        b.let('a',b.lit('Cell',('value',b.v('n')))),b.iff(b.op(b.v('n'),'>',b.i(0)),yes),b.ret(b.v('a'))))
    main=m.Function('main',(),'i32',b.block(b.let('a',b.call('choose',b.i(4))),b.let('z',b.call('choose',b.i(0))),b.ret(b.op(b.f('a','value'),'+',b.f('z','value')))))
    register('control-cleanup_return_join','cleanup-control',m.Program((record,),(main,choose)),{'main':'root','choose':'state'},{'Cell':'state'},positive=True,want=4)

    # Two compact span-variant controls exercise the visitor's nested scalar
    # branches and owned constructor/replacement/discard branches explicitly.
    for owned in (False,True):
        b=m.Builder();records=(m.Record('Cell',(('value','i32'),('flag','bool'))),) if owned else ()
        noop=m.Function('noop',(('n','i32'),),'()',b.block(b.ret()))
        body=[b.let('n',b.i(3),True),b.store('n',b.op(b.v('n'),'+',b.i(1))),
              b.let('yes',b.op(b.op(b.unary('!',b.b(False)),'&&',b.b(True)),'||',b.b(False))),
              b.let('same',b.op(b.v('n'),'==',b.i(4))),b.discard(b.call('noop',b.group(b.v('n'))))]
        if owned:
            body += [b.let('x',b.lit('Cell',('value',b.i(1)),('flag',b.b(True))),True),
                b.store('x',b.lit('Cell',('value',b.i(4)),('flag',b.v('same')))),
                b.discard(b.lit('Cell',('value',b.i(90)),('flag',b.b(False)))),
                b.iff(b.v('yes'),b.block(b.write('x','value',b.i(5)),b.let('scratch',b.lit('Cell',('value',b.i(11)),('flag',b.b(True))))),
                      b.block(b.write('x','value',b.i(6)),b.let('scratch2',b.lit('Cell',('value',b.i(12)),('flag',b.b(False)))))),
                b.ret(b.op(b.f('x','value'),'+',b.v('n')))]
        else:body += [b.iff(b.v('yes'),b.block(b.store('n',b.i(9))),b.block(b.store('n',b.i(0)))),b.ret(b.v('n'))]
        p=m.Program(records,(m.Function('main',(),'i32',b.block(*body)),noop))
        register('control-span-variants-'+('owned' if owned else 'scalar'),'span-control',p,{'main':'root','noop':'state'},
                 {'Cell':'root'} if owned else {},positive=True,want=9)

    # Bounded mutual recursion is valid reference source but rejected by native
    # whole-program call-graph admission, including an unused recursive pair.
    for route,used in [('scalar',True),('scalar',False),('owned',True),('owned',False)]:
        b=m.Builder()
        def recur(name,other):
            return m.Function(name,(('n','i32'),),'i32',b.block(b.iff(b.op(b.v('n'),'<',b.i(1)),b.block(b.ret(b.i(0))),
                b.block(b.ret(b.op(b.call(other,b.op(b.v('n'),'-',b.i(1))),'+',b.i(1)))))))
        p=m.Program((m.Record('Unused',()),) if route=='owned' else (),
            (m.Function('main',(),'i32',b.block(b.ret(b.call('left',b.i(3)) if used else b.i(7)))),recur('left','right'),recur('right','left')))
        c,_=register(f'negative-native_recursion-{route}-{str(used).lower()}','native-negative',p,{'main':'root','left':'state','right':'other'},
                     {'Unused':'other'} if route=='owned' else {},positive=True,want=3 if used else 7)
        c['expected']['native']=dict(status='reject',code='E0700',stage='native-admission',external_tool_invocations=0,output_untouched=True,
                                    span=c['origins']['function.0.name' if used else 'function.1.name'],related=[],
                                    authority='whole linked call graph includes unreachable-from-main originals')
        c['positive_control']='pilot-scalar' if route=='scalar' else 'pilot-counter'

    # Four independently specified layout encodings crossed with four diagnostic
    # families. All file byte offsets and line/scalar-column positions are frozen.
    for unicode,newline in [(False,'\n'),(True,'\n'),(False,'\r\n'),(True,'\r\n')]:
        label=('unicode' if unicode else 'ascii')+('-crlf' if newline=='\r\n' else '-lf')
        prefix='// 雪🦀\n' if unicode else '// layout\n'
        for family in ('scalar_overflow','owned_overflow','owned_loan','missing_root'):
            if family=='scalar_overflow':
                files={'main.ox':'mod state;\nfn main() -> i32 { return crate::state::overflow(); }\n','state.ox':'pub fn overflow() -> i32 { return 2147483647 + 1; }\n'}
                expected=dict(check='accept',run=dict(code='E0604',stage='oir-run',related=[]))
            elif family=='owned_overflow':
                files={'main.ox':'mod state;\nfn main() -> i32 { let mut x = crate::state::make(); return crate::state::overflow(&mut x); }\n',
                    'state.ox':'pub struct Cell { value: i32 }\npub fn make() -> Cell { return Cell { value: 0 }; }\npub fn overflow(p: &mut Cell) -> i32 { p.value = 7; return 2147483647 + 1; }\n'}
                expected=dict(check='accept',run=dict(code='E0604',stage='oir-run',related=[],committed_field_value=7,no_unwind_promise=True))
            elif family=='owned_loan':
                files={'main.ox':'mod state;\nfn main() -> i32 { let mut x = crate::state::make(); return crate::state::pair(&mut x, x.value); }\n',
                    'state.ox':'pub struct Cell { pub value: i32 }\npub fn make() -> Cell { return Cell { value: 4 }; }\npub fn pair(p: &mut Cell, n: i32) -> i32 { return n; }\n'}
                expected=None
            else:
                files={'main.ox':'mod state;\nuse crate::state::main;\n','state.ox':'pub fn main() -> i32 { return 0; }\n'}
                expected=dict(check='accept',run=dict(code='E0600',stage='oir-run',span=None,related=[]),native=dict(code='E0700',stage='native-admission',span=None,related=[]))
            files={p:(prefix+s).replace('\n',newline) for p,s in files.items()}
            if family in ('scalar_overflow','owned_overflow'):expected['run']['span']=loc(files,'state.ox','+')
            if family=='owned_loan':expected=reject(files,'ownership','E0311','main.ox','x.value',secondary=(('main.ox','&mut x'),('main.ox','let mut x')))
            if family=='owned_loan':expected['related'][1]=loc(files,'main.ox','let mut x',sub='x')
            authored('origin-'+label+'-'+family,'origin16',files,expected,positive_control='pilot-scalar' if family=='scalar_overflow' else 'pilot-counter',
                     native=family in ('scalar_overflow','owned_overflow'))

    # Equal (start,end), different file identities. Both operations are reachable
    # in paired sources, so interning solely by byte pair cannot be hidden by an
    # unused operation. Main's addition is safe before the child overflow.
    for route in ('scalar','owned'):
        root='fn main() -> i32 { let n = 1 + 2; return crate::state::overflow(); }\n'
        child='pub fn overflow() -> i32 { return 2147483647 + 1; }\n'
        header='mod state;\n';record='pub struct Marker {}\n' if route=='owned' else ''
        target=max((header+root).index('+'),(record+child).index('+'))+16
        files={'main.ox':header+' '*(target-len(header)-root.index('+'))+root,
               'state.ox':record+' '*(target-len(record)-child.index('+'))+child}
        # Leading spaces on declaration lines are valid syntax. Scanner below
        # is declaration-only and deliberately permits this source layout.
        a=loc(files,'main.ox','+');z=loc(files,'state.ox','+');assert a[1:]==z[1:]
        expected=dict(check='accept',run=dict(code='E0604',stage='oir-run',span=z,related=[]),
                      native=dict(code='E0604',stage='oir-run',span=z,stderr_equals_reference_human=True),
                      diagnostic_intern_keys=[a,z],distinct_intern_entries=True)
        authored('diagnostic-collision-'+route,'collision2',files,expected,positive_control='pilot-scalar' if route=='scalar' else 'pilot-counter',native=True)

    # Add line/scalar-column rendering coordinates for every exact error origin.
    for c in cases:
        files=sources[c['id']];values=list(files.values())
        def position(span):
            if span is None:return None
            file,start,end=span;data=values[file].encode();assert 0<=start<=end<=len(data)
            pre=data[:start].decode();data[:end].decode();line=pre.count('\n')+1
            col=len(pre.rsplit('\n',1)[-1])+1
            return dict(file=file,path=list(files)[file],start=start,end=end,line=line,scalar_column=col)
        def attach(obj):
            if isinstance(obj,dict):
                if 'span' in obj and (obj['span'] is None or isinstance(obj['span'],list) and len(obj['span'])==3):obj['location']=position(obj['span'])
                if 'related' in obj:obj['related_locations']=[position(x) for x in obj['related']]
                for key,value in list(obj.items()):
                    if key not in ('location','related_locations'):attach(value)
            elif isinstance(obj,list):
                for value in obj:attach(value)
        attach(c.get('expected',{}))
