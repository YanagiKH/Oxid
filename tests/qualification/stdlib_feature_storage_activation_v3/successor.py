"""Inactive, bounded successor generation. No write-to-canonical entry point."""
import ast
import copy
import hashlib
import json
from pathlib import Path, PurePosixPath
import types
import sys
sys.dont_write_bytecode=True
HERE=Path(__file__).absolute().parent
REPO=HERE.parents[2]
ROOT='tests/qualification/stdlib_feature_storage_activation_v3/'
Q='tests/qualification/package_json_current_v1/'
S='tests/fixtures/typed_project_source_binding/'
PUBLIC='tests/qualification/unit4_public_v3/'
CANDIDATE='tests/qualification/stdlib_feature_storage_v1/candidate.py'
ORIGINALS=(6372, '15b18d3907cca149822f20343068d92c0321de412d15ace3a65ed6b318f7e1b5')
OUTPUTS=('tests/fixtures/typed_project_source_binding/run.py', 'tests/fixtures/typed_project_source_binding/package-manifest.json', 'tests/qualification/lexer_reservation_current/unit1_current_support.py', 'tests/qualification/lexer_reservation_current/test_current_source.py', 'tests/qualification/unit4_parser_current/test_current.py', 'tests/qualification/unit4_parser_current/test_lexer_reservation.py', 'tests/qualification/unit4_parser_current/authority.json', 'tests/qualification/unit4_parser_current/portable.py', 'tests/qualification/unit4_public_v3/authority.py', 'tests/qualification/unit4_public_v3/test_lexer_reservation.py', 'scripts/test_published_formatter_source.py', 'scripts/verify_owned_array_native.py', 'scripts/test_owned_array_native.py', 'scripts/test_lexer_current_ci.py', 'tests/qualification/package_json_current_v1/test_hosted.py', 'tests/qualification/unit4_ci/common.py', 'tests/qualification/unit4_ci/inputs.json', 'tests/qualification/unit4_ci/test_integration.py', 'tests/qualification/unit4_ci/test_scheduling.py')
def need(ok,message):
    if not ok: raise ValueError(message)
def exact(text,before,after,count=1):
    need(text.count(before)==count,'exact feature successor seam: '+repr(before))
    return text.replace(before,after)
def local(path,raw):
    m=types.ModuleType('feature_'+path.stem);m.__file__=str(path)
    exec(compile(raw,str(path),'exec'),m.__dict__)
    return m

def generation(repo):
    path=Path(repo)/Q/'generation.py';raw=path.read_bytes()
    need((len(raw),hashlib.sha256(raw).hexdigest())==(13680,'66ca7aee20a7d17b332ab3c2f8bad72d5882e7a52f07d86ebbd6ccaa85cae9ea'),'fixed generation bootstrap')
    g=local(path,raw)
    need(g.regular(repo,Q+'generation.py')==raw,'stable generation bootstrap')
    return g

def ci(repo):
    path=Path(repo)/'tests/qualification/wide_span_parser_ci_v1/successor.py'
    raw=generation(repo).regular(repo,path.relative_to(repo).as_posix(),5677)
    need((len(raw),hashlib.sha256(raw).hexdigest())==(5677, 'e4cc663c74652c2b6f3aee98d0df40196bf00716f0ac156017ea04d42d97bb0b'),'fixed CI reader implementation')
    text=raw.decode()
    anchor='        return regular(root,name,size)'
    addition="        if name=='scripts/test_owned_array_native.py':\n            raw=_feature_original(name,root);need(len(raw)==size,'fixed dependency-copy original size');return raw\n"
    text=exact(text,anchor,addition+anchor)
    text=exact(text,'    return regular(root,name,**kw)',"    if name=='scripts/test_owned_array_native.py':return _feature_original(name,root)\n    return regular(root,name,**kw)")
    m=types.ModuleType('feature_historical_ci');m.__file__=str(path)
    m._feature_original=read_original
    exec(compile(text,str(path),'exec'),m.__dict__)
    return m

