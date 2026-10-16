"""Finite trim activation: exact PR64 inputs, unchanged source generations.

--check is read-only; --write emits the explicit output map. Neither mode calls
older activation scripts, fetches, builds, publishes, or rewrites history.
"""
import ast
import copy
import json
from pathlib import Path, PurePosixPath
import sys
import types

sys.dont_write_bytecode=True
HERE=Path(__file__).absolute().parent
REPO=HERE.parents[2]
sys.path.insert(0,str(HERE))
import retained as r
import derive
ROOT=r.ROOT;Q=r.Q;U=r.U;S=r.S
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
    repo=Path(repo);f=r.Fixture(repo);out={}
    g=types.ModuleType('trim_source_generation')
    g.__file__=str(repo/Q/'generation.py')
    exec(compile(f.read(Q+'generation.py'),g.__file__,'exec'),g.__dict__)
    def read(n):return f.read(n)
    def put(n,b):
        r.need(n not in out,'duplicate active output');out[n]=b
    def ident(n):return g.identity(n,out[n])
    def replace(n,old,new,count=1):return r.exact(read(n).decode(),old,new,count).encode()
    helper=derive.derive(read(derive.PREVIOUS));put(ROOT+'predecessor.py',helper)
    raw=read(S+'run.py');node=next(n for n in ast.parse(raw).body if isinstance(n,ast.Assign) and isinstance(n.targets[0],ast.Name) and n.targets[0].id=='_HELPER_SPECS')
    specs=ast.literal_eval(node.value)
    row=next(x for x in specs if x[0]=='predecessor');r.need(row[1]==g.identity(derive.PREVIOUS,read(derive.PREVIOUS)),'fixed old predecessor pin')
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
# Explicit trim successor; historical package_json_lifecycle.py remains intact.
_TRIM_LIFECYCLE = None

def _trim_lifecycle():
    global _TRIM_LIFECYCLE
    from authority import PACKAGE_JSON_LIFECYCLE_HELPER
    import types
    from published_lifecycle import _read_regular
    row = PACKAGE_JSON_LIFECYCLE_HELPER
    root = Path(__file__).absolute().parents[3]
    raw, _ = _read_regular(root / row['path'])
    need(len(raw) == row['bytes'] and sha(raw) == row['sha256'], 'fixed trim lifecycle helper')
    if _TRIM_LIFECYCLE is None:
        module = types.ModuleType('trim_current_lifecycle')
        module.__file__ = str(root / row['path'])
        exec(compile(raw, module.__file__, 'exec'), module.__dict__)
        _TRIM_LIFECYCLE = module
    return _TRIM_LIFECYCLE

'''
    n=PUBLIC+'build.py';raw=replace(n,'    import package_json_lifecycle as current','    current = _trim_lifecycle()',2).decode()
    raw=r.exact(raw,'def verify_lifecycle_successor(raw, source=None):',loader+'def verify_lifecycle_successor(raw, source=None):');put(n,raw.encode())
    n=PUBLIC+'test_lexer_reservation.py';raw=replace(n,'import package_json_lifecycle as m','from build import _trim_lifecycle\nm = _trim_lifecycle()').decode()
    raw=r.exact(raw,"(m.HERE / 'package-json-lifecycle-v1.json')","(m.REPOSITORY / '"+ROOT+"lifecycle-proof.json')");put(n,raw.encode())
    h=f.module('derive_hosted');hosted,receipt=h.derive(read(Q+'predecessor-v1/'+h.FACADE),dispatcher,seal)
    put(ROOT+'hosted_controls.py',hosted)
    facade=r.exact(h.facade(hosted).decode(),Q+'hosted_controls.py',ROOT+'hosted_controls.py').encode();put(h.FACADE,facade)
    receipt['successor']=ident(ROOT+'hosted_controls.py');put(ROOT+'hosted-derivation.json',g.encoded(receipt))
    # Historical hosted tests retain their bodies and assertions in the fixed fixture.
    shim='''"""Retained PR64 hosted mechanical tests; active bootstrap tested separately."""
import sys
from pathlib import Path
import unittest
sys.dont_write_bytecode = True
REPO = Path(__file__).absolute().parents[3]
sys.path.insert(0, str(REPO / 'tests/qualification/stdlib_trim_storage_v1'))
from retained import Fixture

def load_tests(loader, tests, pattern):
    import importlib.util
    suite = Fixture(REPO).hosted_suite()
    for name in ('test_storage', 'test_retained', 'test_active'):
        path = REPO / 'tests/qualification/stdlib_trim_storage_v1' / (name + '.py')
        spec = importlib.util.spec_from_file_location('trim_' + name, path)
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)
        suite.addTests(loader.loadTestsFromModule(module))
    return suite

if __name__ == '__main__':
    unittest.main()
