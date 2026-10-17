"""Finite fixture-test storage successor, composed by the named CI activation."""
import ast
import copy
import hashlib
import json
from pathlib import Path, PurePosixPath
import sys
import types
sys.dont_write_bytecode=True
HERE=Path(__file__).absolute().parent
REPO=HERE.parents[2]
ROOT='tests/qualification/wide_span_fixture_storage_v1/'
Q='tests/qualification/package_json_current_v1/'
S='tests/fixtures/typed_project_source_binding/'
PUBLIC='tests/qualification/unit4_public_v3/'
MANIFEST = (4710, 'f09c7f39a8b6915704719a46fd4f67813380b274d46d10dce29d89f77a5a8369')

def need(ok,message):
    if not ok:raise ValueError(message)
def exact(text,before,after,count=1):
    need(text.count(before)==count,'exact fixture successor seam')
    return text.replace(before,after)
def local(path):
    module=types.ModuleType('wide_fixture_'+path.stem);module.__file__=str(path)
    exec(compile(path.read_bytes(),str(path),'exec'),module.__dict__)
    return module
def ci(repo):return local(Path(repo)/'tests/qualification/wide_span_parser_ci_v1/successor.py')
def reader(repo):return local(Path(repo)/'tests/qualification/stdlib_trim_storage_v1/retained.py')
def rows(repo=REPO):
    raw=reader(repo).regular(repo,ROOT+'originals.json',MANIFEST[0])
    need(hashlib.sha256(raw).hexdigest()==MANIFEST[1],'fixed fixture originals manifest')
    value=json.loads(raw)
    need(value['baseline_tree']=='ebdd96e45c67f472f252f1c7ca918f757891cb52' and value['schema']=='oxid-wide-span-fixture-storage-originals-v1','fixed fixture originals provenance')
    result={row['path']:row for row in value['files']}
    need(len(result)==len(value['files'])==14,'fixed fixture originals membership')
    return result
def names(repo=REPO):return set(rows(repo))
def read_original(name,repo=REPO):
    mapping=rows(repo);need(name in mapping,'unlisted fixture original: '+name)
    row=mapping[name];raw=reader(repo).regular(repo,row['storage'],row['bytes'])
    need(hashlib.sha256(raw).hexdigest()==row['sha256'],'fixed fixture original body: '+name)
    return raw


OUTPUTS = ('tests/fixtures/typed_project_source_binding/run.py', 'tests/fixtures/typed_project_source_binding/package-manifest.json', 'tests/qualification/lexer_reservation_current/unit1_current_support.py', 'tests/qualification/lexer_reservation_current/test_current_source.py', 'tests/qualification/unit4_parser_current/test_current.py', 'tests/qualification/unit4_parser_current/test_lexer_reservation.py', 'tests/qualification/unit4_parser_current/authority.json', 'tests/qualification/unit4_parser_current/portable.py', 'tests/qualification/unit4_public_v3/authority.py', 'tests/qualification/unit4_public_v3/build.py', 'tests/qualification/unit4_public_v3/test_lexer_reservation.py', 'scripts/test_published_formatter_source.py', 'scripts/test_verify_repo_fixture_data.py', 'tests/qualification/wide_span_fixture_storage_v1/predecessor.py', 'tests/qualification/wide_span_fixture_storage_v1/parser-authority.json', 'tests/qualification/wide_span_fixture_storage_v1/lifecycle-proof.json', 'tests/qualification/wide_span_fixture_storage_v1/lifecycle.py', 'tests/qualification/wide_span_fixture_storage_v1/hosted_controls.py', 'tests/qualification/wide_span_fixture_storage_v1/hosted-derivation.json', 'scripts/verify_owned_array_native.py')

def namespace_files(repo=REPO):
    static = ('README.md', 'derive.py', 'successor.py', 'test_derivation.py', 'test_storage.py', 'originals.json')
    return {ROOT+n for n in static} | {n for n in OUTPUTS if n.startswith(ROOT)} | {row["storage"] for row in rows(repo).values()}

