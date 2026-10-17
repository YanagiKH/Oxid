"""Finite ASCII text activation; old PR65 proofs and producers remain intact."""
import ast
import copy
import json
from pathlib import Path, PurePosixPath
import sys
import types
sys.dont_write_bytecode=True
HERE=Path(__file__).absolute().parent
REPO=HERE.parents[2]

def _local(name):
    path=HERE/(name+'.py');module=types.ModuleType('text_'+name);module.__file__=str(path)
    exec(compile(path.read_bytes(),str(path),'exec'),module.__dict__)
    return module

base=_local('baseline')
derive=_local('derive')
r=base
ROOT=base.ROOT;Q=base.Q;U=base.U;S='tests/fixtures/typed_project_source_binding/'
PUBLIC='tests/qualification/unit4_public_v3/'

def assignment(raw,name,value):
    text=raw.decode();nodes=[n for n in ast.parse(text).body if isinstance(n,ast.Assign)
        and len(n.targets)==1 and isinstance(n.targets[0],ast.Name) and n.targets[0].id==name]
    r.need(len(nodes)==1,'exact active assignment: '+name)
    n=nodes[0];lines=text.splitlines(True);lines[n.lineno-1:n.end_lineno]=[name+' = '+repr(value)+'\n']
    return ''.join(lines).encode()


def functions(raw,names,scope):
    # Pure producer subset only; no top-level import/admission or IO is executed.
    tree=ast.parse(raw);nodes=[n for n in tree.body if isinstance(n,ast.FunctionDef) and n.name in names]
    r.need({n.name for n in nodes}==set(names),'closed pure producer functions')
    exec(compile(ast.Module(body=nodes,type_ignores=[]),'<fixed-pure-producers>','exec'),scope)
    return scope


