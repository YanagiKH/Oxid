"""Pure source/mutation controls only; no producer capture or target execution."""
import ast
import copy
import contextlib
import hashlib
import json
import tempfile
import types
import unittest
from pathlib import Path

ROOT = Path(__file__).absolute().parent
runtime = types.ModuleType('current_public_runtime_under_test')
runtime.__file__ = str(ROOT / 'runtime.py')
exec(compile((ROOT / 'runtime.py').read_bytes(), runtime.__file__, 'exec'), runtime.__dict__)


def functions(path, names, namespace):
    tree = ast.parse(Path(path).read_bytes())
    body = [node for node in tree.body if isinstance(node, ast.FunctionDef) and node.name in names]
    if {node.name for node in body} != set(names):
        raise AssertionError('missing pure function')
    exec(compile(ast.Module(body=body, type_ignores=[]), str(path), 'exec'), namespace)
    return namespace


def common_fixture():
    row = {'key': 'one', 'profile': 'debug', 'argv_template': ['{oxid}', 'check', 'main.ox'], 'scope': 'execute'}
    cfg = {'_sha256': 'b' * 64, '_consumer': {'sha256': 'c' * 64},
           '_execution_context': {'source_manifest_sha256': 'f' * 64, 'retained_semantic_source_manifest_sha256': '9' * 64},
           'binaries': {'debug': {'path': '/compiler', 'sha256': 'a' * 64}}, '_binary_stats': {'debug': {'inode': 3}}}
    sources = {'main.ox': b'x'}
    source = [{'path': 'main.ox', 'bytes': 1, 'sha256': runtime.sha(b'x')}]
    process = {'argv': ['/compiler', 'check', 'main.ox'], 'cwd': '/fixture', 'status': 0,
               'timed_out': False, 'stream_limit_exceeded': False, 'started_ns': 1, 'completed_ns': 2,
               'stdout': '', 'stderr': '', 'stdout_sha256': runtime.sha(b''), 'stderr_sha256': runtime.sha(b'')}
    got = {'key': 'one', 'execution_context': copy.deepcopy(cfg['_execution_context']),
           'adapter': copy.deepcopy(cfg['_consumer']), 'candidate_binding_sha256': cfg['_sha256'],
           'freeze_sha256': 'freeze', 'executed': True, 'binary': cfg['binaries']['debug'],
           'binary_stat_before': {'inode': 3}, 'binary_stat_after': {'inode': 3}, 'source_files': source,
           'directory_before': source, 'directory_after': copy.deepcopy(source), 'process': process,
           'fixture_directory': '/fixture', 'tool_invocations': [], 'emitted_llvm': None}
    def tools(calls, _tools, native, ir):
        runtime.need(calls == [] and not native and ir is None, 'unexpected tools')
    namespace = {'need': runtime.need, 'sha': runtime.sha, 'FREEZE_SHA': 'freeze',
                 'receipt_identity': lambda r: {'key': r['key']},
                 '_binding': lambda: types.SimpleNamespace(CURRENT_SOURCE_SHA='f' * 64, PREDECESSOR_SOURCE_SHA='9' * 64),
                 'rt': types.SimpleNamespace(source_map=lambda rows, exclude=(): rows, check_tools=tools)}
    functions(ROOT / 'runtime.py', ('_check_outer', 'check_common'), namespace)
    return namespace['check_common'], row, got, cfg, sources


def isolated_boundary_fixture():
    import threading
    import weakref
    import stat
    class Token: pass
    token = Token()
    control, events = {'auth_calls': 0, 'fail_auth_at': None}, []
    @contextlib.contextmanager
    def checked_scope(invocation):
        events.append('shared-entry')
        try:
            yield
        finally:
            events.append('shared-exit')
    prep = types.SimpleNamespace(checked_scope=checked_scope, public_inputs=lambda inv, permit: checked_scope(inv), public_output_tick=lambda inv: None, check_deadline=lambda inv: events.append('tick'))
    owners = {'binding': object(), 'build': object(), 'machine': object(), 'preparation': prep}
    def authenticate(state, expected=None):
        control['auth_calls'] += 1
        if control['auth_calls'] == control['fail_auth_at']:
            raise runtime.Rejected('helper closure changed')
        if expected is not None:
            runtime.need(all(expected[key] is owners[key] for key in owners), 'owner changed')
        return owners
    state = {'invocation': object(), '_read_owner_seed': prep, 'runtime_permit': object(),
             'input_paths': {'repository': '/isolated-fixture'}, 'helper_input_files': ['fixed']}
    ns = {'contextlib': contextlib, 'threading': threading, 'weakref': weakref, 'Path': Path,
          'stat': stat, 'hashlib': hashlib, 'need': runtime.need, '_PublicReadBoundary': runtime._PublicReadBoundary,
          '_ADMISSIONS': weakref.WeakKeyDictionary({token: state}), '_test_keepalive': token,
          '_READ_BOUNDARIES': weakref.WeakKeyDictionary(), '_READ_LOCAL': threading.local(),
          '_authenticate_read_owners': authenticate, 'retained_helper_inputs': lambda root: ['fixed']}
    functions(ROOT / 'runtime.py', ('_active_boundary', '_read_boundary', '_tick', '_scoped_read_file'), ns)
    return ns, state, events, control