derive=local(HERE/'derive.py')
r=types.SimpleNamespace(need=need,exact=exact)

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
    repo=Path(repo);baseline=ci(repo).baseline(repo).Baseline(repo);r=baseline.module('retained');f=r.Fixture(repo);out={}
    g=types.ModuleType('trim_source_generation')
    g.__file__=str(repo/Q/'generation.py')
    exec(compile(f.read(Q+'generation.py'),g.__file__,'exec'),g.__dict__)
    def read(n):return f.read(n)
    def put(n,b):
        r.need(n not in out,'duplicate active output');out[n]=b
    def ident(n):return g.identity(n,out[n])
    def replace(n,old,new,count=1):return r.exact(read(n).decode(),old,new,count).encode()
    helper=derive.derive(g.regular(repo,derive.PREVIOUS));put(ROOT+'predecessor.py',helper)
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
# Explicit fixture-storage successor; historical package_json_lifecycle.py remains intact.
_FIXTURE_LIFECYCLE = None

def _fixture_lifecycle():
    global _FIXTURE_LIFECYCLE
    from authority import PACKAGE_JSON_LIFECYCLE_HELPER
    import types
    from published_lifecycle import _read_regular
    row = PACKAGE_JSON_LIFECYCLE_HELPER
    root = Path(__file__).absolute().parents[3]
    raw, _ = _read_regular(root / row['path'])
    need(len(raw) == row['bytes'] and sha(raw) == row['sha256'], 'fixed fixture lifecycle helper')
    if _FIXTURE_LIFECYCLE is None:
        module = types.ModuleType('fixture_current_lifecycle')
        module.__file__ = str(root / row['path'])
        exec(compile(raw, module.__file__, 'exec'), module.__dict__)
        _FIXTURE_LIFECYCLE = module
    return _FIXTURE_LIFECYCLE

'''
    n=PUBLIC+'build.py';raw=replace(n,'    import package_json_lifecycle as current','    current = _fixture_lifecycle()',2).decode()
    raw=r.exact(raw,'def verify_lifecycle_successor(raw, source=None):',loader+'def verify_lifecycle_successor(raw, source=None):');put(n,raw.encode())
    n=PUBLIC+'test_lexer_reservation.py';raw=replace(n,'import package_json_lifecycle as m','from build import _fixture_lifecycle\nm = _fixture_lifecycle()').decode()
    raw=r.exact(raw,"(m.HERE / 'package-json-lifecycle-v1.json')","(m.REPOSITORY / '"+ROOT+"lifecycle-proof.json')");put(n,raw.encode())
    h=f.module('derive_hosted');hosted,receipt=h.derive(read(Q+'predecessor-v1/'+h.FACADE),dispatcher,seal)
    put(ROOT+'hosted_controls.py',hosted)
    facade=r.exact(h.facade(hosted).decode(),Q+'hosted_controls.py',ROOT+'hosted_controls.py').encode();put(h.FACADE,facade)
    receipt['successor']=ident(ROOT+'hosted_controls.py');put(ROOT+'hosted-derivation.json',g.encoded(receipt))
    n='scripts/test_verify_repo_fixture_data.py'
    put(n,local(HERE/'test_derivation.py').derive(read_original(n,repo)))
    n='scripts/verify_owned_array_native.py'
    before='                "tests/qualification/stdlib_text_case_v1/",'
    after=before+'\n                "tests/qualification/wide_span_fixture_storage_v1/",\n                "tests/qualification/wide_span_parser_ci_v1/",'
    put(n,exact(read_original(n,repo).decode(),before,after).encode())
    need(set(out)==set(OUTPUTS),'exact fixture output membership')
    return out

if __name__=='__main__':
    need(sys.argv[1:] in (['--check'],['--write']),'use --check or --write')
    output=generate()
    for name,body in output.items():
        path=REPO/name
        if sys.argv[1]=='--write':path.parent.mkdir(parents=True,exist_ok=True);path.write_bytes(body)
        else:need(path.read_bytes()==body,'fixture successor output: '+name)
    print('PASS',len(output),'fixture storage outputs',sys.argv[1])