def generate(repo=REPO):
    repo=Path(repo);baseline=base.Baseline(repo);old_outputs=baseline.outputs();r=baseline.module('retained');f=r.Fixture(repo);out={}
    g=types.ModuleType('trim_source_generation')
    g.__file__=str(repo/Q/'generation.py')
    exec(compile(f.read(Q+'generation.py'),g.__file__,'exec'),g.__dict__)
    def read(n):return f.read(n)
    def put(n,b):
        r.need(n not in out,'duplicate active output');out[n]=b
    def ident(n):return g.identity(n,out[n])
    def replace(n,old,new,count=1):return r.exact(read(n).decode(),old,new,count).encode()
    helper=derive.derive(baseline.read(derive.PREVIOUS));put(ROOT+'predecessor.py',helper)
    raw=read(S+'run.py');node=next(n for n in ast.parse(raw).body if isinstance(n,ast.Assign) and isinstance(n.targets[0],ast.Name) and n.targets[0].id=='_HELPER_SPECS')
    specs=ast.literal_eval(node.value)
    row=next(x for x in specs if x[0]=='predecessor');r.need(row[1]==g.identity(Q+'predecessor.py',read(Q+'predecessor.py')),'fixed old predecessor pin')
    row[1]=ident(ROOT+'predecessor.py');put(S+'run.py',assignment(raw,'_HELPER_SPECS',specs))
    dispatcher=ident(S+'run.py')
    package=json.loads(read(S+'package-manifest.json'))
    for row in package['files']:
        if row['path']=='run.py':row.update(g.identity('run.py',out[S+'run.py']))
    put(S+'package-manifest.json',g.encoded(package))
    support='tests/qualification/lexer_reservation_current/unit1_current_support.py'
    raw=assignment(read(support),'DISPATCHER_SHA',dispatcher['sha256'])
    put(support,assignment(raw,'SOURCE_PACKAGE_SHA',g.sha(out[S+'package-manifest.json'])))
    for n in ['tests/qualification/lexer_reservation_current/test_current_source.py',
              'tests/qualification/unit4_parser_current/test_current.py','tests/qualification/unit4_parser_current/test_lexer_reservation.py']:
        put(n,assignment(read(n),'DISPATCHER',dispatcher))
    # Generate source association in memory only; every old artifact stays exact.
    previous=read(S+g.PREDECESSOR_NAME)
    inputs={row['path']:g.regular(repo,row['path']) for row in json.loads(previous)['files']}
    artifacts,restored=g.generate(previous,inputs,read(Q+'generated/'+g.PATCH_NAME),
        {n:read(Q+'source-proof/'+n+'.json') for n in g.READBACKS},
        read('tests/fixtures/typed_project_source_binding_byte_storage_v1/run.py'))
    for n,b in artifacts.items():r.need(b==read(Q+'generated/'+n),'historical source artifact changed')
    admitted={'current':json.loads(artifacts[g.CURRENT_NAME]),'inputs':inputs,'predecessor_inputs':restored,'artifacts':artifacts}
    old=json.loads(read('tests/fixtures/typed_project_unit4_parser_portable/frozen/v3/authority.json'))
    old['current']=json.loads(read(Q+'predecessor-v1/tests/qualification/unit4_parser_current/authority.json'))
    old['current_source']=json.loads(previous)
    life=json.loads(read(PUBLIC+'published-lifecycle-v1.json'))
    ps=functions(read(Q+'parser.py'),{'ordinary_sources','candidate','transport'},
        {'g':g,'copy':copy,'PurePosixPath':PurePosixPath,'KEY':'package_json_parser',
         'OLD_AUTHORITY_SHA':'900f9e7cfb32027b4406e2860c5bc39abb0d831c89931efaf166911905c35ca4',
         'FIELDS':('schema','reviewed_source_head','source_only_tree','current_source_manifest','source_binding_runner','source_delta','current_base_files','current_derived_files','current_control_derived_files','current_candidate_source_manifest_sha256')})
    authority=ps['transport'](old,admitted,dispatcher,life['inserted_roster'])['current']
    put(ROOT+'parser-authority.json',g.encoded(authority));put('tests/qualification/unit4_parser_current/authority.json',g.encoded(authority))
    n='tests/qualification/unit4_parser_current/portable.py'
    raw=assignment(read(n),'AUTHORITY_SHA',g.sha(g.encoded(authority)))
    raw=assignment(raw,'_PACKAGE_JSON_PARSER_SEAL',{'dispatcher':dispatcher,'support':g.identity(Q+'parser.py',read(Q+'parser.py'))});put(n,raw)
    seal=json.loads(read(Q+'source-seal-v1.json'))
    ls=functions(read(PUBLIC+'package_json_lifecycle.py'),{'sha','need','make_proof'},
                 {'hashlib':__import__('hashlib'),'json':json,'Rejected':ValueError})
    proof=ls['make_proof'](life,admitted['current'],dispatcher,seal)
    put(ROOT+'lifecycle-proof.json',g.encoded(proof))
    n=PUBLIC+'package_json_lifecycle.py';raw=assignment(read(n),'DISPATCHER',dispatcher).decode()
    raw=r.exact(raw,'HERE = Path(__file__).absolute().parent\nREPOSITORY = HERE.parents[2]',
        "REPOSITORY = Path(__file__).absolute().parents[3]\nHERE = REPOSITORY / 'tests/qualification/unit4_public_v3'")
    raw=r.exact(raw,"regular(HERE, 'package-json-lifecycle-v1.json')","regular(REPOSITORY, '"+ROOT+"lifecycle-proof.json')")
    put(ROOT+'lifecycle.py',raw.encode())
    n=PUBLIC+'authority.py';raw=assignment(read(n),'PACKAGE_JSON_LIFECYCLE_HELPER',ident(ROOT+'lifecycle.py'))
    put(n,assignment(raw,'PACKAGE_JSON_LIFECYCLE_PROOF',ident(ROOT+'lifecycle-proof.json')))
    loader='''
# Explicit text successor; historical package_json_lifecycle.py remains intact.
_TEXT_LIFECYCLE = None

def _text_lifecycle():
    global _TEXT_LIFECYCLE
    from authority import PACKAGE_JSON_LIFECYCLE_HELPER
    import types
    from published_lifecycle import _read_regular
    row = PACKAGE_JSON_LIFECYCLE_HELPER
    root = Path(__file__).absolute().parents[3]
    raw, _ = _read_regular(root / row['path'])
    need(len(raw) == row['bytes'] and sha(raw) == row['sha256'], 'fixed text lifecycle helper')
    if _TEXT_LIFECYCLE is None:
        module = types.ModuleType('text_current_lifecycle')
        module.__file__ = str(root / row['path'])
        exec(compile(raw, module.__file__, 'exec'), module.__dict__)
        _TEXT_LIFECYCLE = module
    return _TEXT_LIFECYCLE

'''
    n=PUBLIC+'build.py';raw=replace(n,'    import package_json_lifecycle as current','    current = _text_lifecycle()',2).decode()
    raw=r.exact(raw,'def verify_lifecycle_successor(raw, source=None):',loader+'def verify_lifecycle_successor(raw, source=None):');put(n,raw.encode())
    n=PUBLIC+'test_lexer_reservation.py';raw=replace(n,'import package_json_lifecycle as m','from build import _text_lifecycle\nm = _text_lifecycle()').decode()
    raw=r.exact(raw,"(m.HERE / 'package-json-lifecycle-v1.json')","(m.REPOSITORY / '"+ROOT+"lifecycle-proof.json')");put(n,raw.encode())
    h=f.module('derive_hosted');hosted,receipt=h.derive(read(Q+'predecessor-v1/'+h.FACADE),dispatcher,seal)
    put(ROOT+'hosted_controls.py',hosted)
    facade=r.exact(h.facade(hosted).decode(),Q+'hosted_controls.py',ROOT+'hosted_controls.py').encode();put(h.FACADE,facade)
    receipt['successor']=ident(ROOT+'hosted_controls.py');put(ROOT+'hosted-derivation.json',g.encoded(receipt))
    # Original PR64 mechanical tests remain in their original fixed fixture.
    # The PR65 tests are adapted only at their historical/current byte seams.
    shim='''"""Retained PR64/PR65 controls and current text controls."""
import types
from pathlib import Path
import unittest
REPO=Path(__file__).absolute().parents[3]

def load_tests(loader, tests, pattern):
    path=REPO/'tests/qualification/stdlib_text_case_v1/adapters.py'
    module=types.ModuleType('text_test_adapters');module.__file__=str(path)
    exec(compile(path.read_bytes(),str(path),'exec'),module.__dict__)
    return module.suite(REPO,loader)

if __name__=='__main__':unittest.main()
'''
    put(Q+'test_hosted.py',shim.encode())
    n=U+'test_scheduling.py';raw=old_outputs[n].decode()
    before="import activate; assert all((activate.REPO/n).read_bytes()==b for n,b in activate.generate().items())"
    after="import types; p=Path('tests/qualification/stdlib_text_case_v1/activate.py').absolute(); m=types.ModuleType('text_checkout_activation'); m.__file__=str(p); exec(compile(p.read_bytes(),str(p),'exec'),m.__dict__); m.base.Baseline(Path.cwd()).assert_outputs(); assert all((m.REPO/n).read_bytes()==body for n,body in m.generate().items())"
    put(n,r.exact(raw,before,after).encode())
    n='scripts/verify_owned_array_native.py'
    put(n,r.exact(old_outputs[n].decode(), '                "tests/qualification/stdlib_trim_storage_v1/",',
        '                "tests/qualification/stdlib_trim_storage_v1/",\n                "tests/qualification/stdlib_text_case_v1/",').encode())
    n='scripts/test_lexer_current_ci.py'
    put(n,r.exact(old_outputs[n].decode(), "                 'tests/qualification/stdlib_trim_storage_v1',",
        "                 'tests/qualification/stdlib_trim_storage_v1',\n                 'tests/qualification/stdlib_text_case_v1',").encode())
    # Closed new namespace. Generated files are explicit, never harvested as inputs.
    static=('README.md','review_inputs.py','derive.py','baseline.py','activate.py','adapters.py','test_text_storage.py','pr65-expected-v1.json','archive/stdlib/text.ox.data')
    newnames={ROOT+n for n in static}|{n for n in out if n.startswith(ROOT)}
    newnames|={row['storage'] for row in baseline.rows.values() if row['storage'].startswith(ROOT)}
    physical={p.relative_to(repo).as_posix() for p in (repo/ROOT).rglob('*') if p.is_file()}
    r.need(physical<=newnames,'unlisted text namespace member: '+repr(physical-newnames))
    manifest=json.loads(old_outputs[U+'inputs.json'])
    oldrows={row['path']:row for row in manifest['files']}
    manifest['schema']='oxid-unit4-ci-stdlib-text-inputs-v1'
    manifest['text_predecessor']=g.identity(U+'inputs-stdlib-trim-v1.json',old_outputs[U+'inputs.json'])
    manifest['text_storage']={'baseline':base.HEAD,'source_only':True,'execution_qualified':False}
    manifest['closed_roots'].append(ROOT.rstrip('/'))
    names=set(oldrows)|newnames|{'stdlib/text.ox','tests/stdlib_text.rs'}
    rows=[]
    for n in sorted(names):
        if n in out:body=out[n]
        elif n in newnames or n in ('stdlib/text.ox','tests/stdlib_text.rs'):body=g.regular(repo,n)
        else:
            row=oldrows[n];body=g.regular(repo,n,limit=row['bytes'])
            r.need(g.identity(n,body)==row,'unapproved PR65 input change: '+n)
        rows.append(g.identity(n,body))
    manifest['files']=rows
    put(U+'inputs-stdlib-text-v1.json',g.encoded(manifest));put(U+'inputs.json',g.encoded(manifest))
    put(U+'common.py',assignment(old_outputs[U+'common.py'],'INPUTS_SHA',g.sha(g.encoded(manifest))))
    n=U+'test_integration.py';raw=old_outputs[n].decode()
    raw=r.exact(raw,'inputs-stdlib-trim-v1.json','inputs-stdlib-text-v1.json')
    raw=r.exact(raw,"'tests/qualification/stdlib_trim_storage_v1']",
        "'tests/qualification/stdlib_trim_storage_v1', 'tests/qualification/stdlib_text_case_v1']",2)
    raw=r.exact(raw,'(1092, 18)',repr((len(rows),19)))
    put(n,raw.encode())
    r.need(not any(n.startswith(base.TRIM) for n in out),'immutable trim namespace')
    return out

if __name__=='__main__':
    base.need(sys.argv[1:] in (['--check'],['--write']),'use --check or --write')
    outputs=generate()
    for name,body in outputs.items():
        path=REPO/name
        if sys.argv[1]=='--write':path.parent.mkdir(parents=True,exist_ok=True);path.write_bytes(body)
        else:base.need(path.read_bytes()==body,'text active output mismatch: '+name)
    print('PASS',len(outputs),'finite text outputs',sys.argv[1])