def reader(repo):
    name='tests/qualification/stdlib_trim_storage_v1/retained.py'
    raw=generation(repo).regular(repo,name,RETAINED_PIN[0])
    need((len(raw),hashlib.sha256(raw).hexdigest())==RETAINED_PIN,'fixed retained bootstrap')
    return local(Path(repo)/name,raw)
RETAINED_PIN=(13023, '6ffebac5da972cd1a47467897f16246b847d917c2a31796b194624e074181955')
def read_original(name,repo=REPO):
    raw=reader(repo).regular(repo,ROOT+'originals.json',ORIGINALS[0])
    need((len(raw),hashlib.sha256(raw).hexdigest())==ORIGINALS,'fixed activation originals')
    rows={x['path']:x for x in json.loads(raw)['files']}
    need(set(rows)==set(OUTPUTS),'fixed overlay roster')
    need(name in rows,'unlisted overlay input')
    row=rows[name];raw=reader(repo).regular(repo,row['storage'],row['bytes'])
    need(hashlib.sha256(raw).hexdigest()==row['sha256'],'fixed activation original: '+name)
    return raw
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

def generate_core(repo=REPO):
    repo=Path(repo);baseline=ci(repo).baseline(repo).Baseline(repo);r=baseline.module('retained');f=r.Fixture(repo);out={}
    g=types.ModuleType('trim_source_generation')
    g.__file__=str(repo/Q/'generation.py')
    exec(compile(f.read(Q+'generation.py'),g.__file__,'exec'),g.__dict__)
    def read(n):return read_original(n,repo) if n in OUTPUTS else f.read(n)
    def put(n,b):
        r.need(n not in out,'duplicate active output');out[n]=b
    def ident(n):return g.identity(n,out[n])
    def replace(n,old,new,count=1):return r.exact(read(n).decode(),old,new,count).encode()
    helper=g.regular(repo,CANDIDATE);need(g.sha(helper)=='ff7cd01cad7c7bb414a1973ee6f9ab3a775d8cf218a97e320d90af148c2ed1d4','fixed storage candidate')
    raw=read(S+'run.py');node=next(n for n in ast.parse(raw).body if isinstance(n,ast.Assign) and isinstance(n.targets[0],ast.Name) and n.targets[0].id=='_HELPER_SPECS')
    specs=ast.literal_eval(node.value)
    row=next(x for x in specs if x[0]=='predecessor');r.need(row[1]==g.identity('tests/qualification/wide_span_fixture_storage_v1/predecessor.py',g.regular(repo,'tests/qualification/wide_span_fixture_storage_v1/predecessor.py')),'fixed old predecessor pin')
    row[1]=g.identity(CANDIDATE,helper);put(S+'run.py',assignment(raw,'_HELPER_SPECS',specs))
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
    n=PUBLIC+'test_lexer_reservation.py'
    put(n,exact(read(n).decode(), 'wide_span_fixture_storage_v1/lifecycle-proof.json', 'stdlib_feature_storage_activation_v3/lifecycle-proof.json').encode())
    h=f.module('derive_hosted');hosted,receipt=h.derive(read(Q+'predecessor-v1/'+h.FACADE),dispatcher,seal)
    put(ROOT+'hosted_controls.py',hosted)
    facade=r.exact(h.facade(hosted).decode(),Q+'hosted_controls.py',ROOT+'hosted_controls.py').encode();put(h.FACADE,facade)
    receipt['successor']=ident(ROOT+'hosted_controls.py');put(ROOT+'hosted-derivation.json',g.encoded(receipt))
    n='scripts/verify_owned_array_native.py'
    anchor='                "tests/qualification/wide_span_parser_ci_v1/",'
    addition=anchor+'\n                "tests/qualification/stdlib_feature_storage_v1/",\n                "tests/qualification/stdlib_feature_storage_activation_v3/",'
    put(n,exact(read(n).decode(),anchor,addition).encode())
    return out


