"""Review-time derivation only; requires the real accepted-main Git objects.

Writes the named successor outputs. Never installs tools, builds or publishes.
"""
import ast,json,pathlib,subprocess,sys
sys.dont_write_bytecode=True
repo=pathlib.Path(__file__).absolute().parents[4];root=repo/'tests/qualification/package_json_current_v1';sys.path.insert(0,str(root));import generation as g
package=repo/g.PACKAGE
for p in (root/'generated').iterdir():(package/p.name).write_bytes(p.read_bytes())
(package/'current-source.json').write_bytes((root/'generated'/g.CURRENT_NAME).read_bytes())
# Preserve the exact accepted facades before replacing the active dispatcher.
def old(name):return subprocess.check_output(['git','show',g.MAIN['head']+':'+name],cwd=repo)
original=old(g.PACKAGE+'run.py').decode()
(root/'predecessor-v1'/g.PACKAGE).mkdir(parents=True,exist_ok=True)
for name in ['run.py','current-source.json','package-manifest.json']:
 (root/'predecessor-v1'/g.PACKAGE/name).write_bytes(old(g.PACKAGE+name))
view=json.loads((root/'accepted-view-v1.json').read_bytes())
for row in view['files']:
 name=row['path']
 if row['read_from'] is not None and (root/'predecessor-v1'/name).exists():row['read_from']=g.ROOT+'predecessor-v1/'+name
(root/'accepted-view-v1.json').write_bytes(g.encoded(view))
seal={'source_manifest':g.identity(g.CURRENT_NAME,(root/'generated'/g.CURRENT_NAME).read_bytes()),'predecessor_view':g.identity(g.ROOT+'accepted-view-v1.json',(root/'accepted-view-v1.json').read_bytes())}
(root/'source-seal-v1.json').write_bytes(g.encoded(seal))
values={}
for node in ast.parse(original).body:
 if isinstance(node,ast.Assign) and isinstance(node.targets[0],ast.Name) and node.targets[0].id in ['_HELPER_SPECS','_SOURCE_SEAL']:
  values[node.targets[0].id]=(node,ast.literal_eval(node.value))
helpers=values['_HELPER_SPECS'][1]
for module,filename in [('generation','generation.py'),('predecessor','predecessor.py'),('package_current','package_current.py')]:
 helpers.append([module,g.identity(g.ROOT+filename,(root/filename).read_bytes())])
lines=original.splitlines(True)
for key,value in [('_HELPER_SPECS',helpers),('_SOURCE_SEAL',seal)]:
 node=values[key][0];lines[node.lineno-1:node.end_lineno]=[key+' = '+repr(value)+'\n']
text=''.join(lines).replace("_modules['published_current'].install", "_modules['package_current'].install")
(package/'run.py').write_text(text)
manifest=json.loads(old(g.PACKAGE+'package-manifest.json'))
names=sorted({r['path'] for r in manifest['files']}|{p.name for p in (root/'generated').iterdir()})
manifest['files']=[g.identity(n,(package/n).read_bytes()) for n in names]
(package/'package-manifest.json').write_bytes(g.encoded(manifest))
print('activated dispatcher',len(names),'package entries',seal)
