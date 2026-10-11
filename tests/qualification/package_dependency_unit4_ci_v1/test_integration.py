"""Pure CI admission controls. No producer, compiler or runtime is invoked."""
import hashlib,json,os,sys,tempfile,types,unittest
from pathlib import Path
from unittest.mock import patch
sys.dont_write_bytecode=True
HERE=Path(__file__).resolve().parent
KEY='_oxid_package_dependency_unit4_ci_common_v1'

def load_common():
 m=types.ModuleType(KEY);m.__file__=str(HERE/'common.py');sys.modules[KEY]=m
 exec(compile((HERE/'common.py').read_bytes(),m.__file__,'exec'),m.__dict__)
 return m

class AdmissionTests(unittest.TestCase):
 def setUp(self):
  self.prior=sys.modules.pop(KEY,None);self.q=load_common()
 def tearDown(self):
  for module in self.q._SIBLINGS.values():
   if sys.modules.get(module.__name__) is module:sys.modules.pop(module.__name__)
  sys.modules.pop(KEY,None)
  if self.prior is not None:sys.modules[KEY]=self.prior
 def test_sealed_sibling_canonical_owners(self):
  with patch('subprocess.Popen',side_effect=AssertionError('no process calls')):
   self.q.package()
   for name in ('gate','staging','evidence','join','facade'):
    module=self.q.load_sibling(name);self.assertIs(self.q.load_sibling(name),module)
    self.assertIs(module.common_api() if name=='facade' else module._common(),self.q)
 def test_json_closed_values(self):
  for raw in (b'{"x":1,"x":2}',b'NaN',b'Infinity'):
   with self.subTest(raw=raw),self.assertRaises(ValueError):self.q.decode(raw)
  self.assertEqual(self.q.decode(self.q.canonical({'x':[1,False,None]})),{'x':[1,False,None]})
 def test_safe_paths(self):
  for p in ('','.', '..','a/../b','/tmp/x','a//b','a\\b','C:x'):
   with self.subTest(p=p),self.assertRaises(ValueError):self.q.safe_relative(p)
  self.assertEqual(self.q.safe_relative('a/b.py'),'a/b.py')
 def test_opaque_records(self):
  for cls in (self.q.CIContext,self.q.JoinContext):
   with self.assertRaises(ValueError):cls()
   forged=object.__new__(cls)
   with self.assertRaises(ValueError):forged.document()
   with self.assertRaises(AttributeError):forged.document_value={}
 def test_canonical_owner_slot(self):
  self.assertIs(self.q.owner(),self.q);sys.modules[KEY]=types.ModuleType(KEY)
  with self.assertRaises(ValueError):self.q.owner()
 def test_unissued_sibling_rejected(self):
  with self.assertRaises(ValueError):self.q.owns_sibling(types.ModuleType('fake'))
 def test_five_complete_domains(self):
  q=self.q;self.assertEqual(len(q.DOMAINS),5)
  for d in q.DOMAINS:
   p=q.host_policy(d);self.assertEqual(p['profiles'],['debug','release'])
   self.assertEqual(p['public_required'],d!='linux-parser')
   self.assertEqual(p['parser_required'],d=='linux-parser')
   self.assertEqual(p['root_required'],d=='linux-public')
   self.assertEqual(len(p['public_sections']),5 if p['public_required'] else 0)
  with self.assertRaises(ValueError):q.host_policy('linux-public-subset')
 def test_regular_body_and_link_rejection(self):
  with tempfile.TemporaryDirectory() as td:
   p=Path(td)/'x';p.write_bytes(b'abc');self.assertEqual(self.q.read(p),b'abc')
   with self.assertRaises(ValueError):self.q.read(p,2)
   link=Path(td)/'link';link.symlink_to(p)
   with self.assertRaises(ValueError):self.q.read(link)
 def test_deadline_read(self):
  self.q._CURRENT_DEADLINE=0
  with self.assertRaises(ValueError):self.q.read(HERE/'common.py')
 def test_snapshot_deadline_restored_on_failure(self):
  q=self.q;q._CURRENT_DEADLINE=100
  with patch.object(q,'_snapshot',side_effect=RuntimeError('failure')):
   with self.assertRaises(RuntimeError):q._bounded_snapshot(Path('/x'),'a','b',None)
  self.assertEqual(q._CURRENT_DEADLINE,100)
 def test_activation_is_fixed_closed(self):
  q=self.q;doc={'activation':dict(q.ACTIVATION),'workflow':{'sha256':'a'*64}}
  with patch.object(q,'verify_join_context',return_value=doc):
   with self.assertRaisesRegex(ValueError,'unapproved'):q.require_activation(object())
  self.assertEqual(q.ACTIVATION['job_limit_seconds'],21600)
 def test_snapshot_drift_rejects_live_context(self):
  q=self.q;snap={'repository_path':'/repo','checkout_head':'a'*40,'event_sha':'b'*40,'domain':'linux-public'}
  c=q.CIContext(q._TOKEN);q._CONTEXTS[c]={'document':q.canonical(snap),'snapshot':q.canonical({'x':1})}
  with patch.object(q,'_bounded_snapshot',return_value={'x':2}):
   with self.assertRaisesRegex(ValueError,'drift'):q.verify_ci(c)
 def test_documents_are_detached(self):
  q=self.q;c=q.CIContext(q._TOKEN);q._CONTEXTS[c]={'document':q.canonical({'x':{'v':1}})}
  d=c.document();d['x']['v']=2;self.assertEqual(c.document()['x']['v'],1)
 def test_committed_exact_batched_membership(self):
  q=self.q;head='a'*40;tree='b'*40;calls=[]
  with tempfile.TemporaryDirectory() as td:
   repo=Path(td);names=[('x'*90)+str(i) for i in range(300)]
   for n in names:(repo/n).write_bytes(n.encode())
   def git(r,*args):
    calls.append(args)
    if args==('rev-parse','HEAD'):return (head+'\n').encode()
    if args==('rev-parse','HEAD^{tree}'):return (tree+'\n').encode()
    self.assertEqual(args[:4],('ls-tree','-rz',head,'--'))
    return b''.join(b'100644 blob '+hashlib.sha1(b'blob '+str(len(n)).encode()+b'\0'+n.encode()).hexdigest().encode()+b'\t'+n.encode()+b'\0' for n in args[4:])
   with patch.object(q,'_git',side_effect=git):self.assertEqual(q._committed(repo,head,[{'path':n} for n in names]),tree)
   batches=[c[4:] for c in calls if c[0]=='ls-tree'];self.assertGreater(len(batches),1)
   self.assertEqual([n for b in batches for n in b],sorted(names))
 def test_committed_missing_dirty_and_extra(self):
  q=self.q;head='a'*40
  with tempfile.TemporaryDirectory() as td:
   repo=Path(td);(repo/'a').write_bytes(b'x')
   for raw in (b'',b'100644 blob '+b'0'*40+b'\ta\0',b'100644 blob '+b'0'*40+b'\textra\0'):
    def git(r,*args):return (head+'\n').encode() if args==('rev-parse','HEAD') else raw
    with patch.object(q,'_git',side_effect=git),self.assertRaises(ValueError):q._committed(repo,head,[{'path':'a'}])
 def test_empty_committed_roster_no_pathspec(self):
  q=self.q;head='a'*40;calls=[]
  def git(r,*args):calls.append(args);return (head+'\n').encode()
  with patch.object(q,'_git',side_effect=git):q._committed(Path('/repo'),head,[])
  self.assertTrue(all(c[0]=='rev-parse' for c in calls))
 def test_domain_metadata_rejects_local_and_wrong_jobs(self):
  with patch.dict(os.environ,{'GITHUB_ACTIONS':'false'},clear=True):
   with self.assertRaisesRegex(ValueError,'GitHub Actions'):self.q._metadata(Path('/repo'),'a'*40,'b'*40,'linux-public')
 def test_metadata_exact_run_head_and_domain(self):
  q=self.q;head='a'*40;merge='b'*40
  with tempfile.TemporaryDirectory() as td:
   repo=Path(td);(repo/'.github/workflows').mkdir(parents=True);(repo/'.github/workflows/ci.yml').write_bytes(b'workflow')
   event=repo/'event.json';event.write_text(json.dumps({'repository':{'full_name':'YanagiKH/Oxid'},'pull_request':{'head':{'sha':head},'merge_commit_sha':merge}}))
   env={'GITHUB_ACTIONS':'true','GITHUB_REPOSITORY':'YanagiKH/Oxid','GITHUB_RUN_ID':'123','GITHUB_RUN_ATTEMPT':'2','GITHUB_WORKFLOW':'CI','GITHUB_EVENT_NAME':'pull_request','GITHUB_JOB':'unit4-linux','GITHUB_SHA':merge,'GITHUB_EVENT_PATH':str(event),'GITHUB_WORKSPACE':str(repo),'GITHUB_WORKFLOW_REF':'YanagiKH/Oxid/.github/workflows/ci.yml@refs/pull/1/merge','GITHUB_WORKFLOW_SHA':head,'UNIT4_CI_DOMAIN':'linux-public'}
   with patch.dict(os.environ,env,clear=True),patch.object(q,'_git',return_value=b'workflow'):
    ci,_,_,upstream=q._metadata(repo,head,merge,'linux-public');self.assertEqual(ci['run_attempt'],'2');self.assertIsNone(upstream)
    for key,value in [('GITHUB_RUN_ATTEMPT','0'),('GITHUB_JOB','unit4-parser'),('UNIT4_CI_DOMAIN','linux-parser'),('GITHUB_SHA',head),('UNIT4_CI_NEEDS_JSON','{}')]:
     with self.subTest(key=key),patch.dict(os.environ,{key:value}),self.assertRaises(ValueError):q._metadata(repo,head,merge,'linux-public')
    with self.assertRaises(ValueError):q._metadata(repo,'c'*40,merge,'linux-public')
 def test_join_expected_results_are_own_capture(self):
  q=self.q;head='a'*40
  with tempfile.TemporaryDirectory() as td:
   repo=Path(td);(repo/'.github/workflows').mkdir(parents=True);(repo/'.github/workflows/ci.yml').write_bytes(b'w')
   event=repo/'event.json';event.write_text(json.dumps({'repository':{'full_name':'YanagiKH/Oxid'}}))
   env={'GITHUB_ACTIONS':'true','GITHUB_REPOSITORY':'YanagiKH/Oxid','GITHUB_RUN_ID':'123','GITHUB_RUN_ATTEMPT':'2','GITHUB_WORKFLOW':'CI','GITHUB_EVENT_NAME':'push','GITHUB_JOB':'unit4-qualification','GITHUB_SHA':head,'GITHUB_EVENT_PATH':str(event),'GITHUB_WORKSPACE':str(repo),'GITHUB_WORKFLOW_REF':'YanagiKH/Oxid/.github/workflows/ci.yml@refs/heads/main','GITHUB_WORKFLOW_SHA':head,'UNIT4_CI_DOMAIN':'join','UNIT4_CI_NEEDS_JSON':json.dumps({job:'success' for job in set(q.JOBS.values())})}
   with patch.dict(os.environ,env,clear=True),patch.object(q,'_git',return_value=b'w'):
    _,_,_,upstream=q._metadata(repo,head,head,None);self.assertEqual(set(upstream),set(q.JOBS.values()))
    with patch.dict(os.environ,{'UNIT4_CI_NEEDS_JSON':'{}'}),self.assertRaises(ValueError):q._metadata(repo,head,head,None)
 def test_no_unissued_invocation(self):
  with self.assertRaises(ValueError):self.q.invocation(object())
 def test_no_unissued_api(self):
  with self.assertRaises(ValueError):self.q.apis({})



