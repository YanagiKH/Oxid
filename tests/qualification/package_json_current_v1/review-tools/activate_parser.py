"""Review-time derivation only; requires the real accepted-main Git objects.

Writes the named successor outputs. Never installs tools, builds or publishes.
"""
import ast,json,pathlib,subprocess,sys
sys.dont_write_bytecode=True
repo=pathlib.Path(__file__).absolute().parents[4];root=repo/'tests/qualification/package_json_current_v1';sys.path.insert(0,str(root));import generation as g, parser as support
paths=['tests/qualification/unit4_parser_current/portable.py','tests/qualification/unit4_parser_current/authority.json']
for name in paths:
 target=root/'predecessor-v1'/name;target.parent.mkdir(parents=True,exist_ok=True);target.write_bytes(subprocess.check_output(['git','show',g.MAIN['head']+':'+name],cwd=repo))
subprocess.run([sys.executable,str(pathlib.Path(__file__).with_name('activate_successor.py'))],check=True)
prev=(repo/g.PACKAGE/g.PREDECESSOR_NAME).read_bytes();inputs={r['path']:(repo/r['path']).read_bytes() for r in json.loads(prev)['files']}
outputs,restored=g.generate(prev,inputs,(root/'generated'/g.PATCH_NAME).read_bytes(),{n:(root/'source-proof'/(n+'.json')).read_bytes() for n in g.READBACKS},(repo/'tests/fixtures/typed_project_source_binding_byte_storage_v1/run.py').read_bytes())
admitted={'current':json.loads(outputs[g.CURRENT_NAME]),'inputs':inputs,'predecessor_inputs':restored,'artifacts':outputs}
dispatcher=g.identity(g.PACKAGE+'run.py',(repo/g.PACKAGE/'run.py').read_bytes())
old=json.loads((repo/'tests/fixtures/typed_project_unit4_parser_portable/frozen/v3/authority.json').read_bytes())
old['current']=json.loads((root/'predecessor-v1/tests/qualification/unit4_parser_current/authority.json').read_bytes())
old['current_source']=json.loads(prev)
inserted=json.loads((repo/'tests/qualification/unit4_public_v3/published-lifecycle-v1.json').read_bytes())['inserted_roster']
a=support.transport(old,admitted,dispatcher,inserted)
raw=g.encoded(a['current']);(repo/paths[1]).write_bytes(raw);(root/'generated-parser-authority-v1.json').write_bytes(raw)
seal={'dispatcher':dispatcher,'support':g.identity(g.ROOT+'parser.py',(root/'parser.py').read_bytes())}
original=(root/'predecessor-v1'/paths[0]).read_text();lines=original.splitlines(True)
bodies={
'authority':"def authority():\n    support, admitted, seal, dispatcher, p = _package_json_context()\n    return support.authority(p, admitted, seal, dispatcher)\n",
'phase_previous':"def phase_previous(a):\n    support, admitted, seal, dispatcher, p = _package_json_context()\n    return support.phase_previous(p, a, admitted, seal, dispatcher)\n",
'current_candidate':"def current_candidate(a):\n    support, source = _package_json_modules()\n    return support.candidate(a)\n",
'compose_current_lexer':"def compose_current_lexer(a, raw):\n    support, admitted, seal, dispatcher, p = _package_json_context()\n    return support.compose(p, a, raw, 'src/frontend/lexer.rs', False, 'lexer', admitted, seal, dispatcher)\n",
'compose_array_instrumentation':"def compose_array_instrumentation(a, name, raw, control=False):\n    support, admitted, seal, dispatcher, p = _package_json_context()\n    return support.compose(p, a, raw, name, control, 'array', admitted, seal, dispatcher)\n",
'compose_namespace_resource':"def compose_namespace_resource(a, raw):\n    support, admitted, seal, dispatcher, p = _package_json_context()\n    return support.compose(p, a, raw, 'src/frontend/declaration_index/resource.rs', False, 'namespace', admitted, seal, dispatcher)\n",
'compose_source_read':"def compose_source_read(a, raw):\n    support, admitted, seal, dispatcher, p = _package_json_context()\n    return support.compose(p, a, raw, 'src/frontend/source.rs', False, 'source', admitted, seal, dispatcher)\n"}
edits=[]
for node in ast.parse(original).body:
 if isinstance(node,ast.FunctionDef) and node.name in bodies:edits.append((node.lineno,node.end_lineno,bodies[node.name]))
 elif isinstance(node,ast.Assign) and isinstance(node.targets[0],ast.Name) and node.targets[0].id=='AUTHORITY_SHA':edits.append((node.lineno,node.end_lineno,'AUTHORITY_SHA = '+repr(g.sha(raw))+'\n'))
assert len(edits)==8
for start,end,replacement in sorted(edits,reverse=True):lines[start-1:end]=[replacement]
loader='''
# New named source transport. Every executable dependency is pinned before load.
def _package_json_modules():
    seal = _PACKAGE_JSON_PARSER_SEAL
    bodies = {}
    for key in ('dispatcher', 'support'):
        row = seal[key]
        verify_map(REPOSITORY, [row])
        raw = (REPOSITORY / row['path']).read_bytes()
        same({'path': row['path'], 'bytes': len(raw), 'sha256': sha(raw)}, row, 'package JSON module identity')
        bodies[key] = raw
    source = types.ModuleType('package_json_parser_source')
    source.__file__ = str(REPOSITORY / seal['dispatcher']['path'])
    exec(compile(bodies['dispatcher'], source.__file__, 'exec'), source.__dict__)
    support = types.ModuleType('package_json_parser_support')
    support.__file__ = str(REPOSITORY / seal['support']['path'])
    exec(compile(bodies['support'], support.__file__, 'exec'), support.__dict__)
    return support, source


def _package_json_context():
    support, source = _package_json_modules()
    captured = source.preflight(REPOSITORY)
    admitted = {'current': captured['current'], 'inputs': captured['inputs'],
                'predecessor_inputs': captured['package_json_predecessor']['inputs'],
                'artifacts': {n: captured['package_bytes'][n] for n in
                             ('package-json-source-v1.json', 'package-json-authority-v1.json',
                              'package-json-source-proof-v1.json', 'package-json-transition-v1.patch')}}
    p = types.SimpleNamespace(REPOSITORY=REPOSITORY, AUTHORITY_SHA=AUTHORITY_SHA)
    return support, admitted, source._SOURCE_SEAL, _PACKAGE_JSON_PARSER_SEAL['dispatcher'], p

'''
text=''.join(lines)
# Insert before CLI dispatch so direct execution sees the new loader.
index=text.index("if __name__ == \"__main__\":") if 'if __name__ == "__main__":' in text else text.index("if __name__ == '__main__':")
text=text[:index]+'_PACKAGE_JSON_PARSER_SEAL = '+repr(seal)+'\n'+loader+text[index:]
(repo/paths[0]).write_text(text)
print('parser actual counts',{k:len(a['current'][k]) for k in ['source_delta','current_base_files','current_derived_files','current_control_derived_files']})
