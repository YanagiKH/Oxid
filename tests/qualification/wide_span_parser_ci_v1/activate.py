"""Review-time CI-only successor; explicit byte derivations, never hash refreshes."""
import ast
import json
from pathlib import Path
import sys
import types
sys.dont_write_bytecode=True
HERE=Path(__file__).absolute().parent
REPO=HERE.parents[2]
p=HERE/'successor.py';v=types.ModuleType('wide_ci_activation');v.__file__=str(p)
exec(compile(p.read_bytes(),str(p),'exec'),v.__dict__)
ROOT=v.ROOT;U=v.U

def encoded(value):return (json.dumps(value,indent=2,sort_keys=True)+'\n').encode()
def identity(name,raw):return {'path':name,'bytes':len(raw),'sha256':v.sha(raw)}
def assignment(raw,name,value):
    text=raw.decode();nodes=[n for n in ast.parse(text).body if isinstance(n,ast.Assign) and len(n.targets)==1 and isinstance(n.targets[0],ast.Name) and n.targets[0].id==name]
    v.need(len(nodes)==1,'exact assignment: '+name);n=nodes[0];lines=text.splitlines(True)
    lines[n.lineno-1:n.end_lineno]=[name+' = '+repr(value)+'\n'];return ''.join(lines).encode()
LOADER='''
# Named CI-only successor; historical workflow assertions use its pinned view.
import types as _wide_types
_wide_path = REPO / 'tests/qualification/wide_span_parser_ci_v1/successor.py'
_wide_ci = _wide_types.ModuleType('wide_ci_controls'); _wide_ci.__file__ = str(_wide_path)
exec(compile(_wide_path.read_bytes(), str(_wide_path), 'exec'), _wide_ci.__dict__)
'''

