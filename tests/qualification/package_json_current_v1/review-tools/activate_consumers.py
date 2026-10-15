"""Review-time derivation only; requires the real accepted-main Git objects.

Writes the named successor outputs. Never installs tools, builds or publishes.
"""
import pathlib,sys,subprocess,json,ast,types
sys.dont_write_bytecode=True
repo=pathlib.Path(__file__).absolute().parents[4];root=repo/'tests/qualification/package_json_current_v1';sys.path.insert(0,str(root));import generation as g,derive_consumers as d
public='tests/qualification/unit4_public_v3/'
# All mutable accepted facades are saved before finalizing the view seal.
paths=set(d.SIMPLE)|{d.SUPPORT,d.BINDINGS,public+'authority.py',public+'build.py',public+'test_lexer_reservation.py',
 'scripts/test_lexer_current_ci.py','tests/qualification/unit4_ci/common.py','tests/qualification/unit4_ci/test_integration.py',
 'tests/qualification/unit4_ci/inputs.json'}
for name in sorted(paths):
 target=root/'predecessor-v1'/name;target.parent.mkdir(parents=True,exist_ok=True);target.write_bytes(subprocess.check_output(['git','show',g.MAIN['head']+':'+name],cwd=repo))
subprocess.run([sys.executable,str(pathlib.Path(__file__).with_name('activate_parser.py'))],check=True)
originals={n:(root/'predecessor-v1'/n).read_bytes() for n in set(d.SIMPLE)|{d.SUPPORT,d.BINDINGS}}
readbacks={n:(root/'source-proof'/(n+'.json')).read_bytes() for n in g.READBACKS};tree,_=g.readbacks(readbacks)
source=(repo/g.PACKAGE/'current-source.json').read_bytes();dispatcher=(repo/g.PACKAGE/'run.py').read_bytes();package=(repo/g.PACKAGE/'package-manifest.json').read_bytes()
outputs,receipt=d.derive(originals,tree,source,dispatcher,package,(root/"accepted-view-v1.json").read_bytes())
for n,b in outputs.items():(repo/n).write_bytes(b)
(root/'source-proof/consumer-derivation-v1.json').write_bytes(g.encoded(receipt))
helper=repo/public/'package_json_lifecycle.py';helper.write_bytes(d.assignment(helper.read_bytes(),'DISPATCHER',g.identity(g.PACKAGE+'run.py',dispatcher)))
sys.path.insert(0,str(repo/public));import package_json_lifecycle as l
old=json.loads((repo/public/'published-lifecycle-v1.json').read_bytes());seal=json.loads((root/'source-seal-v1.json').read_bytes())
proof=l.make_proof(old,json.loads(source),g.identity(g.PACKAGE+'run.py',dispatcher),seal)
proof_raw=g.encoded(proof);(repo/public/'package-json-lifecycle-v1.json').write_bytes(proof_raw)
authority=(root/'predecessor-v1'/public/'authority.py').read_bytes()
canonical=lambda v:json.dumps(v,sort_keys=True,separators=(',',':')).encode()
for n,value in {'CURRENT_SOURCE_SHA':g.sha(source),'CURRENT_FILES_SHA':g.sha(canonical(proof['base_files'])),
 'OBSERVER_FILES_SHA':g.sha(canonical(proof['observer_files']))}.items():authority=d.assignment(authority,n,value)
authority+=('PACKAGE_JSON_LIFECYCLE_HELPER = '+repr(g.identity('package_json_lifecycle.py',helper.read_bytes()))+'\nPACKAGE_JSON_LIFECYCLE_PROOF = '+repr(g.identity('package-json-lifecycle-v1.json',proof_raw))+'\n').encode()
(repo/public/'authority.py').write_bytes(authority)
body=(root/'predecessor-v1'/public/'build.py').read_bytes();body=d.exact(body,b'import published_lifecycle as current',b'import package_json_lifecycle as current',2);(repo/public/'build.py').write_bytes(body)
body=(root/'predecessor-v1'/public/'test_lexer_reservation.py').read_bytes();body=d.exact(body,b'import published_lifecycle as m',b'import package_json_lifecycle as m',1);body=d.exact(body,b"'published-lifecycle-v1.json'",b"'package-json-lifecycle-v1.json'",1);(repo/public/'test_lexer_reservation.py').write_bytes(body)
print('Updated finite native/Unit1/workflow/public consumers')
