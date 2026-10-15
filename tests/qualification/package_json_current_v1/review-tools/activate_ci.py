"""Review-time derivation only; requires the real accepted-main Git objects.

Writes the named successor outputs. Never installs tools, builds or publishes.
"""
import pathlib,sys,ast,json
sys.dont_write_bytecode=True
repo=pathlib.Path(__file__).absolute().parents[4];root=repo/'tests/qualification/package_json_current_v1';sys.path.insert(0,str(root));import generation as g,derive_consumers as d,ci
saved=root/'predecessor-v1'; source=(repo/g.PACKAGE/'current-source.json').read_bytes()
oldplan=(repo/'tests/qualification/published_formatter_current/published-direct-consumer-dispositions-v1.json').read_bytes()
oldworkflow=(saved/'.github/workflows/ci.yml').read_bytes();workflow=(repo/'.github/workflows/ci.yml').read_bytes()
plan=ci.derive(oldplan,oldworkflow,workflow,source);(root/ci.NAME).write_bytes(g.encoded(plan))
p='scripts/test_lexer_current_ci.py';raw=(saved/p).read_bytes()
for n,v in {'CURRENT_SHA':g.sha(source),'COMPILER_HEAD':g.CHECKPOINT['head'],'COMPILER_TREE':g.CHECKPOINT['tree'],
 'CURRENT_SOURCE_DESCRIPTOR':g.identity(g.CURRENT_NAME,source),'CURRENT_WORKFLOW_DESCRIPTOR':g.identity('.github/workflows/ci.yml',workflow),
 'CURRENT_DISPOSITIONS_DESCRIPTOR':g.identity(ci.NAME,g.encoded(plan))}.items():raw=d.assignment(raw,n,v)
s=raw.decode();s=s.replace("current_raw = (PUBLISHED / 'published-direct-consumer-dispositions-v1.json').read_bytes()", "current_raw = (REPO / 'tests/qualification/package_json_current_v1/package-json-direct-consumer-dispositions-v1.json').read_bytes()")
a=s.index('        complete = published_ci_inventory.validate(');b=s.index('        self.assertEqual(complete',a)
s=s[:a]+'''        sys.path.insert(0, str(REPO / 'tests/qualification/package_json_current_v1'))
        import ci as successor_ci
        complete = successor_ci.validate(
            (PUBLISHED / 'published-direct-consumer-dispositions-v1.json').read_bytes(),
            (REPO / 'tests/qualification/package_json_current_v1/predecessor-v1/.github/workflows/ci.yml').read_bytes(),
            (REPO / '.github/workflows/ci.yml').read_bytes(),
            (SOURCE / 'current-source.json').read_bytes(), current_raw)
'''+s[b:]
s=s.replace("'tests/qualification/published_formatter_current',\n", "'tests/qualification/published_formatter_current',\n                 'tests/qualification/package_json_current_v1',\n")
s=s.replace("        tracked.update({name: {} for name in dependencies})", "        successor = read(REPO / 'tests/qualification/package_json_current_v1/accepted-view-v1.json')\n        dependencies.update(row['read_from'] for row in successor['files'] if row['read_from'] is not None)\n        tracked.update({name: {} for name in dependencies})")
(repo/p).write_text(s)
# Keep all historical metadata; add an explicit named outer input generation.
p='tests/qualification/unit4_ci/inputs.json';oldraw=(saved/p).read_bytes();value=json.loads(oldraw)
value['schema']='oxid-unit4-ci-package-json-inputs-v1'
value['closed_roots'].append(g.ROOT.rstrip('/'))
value['package_json_source']=json.loads(source)['package_json_source']
value['package_json_predecessor_inputs']=g.identity('tests/qualification/unit4_ci/inputs-published-formatter-v1.json',oldraw)
# Exact file list is emitted and reviewed as input_roster, not runtime discovery.
names={r['path'] for r in value['files']} | {'scripts/test_package_json_source.py'}
for directory in value['closed_roots']:
 names.update(p.relative_to(repo).as_posix() for p in (repo/directory).rglob('*') if p.is_file())
names.add(g.ROOT+'input-roster-v1.json')
(root/'input-roster-v1.json').write_bytes(g.encoded({'schema':'oxid-package-json-ci-input-roster-v1','paths':sorted(names),'source_only':True,'execution_qualified':False}))
value['files']=[g.identity(n,(repo/n).read_bytes()) for n in sorted(names)]
newraw=g.encoded(value);(repo/p).write_bytes(newraw);(repo/'tests/qualification/unit4_ci/inputs-package-json-v1.json').write_bytes(newraw)
p='tests/qualification/unit4_ci/common.py';raw=(saved/p).read_bytes();raw=d.assignment(raw,'CURRENT_SHA',g.sha(source));raw=d.assignment(raw,'INPUTS_SHA',g.sha(newraw));(repo/p).write_bytes(raw)
p='tests/qualification/unit4_ci/test_integration.py';s=(saved/p).read_text()
s=s.replace("(q.HERE / 'inputs-published-formatter-v1.json').read_bytes()", "(q.HERE / 'inputs-package-json-v1.json').read_bytes()")
s=s.replace("['tests/qualification/published_formatter_current'])", "['tests/qualification/published_formatter_current', 'tests/qualification/package_json_current_v1'])")
s=s.replace("'tests/qualification/published_formatter_current'])", "'tests/qualification/published_formatter_current', 'tests/qualification/package_json_current_v1'])")
s=s.replace('(980, 16)',repr((len(value['files']),len(value['closed_roots']))))
s=s.replace("'published_lifecycle.py'}", "'published_lifecycle.py', 'package_json_lifecycle.py', 'package-json-lifecycle-v1.json'}")
(repo/p).write_text(s)
print('CI input roster',len(value['files']),'closed roots',len(value['closed_roots']))