def generate(repo=REPO,composition='combined'):
    repo=Path(repo);g,package_files=producer_inputs(repo)
    verifier=package_files[ROOT+'current_features.py']
    features=local(repo/ROOT/'current_features.py',verifier)
    feature_rows=features.verify(repo,composition)
    expected=('docs/API.md','docs/MODULES.md','stdlib/numbers.ox','tests/legacy_modules.rs')
    need(composition in ('base','numbers','tables','combined'),'fixed producer composition')
    if composition in ('numbers','combined'):expected+=('tests/stdlib_numbers.rs',)
    need(type(feature_rows) is list and tuple(row['path'] for row in feature_rows)==expected,'fixed independent feature roster')
    out=generate_core(repo)
    def put(n,raw):need(n not in out,'duplicate output');out[n]=raw
    def read(n):return read_original(n,repo)
    n='tests/qualification/package_json_current_v1/test_hosted.py'
    put(n,exact(read(n).decode(),'wide_span_parser_ci_v1/adapters.py','stdlib_feature_storage_activation_v3/adapters.py').encode())
    for n in ['scripts/test_lexer_current_ci.py','tests/qualification/unit4_ci/test_integration.py','tests/qualification/unit4_ci/test_scheduling.py']:
        raw=read(n).decode();raw=raw.replace('wide_span_parser_ci_v1/successor.py','stdlib_feature_storage_activation_v3/successor.py')
        if n=='scripts/test_lexer_current_ci.py':
            anchor="                 'tests/qualification/wide_span_parser_ci_v1',"
            raw=exact(raw,anchor,anchor+"\n                 'tests/qualification/stdlib_feature_storage_v1',\n                 'tests/qualification/stdlib_feature_storage_activation_v3',")
            anchor='        tracked.update({name: {} for name in dependencies})'
            raw=exact(raw,anchor,anchor+'\n        tracked.update({name: {} for name in '+repr([r['path'] for r in feature_rows])+'})')
        put(n,raw.encode())
    n='scripts/verify_owned_array_native.py'
    anchor='    selected_names = {name for name in tracked if name.startswith(prefixes)}'
    out[n]=exact(out[n].decode(),anchor,'    names.update('+repr([r['path'] for r in feature_rows])+')\n'+anchor).encode()
    n='scripts/test_owned_array_native.py';raw=read(n).decode()
    anchor='        environment = {"PATH": "/usr/bin:/bin", "HOME": os.environ["HOME"], "GIT_CONFIG_GLOBAL": "/dev/null", "GIT_CONFIG_NOSYSTEM": "1"}'
    insert="        for package in ('stdlib_feature_storage_v1', 'stdlib_feature_storage_activation_v3'):\n            shutil.copytree(source_repo/'tests/qualification'/package, repo/'tests/qualification'/package)\n"
    insert+='        for name in '+repr([r['path'] for r in feature_rows])+':\n            target=repo/name;target.parent.mkdir(parents=True,exist_ok=True);target.write_bytes((source_repo/name).read_bytes())\n'
    put(n,exact(raw,anchor,insert+anchor).encode())
    # The inventory is the sole per-composition physical descriptor. Derived
    # self-references are checked by verify_inventory, not recursively hashed.
    manifest=json.loads(read('tests/qualification/unit4_ci/inputs.json'))
    rows={r['path']:r for r in manifest['files']}

    excluded={ROOT+'inputs-stdlib-feature-storage-v1.json',ROOT+'activation-manifest.json'}
    excluded.update(ROOT+'overlay/'+n for n in OUTPUTS)
    for n,b in {**package_files,**out}.items():
        if n not in excluded and (n in rows or n.startswith(ROOT) or n.startswith('tests/qualification/stdlib_feature_storage_v1/')):
            rows[n]=g.identity(n,b)
    for row in feature_rows:rows[row['path']]={k:row[k] for k in ('path','bytes','sha256')}
    manifest.update(schema='oxid-unit4-ci-stdlib-feature-inputs-v1',files=[rows[n] for n in sorted(rows)],feature_composition=composition,
        feature_storage={'authority_issued':False,'derived_metadata_excluded':sorted(excluded),'source_members':376,'compiler_bodies':291})
    manifest['closed_roots'].extend(['tests/qualification/stdlib_feature_storage_v1',ROOT+'archive'])
    raw=g.encoded(manifest)
    put('tests/qualification/unit4_ci/inputs.json',raw)
    put(ROOT+'inputs-stdlib-feature-storage-v1.json',raw)
    n='tests/qualification/unit4_ci/common.py';raw=assignment(read(n),'INPUTS_SHA',g.sha(raw)).decode()
    anchor="\ndef relative_path_order(paths, root):"
    insert="    need(sha((json.dumps(manifest,indent=2,sort_keys=True)+'\\n').encode()) == INPUTS_SHA, 'fixed complete feature inventory')\n"
    insert+="    import types\n    feature_path=Path(repo)/'"+ROOT+"current_features.py'\n    feature_raw=feature_path.read_bytes()\n    need(sha(feature_raw)=="+repr(g.sha(verifier))+", 'fixed feature verifier')\n    feature=types.ModuleType('current_features');feature.__file__=str(feature_path)\n    exec(compile(feature_raw,str(feature_path),'exec'),feature.__dict__)\n    feature.verify(repo,"+repr(composition)+")\n"
    put(n,exact(raw,anchor,insert+anchor).encode())
    need({n for n in out if not n.startswith(ROOT)}==set(OUTPUTS),'exact canonical overlay output roster')
    return out