# Inert archive and original-owner negative controls supplied by transport writer.
import copy,importlib.util,io,tarfile,time
from unittest import mock
def _load_transport_control(path, name):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def _transport_controls(package):
    package = Path(package).resolve()
    evidence = _load_transport_control(package / 'evidence.py', '_test_only_ci_evidence')
    join = _load_transport_control(package / 'join.py', '_test_only_ci_join')

    class EvidenceJoinControls(unittest.TestCase):
        def raw_transport(self, path='/producer/raw.json', raw=b'{}'):
            bound = {'path': path, 'bytes': len(raw), 'sha256': evidence.sha(raw)}
            row = {'identity': bound, 'role': 'raw-evidence', 'storage': 'compact'}
            seal = {'schema': evidence.SEAL_SCHEMA, 'host': {}, 'inventory': {
                'files': [row], 'links': [], 'directories': [], 'excluded_regenerable_caches': []},
                'started_ns': 1, 'completed_ns': 2, 'status': 'sealed'}
            manifest = {'schema': evidence.CAPSULE_SCHEMA, 'status': 'sealed', 'seal': seal,
                        'members': [{**row, 'member': evidence.member_name(path)}],
                        'full_archive': {'path': 'full.tar.xz', 'bytes': 1, 'sha256': 'a' * 64},
                        'replay_boundary': 'inert synthetic bytes only'}
            payloads = {evidence.member_name(path): raw}
            return manifest, payloads

        def test_live_seal_constructor_is_closed(self):
            with self.assertRaises(TypeError): evidence.LiveSeal()

        def test_unissued_same_class_object_is_rejected(self):
            with self.assertRaises(evidence.Rejected): evidence._live(object.__new__(evidence.LiveSeal))

        def test_dictionary_cannot_export(self):
            with self.assertRaises(evidence.Rejected): evidence.export_capsule({}, '/unused')

        def test_unsealed_sealer_rejects_before_read(self):
            with self.assertRaises(evidence.Rejected):
                evidence.seal_live_host({}, source_context={}, machine={})

        def test_dictionary_expected_context_cannot_read(self):
            with self.assertRaises(evidence.Rejected): evidence.read_capsule('/unused', {})

        def test_independent_join_context_is_required(self):
            result = join.join({}, [])
            self.assertEqual(result['status'], 'failed')
            self.assertIs(result['ci_runtime_proof'], False)
            self.assertIs(result['qualification_complete'], False)

        def test_original_paths_closed(self):
            for value in ('relative', '/a/../b', '/a//b', '/a/./b', '\\\\server\\share\\file', 'C:\\run\\a:stream', ''):
                with self.subTest(value=value), self.assertRaises(evidence.Rejected): evidence.original_path(value)

        def test_transport_member_paths_closed(self):
            for value in ('../x', '/x', 'a\\b', 'a//b', 'a/./b', ''):
                with self.subTest(value=value), self.assertRaises(evidence.Rejected): evidence.safe_member(value)

        def test_windows_paths_stay_windows_on_linux(self):
            root = r'D:\a\Oxid\producer'
            self.assertEqual(evidence.child(root, 'public-runtime', 'collection'), root + r'\public-runtime\collection')
            self.assertEqual(evidence.member_name(root + r'\raw.json'), 'raw/windows/D/a/Oxid/producer/raw.json')
            self.assertEqual(str(join._remote_path(root) / 'ordinary' / 'receipt.json'), root + r'\ordinary\receipt.json')

        def test_windows_inert_raw_transport(self):
            manifest, payloads = self.raw_transport(r'D:\a\producer\raw.json')
            transport = evidence.Transport(manifest, payloads)
            self.assertEqual(transport.raw(r'D:\a\producer\raw.json'), b'{}')
            self.assertNotIsInstance(transport, evidence.LiveSeal)
            transport.close()

        def test_capsule_duplicate_path(self):
            manifest, payloads = self.raw_transport()
            manifest['seal']['inventory']['files'].append(copy.deepcopy(manifest['seal']['inventory']['files'][0]))
            with self.assertRaises(evidence.Rejected): evidence.Transport(manifest, payloads)

        def test_capsule_extra_payload(self):
            manifest, payloads = self.raw_transport(); payloads['raw/posix/extra'] = b''
            with self.assertRaises(evidence.Rejected): evidence.Transport(manifest, payloads)

        def test_capsule_omitted_member(self):
            manifest, payloads = self.raw_transport(); manifest['members'] = []
            with self.assertRaises(evidence.Rejected): evidence.Transport(manifest, payloads)

        def test_capsule_changed_hash(self):
            manifest, payloads = self.raw_transport(); payloads[next(iter(payloads))] = b'[]'
            with self.assertRaises(evidence.Rejected): evidence.Transport(manifest, payloads)

        def test_capsule_changed_storage(self):
            manifest, payloads = self.raw_transport(); manifest['members'][0]['storage'] = 'full-only'
            with self.assertRaises(evidence.Rejected): evidence.Transport(manifest, payloads)

        def test_capsule_nonfinite_and_duplicate_json(self):
            for raw in (b'{"x":NaN}', b'{"x":1,"x":2}'):
                with self.assertRaises(evidence.Rejected): evidence.decode(raw)

        def test_no_source_directory_blanket_omission(self):
            def row(name): return {'path': name, 'bytes': 2, 'sha256': evidence.sha(b'{}')}
            views = {role: {'path': '/run/prepared/' + role,
                     'files': [row('candidate-source-manifest.json'), row('src/main.rs')],
                     'metadata': [row('package-preparation-overlay.json')]}
                     for role in ('parser_observed', 'parser_control')}
            host = {'source_context': {'views': views}, 'public': None,
                    'parser': {'root': '/run/parser', 'builds': []}, 'staging': None}
            full, _, _ = evidence._classification(host)
            self.assertEqual(set(full), {'/run/prepared/parser_observed/src/main.rs', '/run/prepared/parser_control/src/main.rs',
                                       '/run/parser/source/src/main.rs', '/run/parser/control-source/src/main.rs'})
            self.assertTrue(all('candidate-source-manifest' not in path and 'preparation' not in path for path in full))

        def test_metadata_cannot_be_full_only_by_suffix(self):
            views = {'ordinary': {'path': '/run/source', 'files': [],
                      'metadata': [{'path': 'package-preparation-overlay.json', 'bytes': 2, 'sha256': evidence.sha(b'{}')}]}}
            self.assertEqual(evidence._classification({'source_context': {'views': views}, 'public': None, 'parser': None, 'staging': None})[0], {})

        def test_budget_rejects_past_nonfinite(self):
            for value in (time.monotonic() - 1, float('nan'), float('inf')):
                with self.subTest(value=value), self.assertRaises(evidence.Rejected):
                    with evidence.budget(value): pass

        def test_budget_expires_during_chunk_read(self):
            with tempfile.TemporaryDirectory() as directory:
                path = Path(directory) / 'raw'; path.write_bytes(b'abc')
                now = time.monotonic()
                with self.assertRaises(evidence.Rejected):
                    with evidence.budget(now + 60):
                        with mock.patch.object(evidence.time, 'monotonic', return_value=now + 61): evidence.identity(path)

        def test_nested_budget_cannot_expand_parent(self):
            now = time.monotonic()
            with evidence.budget(now + 60):
                with evidence.budget(now + 120): self.assertEqual(evidence._DEADLINE.get(), now + 60)

        def test_archive_rejects_symlink(self):
            with tempfile.TemporaryDirectory() as directory:
                path = Path(directory) / 'a.tar.xz'
                with tarfile.open(path, 'w:xz') as archive:
                    row = tarfile.TarInfo('capsule.json'); row.type = tarfile.SYMTYPE; row.linkname = '/etc/passwd'; archive.addfile(row)
                with self.assertRaises(evidence.Rejected): evidence._read_archive(path)

        def test_archive_rejects_duplicate_member(self):
            with tempfile.TemporaryDirectory() as directory:
                path = Path(directory) / 'a.tar.xz'
                with tarfile.open(path, 'w:xz') as archive:
                    for _ in range(2):
                        row = tarfile.TarInfo('capsule.json'); row.size = 2; archive.addfile(row, io.BytesIO(b'{}'))
                with self.assertRaises(evidence.Rejected): evidence._read_archive(path)

        def test_archive_rejects_oversize_before_read(self):
            with tempfile.TemporaryDirectory() as directory:
                path = Path(directory) / 'a.tar.xz'
                with tarfile.open(path, 'w:xz') as archive:
                    row = tarfile.TarInfo('raw'); row.size = 2; archive.addfile(row, io.BytesIO(b'{}'))
                with mock.patch.object(evidence, 'MAX_MEMBER', 1):
                    with self.assertRaises(evidence.Rejected): evidence._read_archive(path)

        def test_five_domain_stage_catalogue(self):
            self.assertEqual(len(join.DOMAINS), 5)
            linux = join.stage_catalogue('linux-public')
            self.assertIn('stage-installed-dpkg-runtime', linux)
            self.assertIn('join-twelve-hosted-replacements', linux)
            parser = join.stage_catalogue('linux-parser')
            self.assertLess(parser.index('build-parser-control-release'), parser.index('admit-complete-parser-runtime'))
            self.assertNotIn('collect-complete-public', parser)
            for domain in join.DOMAINS[2:]: self.assertNotIn('complete-seven-parser-stages', join.stage_catalogue(domain))
            with self.assertRaises(join.Rejected): join.stage_catalogue('combined')

        def transport_set(self, root, raw, host):
            manifest, _ = self.raw_transport()
            rows, payloads = [], {}
            directories = set()
            original = evidence.original_path(root)
            for path, body in sorted(raw.items()):
                row = {'identity': {'path': path, 'bytes': len(body), 'sha256': evidence.sha(body)},
                       'role': 'raw-evidence', 'storage': 'compact'}
                rows.append(row); payloads[evidence.member_name(path)] = body
                for parent in evidence.original_path(path).parents:
                    if original in parent.parents: directories.add(str(parent))
            manifest['seal']['inventory'].update(files=rows, directories=sorted(directories))
            manifest['seal']['host'] = host
            manifest['members'] = [{**row, 'member': evidence.member_name(row['identity']['path'])} for row in rows]
            return evidence.Transport(manifest, payloads)

        def public_fixture(self, root, *, source=b'prescribed source', extra=False, empty=False):
            child = lambda *parts: evidence.child(root, *parts)
            got = {'executed': False, 'fixture_directory': child('public/00000/ordinary/fixture')}
            roster = [{'scope': 'execute', 'key': 'synthetic'}]
            raw = {child('public/roster.json'): evidence.canonical([{'key': 'synthetic'}]),
                   child('public/observations.jsonl.gz'): b'inert', child('public/comparison.json'): b'{}',
                   child('public/00000/receipt.json'): evidence.canonical(got),
                   child('public/00000/ordinary/receipt.json'): evidence.canonical(got),
                   child('public/00000/ordinary/fixture/unit.ox'): source}
            if extra: raw[child('public/00000/unlisted')] = b'unlisted'
            host = {'public': {'collection_root': root, 'primary': {'reports': {'public': {}}}}}
            t = self.transport_set(root, raw, host)
            if empty: t.seal['inventory']['directories'].append(child('public/empty'))
            # An explicitly inert miniature source provider exercises the actual
            # transport adapter without loading any frozen/live consumer module.
            source = ("def _case_evidence_roster(state, section, index, row, got):\n"
                      "    path = state['collection_root'] / 'public/00000/ordinary/receipt.json'\n"
                      "    need(load(path) == got, 'counterpart load uses original path flavour')\n"
                      "    return {'public/00000/receipt.json', 'public/00000/ordinary/receipt.json', "
                      "'public/00000/ordinary/fixture/unit.ox'}, set(), set()\n")
            return t, roster, got, source

        def check_public_fixture(self, root, **mutation):
            t, roster, got, source = self.public_fixture(root, **mutation)
            with tempfile.TemporaryDirectory() as directory:
                path = Path(directory) / 'inert-source.py'; path.write_text(source)
                provider = types.SimpleNamespace(__file__=str(path), __source_sha256__=evidence.sha(source.encode()),
                    _row_sources=lambda *args: {'unit.ox': b'prescribed source'})
                helpers = {'contracts': types.SimpleNamespace(receipt_identity=lambda row: {'key': row['key']})}
                with mock.patch.object(join, '_evidence', return_value=evidence):
                    join._public_section_closure(t, {'public_runtime': provider}, helpers, None, None, 'public', roster, [got])
            t.close()

        def test_inert_public_fixture_replay_positive(self):
            self.check_public_fixture('/producer/collection')

        def test_windows_public_fixture_replay_on_linux_positive(self):
            self.check_public_fixture(r'D:\work\producer\collection')

        def test_selfconsistent_fixture_hash_mutation_rejects(self):
            with self.assertRaisesRegex(join.Rejected, 'frozen derived source'):
                self.check_public_fixture('/producer/collection', source=b'changed with fresh hash')

        def test_extra_public_case_file_rejects(self):
            with self.assertRaisesRegex(join.Rejected, 'member closure'):
                self.check_public_fixture('/producer/collection', extra=True)

        def test_extra_empty_public_directory_rejects(self):
            with self.assertRaisesRegex(join.Rejected, 'directory closure'):
                self.check_public_fixture('/producer/collection', empty=True)

        def test_archive_inert_round_trip_and_deadline(self):
            manifest, payloads = self.raw_transport()
            with tempfile.TemporaryDirectory() as directory:
                path = Path(directory) / 'capsule.tar.xz'
                with tarfile.open(path, 'w:xz') as archive:
                    for name, body in {'capsule.json': evidence.canonical(manifest), **payloads}.items():
                        row = tarfile.TarInfo(name); row.size = len(body); archive.addfile(row, io.BytesIO(body))
                value = evidence._read_archive(path)
                self.assertEqual(value.raw('/producer/raw.json'), b'{}'); value.close()
                now = time.monotonic()
                with self.assertRaises(evidence.Rejected):
                    with evidence.budget(now + 10):
                        with mock.patch.object(evidence.time, 'monotonic', return_value=now + 11): evidence._read_archive(path)

        def test_directory_member_collision_rejects(self):
            manifest, payloads = self.raw_transport()
            manifest['seal']['inventory']['directories'] = ['/producer/raw.json']
            with self.assertRaises(evidence.Rejected): evidence.Transport(manifest, payloads)

        def test_original_parser_runtime_roster_mutation(self):
            root = '/producer/parser'
            t = self.transport_set(root, {root + '/collect-debug/raw.json': b'{}'}, {})
            t.host['parser'] = {'root': root, 'sealed_runtime': [t.named(root + '/collect-debug/raw.json')]}
            provider = types.SimpleNamespace(_full_roster=lambda state: ({'collect-debug/raw.json'}, {'collect-debug'}))
            with mock.patch.object(join, '_evidence', return_value=evidence):
                join._parser_runtime_closure(t, provider, None, None, None)
                t.host['parser']['sealed_runtime'] = []
                with self.assertRaisesRegex(join.Rejected, 'sealed parser runtime identities'):
                    join._parser_runtime_closure(t, provider, None, None, None)
            t.close()

        def test_owned_process_after_seal_rejects(self):
            root = '/producer'; operation = root + '/operations/0001'
            row = {'schema': 'oxid-owned-probe-v1', 'pid': 1, 'status': 0, 'started_ns': 3, 'completed_ns': 6,
                   'timed_out': False, 'stream_limit_exceeded': False, 'cleanup_complete': True,
                   'cleanup_error': None, 'failure': None, 'invocation_nonce': 'nonce',
                   'timeout_seconds': 1, 'stream_limit_bytes': 10, 'operation_kind': 'probe',
                   'argv': ['inert'], 'cwd': root, 'environment': {}}
            for name in ('stdout', 'stderr'):
                row[name] = {'path': operation + '/' + name, 'bytes': 0, 'sha256': evidence.sha(b'')}
            raw = {operation + '/result.json': evidence.canonical(row), operation + '/stdout': b'', operation + '/stderr': b''}
            t = self.transport_set(root, raw, {'invocation': {'run_root': root, 'nonce': 'nonce', 'started_ns': 1}})
            t.seal['started_ns'] = 5
            with mock.patch.object(join, '_evidence', return_value=evidence):
                with self.assertRaisesRegex(join.Rejected, 'beyond terminal seal'): join._operations(t)
            t.close()

        def phase_catalogue(self, status=101, *, extra=False):
            root = '/producer/parser'; values = {}; processes = []
            for index, (profile, kind) in enumerate((('debug', 'passivity'), ('debug', 'collection'), ('release', 'passivity'), ('release', 'collection'))):
                process = {'pid': index + 1, 'started_ns': 2 + index * 2, 'completed_ns': 3 + index * 2, 'status': 0, 'argv': ['helper', str(index)], 'cwd': root, 'environment': {}}
                processes.append(process)
                directory = ('collect-' if kind == 'collection' else 'passivity-') + profile
                values[evidence.child(root, directory, 'package-source-' + kind + '.json')] = {'owned_process': process}
            phase = {'pid': 5, 'started_ns': 10, 'completed_ns': 11, 'status': status, 'operation_kind': 'parser-phase',
                     'timeout_seconds': 30, 'stream_limit_bytes': 4096, 'argv': ['binary', '--expected-probe'],
                     'cwd': root, 'environment': {}, 'stdout': 'actual-out', 'stderr': 'actual-err'}
            processes.append(phase)
            if extra: processes.append({'pid': 9, 'started_ns': 12, 'completed_ns': 13, 'status': 0, 'argv': ['unknown'], 'cwd': root, 'environment': {}})
            values.update({'comparison': {'u8_policy_controls': 'controls'}, 'controls': {'cases': []},
                           root + '/phase/request.json': {'expected': {'exit_code': 101}},
                           root + '/phase/receipt.json': {'started_ns': 10, 'finished_ns': 11, 'stdout': 'saved-out', 'stderr': 'saved-err'}})
            t = types.SimpleNamespace(host={'parser': {'root': root, 'processes': processes, 'report': {'comparison': 'comparison'}}},
                                      json=lambda path: values[path], raw=lambda value: b'stream', used_operations=set(), seal={'started_ns': 20})
            driver = types.SimpleNamespace(roster=lambda: [('prescribed', 'debug', 'control')], process_dir=lambda row: 'phase',
                expected=lambda *args: {'exit_code': 101}, inputs=lambda *args: (phase['argv'], root, {}))
            old = types.SimpleNamespace(phase_module=lambda *args, **kwargs: driver)
            operations = {(r['pid'], r['started_ns']): r for r in processes}
            builds = {('debug', False): (None, {'helper_probe': {'completed_ns': 1}})}
            with mock.patch.object(join, '_evidence', return_value=evidence):
                join._parser_process_catalogue(t, old, {}, builds, operations, None)

        def test_command_specific_phase101_is_preserved(self):
            self.phase_catalogue()

        def test_phase101_cannot_be_changed_to_generic_success(self):
            with self.assertRaisesRegex(join.Rejected, 'command-specific exit'): self.phase_catalogue(0)

        def test_unknown_unconsumed_parser_operation_rejects(self):
            with self.assertRaisesRegex(join.Rejected, 'duplicate parser process rows'): self.phase_catalogue(extra=True)

        def test_decoded_observation_bound(self):
            import gzip
            raw = gzip.compress(b'{}\n' * 10)
            with mock.patch.object(evidence, 'DECODED_LIMIT', 5):
                with self.assertRaises(evidence.Rejected): evidence._json_lines(raw, compressed=True)

        def environment_fixture(self, *, native=False, windows=False):
            root = r'D:\run\collection' if windows else '/run/collection'
            repository = r'D:\repo' if windows else '/repo'
            directory = evidence.child(root, 'public/00000/ordinary')
            adapter = evidence.child(repository, 'tests/qualification/unit4_public_v3')
            base = {'PATH': '/tools', 'OXID_LLVM_BIN': '/wrappers', 'UNIT4_ADAPTER_ROOT': adapter}
            env = dict(base, UNIT4_TOOL_LOG=evidence.child(directory, 'tools.jsonl'), UNIT4_TOOL_EVIDENCE=directory)
            case = {'section': 'public', 'index': 0, 'variant': 'ordinary'}
            t = types.SimpleNamespace(host={'ci_context': {'repository_path': repository},
                'public': {'collection_root': root, 'runtime_tools': {'environment': base}}})
            got = {'process': {'runtime_case': dict(case), 'environment': env, 'completed_ns': 5}}
            if native:
                got['native_execution'] = {'process': {'runtime_case': dict(case), 'environment': dict(env),
                    'argv': [evidence.child(directory, 'native-source-free/program')],
                    'cwd': evidence.child(directory, 'native-source-free'), 'started_ns': 6}}
            return t, got, case

        def test_public_and_native_environment_positive(self):
            for windows in (False, True):
                t, got, case = self.environment_fixture(native=not windows, windows=windows)
                with mock.patch.object(join, '_evidence', return_value=evidence):
                    join._public_process_environment(t, got, case, {})

        def test_compiler_injected_environment_rejects(self):
            for key in ('LD_PRELOAD', 'PYTHONPATH'):
                t, got, case = self.environment_fixture(); got['process']['environment'][key] = '/injected'
                with mock.patch.object(join, '_evidence', return_value=evidence):
                    with self.assertRaisesRegex(join.Rejected, 'compiler environment'):
                        join._public_process_environment(t, got, case, {})

        def test_wrong_public_log_destination_rejects(self):
            t, got, case = self.environment_fixture(); got['process']['environment']['UNIT4_TOOL_LOG'] = '/elsewhere'
            with mock.patch.object(join, '_evidence', return_value=evidence):
                with self.assertRaisesRegex(join.Rejected, 'compiler environment'):
                    join._public_process_environment(t, got, case, {})

        def test_native_injected_environment_rejects(self):
            t, got, case = self.environment_fixture(native=True)
            got['native_execution']['process']['environment']['LD_PRELOAD'] = '/injected'
            with mock.patch.object(join, '_evidence', return_value=evidence):
                with self.assertRaisesRegex(join.Rejected, 'native environment'):
                    join._public_process_environment(t, got, case, {})

        def test_wrong_public_variant_rejects(self):
            t, got, case = self.environment_fixture(); got['process']['runtime_case']['index'] = 1
            with mock.patch.object(join, '_evidence', return_value=evidence):
                with self.assertRaisesRegex(join.Rejected, 'case/variant'):
                    join._public_process_environment(t, got, case, {})

        def operation_policy_fixture(self, *, mutate=False):
            row = {'pid': 1, 'started_ns': 1, 'operation_kind': 'probe', 'native_workspace': None,
                   'argv': ['inert-worker'], 'cwd': '/producer', 'environment': {}, 'timeout_seconds': 180,
                   'stream_limit_bytes': 8 * 1024 ** 2, 'public_output_ledger': None, 'public_output_failure_preservation': None,
                   'observed_peaks': {'processes': 1, 'rss_bytes': 1, 'output_bytes': 0},
                   'operation_policy': {'processes': 4, 'rss_bytes': 2 * 1024 ** 3, 'output_bytes': 128 * 1024 ** 2,
                       'file_bytes': 64 * 1024 ** 2, 'process_memory_enforced_on': 'linux', 'transient_roots': [],
                       'all_hosts': ['wall', 'stream', 'output_file', 'output_total', 'owned_tree_cleanup']},
                   'preparation_resource_policy': {'wall_seconds': 180, 'stream_bytes': 8 * 1024 ** 2,
                       'output_bytes': 128 * 1024 ** 2, 'file_bytes': 64 * 1024 ** 2, 'linux_group_processes': 4,
                       'linux_group_rss_bytes': 2 * 1024 ** 3, 'linux_child_cpu_seconds': 120,
                       'linux_child_address_space_bytes': 1024 ** 3}}
            if mutate: row['operation_policy']['file_bytes'] *= 2
            t = types.SimpleNamespace(host={'invocation': {'run_root': '/producer'},
                'preparation': {'provenance': {'operation': row}}})
            source = 'def _transient_target(*args):\n    return None\n'
            with tempfile.TemporaryDirectory() as directory:
                path = Path(directory) / 'inert-policy.py'; path.write_text(source)
                provider = types.SimpleNamespace(__file__=str(path), __source_sha256__=evidence.sha(source.encode()))
                with mock.patch.object(join, '_evidence', return_value=evidence):
                    join._operation_policies(t, {'preparation': provider}, {(1, 1): row})

        def test_inert_original_preparation_resource_policy(self):
            self.operation_policy_fixture()

        def test_changed_owned_resource_policy_rejects(self):
            with self.assertRaisesRegex(join.Rejected, 'resource policy'): self.operation_policy_fixture(mutate=True)

        def test_failure_export_is_never_qualification(self):
            result = evidence.export_failure({}, '/unused', original_error={'type': 'TestError', 'message': 'original'}, deadline=time.monotonic()+1)
            self.assertEqual(result['status'], 'failed')
            self.assertEqual(result['original_error'], {'type': 'TestError', 'message': 'original'})
            self.assertIs(result['ci_runtime_proof'], False)
            self.assertIs(result['qualification_complete'], False)

    return EvidenceJoinControls


EvidenceJoinControls=_transport_controls(HERE)

if __name__=="__main__":unittest.main()