class RuntimePureTests(unittest.TestCase):
    def test_import_is_dormant_and_owners_reject_construction(self):
        for cls in (runtime.RuntimeAdmission, runtime.CollectionResult):
            with self.assertRaises(TypeError):
                cls()
        for function in (runtime.collect, runtime.verify_collection, runtime.hosted_context):
            with self.assertRaises(runtime.Rejected):
                function({'status': 'pass', 'execution_qualified': True})

    def test_opaque_instance_forgery_rejected(self):
        for cls, function in ((runtime.RuntimeAdmission, runtime.collect), (runtime.CollectionResult, runtime.verify_collection)):
            forged = object.__new__(cls)
            with self.assertRaises(runtime.Rejected):
                function(forged)

    def test_current_common_positive_is_only_pure_claim(self):
        check, row, got, cfg, sources = common_fixture()
        check(row, got, cfg, sources, {})
        self.assertEqual(len(runtime._ADMISSIONS), 0)
        self.assertEqual(len(runtime._COLLECTIONS), 0)

    def test_common_mutation_matrix(self):
        mutations = [
            lambda x: x['execution_context'].__setitem__('source_manifest_sha256', '9' * 64),
            lambda x: x['execution_context'].__setitem__('retained_semantic_source_manifest_sha256', 'f' * 64),
            lambda x: x.__setitem__('adapter', {}),
            lambda x: x.__setitem__('candidate_binding_sha256', '0' * 64),
            lambda x: x.__setitem__('key', 'other'),
            lambda x: x.__setitem__('binary_stat_after', {}),
            lambda x: x['process']['argv'].append('--invented'),
            lambda x: x['process'].__setitem__('stdout', 'forged'),
            lambda x: x['process'].__setitem__('timed_out', True),
            lambda x: x['process'].__setitem__('completed_ns', 0),
            lambda x: x['directory_after'].append({'path': 'extra'}),
            lambda x: x['directory_after'].clear(),
            lambda x: x.__setitem__('native_execution', {}),
            lambda x: x.__setitem__('tool_invocations', [{}]),
        ]
        for mutation in mutations:
            check, row, got, cfg, sources = common_fixture()
            mutation(got)
            with self.subTest(mutation=mutation), self.assertRaises((runtime.Rejected, KeyError)):
                check(row, got, cfg, sources, {})

    def test_capability_roster_exact_twelve(self):
        class Contracts:
            def roster(self, section, host):
                return [{'scope': 'execute', 'required_capabilities': ['posix_regular_file_mode_denies_open_for_effective_identity'],
                         'key': section + str(i)} for i in range(6)]
        self.assertEqual({k: len(v) for k, v in runtime.capability_rows(Contracts()).items()}, {'public': 6, 'lifecycle': 6})
        class Missing(Contracts):
            def roster(self, section, host):
                return super().roster(section, host)[:-1]
        with self.assertRaises(runtime.Rejected):
            runtime.capability_rows(Missing())

    def test_saved_lifecycle_derivation_cannot_be_normalized(self):
        def comparator(row, ordinary, observed, *args):
            observed['logical_lifecycle'] = {'derived': 1}
        ns = {'copy': copy, 'need': runtime.need, 'lifecycle_compare': comparator}
        functions(ROOT / 'runtime.py', ('_replay_lifecycle',), ns)
        good = {'logical_lifecycle': {'derived': 1}}
        ns['_replay_lifecycle']({}, {}, good, {}, {}, {}, {}, {})
        forged = {'logical_lifecycle': {'derived': 2}}
        with self.assertRaises(runtime.Rejected):
            ns['_replay_lifecycle']({}, {}, forged, {}, {}, {}, {}, {})
        self.assertEqual(forged['logical_lifecycle']['derived'], 2)

    def test_same_shared_runtime_policy_call_shape(self):
        shared = ROOT.parent / 'package_dependency_unit4_current/preparation.py'
        tree = ast.parse(shared.read_bytes())
        defs = {node.name: node for node in tree.body if isinstance(node, ast.FunctionDef)}
        for name in ('owned_probe', 'verify_probe'):
            args = defs[name].args
            names = {a.arg for a in args.args + args.kwonlyargs}
            self.assertTrue({'runtime_case', 'runtime_permit', 'native_permit'} <= names)
        source = ast.parse((ROOT / 'runtime.py').read_bytes())
        for node in ast.walk(source):
            if isinstance(node, ast.Call) and isinstance(node.func, ast.Attribute) and node.func.attr == 'register_public_contracts':
                self.assertEqual(ast.unparse(node.args[3]), 'contracts.path_map')
        generator = next(n for n in source.body if isinstance(n, ast.FunctionDef) and n.name == '_generator_check')
        calls = [n for n in ast.walk(generator) if isinstance(n, ast.Call) and isinstance(n.func, ast.Attribute)
                 and n.func.attr in ('owned_probe', 'verify_probe')]
        self.assertEqual(len(calls), 2)
        self.assertTrue(all(next(k.value.value for k in c.keywords if k.arg == 'operation_kind') == 'generator' for c in calls))

    def test_standard_case_paths_match_shared_policy(self):
        shared = ROOT.parent / 'package_dependency_unit4_current/preparation.py'
        permit, invocation = object(), object()
        row = {'scope': 'execute', 'profile': 'debug', 'argv_template': ['{oxid}', 'check', 'main.ox']}
        ns = {'Path': Path, 'need': runtime.need, 'identity': lambda path: {'path': path},
              '_runtime_permit': lambda p, i, k: {'root': '/run', 'binaries': {'ordinary-debug': {'path': '/compiler'}}},
              '_PUBLIC_CONTRACTS': {permit: {'helper': {'path': '/helper'}, 'rosters': {'public': [row]}}}}
        functions(shared, ('_public_case', '_public_case_verified'), ns)
        d, actual, argv, fixture, evidence = ns['_public_case'](permit, invocation, {'section': 'public', 'index': 0, 'variant': 'ordinary'})
        self.assertEqual(argv, ['/compiler', 'check', 'main.ox'])
        self.assertEqual(str(fixture), '/run/public-runtime/collection/public/00000/ordinary/fixture')
        self.assertEqual(str(evidence), '/run/public-runtime/collection/public/00000/ordinary')
        for case in ({'section': 'public', 'index': True, 'variant': 'ordinary'},
                     {'section': 'public', 'index': 1, 'variant': 'ordinary'},
                     {'section': 'public', 'index': 0, 'variant': 'other'}):
            with self.assertRaises(runtime.Rejected):
                ns['_public_case'](permit, invocation, case)

    def test_no_historical_module_replacement(self):
        tree = ast.parse((ROOT / 'runtime.py').read_bytes())
        text = ast.get_source_segment((ROOT / 'runtime.py').read_text(),
            next(n for n in tree.body if isinstance(n, ast.FunctionDef) and n.name == 'retained_helpers'))
        self.assertIn("sys.modules[name] is _ISSUED_HELPERS[name]", text)
        self.assertIn("'unissued/conflicting retained short-name module'", text)
        self.assertNotIn('monkeypatch', text)

    def test_full_collection_has_no_section_or_pilot_override(self):
        tree = ast.parse((ROOT / 'runtime.py').read_bytes())
        function = next(n for n in tree.body if isinstance(n, ast.FunctionDef) and n.name == 'collect')
        self.assertEqual([a.arg for a in function.args.args], ['admission'])
        self.assertEqual(runtime.SECTIONS, ('public', 'original', 'predecessors', 'lifecycle', 'guards'))
        self.assertEqual(runtime.PROFILES, ('debug', 'release'))


    def test_retained_closure_rejects_empty_directory_and_special_entries(self):
        import os
        import stat
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp).resolve()
            entries = []
            for directory, count in ((runtime.PUBLIC, 31), (runtime.HOSTED, 4), (runtime.COMPAT, 7)):
                (root / directory).mkdir(parents=True)
                for index in range(count):
                    path = root / directory / str(index)
                    path.write_bytes(b'pure fixture')
                    entries.append({'path': path.relative_to(root).as_posix(), 'bytes': 12, 'sha256': runtime.sha(b'pure fixture')})
            inventory = root / 'tests/qualification/unit4_ci/inputs.json'
            inventory.parent.mkdir()
            inventory.write_bytes(runtime.canonical({'files': entries}))
            def identity(path):
                path = Path(path)
                runtime.need(stat.S_ISREG(path.lstat().st_mode), 'regular fixture')
                raw = path.read_bytes()
                return {'path': str(path), 'bytes': len(raw), 'sha256': runtime.sha(raw)}
            ns = {'Path': Path, 'stat': stat, 'need': runtime.need, 'sha': runtime.sha,
                  '_read': lambda path: Path(path).read_bytes(), '_identity': identity,
                  '_binding': lambda: types.SimpleNamespace(decode=json.loads),
                  'PUBLIC': runtime.PUBLIC, 'HOSTED': runtime.HOSTED, 'COMPAT': runtime.COMPAT,
                  'SOURCE': runtime.SOURCE, 'SEMANTIC_FILES': runtime.SEMANTIC_FILES,
                  'INPUTS_SHA256': runtime.sha(inventory.read_bytes())}
            functions(ROOT / 'runtime.py', ('retained_helper_inputs',), ns)
            self.assertEqual(len(ns['retained_helper_inputs'](root)), 43)
            extra = root / runtime.PUBLIC / 'empty'
            extra.mkdir()
            with self.assertRaises(runtime.Rejected):
                ns['retained_helper_inputs'](root)
            extra.rmdir()
            extra.symlink_to('0')
            with self.assertRaises(runtime.Rejected):
                ns['retained_helper_inputs'](root)
            extra.unlink()
            if hasattr(os, 'mkfifo'):
                os.mkfifo(extra)
                with self.assertRaises(runtime.Rejected):
                    ns['retained_helper_inputs'](root)

    def test_collection_roster_rejects_extra_file_and_empty_directory(self):
        import stat
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            required = ['input-integrity.json', 'tools.json', 'qualified-paths-amendment.json']
            for section in runtime.SECTIONS:
                (root / section).mkdir()
                required += [section + '/' + name for name in ('roster.json', 'observations.jsonl.gz', 'comparison.json')]
            required.append('predecessors/historical-comparison.json')
            for name in required:
                (root / name).write_bytes(b'pure structural fixture')
            ns = {'Path': Path, 'stat': stat, 'need': runtime.need, 'Rejected': runtime.Rejected,
                  'SECTIONS': runtime.SECTIONS, '_tick': lambda invocation: None}
            functions(ROOT / 'runtime.py', ('_validate_evidence_roster', '_case_evidence_roster', '_validate_evidence_members'), ns)
            state = {'collection_root': root, 'invocation': None, 'rosters': {s: [] for s in runtime.SECTIONS}}
            receipts = {s: [] for s in runtime.SECTIONS}
            self.assertTrue(ns['_validate_evidence_roster'](state, receipts))
            (root / 'unlisted').write_bytes(b'x')
            with self.assertRaises(runtime.Rejected):
                ns['_validate_evidence_roster'](state, receipts)
            (root / 'unlisted').unlink()
            (root / 'empty').mkdir()
            with self.assertRaises(runtime.Rejected):
                ns['_validate_evidence_roster'](state, receipts)

    def test_forged_join_is_rejected_before_any_read(self):
        with self.assertRaises(TypeError):
            runtime.JoinedResult()
        with self.assertRaises(runtime.Rejected):
            runtime.verify_join(object.__new__(runtime.JoinedResult))


    def test_runtime_caller_matches_shared_finite_command_and_environment(self):
        shared = ROOT.parent / 'package_dependency_unit4_current/preparation.py'
        permit, invocation = object(), object()
        case = {'section': 'public', 'index': 0, 'variant': 'ordinary'}
        row = {'scope': 'execute', 'profile': 'debug', 'argv_template': ['{oxid}', 'check', 'main.ox']}
        base_env = {'PATH': '/producer', 'UNIT4_ADAPTER_ROOT': '/repo/tests/qualification/unit4_public_v3',
                    'OXID_LLVM_BIN': '/run/public-runtime/tools/main/tool-wrappers'}
        policy = {'Path': Path, 'need': runtime.need, 'identity': lambda path: {'path': path},
                  '_runtime_permit': lambda p, i, k: {'root': '/run', 'binaries': {'ordinary-debug': {'path': '/compiler'}}},
                  '_PUBLIC_CONTRACTS': {permit: {'helper': {'path': '/helper'}, 'rosters': {'public': [row]}}},
                  '_PUBLIC_TOOLS': {permit: {'pins': [], 'environment': base_env}},
                  'check_deadline': lambda inv: None}
        policy['_PUBLIC_REPLAY'] = None
        policy['_runtime_permit_owner'] = policy['_runtime_permit']
        policy['_public_selected_pair'] = lambda *args: None  # Separate shared body-drift controls own this isolated I/O seam.
        functions(shared, ('_public_case', '_public_case_verified', '_public_environment', '_public_environment_verified', '_public_runtime_recipe'), policy)
        expected_env = policy['_public_environment'](permit, invocation, 'public-runtime', case)
        argv, cwd = ['/compiler', 'check', 'main.ox'], Path('/run/public-runtime/collection/public/00000/ordinary/fixture')
        calls = []
        class Record:
            def __init__(self, doc): self.doc = doc
            def document(self): return copy.deepcopy(self.doc)
            def stdout(self): return b''
            def stderr(self): return b''
        class API:
            def owned_probe(self, inv, command, directory, environment, **kw):
                policy['_public_runtime_recipe'](kw['runtime_permit'], kw['native_permit'], inv,
                    kw['operation_kind'], kw['runtime_case'], command, directory, environment,
                    kw['timeout_seconds'], kw['stream_limit_bytes'])
                calls.append(copy.deepcopy(kw))
                return Record({'pid': 1, 'started_ns': 1, 'completed_ns': 2, 'status': 0,
                               'argv': command, 'cwd': str(directory), 'environment': environment, **kw})
            def verify_probe(self, record, inv, **kw):
                policy['_public_runtime_recipe'](kw['runtime_permit'], kw['native_permit'], inv,
                    kw['operation_kind'], kw['runtime_case'], kw['argv'], kw['cwd'], kw['environment'],
                    kw['timeout_seconds'], kw['stream_limit_bytes'])
                calls.append(copy.deepcopy(kw))
                return record.document()
        api = API()
        ns = {'copy': copy, 'sha': runtime.sha, '_checkpoint': lambda state: None,
              '_binding': lambda: types.SimpleNamespace(preparation_api=lambda: api),
              '_preparation': lambda: api, '_read_boundary': lambda state: contextlib.nullcontext(),
              '_active_boundary': lambda: None, 'need': runtime.need}
        functions(ROOT / 'runtime.py', ('_owned_process',), ns)
        state = {'invocation': invocation, 'runtime_permit': permit, 'active_case': case,
                 'native_permits': {}, 'probes': {}, 'process_cases': {}}
        got = ns['_owned_process'](state, argv, cwd, expected_env)
        self.assertEqual(got['stdout'], '')
        self.assertEqual(len(calls), 2)
        self.assertEqual(state['process_cases'][(1, 1)], case)
        for bad in ({**expected_env, 'LD_PRELOAD': '/evil'}, {**expected_env, 'UNIT4_TOOL_LOG': '/other'}):
            with self.assertRaises(runtime.Rejected):
                ns['_owned_process'](state, argv, cwd, bad)
        with self.assertRaises(runtime.Rejected):
            ns['_owned_process'](state, argv + ['--invented'], cwd, expected_env)


    def test_genuine_all_host_rosters_from_bounded_public_fixture(self):
        import gzip
        import io
        import os
        import tarfile
        repo = Path(os.environ.get('OXID_RETAINED_SOURCE_TEST_REPOSITORY', str(ROOT.parents[2]))).resolve()
        transport = repo / 'tests/fixtures/typed_project_unit4_contracts'
        if not transport.is_dir():
            self.skipTest('installed retained source fixture is not present in this isolated candidate')
        manifest_raw = (transport / 'transport-manifest.json').read_bytes()
        self.assertEqual(runtime.sha(manifest_raw), '631bd4dcc5e3c2e2887debddf8a8657af5247fb985a8d6fcc45cbf78b701c687')
        manifest = json.loads(manifest_raw)
        archived = (transport / 'authorities.tar.gz').read_bytes()
        self.assertEqual(len(archived), 2301283)
        self.assertEqual(runtime.sha(archived), '3814e436ee6364c42e7942da68141bfada99ba692eb5b491c30520ea2e881c3a')
        with gzip.GzipFile(fileobj=io.BytesIO(archived)) as source:
            decoded = source.read(10 * 1024 ** 2 + 1)
        self.assertEqual(len(decoded), 8755200)
        self.assertLess(len(decoded), 10 * 1024 ** 2)
        self.assertEqual(runtime.sha(decoded), manifest['archive']['decoded_sha256'])
        selected = {r['archive_path']: r for r in manifest['members'] if any(role.startswith('public-') or role == 'active-public-freeze' for role in r['roles'])}
        self.assertEqual(len(selected), 141)
        self.assertEqual(sum(r['bytes'] for r in selected.values()), 7661184)
        self.assertLess(sum(r['bytes'] for r in selected.values()), 8 * 1024 ** 2)
        helper_path = repo / runtime.PUBLIC / 'contracts.py'
        helper = helper_path.read_bytes()
        self.assertEqual(runtime.sha(helper), '170515201950aa6018aa176e9d02720d7fdca5f9e52fae123a3a01c71d8a6ac9')
        contracts_module = types.ModuleType('pure_frozen_contracts_fixture')
        contracts_module.__file__ = str(helper_path)
        exec(compile(helper, str(helper_path), 'exec'), contracts_module.__dict__)
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            seen, written, path_map = set(), set(), {}
            with tarfile.open(fileobj=io.BytesIO(decoded), mode='r:') as archive:
                for member in archive:
                    name = member.name
                    path = Path(name)
                    self.assertTrue(member.isfile() and not path.is_absolute() and '..' not in path.parts)
                    self.assertNotIn(name, seen)
                    seen.add(name)
                    if name not in selected:
                        continue
                    row = selected[name]
                    self.assertEqual(member.size, row['bytes'])
                    raw = archive.extractfile(member).read(row['bytes'] + 1)
                    self.assertEqual(len(raw), row['bytes'])
                    self.assertEqual(runtime.sha(raw), row['sha256'])
                    destination = root / name
                    destination.parent.mkdir(parents=True, exist_ok=True)
                    with destination.open('xb') as sink:
                        sink.write(raw)
                    path_map[row['logical_path']] = str(destination)
                    written.add(name)
            self.assertEqual(written, set(selected))
            self.assertEqual(seen, {row['archive_path'] for row in manifest['members']})
            self.assertEqual(len(seen), 158)
            contract_root = Path(path_map[manifest['active_contracts']['public']['logical_path']]).parent
            amendment = repo / 'tests/fixtures/typed_project_unit4_public_location_amendment_v1'
            contracts = contracts_module.Contracts(contract_root, path_map, amendment)
            totals = None
            self.roster_summary = {}
            for host in contracts_module.HOSTS:
                rosters = {section: contracts.roster(section, host) for section in runtime.SECTIONS}
                actual = {section: len(rows) for section, rows in rosters.items()}
                self.roster_summary[host] = {section: {'total': len(rows), 'local': sum(row['scope'] == 'execute' for row in rows)} for section, rows in rosters.items()}
                if totals is None:
                    totals = actual
                self.assertEqual(actual, totals)
                for section, rows in rosters.items():
                    self.assertEqual(len({row['key'] for row in rows}), len(rows))
                    self.assertEqual({row['profile'] for row in rows}, {'debug', 'release'})
                    self.assertEqual({row['host'] for row in rows}, set(contracts_module.HOSTS))
                    self.assertTrue(all(row['execution_host'] == host for row in rows))
                    self.assertTrue(all(row['host'] == host for row in rows if row['scope'] == 'execute'))
            self.assertEqual({section: len(rows) for section, rows in runtime.capability_rows(contracts).items()},
                             {'public': 6, 'lifecycle': 6})


    def test_read_boundary_cannot_be_forged(self):
        with self.assertRaises(TypeError):
            runtime._PublicReadBoundary()
        with self.assertRaises(runtime.Rejected):
            with runtime._read_boundary({'invocation': object()}):
                self.fail('unissued boundary entered')
        with self.assertRaises(runtime.Rejected):
            runtime._scoped_read_file(Path(__file__))

    def test_selected_checkpoint_uses_frozen_role_and_profile(self):
        row = {'scope': 'execute', 'profile': 'release'}
        state = {'active_case': {'section': 'lifecycle', 'index': 0, 'variant': 'observer'},
                 'rosters': {'lifecycle': [row]}}
        self.assertEqual(runtime._selected_case(state), ('lifecycle', 'release'))
        state['active_case']['variant'] = 'ordinary'
        self.assertEqual(runtime._selected_case(state), ('ordinary', 'release'))
        for bad in ({'section': 'lifecycle', 'index': True, 'variant': 'ordinary'},
                    {'section': 'lifecycle', 'index': 1, 'variant': 'ordinary'},
                    {'section': 'lifecycle', 'index': 0, 'variant': 'wrong'},
                    {'section': 'public', 'index': 0, 'variant': 'observer'}):
            with self.assertRaises(runtime.Rejected):
                runtime._selected_case({'active_case': bad, 'rosters': {'lifecycle': [row], 'public': [row]}})

    def test_selected_checkpoint_reads_only_actual_role_profile(self):
        reads, views, tools = [], [], []
        row = {'scope': 'execute', 'profile': 'release'}
        state = {'invocation': object(), 'active_case': {'section': 'lifecycle', 'index': 0, 'variant': 'observer'},
                 'rosters': {'lifecycle': [row]}, 'cfgs': {}, 'runtime_tools': object(), 'machine': object(),
                 'helpers': {'runtime': types.SimpleNamespace(file_identity=lambda path: {'stat': path})}}
        for role in ('ordinary', 'lifecycle'):
            state['cfgs'][role] = {'build_binding': {'source_binding': {'view': {'role': role}}},
                                   'binaries': {p: {'path': role + '-' + p} for p in runtime.PROFILES},
                                   '_binary_stats': {p: {'stat': role + '-' + p} for p in runtime.PROFILES}}
        def identity(path, invocation):
            reads.append(path)
            return {'path': path}
        ns = {'need': runtime.need, '_tick': lambda inv: None, '_selected_case': runtime._selected_case,
              '_verify_view': lambda view, inv: views.append(view['role']), '_identity': identity,
              '_machine': lambda: types.SimpleNamespace(verify_runtime_inputs=lambda *args: tools.append(args))}
        functions(ROOT / 'runtime.py', ('_checkpoint_reads',), ns)
        self.assertTrue(ns['_checkpoint_reads'](state))
        self.assertEqual(views, ['lifecycle'])
        self.assertEqual(reads, ['lifecycle-release'])
        self.assertEqual(len(tools), 1)

    def test_isolated_read_scope_auth_count_and_no_cross_operation_hash_cache(self):
        import threading
        import weakref
        import stat
        class Token: pass
        calls, ticks = [], []
        @contextlib.contextmanager
        def checked_scope(invocation):
            calls.append('shared-entry')
            try:
                yield
            finally:
                calls.append('shared-exit')
        prep = types.SimpleNamespace(checked_scope=checked_scope, public_inputs=lambda inv, permit: checked_scope(inv), public_output_tick=lambda inv: None, check_deadline=lambda inv: ticks.append(inv))
        owners = {'binding': object(), 'build': object(), 'machine': object(), 'preparation': prep}
        def authenticate(state, expected=None):
            calls.append('authenticate')
            if expected is not None:
                self.assertTrue(all(expected[key] is owners[key] for key in owners))
            return owners
        token = Token()
        state = {'invocation': object(), '_read_owner_seed': prep, 'runtime_permit': object(), 'input_paths': {'repository': '/synthetic'},
                 'helper_input_files': ['fixed']}
        registry = weakref.WeakKeyDictionary({token: state})
        ns = {'contextlib': contextlib, 'threading': threading, 'weakref': weakref, 'Path': Path,
              'stat': stat, 'hashlib': hashlib, 'need': runtime.need, '_PublicReadBoundary': runtime._PublicReadBoundary,
              '_ADMISSIONS': registry, '_READ_BOUNDARIES': weakref.WeakKeyDictionary(), '_READ_LOCAL': threading.local(),
              '_authenticate_read_owners': authenticate, 'retained_helper_inputs': lambda root: ['fixed']}
        functions(ROOT / 'runtime.py', ('_active_boundary', '_read_boundary', '_tick', '_scoped_read_file'), ns)
        with tempfile.TemporaryDirectory() as temp:
            path = Path(temp).resolve() / 'body'
            path.write_bytes(b'first')
            with ns['_read_boundary'](state):
                for _ in range(100):
                    self.assertEqual(ns['_scoped_read_file'](path)[0]['sha256'], runtime.sha(b'first'))
                with ns['_read_boundary'](state):
                    self.assertIs(ns['_active_boundary']()['state'], state)
            self.assertEqual(calls.count('authenticate'), 2)
            self.assertEqual(calls.count('shared-entry'), 1)
            self.assertGreaterEqual(len(ticks), 400)
            self.assertIsNone(ns['_active_boundary']())
            path.write_bytes(b'second')
            with ns['_read_boundary'](state):
                self.assertEqual(ns['_scoped_read_file'](path)[0]['sha256'], runtime.sha(b'second'))
            self.assertEqual(calls.count('authenticate'), 4)
        self.assertEqual(len(runtime._ADMISSIONS), 0)
        self.assertEqual(len(runtime._READ_BOUNDARIES), 0)

    def test_full_checkpoint_remains_outside_cheap_scope_and_poison_on_error(self):
        events = []
        @contextlib.contextmanager
        def boundary(state):
            events.append('entry')
            yield
            events.append('exit')
        ns = {'need': runtime.need, '_active_boundary': lambda: None,
              '_preparation': lambda: types.SimpleNamespace(full_verify_invocation=lambda inv: events.append('full-source')),
              '_read_boundary': boundary, '_checkpoint_reads': lambda state, full: events.append(('reads', full))}
        functions(ROOT / 'runtime.py', ('_checkpoint',), ns)
        state = {'invocation': object()}
        ns['_checkpoint'](state, full=True)
        self.assertEqual(events, ['full-source', 'entry', ('reads', True), 'exit', 'full-source'])
        def changed(state, full):
            raise runtime.Rejected('changed selected input')
        ns['_checkpoint_reads'] = changed
        with self.assertRaises(runtime.Rejected):
            ns['_checkpoint'](state)
        self.assertTrue(state['invalidated'])
        with self.assertRaises(runtime.Rejected):
            ns['_checkpoint'](state)

    def test_materialized_targets_and_skips_have_selected_checks(self):
        tree = ast.parse((ROOT / 'runtime.py').read_bytes())
        functions_by_name = {node.name: node for node in tree.body if isinstance(node, ast.FunctionDef)}
        collect = ast.unparse(functions_by_name['_collect_one'])
        self.assertLess(collect.index('rt.materialize('), collect.index('_owned_process('))
        self.assertIn("if owner.get('worker') is None:\n                _checkpoint(owner)", collect)
        self.assertNotIn('_checkpoint(state)', ast.unparse(functions_by_name['collect']))
        owned = ast.unparse(functions_by_name['_owned_process'])
        self.assertLess(owned.index('_checkpoint(state)'), owned.index('api.owned_probe('))
        self.assertLess(owned.index('api.owned_probe('), owned.rindex('_checkpoint(state)'))
        self.assertIn("_active_boundary() is None", owned)


    def test_full_checkpoint_reads_both_roles_all_profiles_and_machine(self):
        reads, views, machine_calls = [], [], []
        invocation, machine, tools = object(), object(), object()
        row = {'key': 'fixed', 'scope': 'execute', 'profile': 'debug'}
        class Contracts:
            def roster(self, section, host): return [row]
        contracts = Contracts()
        state = {'invocation': invocation, 'ordinary': 'ordinary-result', 'lifecycle': 'lifecycle-result',
                 'runtime_tools': tools, 'machine': machine, 'cfgs': {}, 'contracts': contracts,
                 'contract_snapshot': b'{}', 'rosters': {s: [row] for s in runtime.SECTIONS},
                 'row_indices': {s: {'fixed': 0} for s in runtime.SECTIONS},
                 'execution_context': {'host': 'Linux x86_64'},
                 'input_paths': {'contract_root': '/contract', 'path_map': '/map', 'amendment_root': '/amendment'},
                 'helpers': {'runtime': types.SimpleNamespace(file_identity=lambda path: {'stat': path}),
                             'contracts': types.SimpleNamespace(Contracts=lambda *args: contracts)}}
        for role in ('ordinary', 'lifecycle'):
            state['cfgs'][role] = {'build_binding': {'source_binding': {'view': {'role': role}}},
                                   'binaries': {p: {'path': role + '-' + p} for p in runtime.PROFILES},
                                   '_binary_stats': {p: {'stat': role + '-' + p} for p in runtime.PROFILES}}
        def identity(path, inv):
            reads.append(path)
            return {'path': path}
        api = types.SimpleNamespace(verify_runtime_tools=lambda *args: machine_calls.append('full'),
                                    verify_runtime_inputs=lambda *args: machine_calls.append('selected'))
        ns = {'need': runtime.need, '_tick': lambda inv: None, '_selected_case': runtime._selected_case,
              '_verify_view': lambda view, inv: views.append(view['role']), '_identity': identity,
              '_machine': lambda: api, '_cfg': lambda result: state['cfgs'][result.split('-')[0]],
              '_binding': lambda: types.SimpleNamespace(decode=json.loads), '_read': lambda *args: b'{}',
              '_contract_snapshot': lambda contracts: {}, 'canonical': runtime.canonical, 'SECTIONS': runtime.SECTIONS}
        functions(ROOT / 'runtime.py', ('_checkpoint_reads',), ns)
        ns['_checkpoint_reads'](state, full=True)
        self.assertEqual(views, ['ordinary', 'lifecycle'])
        self.assertEqual(reads, ['ordinary-debug', 'ordinary-release', 'lifecycle-debug', 'lifecycle-release'])
        self.assertEqual(machine_calls, ['full', 'selected'])
        ns['_identity'] = lambda path, inv: {'path': path, 'sha256': 'changed'}
        with self.assertRaises(runtime.Rejected):
            ns['_checkpoint_reads'](state, full=True)

    def test_real_full_verifier_bodies_bypass_cheap_association(self):
        shared = ROOT.parent / 'package_dependency_unit4_current/preparation.py'
        machine_path = ROOT.parent / 'package_dependency_unit4_machine_v1/machine.py'
        calls = []
        context = types.SimpleNamespace(_invocation=lambda inv: calls.append(('full-source', inv)),
                                        _invocation_association=lambda inv: self.fail('cheap association used'))
        view_checks = []
        ns = {'_manifest': lambda: None, 'context_api': lambda: context, '_CHECKED': [object()],
              '_VIEWS': {}, '_verify_view_rows': lambda rows, **kw: view_checks.append(kw)}
        functions(shared, ('full_verify_invocation',), ns)
        invocation = object()
        ns['full_verify_invocation'](invocation)
        self.assertEqual(calls, [('full-source', invocation)])
        self.assertEqual(view_checks, [{'_force_full': True}])
        snapshot = {'rust': {'root': '/rust'}, 'resolver': {}, 'direct_discovery': {}}
        document = {'snapshot': snapshot, 'probes': [], 'probe_records': []}
        issued = {'invocation': invocation, 'raw': runtime.canonical(document), 'probes': []}
        def actual_snapshot(api, inv, **kwargs):
            calls.append(('machine-snapshot', inv))
            return copy.deepcopy(snapshot), None
        mns = {'_issued': lambda admission: issued, 'need': runtime.need, 'decode': json.loads,
               '_runtime_state': lambda admission: {}, '_runtime_projection': lambda state: b'{}',
               '_snapshot': actual_snapshot, 'PROBE_TIMEOUT': 30, 'STREAM_LIMIT': 8 * 1024 ** 2}
        functions(machine_path, ('_verify_machine',), mns)
        self.assertTrue(mns['_verify_machine'](object(), object(), invocation))
        self.assertEqual(calls[-1], ('machine-snapshot', invocation))
        self.assertEqual(len(runtime._ADMISSIONS), 0)

    def test_changed_selected_source_or_binary_rejects_before_spawn(self):
        import stat
        for changed_name in ('source.ox', 'compiler'):
            with tempfile.TemporaryDirectory() as temp:
                root = Path(temp).resolve()
                view = root / 'source'
                view.mkdir()
                source, binary = view / 'source.ox', root / 'compiler'
                source.write_bytes(b'fixed source')
                binary.write_bytes(b'fixed compiler')
                def identity(path, invocation=None):
                    path = Path(path)
                    return {'path': str(path), 'bytes': path.stat().st_size, 'sha256': runtime.sha(path.read_bytes())}
                def file_stat(path):
                    info = Path(path).stat()
                    return {'st_size': info.st_size, 'st_mtime_ns': info.st_mtime_ns, 'st_ino': info.st_ino}
                state = {'invocation': object(), 'active_case': {'section': 'public', 'index': 0, 'variant': 'ordinary'},
                         'rosters': {'public': [{'scope': 'execute', 'profile': 'debug'}]},
                         'runtime_tools': object(), 'machine': object(),
                         'helpers': {'runtime': types.SimpleNamespace(file_identity=file_stat)},
                         'cfgs': {'ordinary': {'build_binding': {'source_binding': {'view': {
                             'path': str(view), 'files': [{**identity(source), 'path': 'source.ox'}], 'metadata': []}}},
                             'binaries': {'debug': identity(binary)}, '_binary_stats': {'debug': file_stat(binary)}}}}
                ns = {'Path': Path, 'stat': stat, 'need': runtime.need, '_tick': lambda inv: None,
                      '_identity': identity, '_selected_case': runtime._selected_case,
                      '_machine': lambda: types.SimpleNamespace(verify_runtime_inputs=lambda *args: True)}
                functions(ROOT / 'runtime.py', ('_verify_view', '_checkpoint_reads'), ns)
                self.assertTrue(ns['_checkpoint_reads'](state))
                (source if changed_name == 'source.ox' else binary).write_bytes(b'changed actual body')
                api = types.SimpleNamespace(owned_probe=lambda *args, **kwargs: self.fail('target spawned after actual byte drift'))
                ns.update(_active_boundary=lambda: None, _checkpoint=lambda value: ns['_checkpoint_reads'](value),
                          _preparation=lambda: api)
                functions(ROOT / 'runtime.py', ('_owned_process',), ns)
                with self.assertRaises(runtime.Rejected):
                    ns['_owned_process'](state, [str(binary)], view, {})


    def test_helper_drift_at_scope_entry_or_exit_invalidates_whole_state(self):
        for fail_at in (1, 2):
            ns, state, events, control = isolated_boundary_fixture()
            control['fail_auth_at'] = fail_at
            with self.assertRaises(runtime.Rejected):
                with ns['_read_boundary'](state):
                    pass
            self.assertTrue(state['invalidated'])
            self.assertIsNone(ns['_active_boundary']())
            self.assertEqual(len(ns['_READ_BOUNDARIES']), 0)
            self.assertEqual(events[0], 'shared-entry')
            self.assertEqual(events[-1], 'shared-exit')
            with self.assertRaises(runtime.Rejected):
                with ns['_read_boundary'](state):
                    self.fail('invalidated state resumed')

    def test_scope_preserves_original_error_when_exit_authentication_fails(self):
        ns, state, events, control = isolated_boundary_fixture()
        control['fail_auth_at'] = 2
        original = ValueError('original evidence failure')
        with self.assertRaises(ValueError) as caught:
            with ns['_read_boundary'](state):
                raise original
        self.assertIs(caught.exception, original)
        self.assertTrue(state['invalidated'])
        self.assertIn('public read-boundary preservation failed', original.__notes__[0])
        self.assertIsNone(ns['_active_boundary']())

    def test_scope_rejects_mixed_invocation_and_reader_symlinks(self):
        ns, state, events, control = isolated_boundary_fixture()
        with tempfile.TemporaryDirectory() as temp:
            path = Path(temp).resolve() / 'body'
            path.write_bytes(b'actual bounded body')
            with ns['_read_boundary'](state):
                with self.assertRaises(runtime.Rejected):
                    ns['_tick'](object())
                with self.assertRaises(runtime.Rejected):
                    ns['_scoped_read_file'](path, maximum=2)
                alias = path.with_name('alias')
                alias.symlink_to(path.name)
                with self.assertRaises(runtime.Rejected):
                    ns['_scoped_read_file'](alias)
                ns['_scoped_read_file'](path, maximum=path.stat().st_size)

    def test_read_only_replay_scopes_are_finite_and_never_launch(self):
        tree = ast.parse((ROOT / 'runtime.py').read_bytes())
        owners = {}
        for function in (n for n in tree.body if isinstance(n, ast.FunctionDef)):
            for node in ast.walk(function):
                if not isinstance(node, ast.With):
                    continue
                calls = [item.context_expr for item in node.items]
                if not any(isinstance(call, ast.Call) and isinstance(call.func, ast.Attribute)
                           and call.func.attr == 'public_replay' for call in calls):
                    continue
                owners[function.name] = owners.get(function.name, 0) + 1
                self.assertEqual(len(calls), 2)
                self.assertEqual(ast.unparse(calls[0]), '_read_boundary(state)')
                body = ast.Module(body=node.body, type_ignores=[])
                invoked = [ast.unparse(n.func) for n in ast.walk(body) if isinstance(n, ast.Call)]
                self.assertNotIn('_owned_process', invoked)
                self.assertNotIn('_collect_one', invoked)
                self.assertNotIn('_generator_check', invoked)
                self.assertIn('_check_receipt', invoked)
        self.assertEqual(owners, {'collect': 1, 'verify_collection': 1})

    def test_case_monitor_close_reuses_exact_roster_and_source_bodies(self):
        import stat
        import os
        with tempfile.TemporaryDirectory() as temporary:
            root = Path(temporary)
            case = root / 'public/00000'
            fixture = case / 'ordinary/fixture'
            fixture.mkdir(parents=True)
            (fixture / 'main.ox').write_bytes(b'original source')
            (case / 'receipt.json').write_bytes(b'{}')
            (case / 'ordinary/receipt.json').write_bytes(b'{}')
            got = {'executed': False}
            state = {'collection_root': root, 'invocation': None,
                     'rosters': {'public': [{'scope': 'execute'}]}}
            checked = []
            ns = {'Path': Path, 'stat': stat, 'os': os, 'need': runtime.need,
                  'Rejected': runtime.Rejected, '_tick': lambda inv: None,
                  '_row_sources': lambda *args: {'main.ox': b'original source'},
                  '_read': lambda path, inv: path.read_bytes(),
                  '_verify_row_files': lambda receipt: checked.append(receipt)}
            functions(ROOT / 'runtime.py', ('_case_evidence_roster', '_validate_evidence_members', '_validate_case_evidence'), ns)
            self.assertTrue(ns['_validate_case_evidence'](state, 'public', 0, got))
            self.assertEqual(checked, [got])
            (case / 'invented').write_bytes(b'extra')
            with self.assertRaises(runtime.Rejected):
                ns['_validate_case_evidence'](state, 'public', 0, got)
            (case / 'invented').unlink()
            (case / 'empty').mkdir()
            with self.assertRaises(runtime.Rejected):
                ns['_validate_case_evidence'](state, 'public', 0, got)
            (case / 'empty').rmdir()
            (fixture / 'main.ox').write_bytes(b'changed source')
            with self.assertRaises(runtime.Rejected):
                ns['_validate_case_evidence'](state, 'public', 0, got)
            (fixture / 'main.ox').unlink()
            with self.assertRaises(runtime.Rejected):
                ns['_validate_case_evidence'](state, 'public', 0, got)

    def test_monitor_scopes_close_after_case_validation(self):
        tree = ast.parse((ROOT / 'runtime.py').read_bytes())
        scopes = []
        phases = []
        for node in ast.walk(tree):
            if not isinstance(node, ast.With):
                continue
            for item in node.items:
                call = item.context_expr
                if not isinstance(call, ast.Call) or not isinstance(call.func, ast.Attribute):
                    continue
                if call.func.attr == 'public_case_scope':
                    scopes.append(node)
                    self.assertIsInstance(node.body[-1], ast.With)
                    final = node.body[-1].body[-1]
                    self.assertEqual(ast.unparse(final.value.func), '_validate_case_evidence')
                    self.assertTrue(any(isinstance(n, ast.Call) and ast.unparse(n.func) in ('_collect_one', '_guard_execute') for n in ast.walk(node)))
                if call.func.attr == 'public_output_boundary':
                    phases.append(call.args[2].value)
        self.assertEqual(len(scopes), 2)
        self.assertEqual(sorted(phases), ['collection-close', 'section-close', 'section-open'])

    def test_failed_collection_retains_output_audit_and_original_error(self):
        tree = ast.parse((ROOT / 'runtime.py').read_bytes())
        collector = next(n for n in tree.body if isinstance(n, ast.FunctionDef) and n.name == 'collect')
        guarded = next(n for n in collector.body if isinstance(n, ast.Try))
        handler = copy.deepcopy(guarded.handlers[0])
        fake = ast.parse('def isolated_failed_collection(state, original):\n    try:\n        raise original\n    except BaseException as error:\n        pass\n').body[0]
        fake.body[0].handlers = [handler]
        ns = {'canonical': runtime.canonical, 'need': runtime.need,
              '_checkpoint': lambda *args, **kwargs: (_ for _ in ()).throw(RuntimeError('full preservation failed'))}
        exec(compile(ast.fix_missing_locations(ast.Module(body=[fake], type_ignores=[])), 'isolated_failed_collection', 'exec'), ns)
        for accessor_fails, many_operations in ((False, False), (True, False), (False, True), (True, True)):
            with self.subTest(accessor_fails=accessor_fails, many_operations=many_operations), tempfile.TemporaryDirectory() as temporary:
                calls = []
                def output_failure(invocation, permit):
                    calls.append((invocation, permit))
                    if accessor_fails:
                        raise RuntimeError('output audit unavailable')
                    return {'status': 'verified', 'retained_failure_audit': True}
                state = {'root': Path(temporary), 'invocation': object(), 'runtime_permit': object(),
                         'probes': {}, '_read_owner_seed': types.SimpleNamespace(public_output_failure=output_failure)}
                if many_operations:
                    state['probes'] = {index: types.SimpleNamespace(document=lambda: {'synthetic_payload': 'x' * 20000}) for index in range(501)}
                original = ValueError('original collection failure')
                with self.assertRaises(ValueError) as raised:
                    ns['isolated_failed_collection'](state, original)
                self.assertIs(raised.exception, original)
                saved = json.loads((state['root'] / 'collection-failed.json').read_bytes())
                self.assertLess((state['root'] / 'collection-failed.json').stat().st_size, 8 * 1024 ** 2)
                if many_operations:
                    self.assertEqual(saved['owned_operation_count'], 501)
                    self.assertNotIn('owned_operations', saved)
                    self.assertEqual(saved['owned_operation_evidence_root'], str(state['root'].parent / 'operations'))
                self.assertEqual(saved['error'], str(original))
                self.assertEqual(saved['preservation'], 'failed')
                self.assertEqual(len(calls), 1)
                if accessor_fails:
                    self.assertEqual(saved['public_output_preservation'], {'status': 'unavailable'})
                    self.assertEqual(saved['public_output_preservation_error'], 'output audit unavailable')
                else:
                    self.assertEqual(saved['public_output_preservation'], {'status': 'verified', 'retained_failure_audit': True})
                    self.assertNotIn('public_output_preservation_error', saved)


if __name__ == '__main__':
    unittest.main()