# Historical control APIs remain explicitly routed to their unchanged owner.
_old=ci(REPO)
TEXT=_old.TEXT
baseline=_old.baseline
activation=_old.activation
predecessor_output=_old.predecessor_output
predecessor=_old.predecessor
verify_workflow=_old.verify_workflow
fixture=_old.fixture
check_predecessor_outputs=_old.check_predecessor_outputs
rows=_old.rows
storage=_old.storage
sha=_old.sha
workflow=_old.workflow
STEP=_old.STEP
ANCHOR=_old.ANCHOR
U=_old.U
STORAGE=_old.STORAGE

def load(path,raw=None,**scope):
    path=Path(path)
    if raw is None and path==REPO/'tests/qualification/wide_span_parser_ci_v1/test_successor.py':
        raw=path.read_text()
        raw=exact(raw,"exec(compile(p.read_bytes(),str(p),'exec'),v.__dict__)","v=_historical_ci")
        raw=exact(raw,"manifest=json.loads((v.REPO/v.U/'inputs.json').read_bytes())", "manifest=json.loads(_original(v.U+'inputs.json'))")
        raw=exact(raw,'        v.verify_inventory()','        _verify_current()')
        scope.update(_historical_ci=_old,_original=read_original,_verify_current=verify_inventory)
    return _old.load(path,raw,**scope)

