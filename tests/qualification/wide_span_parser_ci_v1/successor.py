"""One exact CI-only step, with a finite authenticated predecessor byte view."""
import hashlib
import json
from pathlib import Path
import types
import sys
sys.dont_write_bytecode = True
HERE = Path(__file__).absolute().parent
REPO = HERE.parents[2]
ROOT = 'tests/qualification/wide_span_parser_ci_v1/'
TEXT = 'tests/qualification/stdlib_text_case_v1/'
U = 'tests/qualification/unit4_ci/'
MANIFEST_SHA = '1f6412ff8bb58a7386ab937ac96f3b5cfe470c2a5207c45f5fb52615f3051911'
WORKFLOW_SHA = '9c6e7b902f18e24d4b4f81af77e915463171bdddb2d7cd8fa504c0be9135ec51'
STEP = '''      - name: Verify wide-span parser runtime in both qualified ordinary profiles
        run: |
          python3 -B scripts/run_wide_span_parser_ci.py \\
            --receipt "$RUNNER_TEMP/local-hir-producers/receipt.json" \\
            --llvm-bin "$OXID_LLVM_BIN" \\
            --output "$RUNNER_TEMP/local-hir-producers/wide-span"
'''
ANCHOR = '      - name: Retain producer builds, observations and failure evidence\n'

def sha(raw): return hashlib.sha256(raw).hexdigest()
def need(ok, message):
    if not ok: raise ValueError(message)
def exact(raw, old, new, count=1):
    need(raw.count(old) == count, 'exact successor seam: '+repr(old))
    return raw.replace(old, new)
def load(path, raw=None, **scope):
    m=types.ModuleType('wide_span_'+path.stem);m.__file__=str(path)
    m.__dict__.update(scope)
    exec(compile(path.read_bytes() if raw is None else raw,str(path),'exec'),m.__dict__)
    return m

def reader(repo=REPO):
    # Reuse the unchanged strict nonaliasing reader, without monkeypatching it.
    return load(Path(repo)/'tests/qualification/stdlib_trim_storage_v1/retained.py')

def rows(repo=REPO):
    r=reader(repo);raw=r.regular(repo,ROOT+'predecessor.json',2189)
    need(sha(raw)==MANIFEST_SHA,'fixed CI predecessor manifest')
    value=json.loads(raw)
    need(value['baseline_tree']=='ebdd96e45c67f472f252f1c7ca918f757891cb52','CI predecessor tree')
    return {row['path']:row for row in value['files']}

def predecessor(name, repo=REPO):
    row=rows(repo)[name];raw=reader(repo).regular(repo,row['storage'],row['bytes'])
    need(sha(raw)==row['sha256'],'fixed CI predecessor body: '+name)
    return raw

STORAGE = 'tests/qualification/wide_span_fixture_storage_v1/'

def storage(repo=REPO):
    return load(Path(repo)/STORAGE/'successor.py')

def predecessor_output(name, repo=REPO):
    if name in rows(repo):return predecessor(name,repo)
    st=storage(repo)
    return st.read_original(name,repo) if name in st.names(repo) else (Path(repo)/name).read_bytes()

def workflow(raw):
    need(len(raw)==71965 and sha(raw)==WORKFLOW_SHA,'fixed complete workflow predecessor')
    return exact(raw,ANCHOR.encode(),(STEP+ANCHOR).encode())

def verify_workflow(repo=REPO):
    old=predecessor('.github/workflows/ci.yml',repo)
    need((Path(repo)/'.github/workflows/ci.yml').read_bytes()==workflow(old),'exact wide-span workflow successor')
    return old

def fixture(repo=REPO):
    r=reader(repo);regular=r.regular;st=storage(repo);storage_names=st.names(repo)
    def fixed(root,name,size):
        if name=='.github/workflows/ci.yml':
            raw=predecessor(name,root);need(size==71965 and len(raw)==71965 and sha(raw)==WORKFLOW_SHA,'fixed predecessor workflow size/hash');return raw
        if name in storage_names:
            raw=st.read_original(name,root);need(len(raw)==size,'fixed storage original size');return raw
        return regular(root,name,size)
    r.regular=fixed
    return r.Fixture(repo)

def baseline(repo=REPO):
    b=load(Path(repo)/TEXT/'baseline.py');Original=b.Baseline
    class FixedBaseline(Original):
        def execute(self,name,path,raw,extra=None):
            m=super().execute(name,path,raw,extra)
            if name=='retained':
                fixed=fixture(self.repo)
                m.regular=fixed.__class__.__init__.__globals__['regular']
            return m
    b.Baseline=FixedBaseline
    return b

def activation(repo=REPO):
    path=Path(repo)/TEXT/'activate.py';raw=path.read_text()
    raw=exact(raw,"base=_local('baseline')","base=_view.baseline(REPO)")
    # Only the workflow changed among the predecessor generator's physical inputs.
    anchor="    exec(compile(f.read(Q+'generation.py'),g.__file__,'exec'),g.__dict__)\n"
    raw=exact(raw,anchor,anchor+"    _regular=g.regular;_storage=_view.storage(REPO);_names=_storage.names(REPO)\n    g.regular=lambda root,name,**kw: _view.historical_input(root,name,_regular,_storage,_names,**kw)\n")
    return load(path,raw,_view=sys.modules[__name__] if __name__ in sys.modules else _SELF)

# load() users need no process-global module registration.
def historical_input(root,name,regular,st,names,**kw):
    if name=='.github/workflows/ci.yml':return predecessor(name,root)
    if name in names:return st.read_original(name,root)
    return regular(root,name,**kw)

_SELF = types.SimpleNamespace(baseline=baseline,predecessor=predecessor,historical_input=historical_input,storage=storage)

def check_predecessor_outputs(repo=REPO):
    b=baseline(repo);b.Baseline(repo).assert_outputs()
    outputs=activation(repo).generate(repo)
    for name,body in outputs.items():
        need(predecessor_output(name,repo)==body,'reproduced text predecessor: '+name)
    return outputs

def verify_inventory(repo=REPO):
    repo=Path(repo)
    a=load(repo/ROOT/'activate.py')
    for name,body in a.generate(repo).items():
        need((repo/name).read_bytes()==body,'exact successor output: '+name)
    q=load(repo/U/'common.py')
    raw=(repo/U/'inputs.json').read_bytes()
    need(sha(raw)==q.INPUTS_SHA,'active CI inventory pin')
    q.verify_package(repo,json.loads(raw))