def generate(repo=REPO):
    repo=Path(repo);out={}
    v.check_predecessor_outputs(repo)
    storage=v.storage(repo);storage_outputs=storage.generate(repo)
    out.update(storage_outputs)
    n='.github/workflows/ci.yml';out[n]=v.workflow(v.predecessor(n,repo))
    disposition=(repo/'tests/qualification/package_json_current_v1/package-json-scheduling-dispositions-v1.json').read_bytes()
    v.need(len(disposition)==195245 and v.sha(disposition)=='aeb5c6b0d0cd487c16d97b4562250f975d7da2a3588b324c9053459252270003','unchanged 173 dispositions')
    out[ROOT+'workflow-proof.json']=encoded({
        'schema':'oxid-wide-span-parser-ci-derivation-v1',
        'predecessor':identity(n,v.predecessor(n,repo)),
        'successor':identity(n,out[n]),
        'insertion_anchor':v.ANCHOR,'inserted_step':v.STEP,
        'old_dispositions':identity('tests/qualification/package_json_current_v1/package-json-scheduling-dispositions-v1.json',
            (repo/'tests/qualification/package_json_current_v1/package-json-scheduling-dispositions-v1.json').read_bytes()),
        'scope':{'job':'local-hir-producers','added_jobs':0,'job_timeout_minutes':40,
                 'retained_direct_obligations':173,'source_only':True,'execution_qualified':False}})
    n='scripts/test_lexer_current_ci.py';raw=v.predecessor(n,repo).decode()
    raw=v.exact(raw,"HELPER = REPO / 'tests/qualification/lexer_reservation_current'",LOADER+"\nHELPER = REPO / 'tests/qualification/lexer_reservation_current'")
    raw=v.exact(raw,"(REPO / '.github/workflows/ci.yml').read_bytes()","_wide_ci.verify_workflow(REPO)",3)
    raw=v.exact(raw,"                 'tests/qualification/stdlib_text_case_v1',",
        "                 'tests/qualification/stdlib_text_case_v1',\n                 'tests/qualification/wide_span_fixture_storage_v1',\n                 'tests/qualification/wide_span_parser_ci_v1',")
    hook="\ndef load_tests(loader, tests, pattern):\n    module=_wide_ci.load(REPO / 'tests/qualification/wide_span_parser_ci_v1/test_successor.py')\n    tests.addTests(loader.loadTestsFromModule(module))\n    storage_module=_wide_ci.load(REPO / 'tests/qualification/wide_span_fixture_storage_v1/test_storage.py')\n    tests.addTests(loader.loadTestsFromModule(storage_module))\n    return tests\n\n"
    raw=v.exact(raw,"if __name__ == '__main__':",hook+"if __name__ == '__main__':")
    out[n]=raw.encode()
    n='tests/qualification/package_json_current_v1/test_hosted.py';raw=v.predecessor(n,repo).decode()
    out[n]=v.exact(raw,'stdlib_text_case_v1/adapters.py','wide_span_parser_ci_v1/adapters.py').encode()
    n=U+'test_scheduling.py';raw=v.predecessor(n,repo).decode()
    raw=v.exact(raw,'import derive_scheduling as activation','import derive_scheduling as activation\nREPO = activation.REPO\n'+LOADER)
    raw=v.exact(raw,'fixture = Fixture(activation.REPO)','fixture = _wide_ci.fixture(activation.REPO)',2)
    raw=v.exact(raw,"(activation.REPO/'.github/workflows/ci.yml').read_bytes()","_wide_ci.verify_workflow(activation.REPO)")
    raw=v.exact(raw,"from retained import Fixture; Fixture(Path.cwd()).assert_scheduling_outputs(); ","")
    old="import types; p=Path('tests/qualification/stdlib_text_case_v1/activate.py').absolute(); m=types.ModuleType('text_checkout_activation'); m.__file__=str(p); exec(compile(p.read_bytes(),str(p),'exec'),m.__dict__); m.base.Baseline(Path.cwd()).assert_outputs(); assert all((m.REPO/n).read_bytes()==body for n,body in m.generate().items())"
    new="import types; p=Path('tests/qualification/wide_span_parser_ci_v1/successor.py').absolute(); m=types.ModuleType('wide_checkout_activation'); m.__file__=str(p); exec(compile(p.read_bytes(),str(p),'exec'),m.__dict__); m.fixture(Path.cwd()).assert_scheduling_outputs(); m.check_predecessor_outputs(Path.cwd()); m.verify_workflow(Path.cwd()); m.verify_inventory(Path.cwd())"
    raw=v.exact(raw,old,new);out[n]=raw.encode()
    n=U+'test_integration.py';raw=v.predecessor(n,repo).decode()
    # Keep all old inventory ancestry/count/manifest assertions against the fixed
    # predecessor, then independently admit the complete active successor.
    anchor="class LexerInputClosureControls(unittest.TestCase):"
    raw=v.exact(raw,anchor,LOADER+'\n'+anchor)
    raw=v.exact(raw,"raw = (q.HERE / 'inputs.json').read_bytes()", "raw = _wide_ci.predecessor('tests/qualification/unit4_ci/inputs.json', REPO)")
    raw=v.exact(raw,'self.assertEqual(q.sha(raw), q.INPUTS_SHA)',"self.assertEqual(q.sha(raw), '3f43f3077a0b2924d5b9a7c84671441fd3d3da7f0728f2ba493cfc345f6b76a2')")
    raw=v.exact(raw,'q.verify_package(REPO, current)','_wide_ci.verify_inventory(REPO)')
    out[n]=raw.encode()
    oldraw=v.predecessor(U+'inputs.json',repo);manifest=json.loads(oldraw)
    oldrows={row['path']:row for row in manifest['files']}
    extras={ROOT+n for n in ('README.md','successor.py','activate.py','adapters.py','test_successor.py','predecessor.json','workflow-proof.json')}
    extras.update(row['storage'] for row in v.rows(repo).values())
    physical={p.relative_to(repo).as_posix() for p in (repo/ROOT).rglob('*') if p.is_file()}
    v.need(physical<=extras,'unlisted CI successor namespace member')
    v.need(not any('__pycache__' in n or n.endswith(('.pyc','.pyo')) for n in extras),'no generated caches')
    # These are source-only runtime entrypoints, not qualification evidence.
    extras.update(['.github/workflows/ci.yml','scripts/run_wide_span_parser_ci.py','scripts/build_wide_span_parser.py',
                   'scripts/verify_wide_span_parser.py','scripts/test_wide_span_parser.py'])
    builder=v.load(repo/'scripts/build_wide_span_parser.py')
    builder.sources(repo)
    extras.update(builder.OVERLAY_FILES)
    extras.add('fixtures/typed-parser-wide-span/README.md')
    extras.update(storage.namespace_files(repo))
    allowed={'.github/workflows/ci.yml','scripts/test_lexer_current_ci.py','tests/qualification/package_json_current_v1/test_hosted.py'}
    allowed.update(storage_outputs)
    rows=[]
    for n in sorted(set(oldrows)|extras):
        body=out[n] if n in out else (repo/n).read_bytes();row=identity(n,body)
        if n in oldrows and n not in allowed:v.need(row==oldrows[n],'unapproved successor input change: '+n)
        rows.append(row)
    manifest.update(schema='oxid-unit4-ci-wide-span-parser-inputs-v1',files=rows,
        wide_span_ci_predecessor=identity(U+'inputs-stdlib-text-v1.json',oldraw),
        wide_span_ci={'source_only':True,'execution_qualified':False,'added_steps':1,'added_jobs':0})
    manifest['closed_roots'].extend([ROOT.rstrip('/'), 'fixtures/typed-parser-wide-span',storage.ROOT.rstrip('/')])
    raw=encoded(manifest);out[U+'inputs-wide-span-parser-ci-v1.json']=raw;out[U+'inputs.json']=raw
    out[U+'common.py']=assignment(v.predecessor(U+'common.py',repo),'INPUTS_SHA',v.sha(raw))
    return out

if __name__=='__main__':
    v.need(sys.argv[1:] in (['--check'],['--write']),'use --check or --write')
    outputs=generate()
    for name,body in outputs.items():
        path=REPO/name
        if sys.argv[1]=='--write':path.parent.mkdir(parents=True,exist_ok=True);path.write_bytes(body)
        else:v.need(path.read_bytes()==body,'successor output mismatch: '+name)
    print('PASS',len(outputs),'CI-only successor outputs',sys.argv[1])