def verify_inventory(repo=REPO):
    repo=Path(repo);raw=(repo/U/'inputs.json').read_bytes();manifest=json.loads(raw)
    expected=generate(repo,manifest['feature_composition'])
    for n,b in expected.items():need((repo/n).read_bytes()==b,'exact feature successor output: '+n)
    logical=('README.md','derive.py','successor.py','adapters.py','test_activation.py','current-feature-files-v1.json','current_features.py','parser-authority.json','lifecycle.py','lifecycle-proof.json','hosted_controls.py','hosted-derivation.json','consumer-routes.json','originals.json','inputs-stdlib-feature-storage-v1.json','activation-manifest.json')
    names={ROOT+n for n in logical}|{ROOT+'archive/'+n+'.data' for n in OUTPUTS}|{ROOT+'overlay/'+n for n in OUTPUTS}
    actual=set()
    for p in (repo/ROOT).rglob('*'):
        need(not p.is_symlink() and (p.is_file() or p.is_dir()),'unsafe activation namespace member')
        if p.is_file():actual.add(p.relative_to(repo).as_posix())
    need(actual==names,'closed activation namespace')
    for n in OUTPUTS:need((repo/ROOT/'overlay'/n).read_bytes()==expected[n],'review overlay differs: '+n)
    receipt={'schema':'oxid-inactive-feature-overlay-v1','composition':manifest['feature_composition'],'authority_issued':False,
             'outputs':[{'path':n,'bytes':len(b),'sha256':sha(b)} for n,b in sorted(expected.items())]}
    need(json.loads((repo/ROOT/'activation-manifest.json').read_bytes())==receipt,'exact inactive output receipt')
    q=local(repo/U/'common.py',expected[U+'common.py']);q.verify_package(repo,manifest)


def producer_inputs(repo):
    """Authenticate finite source inputs before execution; do not learn a roster."""
    g=generation(repo)
    fixed={name:pin for name,pin in PRODUCER_INPUTS}
    logical=('README.md','derive.py','successor.py','adapters.py','test_activation.py','current-feature-files-v1.json','current_features.py','parser-authority.json','lifecycle.py','lifecycle-proof.json','hosted_controls.py','hosted-derivation.json','consumer-routes.json','originals.json','inputs-stdlib-feature-storage-v1.json','activation-manifest.json')
    allowed={ROOT+n for n in logical}|{ROOT+'archive/'+n+'.data' for n in OUTPUTS}|{ROOT+'overlay/'+n for n in OUTPUTS}
    allowed|={n for n in fixed if not n.startswith(ROOT)}
    expected_dirs={p.as_posix() for n in allowed for p in Path(n).parents}
    actual=set()
    for directory in (ROOT,'tests/qualification/stdlib_feature_storage_v1/'):
        for p in (Path(repo)/directory).rglob('*'):
            name=p.relative_to(repo).as_posix()
            need(not p.is_symlink() and (p.is_file() or p.is_dir()),'unsafe producer namespace member')
            if p.is_file():actual.add(name)
            else:need(name in expected_dirs,'unlisted producer directory')
    need(actual<=allowed and set(fixed)<=actual,'closed producer source roster')
    captured={}
    for name,pin in PRODUCER_INPUTS:
        raw=g.regular(repo,name,pin[0]);need((len(raw),g.sha(raw))==pin,'fixed producer input: '+name);captured[name]=raw
    for name in OUTPUTS:captured[ROOT+'archive/'+name+'.data']=read_original(name,repo)
    captured[ROOT+'successor.py']=g.regular(repo,ROOT+'successor.py')
    derive=local(Path(repo)/ROOT/'derive.py',captured[ROOT+'derive.py'])
    previous=g.regular(repo,'tests/qualification/wide_span_parser_ci_v1/adapters.py')
    need(derive.derive(previous)==captured[ROOT+'adapters.py'],'exact generated direct adapters')
    routes=json.loads(captured[ROOT+'consumer-routes.json'])
    need(routes=={'schema':'oxid-feature-consumer-routes-v1','predecessor':CANDIDATE,'lifecycle':ROOT+'lifecycle.py','hosted':ROOT+'hosted_controls.py','adapters':ROOT+'adapters.py','canonical_overlay_targets':list(OUTPUTS),'authority_issued':False},'exact consumer routes')
    return g,captured