'''
    put(Q+'test_hosted.py',shim.encode())
    # Keep generation tests' old dispatcher/expected parser pairing historical.
    n=Q+'test_generation.py';raw=read(n).decode()
    raw=r.exact(raw,"        cls.dispatcher = g.identity(g.PACKAGE+'run.py', g.regular(REPO, g.PACKAGE+'run.py'))",
        "        from pathlib import Path as _Path\n        sys.path.insert(0, str(REPO / 'tests/qualification/stdlib_trim_storage_v1'))\n        from retained import Fixture\n        cls.dispatcher = g.identity(g.PACKAGE+'run.py', Fixture(REPO).read(g.PACKAGE+'run.py'))")
    raw=r.exact(raw,"g.regular(REPO,'tests/qualification/unit4_parser_current/authority.json')","g.regular(REPO,'tests/qualification/package_json_current_v1/generated-parser-authority-v1.json')");put(n,raw.encode())
    # Replace only the two existing mechanical method bodies with scoped adapters.
    n=U+'test_scheduling.py';raw=read(n).decode();lines=raw.splitlines(True)
    cls=next(x for x in ast.parse(raw).body if isinstance(x,ast.ClassDef) and x.name=='SchedulingControls')
    names=('test_named_derivation_outputs_and_immutable_history','test_original_consumer_generator_emits_named_workflow')
    edits=[]
    for node in cls.body:
        if isinstance(node,ast.FunctionDef) and node.name in names:
            code="    def "+node.name+"(self):\n        sys.path.insert(0, str(activation.REPO / 'tests/qualification/stdlib_trim_storage_v1'))\n        from retained import Fixture\n        fixture = Fixture(activation.REPO)\n        suite = fixture.scheduling_suite(lambda: q.verify_package(activation.REPO, q.read(q.HERE/'inputs.json')))\n        case = next(case for case in suite if case._testMethodName == "+repr(node.name)+")\n        case.debug()\n"
            edits.append((node.lineno,node.end_lineno,code))
    r.need(len(edits)==2,'two scoped scheduling seams')
    for start,end,code in sorted(edits,reverse=True):lines[start-1:end]=[code]
    raw=''.join(lines)
    raw=r.exact(raw, '"import derive_scheduling as a; "\n            "assert all((a.REPO/n).read_bytes()==b for n,b in a.generate().items())"',
        '"from pathlib import Path; sys.path.insert(0, \'tests/qualification/stdlib_trim_storage_v1\'); "\n            "from retained import Fixture; Fixture(Path.cwd()).assert_scheduling_outputs(); "\n            "import activate; assert all((activate.REPO/n).read_bytes()==b for n,b in activate.generate().items())"')
    put(n,raw.encode())
    n='scripts/verify_owned_array_native.py'
    put(n,replace(n, '                "tests/qualification/package_json_current_v1/",',
        '                "tests/qualification/package_json_current_v1/",\n                "tests/qualification/stdlib_trim_storage_v1/",'))
    n='scripts/test_lexer_current_ci.py'
    put(n,replace(n, "                 'tests/qualification/package_json_current_v1',", 
        "                 'tests/qualification/package_json_current_v1',\n                 'tests/qualification/stdlib_trim_storage_v1',"))
    # Inventory is the last output; explicit source/input roster closes the new root.
    static=('README.md','derive.py','retained.py','review_inputs.py','activate.py','test_storage.py','test_retained.py','test_active.py','mechanical-inputs-v1.json','archive/stdlib/strings.ox')
    newnames={ROOT+n for n in static}|{n for n in out if n.startswith(ROOT)}
    newnames|={row['storage'] for row in list(f.rows.values())+list(f.git_rows.values()) if row['storage'].startswith(ROOT)}
    physical={p.relative_to(repo).as_posix() for p in (repo/ROOT).rglob('*') if p.is_file()}
    r.need(physical<=newnames,'unlisted trim namespace member: '+repr(physical-newnames))
    oldmanifest=json.loads(f.read(U+'inputs.json'))
    names={row['path'] for row in oldmanifest['files']}|newnames|{'stdlib/strings.ox','tests/stdlib_strings.rs'}
    manifest=copy.deepcopy(oldmanifest);manifest['schema']='oxid-unit4-ci-stdlib-trim-inputs-v1'
    manifest['trim_predecessor']=g.identity(U+'inputs-scheduling-v1.json',f.read(U+'inputs.json'))
    manifest['trim_storage']={'baseline':'8debaa96c0238e3376567fa2c76517fda9937037','source_only':True,'execution_qualified':False}
    manifest['closed_roots'].append(ROOT.rstrip('/'))
    rows=[]
    for n in sorted(names):
        if n in out:b=out[n]
        elif n in newnames or n in ('stdlib/strings.ox','tests/stdlib_strings.rs'):b=g.regular(repo,n)
        else:
            expected=f.read(n);b=r.regular(repo,n,len(expected));r.need(b==expected,'unapproved active input change: '+n)
        rows.append(g.identity(n,b))
    manifest['files']=rows
    put(U+'inputs-stdlib-trim-v1.json',g.encoded(manifest));put(U+'inputs.json',g.encoded(manifest))
    put(U+'common.py',assignment(read(U+'common.py'),'INPUTS_SHA',g.sha(g.encoded(manifest))))
    # Live inventory expectations; all historical metadata assertions retained.
    n=U+'test_integration.py';original=read(n)
    raw=original.decode().replace("inputs-scheduling-v1.json", "inputs-stdlib-trim-v1.json")
    old="'tests/qualification/published_formatter_current', 'tests/qualification/package_json_current_v1']"
    raw=r.exact(raw,old,"'tests/qualification/published_formatter_current', 'tests/qualification/package_json_current_v1', 'tests/qualification/stdlib_trim_storage_v1']",2)
    raw=r.exact(raw,'(1050, 17)',repr((len(rows),18)))
    put(n,raw.encode())
    return out


if __name__=='__main__':
    r.need(sys.argv[1:] in (['--check'],['--write']),'use --check or --write')
    outputs=generate()
    for name,body in outputs.items():
        path=REPO/name
        if sys.argv[1]=='--write':path.parent.mkdir(parents=True,exist_ok=True);path.write_bytes(body)
        else:r.need(path.read_bytes()==body,'active output mismatch: '+name)
    print('PASS',len(outputs),'finite trim outputs',sys.argv[1])