PRODUCER_INPUTS=(('tests/qualification/stdlib_feature_storage_activation_v3/README.md', (3887, 'a5e78610cef3e7bab881828ca86a1b1940299730e5dc7fb3de9238badf3699dc')), ('tests/qualification/stdlib_feature_storage_activation_v3/adapters.py', (2074, '68d88019b48d53d93f6a7ec3d46e0a3d6ffc62259fe9cb8ba5b5b9fff8f22a9f')), ('tests/qualification/stdlib_feature_storage_activation_v3/consumer-routes.json', (1578, '2b4ada32ac2879dcbd74a337e99ce80ada5027d3ce97a8b9df00812887cc4987')), ('tests/qualification/stdlib_feature_storage_activation_v3/current-feature-files-v1.json', (4937, 'e25d24ec64d951e7607dddbb9780737bf28fa2abbf75a138006adfc685ee07c4')), ('tests/qualification/stdlib_feature_storage_activation_v3/current_features.py', (1793, '20df3a6b5ce7b39f2ec10eb55699b420962144b8d81160d65ac0a3a11999d29c')), ('tests/qualification/stdlib_feature_storage_activation_v3/derive.py', (989, '3b1419dbc93e0eddbcedae6bbc94e1b99c1df45e78ec9216c8b31c1951b5b22f')), ('tests/qualification/stdlib_feature_storage_activation_v3/originals.json', (6372, '15b18d3907cca149822f20343068d92c0321de412d15ace3a65ed6b318f7e1b5')), ('tests/qualification/stdlib_feature_storage_activation_v3/test_activation.py', (12708, 'e7eb934bda1c7b264230ff78ea81e65d0ebf7e1e94f5ebc9bb74b4cf1ca1fdbf')), ('tests/qualification/stdlib_feature_storage_v1/README.md', (1022, '5e4e4d705a0000ffc107056aebabcee782477c86b3b2a4243487a4884caaa2e4')), ('tests/qualification/stdlib_feature_storage_v1/archive/docs/API.md.data', (2358, 'a022ce558db0032ae34716a0dfa18d52b30fe1e99dc664b50ac030e5f72315d8')), ('tests/qualification/stdlib_feature_storage_v1/archive/docs/MODULES.md.data', (3249, '2787a21a75d110724bb53c32693c46e3efc72e08ee0d7fbf8b20044020392da7')), ('tests/qualification/stdlib_feature_storage_v1/archive/stdlib/numbers.ox.data', (315, 'a8c0d74e2e8b7e7964122ce8c9ce8806334dda6b8c55cb55c75a5a5695b14b3d')), ('tests/qualification/stdlib_feature_storage_v1/archive/test_storage.py.data', (10003, '4a6c3adb9db770e8d0fa5ab2aa5377e7ad7b29c3d5a0a3b8d2f7835c74c00494')), ('tests/qualification/stdlib_feature_storage_v1/archive/tests/legacy_modules.rs.data', (7782, '53cb238bba1626bd45868ea1e640cb7a3214c5474a744b5cec60a3cb1f9349f4')), ('tests/qualification/stdlib_feature_storage_v1/candidate-manifest.json', (2027, '89c3db83542db47d70dcd9c979afc53ff2374ca79ab547c7c3e8223e31013017')), ('tests/qualification/stdlib_feature_storage_v1/candidate.py', (11178, 'ff7cd01cad7c7bb414a1973ee6f9ab3a775d8cf218a97e320d90af148c2ed1d4')), ('tests/qualification/stdlib_feature_storage_v1/derive.py', (3108, 'aaffbbcd59d486831f9b931f829271f1c2b8642a3367eb135dc91502bc643b28')), ('tests/qualification/stdlib_feature_storage_v1/originals.json', (1663, 'c1acb6ed7e8f10ae053a028991b195dc2c13e297a8108875b37be168719b70cc')), ('tests/qualification/stdlib_feature_storage_v1/storage_test_derivation.py', (6904, '27f452f8128a0fcbca4431c9debd908bf57033ac8aba4524c705174ac59b1f65')), ('tests/qualification/stdlib_feature_storage_v1/test_storage.py', (14925, '8dcd07e0b8ef80685e270622e35e42a376aee91fc1da7dd8e47bb884e30dbc43')))
