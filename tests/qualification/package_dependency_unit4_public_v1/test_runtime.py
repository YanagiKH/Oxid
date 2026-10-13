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
    if '_validate_evidence_roster' in names:
        names = (*names, '_validate_evidence_sections')
    for name in ('_infrastructure', '_failure_details', '_safe_error_text', '_is_semantic_error', 'InfrastructureFailure', '_CheckpointedRows',
                 '_work_checkpoint', '_copy_receipts', '_replay_receipts'):
        namespace.setdefault(name, getattr(runtime, name))
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


@contextlib.contextmanager
def authenticated_contract_fixture(test):
    import gzip
    import io
    import os
    import tarfile
    repo = Path(os.environ.get('OXID_RETAINED_SOURCE_TEST_REPOSITORY', str(ROOT.parents[2]))).resolve()
    transport = repo / 'tests/fixtures/typed_project_unit4_contracts'
    if not transport.is_dir():
        test.skipTest('installed retained source fixture is not present in this isolated candidate')
    manifest_raw = (transport / 'transport-manifest.json').read_bytes()
    test.assertEqual(runtime.sha(manifest_raw), '631bd4dcc5e3c2e2887debddf8a8657af5247fb985a8d6fcc45cbf78b701c687')
    manifest = json.loads(manifest_raw)
    archived = (transport / 'authorities.tar.gz').read_bytes()
    test.assertEqual(len(archived), 2301283)
    test.assertEqual(runtime.sha(archived), '3814e436ee6364c42e7942da68141bfada99ba692eb5b491c30520ea2e881c3a')
    with gzip.GzipFile(fileobj=io.BytesIO(archived)) as source:
        decoded = source.read(10 * 1024 ** 2 + 1)
    test.assertEqual(len(decoded), 8755200)
    test.assertLess(len(decoded), 10 * 1024 ** 2)
    test.assertEqual(runtime.sha(decoded), manifest['archive']['decoded_sha256'])
    selected = {r['archive_path']: r for r in manifest['members'] if any(role.startswith('public-') or role == 'active-public-freeze' for role in r['roles'])}
    test.assertEqual(len(selected), 141)
    test.assertEqual(sum(r['bytes'] for r in selected.values()), 7661184)
    test.assertLess(sum(r['bytes'] for r in selected.values()), 8 * 1024 ** 2)
    helper_path = repo / runtime.PUBLIC / 'contracts.py'
    helper = helper_path.read_bytes()
    test.assertEqual(runtime.sha(helper), '170515201950aa6018aa176e9d02720d7fdca5f9e52fae123a3a01c71d8a6ac9')
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
                test.assertTrue(member.isfile() and not path.is_absolute() and '..' not in path.parts)
                test.assertNotIn(name, seen)
                seen.add(name)
                if name not in selected:
                    continue
                row = selected[name]
                test.assertEqual(member.size, row['bytes'])
                raw = archive.extractfile(member).read(row['bytes'] + 1)
                test.assertEqual(len(raw), row['bytes'])
                test.assertEqual(runtime.sha(raw), row['sha256'])
                destination = root / name
                destination.parent.mkdir(parents=True, exist_ok=True)
                with destination.open('xb') as sink:
                    sink.write(raw)
                path_map[row['logical_path']] = str(destination)
                written.add(name)
        test.assertEqual(written, set(selected))
        test.assertEqual(seen, {row['archive_path'] for row in manifest['members']})
        test.assertEqual(len(seen), 158)
        contract_root = Path(path_map[manifest['active_contracts']['public']['logical_path']]).parent
        amendment = repo / 'tests/fixtures/typed_project_unit4_public_location_amendment_v1'
        contracts = contracts_module.Contracts(contract_root, path_map, amendment)
        yield repo, contracts_module, contracts


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
        with authenticated_contract_fixture(self) as (repo, contracts_module, contracts):
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
            with self.assertRaises(runtime.Rejected if fail_at == 1 else runtime.InfrastructureFailure):
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
                with self.assertRaises(runtime.InfrastructureFailure):
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
        self.assertEqual(owners, {'_collect_section': 1, 'verify_collection': 1,
                                  '_verify_diagnostic_payload': 1})

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
        ns = {'canonical': runtime.canonical, 'need': runtime.need, '_failure_details': runtime._failure_details, '_safe_error_text': runtime._safe_error_text,
              '_checkpoint': lambda *args, **kwargs: (_ for _ in ()).throw(RuntimeError('full preservation failed'))}
        exec(compile(ast.fix_missing_locations(ast.Module(body=[fake], type_ignores=[])), 'isolated_failed_collection', 'exec'), ns)
        for accessor_fails, many_operations in ((False, False), (True, False), (False, True), (True, True)):
            with self.subTest(accessor_fails=accessor_fails, many_operations=many_operations), tempfile.TemporaryDirectory() as temporary:
                calls = []
                def output_failure(invocation, permit):
                    calls.append((invocation, permit))
                    if accessor_fails:
                        raise RuntimeError('output audit unavailable')
                    return {'status': 'verified', 'retained_failure_audit': True,
                            'first_failed_attempt': {'phase': 'sealed', 'started': 3.0, 'deadline': 4.0,
                                                     'completed': 4.0, 'counts': {'sealed': 7},
                                                     'error': 'original output audit timeout'},
                            'preservation_attempt': {'success': False, 'error': 'later preservation failure'}}
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
                self.assertEqual(saved['original_failure']['type'], type(original).__name__)
                self.assertEqual(saved['original_failure']['error'], str(original))
                self.assertTrue(saved['original_failure']['stack'])
                self.assertEqual(saved['preservation'], 'failed')
                self.assertEqual(len(calls), 1)
                if accessor_fails:
                    self.assertEqual(saved['public_output_preservation'], {'status': 'unavailable'})
                    self.assertEqual(saved['public_output_preservation_error'], 'output audit unavailable')
                else:
                    retained = saved['public_output_preservation']
                    self.assertTrue(retained['retained_failure_audit'])
                    self.assertEqual(retained['first_failed_attempt']['error'], 'original output audit timeout')
                    self.assertEqual(retained['first_failed_attempt']['counts'], {'sealed': 7})
                    self.assertEqual(retained['preservation_attempt']['error'], 'later preservation failure')
                    self.assertEqual(retained['first_failed_attempt']['completed'], 4.0)
                    self.assertNotIn('public_output_preservation_error', saved)




def retained_inventory_fixture():
    """AST-selected frozen predicates, never an import or producer admission."""
    import os
    repo = Path(os.environ.get('OXID_RETAINED_SOURCE_TEST_REPOSITORY', str(ROOT.parents[2]))).resolve()
    path = repo / 'tests/qualification/unit4_public_v3/contracts.py'
    if not path.is_file():
        raise unittest.SkipTest('installed retained inventory is absent from this isolated candidate')
    tree = ast.parse(path.read_bytes())
    class RetainedReject(Exception): pass
    ns = {'Reject': RetainedReject}
    selected = {'IDENTITY_FIELDS', 'EFFECTIVE_FIELDS', 'EXCLUDED_REASON', 'REMOTE_REASON', 'CAPABILITY_REASON'}
    for node in tree.body:
        if isinstance(node, ast.Assign) and len(node.targets) == 1 and isinstance(node.targets[0], ast.Name) and node.targets[0].id in selected:
            ns[node.targets[0].id] = ast.literal_eval(node.value)
    if not selected <= set(ns):
        raise AssertionError('missing retained inventory constants')
    functions(path, ('need', 'receipt_identity', 'check_inventory'), ns)
    return ns, tree


def negative_control_fixture():
    retained, tree = retained_inventory_fixture()
    checker, row, got, cfg, sources = common_fixture()
    row.update(contract_identity='test-contract', group='test', case_id='one', observation_index=0,
               host='Linux x86_64', execution_host='Linux x86_64', required_hosts=['Linux x86_64'],
               format='text', required_capabilities=[])
    got.update(retained['receipt_identity'](row), status='pass')
    ns = {'need': runtime.need, 'Rejected': runtime.Rejected, 'Reject': retained['Reject'],
          'copy': copy, 'json': json, 'check_inventory': retained['check_inventory']}
    functions(ROOT / 'runtime.py', ('aggregate', 'negative_controls'), ns)
    return ns, [row], [got], lambda r, g: checker(r, g, cfg, sources, {}), retained, tree


class ReadbackCheckpointTests(unittest.TestCase):
    def test_adapter_preserves_exact_retained_access_protocol(self):
        retained, tree = retained_inventory_fixture()
        checker = next(n for n in tree.body if isinstance(n, ast.FunctionDef) and n.name == 'check_inventory')
        parents = {child: parent for parent in ast.walk(checker) for child in ast.iter_child_nodes(parent)}
        for node in ast.walk(checker):
            if not isinstance(node, ast.Name) or node.id not in ('rows', 'receipts'):
                continue
            parent = parents[node]
            allowed = (isinstance(parent, ast.BoolOp) or
                       isinstance(parent, (ast.For, ast.comprehension)) and parent.iter is node or
                       isinstance(parent, ast.Call) and isinstance(parent.func, ast.Name) and
                       parent.func.id in ('bool', 'len', 'iter') and parent.args == [node])
            self.assertTrue(allowed, 'unsupported retained inventory access: ' + ast.dump(parent))
        ticks = []
        empty = runtime._CheckpointedRows([], lambda: ticks.append(1))
        self.assertFalse(empty)
        self.assertEqual(len(empty), 0)
        rows = [{'key': 'a'}, {'key': 'b'}]
        view = runtime._CheckpointedRows(rows, lambda: ticks.append(1))
        self.assertTrue(view)
        self.assertEqual(len(view), len(rows))
        self.assertEqual(list(view), rows)
        self.assertEqual(list(view), rows)
        self.assertEqual(len(ticks), 8)
        self.assertIs(list(view)[0], rows[0])
        with self.assertRaises(retained['Reject']):
            retained['check_inventory'](empty, empty)

    def test_complete_copy_preserves_aliases_and_originals(self):
        shared = {'nested': [1, 2]}
        original = [shared, shared, {'remote': True}]
        original.append({'outer': original})
        ticks = []
        cloned = runtime._copy_receipts(original, lambda: ticks.append(1))
        self.assertEqual(len(ticks), 8)
        self.assertIs(cloned[0], cloned[1])
        self.assertIs(cloned[3]['outer'], cloned)
        self.assertIsNot(cloned[0], shared)
        cloned[0]['nested'].append(3)
        self.assertEqual(shared['nested'], [1, 2])
        self.assertTrue(cloned[2]['remote'])

    def test_all_fourteen_semantic_controls_still_reject(self):
        ns, rows, receipts, checker, retained, _ = negative_control_fixture()
        original = copy.deepcopy(receipts)
        ticks = []
        plain = retained['check_inventory'](rows, receipts)
        measured = ns['aggregate'](rows, receipts, checkpoint=lambda: ticks.append(1))
        self.assertEqual({key: measured[key] for key in plain}, plain)
        controls = ns['negative_controls'](rows, receipts, checker, checkpoint=lambda: ticks.append(1))
        self.assertEqual([c['control'] for c in controls], ['zero', 'missing', 'duplicate', 'extra',
            'false-skip', 'failed-predicate', 'wrong-host', 'wrong-profile', 'wrong-format',
            'stale-contract', 'stale-candidate', 'wrong-argv', 'changed-source', 'wrong-stream'])
        self.assertTrue(all(c['status'] == 'REJECTED' for c in controls))
        self.assertEqual(receipts, original)
        self.assertGreater(len(ticks), 14)
        # Every conditional mutation remains in the actual function body.
        source = ast.unparse(next(n for n in ast.parse((ROOT / 'runtime.py').read_bytes()).body
                                 if isinstance(n, ast.FunctionDef) and n.name == 'negative_controls'))
        for name in ('missing-native', 'sources-available', 'changed-ELF', 'missing-tools',
                     'missing-passivity-binding', 'missing-trace', 'duplicate-checker', 'source-reopen',
                     'changed-competitor', 'missing-race-barrier', 'fabricated-execution'):
            self.assertIn(repr(name), source)

    def test_zero_local_domain_preserves_all_five_semantic_controls(self):
        ns, rows, receipts, _checker, retained, _ = negative_control_fixture()
        rows[0]['scope'] = 'excluded-host'
        rows[0]['host'] = 'macOS x86_64'
        receipts[0] = {**retained['receipt_identity'](rows[0]), 'executed': False,
                       'status': 'excluded-host', 'reason': retained['EXCLUDED_REASON']}
        checker_calls, ticks = [], []
        controls = ns['negative_controls'](rows, receipts, lambda *args: checker_calls.append(args),
                                           checkpoint=lambda: ticks.append(1))
        self.assertEqual([c['control'] for c in controls],
                         ['zero', 'missing', 'duplicate', 'extra', 'fabricated-execution'])
        self.assertTrue(all(c['status'] == 'REJECTED' for c in controls))
        self.assertEqual(checker_calls, [])
        self.assertGreater(len(ticks), 5)

    def test_every_production_inventory_and_history_call_supplies_checkpoint(self):
        tree = ast.parse((ROOT / 'runtime.py').read_bytes())
        calls = [node for node in ast.walk(tree) if isinstance(node, ast.Call) and
                 isinstance(node.func, ast.Name) and node.func.id in ('aggregate', 'negative_controls', 'predecessor_history')]
        self.assertGreaterEqual(len(calls), 8)
        for call in calls:
            self.assertIn('checkpoint', {k.arg for k in call.keywords}, (call.lineno, call.func.id))

    def test_no_generic_or_infrastructure_exception_can_pass_a_control(self):
        ns, rows, receipts, _checker, _retained, _ = negative_control_fixture()
        class ProducerRejected(ValueError): pass
        class Cancelled(RuntimeError): pass
        errors = [ValueError('unexpected'), TypeError('unexpected'), KeyError('unexpected'),
                  AssertionError('unexpected'), OSError('input read'), Cancelled('cancelled'),
                  ProducerRejected('producer'), runtime.InfrastructureFailure('cadence', ProducerRejected('gap')),
                  runtime.InfrastructureFailure('deadline', ProducerRejected('expired')),
                  runtime.InfrastructureFailure('producer-cleanup', OSError('group remains')),
                  runtime.InfrastructureFailure('owner', ProducerRejected('changed'))]
        for error in errors:
            with self.subTest(error=type(error).__name__, message=str(error)):
                calls = []
                def checker(row, got):
                    calls.append(row['key'])
                    raise error
                with self.assertRaises(type(error)) as raised:
                    ns['negative_controls'](rows, receipts, checker, checkpoint=lambda: None)
                self.assertIs(raised.exception, error)
                self.assertEqual(len(calls), 1)

    def test_semantic_rejection_cannot_hide_poisoned_checkpoint(self):
        ns, rows, receipts, _checker, _retained, _ = negative_control_fixture()
        poisoned, calls = [False], []
        primary = OSError('cleanup failed after semantic result')
        def checker(row, got):
            calls.append(row['key'])
            poisoned[0] = True
            raise runtime.Rejected('semantic mismatch')
        def checkpoint():
            if poisoned[0]:
                raise primary
        with self.assertRaises(runtime.InfrastructureFailure) as raised:
            ns['negative_controls'](rows, receipts, checker, checkpoint=checkpoint)
        self.assertIs(raised.exception.original, primary)
        self.assertEqual(len(calls), 1)

    def test_virtual_cadence_services_copy_inventory_and_replay(self):
        class Cadence(ValueError): pass
        class Clock:
            due, start_limit, execution_budget = 3.0, 5.0, 14.0
            def __init__(self): self.now, self.anchor, self.calls, self.failed = 0.0, 0.0, 0, False
            def tick(self):
                self.calls += 1
                if self.failed or self.now - self.anchor > self.start_limit:
                    self.failed = True
                    raise Cadence('scheduled start gap')
                if self.now - self.anchor >= self.due:
                    self.anchor = self.now
        class SlowCopy:
            def __deepcopy__(self, memo):
                clock.now += 2.0
                return {'copied': True}
        clock = Clock()
        source = [SlowCopy() for _ in range(4)]
        copy.deepcopy(source)
        with self.assertRaises(runtime.InfrastructureFailure): runtime._work_checkpoint(clock.tick)
        self.assertEqual(clock.anchor, 0)
        clock = Clock()
        self.assertEqual(runtime._copy_receipts(source, clock.tick), [{'copied': True}] * 4)
        self.assertFalse(clock.failed)
        self.assertEqual(clock.calls, 8)
        clock = Clock()
        runtime._replay_receipts([1] * 4, [{'executed': True}] * 4,
            lambda row, got: setattr(clock, 'now', clock.now + 2.0), clock.tick)
        self.assertFalse(clock.failed)
        ns, rows, receipts, _checker, _retained, _ = negative_control_fixture()
        class CostlyRow(dict):
            def __getitem__(self, key):
                clock.now += 0.01
                return super().__getitem__(key)
            def get(self, key, default=None):
                clock.now += 0.01
                return super().get(key, default)
        many_rows, many_receipts = [], []
        for i in range(32):
            row, got = copy.deepcopy(rows[0]), copy.deepcopy(receipts[0])
            row['key'] = got['key'] = str(i)
            many_rows.append(CostlyRow(row)); many_receipts.append(CostlyRow(got))
        clock = Clock()
        ns['aggregate'](many_rows, many_receipts)
        with self.assertRaises(runtime.InfrastructureFailure): runtime._work_checkpoint(clock.tick)
        clock = Clock()
        ns['aggregate'](many_rows, many_receipts, checkpoint=clock.tick)
        self.assertFalse(clock.failed)
        self.assertGreater(clock.calls, 32)

    def test_original_producer_arguments_and_cleanup_are_distinct_from_mutation(self):
        calls = []
        document = {'pid': 4, 'started_ns': 8, 'argv': ['/compiler', 'check'], 'cwd': '/fixture',
                    'environment': {}, 'operation_kind': 'public-runtime', 'cleanup_complete': True,
                    'cleanup_error': None, 'failure': None, 'timed_out': False, 'stream_limit_exceeded': False}
        record = types.SimpleNamespace(document=lambda: copy.deepcopy(document), stdout=lambda: b'ok', stderr=lambda: b'')
        def verify(record, invocation, **kwargs):
            calls.append(kwargs)
            return copy.deepcopy(document)
        prep = types.SimpleNamespace(verify_probe=verify)
        case = {'section': 'predecessors', 'index': 0, 'variant': 'ordinary'}
        owner = {'probes': {(4, 8): record}, 'process_cases': {(4, 8): case}, 'native_permits': {},
                 'invocation': object(), 'runtime_permit': object()}
        ns = {'need': runtime.need, 'sha': runtime.sha, '_preparation': lambda: prep}
        functions(ROOT / 'runtime.py', ('_verify_process',), ns)
        process = copy.deepcopy(document)
        for name, raw in (('stdout', b'ok'), ('stderr', b'')):
            process[name], process[name + '_sha256'] = raw.decode(), runtime.sha(raw)
        ns['_verify_process'](owner, process, case)
        mutated = copy.deepcopy(process); mutated['argv'].append('--invented')
        with self.assertRaises(runtime.Rejected): ns['_verify_process'](owner, mutated, case)
        self.assertEqual(calls[-1]['argv'], document['argv'])
        self.assertNotEqual(calls[-1]['argv'], mutated['argv'])
        document['cleanup_complete'], document['cleanup_error'] = False, 'group remains'
        before = len(calls)
        with self.assertRaises(runtime.InfrastructureFailure) as raised:
            ns['_verify_process'](owner, process, case)
        self.assertEqual(raised.exception.phase, 'producer-cleanup')
        self.assertEqual(len(calls), before)
        document['cleanup_complete'], document['cleanup_error'] = True, None
        original = ValueError('producer owner changed')
        def failed(*args, **kwargs): raise original
        prep.verify_probe = failed
        with self.assertRaises(runtime.InfrastructureFailure) as raised:
            ns['_verify_process'](owner, process, case)
        self.assertIs(raised.exception.original, original)
        detail = runtime._failure_details(raised.exception)
        self.assertEqual(detail['type'], 'ValueError')
        self.assertEqual(detail['phase'], 'producer-readmission')
        self.assertTrue(any(frame['function'] == 'failed' for frame in detail['stack']))
        self.assertLessEqual(len(detail['stack']), 32)

    def test_owner_and_document_failures_are_wrapped_before_semantics(self):
        original = runtime.Rejected('original owner drift')
        def failed(*args, **kwargs): raise original
        for selected in ('_identity', '_read'):
            ns = {'_active_boundary': failed, 'Path': Path}
            functions(ROOT / 'runtime.py', (selected,), ns)
            with self.assertRaises(runtime.InfrastructureFailure) as raised:
                ns[selected]('/inert-evidence')
            self.assertIs(raised.exception.original, original)
            self.assertEqual(raised.exception.phase, 'read-owner')
        record = types.SimpleNamespace(document=lambda: {})
        owner = {'probes': {(1, 2): record}, 'process_cases': {(1, 2): {'case': 'one'}}}
        ns = {'need': runtime.need, '_preparation': failed}
        functions(ROOT / 'runtime.py', ('_verify_process',), ns)
        with self.assertRaises(runtime.InfrastructureFailure) as raised:
            ns['_verify_process'](owner, {'pid': 1, 'started_ns': 2}, {'case': 'one'})
        self.assertIsInstance(raised.exception.original, KeyError)
        self.assertEqual(raised.exception.phase, 'producer-readmission')
        record.document = lambda: {'cleanup_complete': True, 'cleanup_error': None}
        with self.assertRaises(runtime.InfrastructureFailure) as raised:
            ns['_verify_process'](owner, {'pid': 1, 'started_ns': 2}, {'case': 'one'})
        self.assertIs(raised.exception.original, original)
        self.assertEqual(raised.exception.phase, 'producer-readmission')

    def test_diagnostic_formatting_cannot_replace_original_failure(self):
        class Unprintable(ValueError):
            def __str__(self): raise RuntimeError('formatting failed')
        original = Unprintable()
        wrapped = runtime.InfrastructureFailure('input', original)
        self.assertIn('exception text unavailable', str(wrapped))
        def failed(*args, **kwargs): raise OSError('traceback formatter unavailable')
        ns = {'traceback': types.SimpleNamespace(StackSummary=types.SimpleNamespace(extract=failed),
                                                walk_tb=lambda tb: [])}
        functions(ROOT / 'runtime.py', ('_safe_error_text', '_failure_details'), ns)
        result = ns['_failure_details'](wrapped)
        self.assertEqual(result['stack'], [])
        self.assertIn('diagnostic_error', result)
        self.assertIs(wrapped.original, original)

    def test_failure_stack_does_not_open_source_or_request_lines(self):
        import builtins
        import linecache
        saved_open, saved_getline = builtins.open, linecache.getline
        def forbidden(*args, **kwargs):
            raise AssertionError('failure diagnostics attempted source lookup')
        original = ValueError('first failure')
        try:
            raise original
        except ValueError:
            builtins.open, linecache.getline = forbidden, forbidden
            try:
                details = runtime._failure_details(original)
            finally:
                builtins.open, linecache.getline = saved_open, saved_getline
        self.assertEqual(details['error'], 'first failure')
        self.assertNotIn('diagnostic_error', details)
        self.assertTrue(details['stack'])
        self.assertTrue(all(set(frame) == {'file', 'function', 'line'} for frame in details['stack']))

    def test_actual_tick_rejects_cadence_deadline_poison_and_input_failures(self):
        class ProducerRejected(ValueError): pass
        class Cadence(ProducerRejected): pass
        class Deadline(ProducerRejected): pass
        class Poisoned(ProducerRejected): pass
        for error in (Cadence('missed'), Deadline('expired'), Poisoned('failed'), OSError('input changed')):
            for target in ('check_deadline', 'public_output_tick'):
                with self.subTest(error=type(error).__name__, target=target):
                    def failed(*args, **kwargs): raise error
                    prep = types.SimpleNamespace(check_deadline=lambda inv: None, public_output_tick=lambda inv: None)
                    setattr(prep, target, failed)
                    invocation = object()
                    ns = {'_active_boundary': lambda: {'invocation': invocation, 'preparation': prep}}
                    functions(ROOT / 'runtime.py', ('_tick',), ns)
                    with self.assertRaises(runtime.InfrastructureFailure) as raised:
                        ns['_tick'](invocation)
                    self.assertIs(raised.exception.original, error)
                    self.assertEqual(raised.exception.phase, 'checkpoint' if target == 'check_deadline' else 'output-audit')
                    self.assertTrue(runtime._failure_details(raised.exception)['stack'])

    def test_semantic_primary_plus_failed_preservation_is_infrastructure(self):
        ns, state, events, control = isolated_boundary_fixture()
        semantic = runtime.Rejected('expected semantic mismatch')
        control['fail_auth_at'] = 2
        with self.assertRaises(runtime.InfrastructureFailure) as raised:
            with ns['_read_boundary'](state): raise semantic
        self.assertEqual(raised.exception.phase, 'read-boundary-preservation')
        self.assertIs(raised.exception.preceding_semantic, semantic)
        self.assertIs(raised.exception.__cause__, raised.exception.original)
        details = runtime._failure_details(raised.exception)
        self.assertEqual(details['preceding_semantic']['type'], 'Rejected')
        self.assertEqual(details['preceding_semantic']['error'], str(semantic))
        self.assertTrue(state['invalidated'])
        self.assertIsNone(ns['_active_boundary']())

    def test_boundary_preservation_keeps_first_error_and_invalidates(self):
        ns, state, events, control = isolated_boundary_fixture()
        primary = runtime.InfrastructureFailure('cadence', ValueError('first cadence failure'))
        control['fail_auth_at'] = 2
        with self.assertRaises(runtime.InfrastructureFailure) as raised:
            with ns['_read_boundary'](state): raise primary
        self.assertIs(raised.exception, primary)
        self.assertTrue(state['invalidated'])
        self.assertTrue(any('preservation failed' in n for n in primary.__notes__))
        self.assertIsNone(ns['_active_boundary']())
        self.assertEqual(len(runtime._ADMISSIONS), 0)
        self.assertEqual(len(runtime._READ_BOUNDARIES), 0)


def structural_membership_fixture(test, repo, contracts, root):
    """Actual source/membership/readback functions; envelopes are inert test data."""
    import os
    import stat
    retained = repo / runtime.PUBLIC
    for filename, expected in (
        ('run.py', '92d959a2250f3e171412f9a0ca5fb36612426ddc70e7223a3247d9580db8c8da'),
        ('runtime.py', '866f839279f65ff4d8e57a49190202785aa2b4139097156a00aa1e952b3b9db8')):
        test.assertEqual(runtime.sha((retained / filename).read_bytes()), expected)
    ns = {'Path': Path, 'stat': stat, 'os': os, 'json': json, 'need': runtime.need,
          'Rejected': runtime.Rejected, 'sha': runtime.sha, 'SECTIONS': runtime.SECTIONS,
          'TOOL_LOG_LIMIT': 2 * 1024 ** 2, '_tick': lambda invocation: None,
          '_read': lambda path, invocation=None: Path(path).read_bytes(),
          'load': lambda path: json.loads(Path(path).read_bytes()),
          '_binding': lambda: types.SimpleNamespace(decode=json.loads)}
    def identity(path):
        raw = Path(path).read_bytes()
        return {'path': str(path), 'bytes': len(raw), 'sha256': runtime.sha(raw)}
    ns['_identity'] = identity
    functions(retained / 'run.py', ('tuple_sources',), ns)
    functions(retained / 'runtime.py', ('materialize', 'read_events'), ns)
    ns['rt'] = types.SimpleNamespace(read_events=ns['read_events'])
    functions(ROOT / 'runtime.py', ('_row_sources', '_verify_row_files', '_case_evidence_roster',
              '_validate_evidence_members', '_validate_case_evidence', '_validate_evidence_roster'), ns)
    state = {'collection_root': root, 'invocation': None, 'contracts': contracts,
             'helpers': {'run': types.SimpleNamespace(tuple_sources=ns['tuple_sources'])},
             'predecessors': None,
             'rosters': {section: contracts.roster(section, 'Linux x86_64') for section in runtime.SECTIONS}}
    return ns, state


def emit_structural_case(test, ns, state, section, index, *, executed=True,
                         optional_logs=False, emitted=False, native=False, events=False):
    """Build synthetic structural envelopes independently of the roster validator."""
    row = state['rosters'][section][index]
    test.assertEqual(row['scope'], 'execute')
    root = state['collection_root']
    case = root / section / (('%03d' if section == 'guards' else '%05d') % index)
    sources = ns['_row_sources'](state, section, row)
    roles = ('ordinary', 'observer') if section in ('guards', 'lifecycle') else ('ordinary',)
    fixtures = ([case / 'fixtures' / row['case']['source_case_id']] if section == 'guards' else
                [case / role / 'fixture' for role in roles])
    for fixture in fixtures:
        ns['materialize'](sources, fixture)
    receipts = []
    def write(path, raw):
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(raw)
    for role in roles:
        evidence = case / role
        evidence.mkdir(parents=True, exist_ok=True)
        fixture = fixtures[0] if section == 'guards' else evidence / 'fixture'
        got = {'executed': executed, 'status': 'synthetic-structural-only',
               'fixture_directory': str(fixture), 'source_files': []}
        if executed:
            got.update(tool_invocations=[{'synthetic': 'tool-event'}] if events else [],
                       trace=[{'synthetic': 'lifecycle-event'}] if events else [],
                       process={'stdout': 'inert structural envelope\n', 'stderr': '', 'status': 1})
            if section == 'guards':
                got['fixture_root'] = str(case / 'fixtures')
                got['process']['environment'] = {'UNIT4_TOOL_EVIDENCE': str(evidence)}
            for stream in ('stdout', 'stderr'):
                write(evidence / stream, got['process'][stream].encode())
            write(evidence / 'process.json', runtime.canonical(got['process']))
            for name, field in (('tools.jsonl', 'tool_invocations'), ('lifecycle.jsonl', 'trace')):
                if optional_logs or got[field]:
                    write(evidence / name, b''.join(runtime.canonical(item) + b'\n' for item in got[field]))
            if emitted:
                write(evidence / 'program.ll', b'; inert structural LLVM-shaped file\n')
                got['emitted_llvm'] = ns['_identity'](evidence / 'program.ll')
            if native:
                write(fixture / 'out.bin', b'inert source-free structural bytes')
                write(evidence / 'native-source-free/program', b'inert source-free structural bytes')
                got['native_execution'] = {
                    'original_elf': ns['_identity'](fixture / 'out.bin'),
                    'copied_elf': ns['_identity'](evidence / 'native-source-free/program')}
        write(evidence / 'receipt.json', runtime.canonical(got))
        receipts.append(got)
    got = receipts[-1]
    if len(roles) == 2:
        got['ordinary_receipt'] = ns['_identity'](case / 'ordinary/receipt.json')
        write(case / 'observer/receipt.json', runtime.canonical(got))
    write(case / 'receipt.json', runtime.canonical(got))
    if section == 'guards':
        if row['group'] == 'race':
            for path in (fixtures[0] / 'out.bin', case / 'ordinary/competitor.bytes',
                         case / 'ordinary/linked-before-publication.elf', case / 'observer/linked-before-publication.elf'):
                write(path, b'inert race structural bytes')
        else:
            write(case / 'preserved-target', b'inert preserved structural bytes')
            if row['case']['output_kind'] == 'symlink':
                (case / 'occupied-output').symlink_to('preserved-target')
            else:
                write(case / 'occupied-output', b'inert occupied structural bytes')
    return got


def emit_structural_section_closes(root):
    names = ['input-integrity.json', 'tools.json', 'qualified-paths-amendment.json',
             'predecessors/historical-comparison.json']
    names += [section + '/' + name for section in runtime.SECTIONS
              for name in ('roster.json', 'observations.jsonl.gz', 'comparison.json')]
    for name in names:
        path = root / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(b'explicitly synthetic collection close; no runtime success claim')


class RequiredFixtureDirectoryTests(unittest.TestCase):
    def test_authenticated_empty_source_rosters_all_hosts(self):
        with authenticated_contract_fixture(self) as (repo, module, contracts):
            ns, state = structural_membership_fixture(self, repo, contracts, Path('/unused-structural-root'))
            for host in module.HOSTS:
                rows = contracts.roster('lifecycle', host)
                self.assertEqual(len(rows), 1328)
                empty = [(index, row) for index, row in enumerate(rows)
                         if not ns['_row_sources'](state, 'lifecycle', row)]
                self.assertEqual([index for index, _ in empty], list(range(1216, 1224)))
                self.assertEqual({row['case_id'] for _, row in empty}, {'missing-root-open'})
                self.assertEqual(sum(row['scope'] == 'execute' for _, row in empty), 2)
                self.assertEqual(sum(row['scope'] == 'unavailable-host' for _, row in empty), 6)
                for _, row in empty:
                    self.assertEqual(row['scope'], 'execute' if row['host'] == host else 'unavailable-host')
                    self.assertEqual(json.loads(row['key']), ['unit4-lifecycle-replacement-v3', 'heldout',
                                     'missing-root-open', 0, row['profile'], row['host']])
            rows = state['rosters']['lifecycle']
            self.assertEqual({scope: sum(row['scope'] == scope for row in rows)
                              for scope in ('execute', 'unavailable-host', 'excluded-host')},
                             {'execute': 318, 'unavailable-host': 462, 'excluded-host': 548})

    def test_all_318_lifecycle_rows_synthetic_structural_case_and_collection(self):
        with authenticated_contract_fixture(self) as (repo, module, contracts), tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            ns, state = structural_membership_fixture(self, repo, contracts, root)
            # Only lifecycle rows are populated; all five collection-close shapes are synthetic.
            for section in runtime.SECTIONS:
                if section != 'lifecycle':
                    state['rosters'][section] = []
            receipts = {section: [] for section in runtime.SECTIONS}
            executed, empty_roots, tail = [], set(), []
            for index, row in enumerate(state['rosters']['lifecycle']):
                if row['scope'] != 'execute':
                    receipts['lifecycle'].append({'executed': False, 'status': row['scope']})
                    self.assertFalse((root / 'lifecycle' / ('%05d' % index)).exists())
                    continue
                got = emit_structural_case(self, ns, state, 'lifecycle', index)
                self.assertTrue(ns['_validate_case_evidence'](state, 'lifecycle', index, got))
                empty_roots.update(ns['_case_evidence_roster'](state, 'lifecycle', index, row, got)[3])
                executed.append(index)
                if index > 1216:
                    tail.append(index)
                receipts['lifecycle'].append(got)
            self.assertEqual(len(executed), 318)
            self.assertEqual(len([index for index in executed if index <= 1216]), 291)
            self.assertEqual(tail, [1220] + list(range(1224, 1328, 4)))
            self.assertEqual(len(tail), 27)
            self.assertEqual(empty_roots, {f'lifecycle/{index:05d}/{role}/fixture'
                                           for index in (1216, 1220) for role in ('ordinary', 'observer')})
            emit_structural_section_closes(root)
            self.assertTrue(ns['_validate_evidence_roster'](state, receipts))
            # Collection aggregation also requires the source-derived empty roots.
            empty = root / 'lifecycle/01220/observer/fixture'
            empty.rmdir()
            with self.assertRaisesRegex(runtime.Rejected, 'directory'):
                ns['_validate_evidence_roster'](state, receipts)
            empty.mkdir()
            extra = root / 'lifecycle/01217/ordinary/fixture'
            extra.mkdir(parents=True)
            with self.assertRaisesRegex(runtime.Rejected, 'directory'):
                ns['_validate_evidence_roster'](state, receipts)

    def test_empty_roots_exact_mutation_matrix(self):
        import shutil
        with authenticated_contract_fixture(self) as (repo, module, contracts), tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            ns, state = structural_membership_fixture(self, repo, contracts, root)
            got = emit_structural_case(self, ns, state, 'lifecycle', 1216)
            case = root / 'lifecycle/01216'
            validate = lambda: ns['_validate_case_evidence'](state, 'lifecycle', 1216, got)
            self.assertTrue(validate())
            for role in ('ordinary', 'observer'):
                fixture = case / role / 'fixture'
                fixture.rmdir()
                with self.subTest(role=role, mutation='missing'), self.assertRaises(runtime.Rejected):
                    validate()
                fixture.write_bytes(b'not a directory')
                with self.subTest(role=role, mutation='regular-file'), self.assertRaises(runtime.Rejected):
                    validate()
                fixture.unlink()
                fixture.symlink_to('../' + ('observer' if role == 'ordinary' else 'ordinary') + '/fixture')
                with self.subTest(role=role, mutation='symlink'), self.assertRaises(runtime.Rejected):
                    validate()
                fixture.unlink()
                fixture.mkdir()
                self.assertTrue(validate())
            for relative in ('extra', 'ordinary/extra', 'ordinary/fixture/nested', 'third-role/fixture', 'fixture'):
                path = case / relative
                path.mkdir(parents=True)
                with self.subTest(mutation=relative), self.assertRaises(runtime.Rejected):
                    validate()
                shutil.rmtree(case / relative.split('/')[0] if relative.startswith('third-role/') else path)
            for relative in ('unexpected', 'ordinary/fixture/main.ox', 'observer/fixture/main.ox'):
                path = case / relative
                path.write_bytes(b'extra source')
                with self.subTest(mutation=relative), self.assertRaises(runtime.Rejected):
                    validate()
                path.unlink()
                path.symlink_to('preserved-target')
                with self.subTest(mutation=relative + '-link'), self.assertRaises(runtime.Rejected):
                    validate()
                path.unlink()
            for path in (case / 'ordinary/stdout', case / 'observer/process.json', case / 'observer/lifecycle.jsonl'):
                before = path.read_bytes() if path.exists() else None
                path.write_bytes(b'{}\n')
                with self.subTest(mutation=path.name), self.assertRaises(runtime.Rejected):
                    validate()
                path.unlink() if before is None else path.write_bytes(before)
            wrong = copy.deepcopy(got)
            wrong['ordinary_receipt']['path'] = str(root / 'lifecycle/01220/ordinary/receipt.json')
            with self.assertRaisesRegex(runtime.Rejected, 'exact indexed path'):
                ns['_validate_case_evidence'](state, 'lifecycle', 1216, wrong)
            with self.assertRaises(runtime.Rejected):
                ns['_validate_case_evidence'](state, 'lifecycle', 1220, got)
            self.assertTrue(validate())

    def test_only_indexed_execute_rows_can_request_empty_roots(self):
        with authenticated_contract_fixture(self) as (repo, module, contracts), tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            ns, state = structural_membership_fixture(self, repo, contracts, root)
            rows = state['rosters']['lifecycle']
            forged_got = {'executed': True, 'status': 'pass', 'source_files': [],
                          'ordinary_receipt': {'path': str(root / 'lifecycle/01216/ordinary/receipt.json')}}
            for index, row in enumerate(rows):
                if row['scope'] == 'execute':
                    continue
                with self.subTest(index=index), self.assertRaisesRegex(runtime.Rejected, 'only indexed local'):
                    ns['_case_evidence_roster'](state, 'lifecycle', index, row, forged_got)
                forged = copy.deepcopy(row)
                forged.update(scope='execute', group='heldout', case={'files_hex': {}})
                with self.subTest(forged=index), self.assertRaisesRegex(runtime.Rejected, 'only indexed local'):
                    ns['_case_evidence_roster'](state, 'lifecycle', index, forged, forged_got)
            for index in (0, 1216):
                forged = copy.deepcopy(rows[index])
                with self.assertRaisesRegex(runtime.Rejected, 'only indexed local'):
                    ns['_case_evidence_roster'](state, 'lifecycle', index, forged, forged_got)
            self.assertEqual(list(root.iterdir()), [])

    def test_nonempty_authority_ignores_receipt_source_claim_and_keeps_readback(self):
        with authenticated_contract_fixture(self) as (repo, module, contracts), tempfile.TemporaryDirectory() as temp:
            ns, state = structural_membership_fixture(self, repo, contracts, Path(temp))
            index = 1232  # oxbc-before-lex has an authoritative source file.
            got = emit_structural_case(self, ns, state, 'lifecycle', index)
            self.assertEqual(got['source_files'], [])  # Intentionally irrelevant structural metadata.
            required = ns['_case_evidence_roster'](state, 'lifecycle', index, state['rosters']['lifecycle'][index], got)
            self.assertEqual(required[3], set())
            self.assertTrue(ns['_validate_case_evidence'](state, 'lifecycle', index, got))
            fixture = Path(temp) / 'lifecycle/01232/ordinary/fixture'
            source = next(fixture.iterdir())
            original = source.read_bytes()
            source.write_bytes(b'changed')
            with self.assertRaisesRegex(runtime.Rejected, 'source changed'):
                ns['_validate_case_evidence'](state, 'lifecycle', index, got)
            source.unlink()
            with self.assertRaises(runtime.Rejected):
                ns['_validate_case_evidence'](state, 'lifecycle', index, got)
            source.write_bytes(original)
            extra = fixture / 'invented-empty'
            extra.mkdir()
            with self.assertRaisesRegex(runtime.Rejected, 'directory'):
                ns['_validate_case_evidence'](state, 'lifecycle', index, got)

    def test_existing_role_layouts_and_parent_closure(self):
        with authenticated_contract_fixture(self) as (repo, module, contracts), tempfile.TemporaryDirectory() as temp:
            ns, state = structural_membership_fixture(self, repo, contracts, Path(temp))
            selections = [('public', 0, {'optional_logs': True, 'emitted': True, 'native': True}),
                          ('original', 0, {'events': True}), ('lifecycle', 0, {'executed': False})]
            for group, kind in (('occupied', 'regular'), ('occupied', 'symlink'), ('race', None)):
                index = next(index for index, row in enumerate(state['rosters']['guards'])
                             if row['scope'] == 'execute' and row['group'] == group and
                             (kind is None or row['case']['output_kind'] == kind))
                selections.append(('guards', index, {'events': True}))
            for section, index, options in selections:
                with self.subTest(section=section, index=index):
                    got = emit_structural_case(self, ns, state, section, index, **options)
                    self.assertTrue(ns['_validate_case_evidence'](state, section, index, got))
                    self.assertEqual(ns['_case_evidence_roster'](state, section, index,
                                     state['rosters'][section][index], got)[3], set())
            # Pure validator API checks parent closure; this is not a new authenticated guard case.
            closure = Path(temp) / 'parent-closure-only'
            (closure / 'fixtures/source').mkdir(parents=True)
            self.assertTrue(ns['_validate_evidence_members'](closure, set(), set(), set(), None,
                                                            required_dirs={'fixtures/source'}))
            (closure / 'fixtures/source/extra').mkdir()
            with self.assertRaises(runtime.Rejected):
                ns['_validate_evidence_members'](closure, set(), set(), set(), None,
                                                 required_dirs={'fixtures/source'})

    def test_retained_predecessor_one_role_layouts(self):
        import base64
        import gzip
        import io
        import tarfile
        with authenticated_contract_fixture(self) as (repo, module, contracts), tempfile.TemporaryDirectory() as temp:
            path = repo / runtime.PUBLIC / 'predecessors.py'
            self.assertEqual(runtime.sha(path.read_bytes()),
                             '8d278c421c3a132d48c09e93a0a79c98092f6caacb71ae3d30e69274a694700a')
            source = ast.parse(path.read_bytes())
            selected = [node for node in source.body
                        if isinstance(node, ast.ClassDef) and node.name == 'Predecessors']
            self.assertEqual(len(selected), 1)
            old = {'Path': Path, 'base64': base64, 'gzip': gzip, 'io': io, 'tarfile': tarfile,
                   'json': json, 'need': runtime.need, 'sha': runtime.sha, 'load': module.load}
            functions(path, ('authority_path',), old)
            exec(compile(ast.Module(body=selected, type_ignores=[]), str(path), 'exec'), old)
            ns, state = structural_membership_fixture(self, repo, contracts, Path(temp))
            state['predecessors'] = old['Predecessors'](contracts)
            for group in ('Unit2', 'Unit3'):
                index = next(index for index, row in enumerate(state['rosters']['predecessors'])
                             if row['scope'] == 'execute' and row['group'] == group)
                with self.subTest(group=group):
                    got = emit_structural_case(self, ns, state, 'predecessors', index)
                    self.assertTrue(ns['_validate_case_evidence'](state, 'predecessors', index, got))
                    self.assertEqual(ns['_case_evidence_roster'](state, 'predecessors', index,
                                     state['rosters']['predecessors'][index], got)[3], set())

    def test_original_v9_rejects_exact_legitimate_empty_fixture(self):
        with authenticated_contract_fixture(self) as (repo, module, contracts), tempfile.TemporaryDirectory() as temp:
            ns, state = structural_membership_fixture(self, repo, contracts, Path(temp))
            got = emit_structural_case(self, ns, state, 'lifecycle', 1216)
            old_path = repo / 'tests/qualification/package_dependency_unit4_public_v1/runtime.py'
            self.assertEqual(runtime.sha(old_path.read_bytes()),
                             '2cb2547e77308ccf4d809cf0204d4434edabea2e2de4ca843d772a59fea5f0b6')
            old = dict(ns)
            functions(old_path, ('_case_evidence_roster', '_validate_evidence_members', '_validate_case_evidence'), old)
            with self.assertRaisesRegex(runtime.Rejected, 'unlisted or missing runtime evidence directory'):
                old['_validate_case_evidence'](state, 'lifecycle', 1216, got)
            self.assertTrue(ns['_validate_case_evidence'](state, 'lifecycle', 1216, got))



def diagnostic_runtime_facade(test):
    """Real consumer capability APIs, inert deterministic owner dependencies."""
    import threading
    import os
    temporary = tempfile.TemporaryDirectory()
    test.addCleanup(temporary.cleanup)
    root = Path(temporary.name)
    q = types.ModuleType('isolated_diagnostic_consumer')
    q.__file__ = str(ROOT / 'runtime.py')
    exec(compile((ROOT / 'runtime.py').read_bytes(), q.__file__, 'exec'), q.__dict__)
    events, terminals = [], {}
    q._tick = lambda invocation: None
    q._checkpoint = lambda state, full=False: events.append(('checkpoint', full))
    q._read_boundary = lambda state: contextlib.nullcontext()
    q._identity = lambda path, invocation=None: {'path': str(path), 'bytes': len(Path(path).read_bytes()),
                                               'sha256': q.sha(Path(path).read_bytes())}
    q._read = lambda path, invocation=None: Path(path).read_bytes()
    def write(path, value, invocation=None):
        Path(path).write_bytes(q.canonical(value))
        return q._identity(path)
    q._write = write
    @contextlib.contextmanager
    def boundary(inv, permit, phase, section=None):
        events.append((phase, 'enter'))
        yield
        events.append((phase, 'exit'))
    policy = object()
    def issue(inv, permit):
        events.append(('issue',))
        return policy
    def terminal(inv, permit, selected, document):
        test.assertIs(selected, policy)
        test.assertEqual(len(q._DIAGNOSTIC_RESULTS), 0)
        events.append(('terminal',))
        token = object()
        path = root / 'diagnostic-collection.json'
        write(path, document)
        terminals[token] = q._identity(path)
        return token
    def verify_terminal(inv, permit, selected, token):
        test.assertIs(selected, policy)
        q.need(token in terminals and q._identity(terminals[token]['path']) == terminals[token], 'terminal drift')
        events.append(('verify-terminal',))
        return dict(terminals[token])
    prep = types.SimpleNamespace(issue_public_diagnostic=issue,
        public_diagnostic_start=lambda *args: events.append(('diagnostic-start',)),
        public_output_start=lambda *args: events.append(('full-start',)),
        public_output_boundary=boundary, public_diagnostic_terminal=terminal,
        verify_public_diagnostic_terminal=verify_terminal,
        cancel_invocation=lambda *args: events.append(('poison',)),
        public_output_failure=lambda *args: events.append(('failure-audit',)) or {'verified': True})
    q._preparation = lambda: prep
    invocation, permit = object(), object()
    state = {'root': root, 'invocation': invocation, 'runtime_permit': permit,
        'cfgs': {}, 'contracts': types.SimpleNamespace(verified=[], effective_identity=None),
        'execution_context': {'invocation': {'nonce': 'a' * 32}},
        'predecessors': types.SimpleNamespace(qualified_paths_amendment={}),
        'runtime_tools': types.SimpleNamespace(environment=lambda: {}, tools=lambda: {}, document=lambda: {}),
        'probes': {}, '_read_owner_seed': prep}
    admission = object.__new__(q.RuntimeAdmission)
    q._ADMISSIONS[admission] = state
    q._generator_check = lambda state: events.append(('generator',)) or {'synthetic': 'generator'}
    def section(state, section, out, *args):
        events.append(('section', section))
        return [], {'status': 'COMPLETE_LOCAL_OBSERVATIONS'}
    q._collect_section = section
    q._verify_diagnostic_payload = lambda *args: events.append(('payload',)) or True
    q._validate_evidence_roster = lambda *args: True
    q._tree_snapshot = lambda *args: []
    return q, state, admission, prep, events


class DiagnosticRuntimeControls(unittest.TestCase):
    def test_separate_constructors_foreign_handles_and_full_promotion_reject(self):
        for cls in (runtime.DiagnosticSelection, runtime.DiagnosticResult):
            with self.assertRaises(TypeError): cls()
        for value in (object(), object.__new__(runtime.DiagnosticSelection), object.__new__(runtime.DiagnosticResult)):
            with self.assertRaises(runtime.Rejected): runtime.collect_downstream_diagnostic(value)
            with self.assertRaises(runtime.Rejected): runtime.verify_downstream_diagnostic(value)
            with self.assertRaises(runtime.Rejected): runtime.verify_collection(value)
        q, state, admission, prep, events = diagnostic_runtime_facade(self)
        selection = q.issue_downstream_diagnostic(admission)
        with self.assertRaises(runtime.Rejected): runtime.collect_downstream_diagnostic(selection)
        with self.assertRaises(q.Rejected): q.collect(admission)
        with self.assertRaises(q.Rejected): q.issue_downstream_diagnostic(admission)
        state['collection_started'] = True
        with self.assertRaises(q.Rejected): q.collect_downstream_diagnostic(selection)

    def test_full_start_cannot_convert_to_diagnostic(self):
        q, state, admission, prep, events = diagnostic_runtime_facade(self)
        state['collection_started'] = True
        with self.assertRaises(q.Rejected): q.issue_downstream_diagnostic(admission)
        self.assertNotIn(('issue',), events)

    def test_full_collector_default_order_and_failure_stays_full(self):
        for fail in (False, True):
            with self.subTest(fail=fail):
                q, state, admission, prep, events = diagnostic_runtime_facade(self)
                original_section = q._collect_section
                primary = ValueError('original synthetic full-section failure')
                def section(state, name, *args):
                    result = original_section(state, name, *args)
                    if fail and name == 'original': raise primary
                    return result
                q._collect_section = section
                q.verify_collection = lambda result: self.assertIs(type(result), q.CollectionResult)
                if fail:
                    with self.assertRaises(ValueError) as raised: q.collect(admission)
                    self.assertIs(raised.exception, primary)
                    self.assertTrue((state['root'] / 'collection-failed.json').is_file())
                    self.assertNotIn(('collection-close', 'enter'), events)
                else:
                    result = q.collect(admission)
                    self.assertIs(type(result), q.CollectionResult)
                    self.assertEqual([e[1] for e in events if e[0] == 'section'], list(q.SECTIONS))
                    self.assertLess(events.index(('generator',)), events.index(('full-start',)))
                    self.assertGreater(events.index(('collection-close', 'enter')), events.index(('section', 'guards')))
                    self.assertEqual(len(q._DIAGNOSTIC_RESULTS), 0)
                self.assertNotIn(('diagnostic-start',), events)
                self.assertNotIn(('terminal',), events)

    def test_selected_collection_issues_only_after_exact_route_and_terminal(self):
        q, state, admission, prep, events = diagnostic_runtime_facade(self)
        selection = q.issue_downstream_diagnostic(admission)
        result = q.collect_downstream_diagnostic(selection)
        self.assertIs(type(result), q.DiagnosticResult)
        self.assertEqual([e[1] for e in events if e[0] == 'section'], ['lifecycle', 'guards'])
        self.assertLess(events.index(('generator',)), events.index(('diagnostic-start',)))
        self.assertLess(events.index(('section', 'guards')), events.index(('payload',)))
        self.assertLess(events.index(('payload',)), events.index(('terminal',)))
        self.assertEqual(len(q._COLLECTIONS), 0)
        doc = result.document()
        self.assertTrue(doc['selected_sections_complete'])
        for field in ('collection_complete', 'local_complete', 'execution_qualified', 'global_host_qualification'):
            self.assertIs(doc[field], False)
        with self.assertRaises(q.Rejected): q.verify_collection(result)
        with self.assertRaises(q.Rejected): q.collect_downstream_diagnostic(selection)
        (state['root'] / 'parser').mkdir()
        (state['root'] / 'parser/observations').write_bytes(b'inert parser evidence')
        self.assertTrue(q.verify_downstream_diagnostic(result))
        self.assertNotIn(('full-start',), events)

    def test_incomplete_capability_never_claims_selected_complete(self):
        q, state, admission, prep, events = diagnostic_runtime_facade(self)
        q._collect_section = lambda state, section, *args: ([], {'status': 'INCOMPLETE_LOCAL_CAPABILITY'})
        result = q.collect_downstream_diagnostic(q.issue_downstream_diagnostic(admission))
        self.assertFalse(result.document()['selected_sections_complete'])
        self.assertEqual(result.document()['status'], 'INCOMPLETE_LOCAL_CAPABILITY')
        self.assertFalse(result.document()['collection_complete'])

    def test_original_error_survives_poison_and_preservation_failure(self):
        q, state, admission, prep, events = diagnostic_runtime_facade(self)
        primary = RuntimeError('original diagnostic error')
        def fail(*args): raise primary
        q._collect_section = fail
        prep.cancel_invocation = lambda *args: (_ for _ in ()).throw(ValueError('secondary poison error'))
        prep.public_output_failure = lambda *args: (_ for _ in ()).throw(ValueError('secondary preservation error'))
        with self.assertRaises(RuntimeError) as raised:
            q.collect_downstream_diagnostic(q.issue_downstream_diagnostic(admission))
        self.assertIs(raised.exception, primary)
        saved = json.loads((state['root'] / 'diagnostic-failed.json').read_bytes())
        self.assertEqual(saved['error'], str(primary))
        self.assertEqual(saved['public_output_preservation_error'], 'secondary preservation error')
        self.assertTrue(state['invalidated'])
        self.assertEqual(len(q._DIAGNOSTIC_RESULTS), 0)

    def test_result_replay_foreign_and_changed_carrier_reject(self):
        q, state, admission, prep, events = diagnostic_runtime_facade(self)
        result = q.collect_downstream_diagnostic(q.issue_downstream_diagnostic(admission))
        with self.assertRaises(runtime.Rejected): runtime.verify_downstream_diagnostic(result)
        (state['root'] / 'diagnostic-collection.json').write_bytes(b'{}')
        with self.assertRaises(q.Rejected): q.verify_downstream_diagnostic(result)
        self.assertTrue(state['invalidated'])
        self.assertIn(('poison',), events)

    def test_complete_selected_structural_roster_all_340_cases(self):
        with authenticated_contract_fixture(self) as (repo, module, contracts), tempfile.TemporaryDirectory() as temp:
            root = Path(temp)
            ns, state = structural_membership_fixture(self, repo, contracts, root)
            receipts = {section: [] for section in ('lifecycle', 'guards')}
            counts = {}
            for section in receipts:
                counts[section] = 0
                for index, row in enumerate(state['rosters'][section]):
                    if row['scope'] != 'execute':
                        receipts[section].append({'executed': False, 'status': row['scope']})
                        continue
                    got = emit_structural_case(self, ns, state, section, index)
                    self.assertTrue(ns['_validate_case_evidence'](state, section, index, got))
                    receipts[section].append(got)
                    counts[section] += 1
                for name in ('roster.json', 'comparison.json', 'observations.jsonl.gz'):
                    (root / section / name).write_bytes(b'inert section metadata')
            for name in ('input-integrity.json', 'tools.json', 'qualified-paths-amendment.json'):
                (root / name).write_bytes(b'inert header')
            self.assertEqual(counts, {'lifecycle': 318, 'guards': 22})
            self.assertTrue(ns['_validate_evidence_sections'](state, receipts, ('lifecycle', 'guards')))
            with self.assertRaises(runtime.Rejected): ns['_validate_evidence_roster'](state, receipts)
            for section in ('public', 'original', 'predecessors'):
                self.assertFalse((root / section).exists())
            empty = root / 'lifecycle/01220/observer/fixture'
            empty.rmdir()
            with self.assertRaises(runtime.Rejected): ns['_validate_evidence_sections'](state, receipts, ('lifecycle', 'guards'))
            empty.mkdir()
            extra = root / 'lifecycle/01217/ordinary/fixture'
            extra.mkdir(parents=True)
            with self.assertRaises(runtime.Rejected): ns['_validate_evidence_sections'](state, receipts, ('lifecycle', 'guards'))

    def test_shared_section_and_evidence_factoring_inverse_is_exact(self):
        # AST inverse proof against the independently pinned V10 source body.
        new = ast.parse((ROOT / 'runtime.py').read_bytes())
        after = {n.name: n for n in new.body if isinstance(n, ast.FunctionDef)}
        reconstructed = copy.deepcopy(after['_collect_section'].body[:-1])
        close = next(n for n in reconstructed if isinstance(n, ast.With) and any(
            isinstance(i.context_expr, ast.Call) and len(i.context_expr.args) > 2 and
            isinstance(i.context_expr.args[2], ast.Constant) and i.context_expr.args[2].value == 'section-close'
            for i in n.items))
        close.body.append(ast.parse('receipts_by_section[section], reports[section] = receipts, report').body[0])
        self.assertEqual(runtime.sha(ast.dump(ast.Module(body=reconstructed, type_ignores=[])).encode()),
                         '4cdcfba7b5c92f2a7aca197656f1d3b65b1c1a3db89cc24977a3e9e811328393')
        evidence = copy.deepcopy(after['_validate_evidence_sections'])
        evidence.name = '_validate_evidence_roster'
        evidence.args.args.pop()
        for node in ast.walk(evidence):
            if isinstance(node, ast.Name) and node.id == 'sections': node.id = 'SECTIONS'
        self.assertEqual(runtime.sha(ast.dump(evidence).encode()),
                         'ade539df618ea3113b85b5017474b321247a871e41dbf56c5c2eb3533124ba48')



def diagnostic_payload_fixture(test):
    """Actual selected verifier and original ProbeRecord readback; synthetic inputs only."""
    import gzip
    import os
    temp = tempfile.TemporaryDirectory()
    test.addCleanup(temp.cleanup)
    root = Path(temp.name)
    out = root / 'public-runtime/collection'
    out.mkdir(parents=True)
    q = types.ModuleType('isolated_selected_payload')
    q.__file__ = str(ROOT / 'runtime.py')
    exec(compile((ROOT / 'runtime.py').read_bytes(), q.__file__, 'exec'), q.__dict__)
    prep = types.ModuleType('isolated_original_probe_readback')
    path = ROOT.parent / 'package_dependency_unit4_current/preparation.py'
    exec(compile(path.read_bytes(), str(path), 'exec'), prep.__dict__)
    inv, permit = object(), object()
    original_inputs = root / 'input'
    original_inputs.write_bytes(b'inert executable/source input')
    original_binding = prep.identity(original_inputs)
    q._checkpoint = lambda state, full=False: q.need(prep.identity(original_inputs) == original_binding, 'original input changed')
    q._tick = lambda inv: None
    q._read_boundary = lambda state: contextlib.nullcontext()
    q._preparation = lambda: prep
    q._identity = lambda path, invocation=None: prep.identity(path)
    q._read = lambda path, invocation=None: Path(path).read_bytes()
    q._binding = lambda: types.SimpleNamespace(decode=json.loads)
    q.load = lambda path: json.loads(Path(path).read_bytes())
    q.receipt_identity = lambda row: {'key': row['key']}
    q.rt = types.SimpleNamespace(read_events=lambda path: [] if not Path(path).exists() else [json.loads(line) for line in Path(path).read_bytes().splitlines()])
    prep.public_replay = lambda *args: contextlib.nullcontext()
    prep.verify_invocation = lambda value: q.need(value is inv, 'original invocation required')
    prep._public_runtime_recipe = lambda *args: None
    prep._probe_body_identity = lambda row, record, value: prep.identity(row['path'])
    prep._verify_view_rows = lambda *args, **kwargs: None
    prep._public_probe_view_owner = lambda *args: None
    state = {'collection_root': out, 'invocation': inv, 'runtime_permit': permit,
             'root': out.parent, 'execution_context': {'nonce': 'synthetic-only'},
             'runtime_tools': types.SimpleNamespace(document=lambda: {'tools': 'synthetic'}),
             'predecessors': types.SimpleNamespace(qualified_paths_amendment={'amendment': 'synthetic'}),
             'contracts': types.SimpleNamespace(verified={'input': 'synthetic'}, effective_identity=None,
                 tables={s: {'identity': s + '-synthetic'} for s in ('lifecycle', 'guards')}),
             'rosters': {}, 'probes': {}, 'process_cases': {}, 'native_permits': {}}
    receipts, reports, originals, events = {}, {}, {}, []
    q._row_sources = lambda state, section, row: {'main.ox': b'inert source'}
    def write(path, value):
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(q.canonical(value))
    for sindex, section in enumerate(('lifecycle', 'guards')):
        row = {'key': section, 'scope': 'execute', 'profile': 'debug', 'group': 'occupied',
               'case': {'source_case_id': 'frozen', 'output_kind': 'file'}}
        state['rosters'][section] = [row]
        case = out / section / ('000' if section == 'guards' else '00000')
        variants = []
        for vindex, variant in enumerate(('ordinary', 'observer')):
            evidence = case / variant
            fixture = case / 'fixtures/frozen' if section == 'guards' else evidence / 'fixture'
            fixture.mkdir(parents=True, exist_ok=True)
            (fixture / 'main.ox').write_bytes(b'inert source')
            evidence.mkdir(exist_ok=True)
            pid = 100 + 2*sindex + vindex
            runtime_case = {'section': section, 'index': 0, 'variant': variant}
            operation = root / 'operations' / str(pid)
            operation.mkdir(parents=True)
            for stream in ('stdout', 'stderr'):
                (operation / stream).write_bytes(b'inert stdout' if stream == 'stdout' else b'')
            command = {'argv': [str(original_inputs), 'check', 'main.ox'], 'cwd': str(fixture),
                       'environment': {'UNIT4_TOOL_EVIDENCE': str(evidence)}, 'timeout_seconds': 120,
                       'stream_limit_bytes': 8*1024**2, 'operation_kind': 'public-runtime'}
            write(operation / 'command.json', command)
            doc = {**command, 'pid': pid, 'started_ns': pid, 'cleanup_complete': True,
                   'cleanup_error': None, 'failure': None, 'timed_out': False, 'stream_limit_exceeded': False,
                   'runtime_case': runtime_case, 'executable': original_binding, 'process_tree': original_binding,
                   'executable_route': {'path': str(original_inputs), 'target': str(original_inputs)},
                   'stdout': prep.identity(operation / 'stdout'), 'stderr': prep.identity(operation / 'stderr')}
            write(operation / 'result.json', doc)
            record = prep.ProbeRecord(doc, prep._TOKEN)
            prep._PROBES[record] = {'invocation': inv, 'runtime_permit': permit, 'native_permit': None,
                'directory': operation, 'result': prep.identity(operation / 'result.json'),
                'command': prep.identity(operation / 'command.json')}
            state['probes'][pid, pid] = record
            state['process_cases'][pid, pid] = runtime_case
            process = dict(doc)
            for stream in ('stdout', 'stderr'):
                raw = getattr(record, stream)()
                process[stream], process[stream+'_sha256'] = raw.decode(), q.sha(raw)
                (evidence / stream).write_bytes(raw)
            write(evidence / 'process.json', process)
            got = {'key': section, 'executed': True, 'status': 'pass', 'process': process,
                   'fixture_directory': str(fixture), 'source_files': [prep.identity(fixture / 'main.ox')],
                   'tool_invocations': [], 'trace': [], 'host': 'synthetic', 'profile': 'debug',
                   'format': 'text', 'freeze_sha256': 'f'*64, 'candidate_binding_sha256': 'c'*64, 'group': 'occupied'}
            if section == 'guards': got['fixture_root'] = str(case / 'fixtures')
            write(evidence / 'receipt.json', got)
            variants.append(got)
        got = variants[-1]
        got['ordinary_receipt'] = prep.identity(case / 'ordinary/receipt.json')
        write(case / 'observer/receipt.json', got)
        write(case / 'receipt.json', got)
        if section == 'guards':
            (case / 'preserved-target').write_bytes(b'inert target')
            (case / 'occupied-output').write_bytes(b'inert target')
        receipts[section] = [got]
        originals[section] = copy.deepcopy(got)
    def inventory(rows, values):
        rows, values = list(rows), list(values)
        q.need(len(rows) == len(values) and [r['key'] for r in rows] == [r['key'] for r in values], 'exact synthetic roster')
        q.need(all(g['executed'] and g['status'] == 'pass' for g in values), 'required synthetic execution')
        return {'required_local': len(rows), 'unsupported_capability': 0, 'unavailable_host': 0}
    q.check_inventory = inventory
    q.Reject = q.Rejected
    def checker(state, section, row, got):
        events.append(section)
        original = originals[section]
        for name in ('key','executed','status','host','profile','format','freeze_sha256','candidate_binding_sha256','source_files'):
            q.need(got[name] == original[name], 'synthetic field drift ' + name)
        for variant, current in (('observer', got), ('ordinary', q.load(got['ordinary_receipt']['path']))):
            q._verify_process(state, current['process'], {'section': section, 'index': 0, 'variant': variant})
            q._verify_row_files(current)
    q._check_receipt = checker
    generator_path = root / 'generator'
    generator_path.write_bytes(b'inert original generator')
    generator = prep.identity(generator_path)
    q._verify_generator = lambda state, receipt: q.need(prep.identity(generator_path) == receipt, 'generator drift')
    headers = {'input-integrity.json': {'execution_context': state['execution_context'],
               'verified': state['contracts'].verified, 'effective_authority': None},
               'tools.json': state['runtime_tools'].document(),
               'qualified-paths-amendment.json': state['predecessors'].qualified_paths_amendment}
    for name, value in headers.items(): write(out / name, value)
    for section in ('lifecycle', 'guards'):
        rows, values = state['rosters'][section], receipts[section]
        report = q.aggregate(rows, values, checkpoint=lambda: None)
        report['negative_controls'] = q.negative_controls(rows, values,
            lambda row, got: checker(state, section, row, got), checkpoint=lambda: None)
        report.update(contract_identity=state['contracts'].tables[section]['identity'],
            historical_qualification_claim=False, private_raw_event_fuel_claim=False,
            execution_context=state['execution_context'])
        reports[section] = report
        write(out / section / 'comparison.json', report)
        write(out / section / 'roster.json', [q.receipt_identity(row) for row in rows])
        with gzip.open(out / section / 'observations.jsonl.gz', 'wb') as stream:
            stream.write(q.canonical(values[0]) + b'\n')
    snapshot = q._tree_snapshot(out, inv)
    return q, prep, state, receipts, reports, generator, snapshot, root, events


class DiagnosticPayloadControls(unittest.TestCase):
    def test_original_probe_owners_and_payload_verify_after_parser_writes(self):
        q, prep, state, receipts, reports, generator, snapshot, root, events = diagnostic_payload_fixture(self)
        self.assertTrue(q._verify_diagnostic_payload(state, receipts, reports, generator, snapshot))
        self.assertEqual(len(state['probes']), 4)
        self.assertGreater(events.count('lifecycle'), 1)
        self.assertGreater(events.count('guards'), 1)
        (root / 'parser').mkdir()
        (root / 'parser/new-observation').write_bytes(b'inert later parser evidence')
        self.assertTrue(q._verify_diagnostic_payload(state, receipts, reports, generator, snapshot))

    def test_changed_selected_files_headers_generator_and_operation_streams_reject(self):
        for relative in ('public-runtime/collection/input-integrity.json', 'public-runtime/collection/tools.json',
                         'public-runtime/collection/qualified-paths-amendment.json',
                         'public-runtime/collection/lifecycle/roster.json',
                         'public-runtime/collection/guards/comparison.json',
                         'public-runtime/collection/lifecycle/observations.jsonl.gz',
                         'public-runtime/collection/lifecycle/00000/ordinary/fixture/main.ox',
                         'public-runtime/collection/guards/000/observer/stdout',
                         'operations/100/stdout', 'operations/101/stderr',
                         'operations/102/command.json', 'operations/103/result.json', 'generator', 'input'):
            with self.subTest(relative=relative):
                q, prep, state, receipts, reports, generator, snapshot, root, events = diagnostic_payload_fixture(self)
                (root / relative).write_bytes(b'changed')
                with self.assertRaises((q.Rejected, q.InfrastructureFailure, ValueError, OSError)):
                    q._verify_diagnostic_payload(state, receipts, reports, generator, snapshot)

    def test_selected_missing_extra_directory_omission_reordering_and_process_coverage_reject(self):
        for mode in ('extra-dir','missing-dir','omitted-section','reordered-sections','omitted-row','duplicate-row',
                     'false-skip','foreign-process','duplicate-process','extra-process'):
            with self.subTest(mode=mode):
                q, prep, state, receipts, reports, generator, snapshot, root, events = diagnostic_payload_fixture(self)
                out = state['collection_root']
                if mode == 'extra-dir': (out / 'public').mkdir()
                elif mode == 'missing-dir':
                    p = out / 'lifecycle/00000/ordinary/fixture'
                    (p/'main.ox').unlink(); p.rmdir()
                elif mode == 'omitted-section': receipts.pop('guards')
                elif mode == 'reordered-sections': receipts = dict(reversed(list(receipts.items())))
                elif mode == 'omitted-row': receipts['guards'].clear()
                elif mode == 'duplicate-row': receipts['guards'].append(copy.deepcopy(receipts['guards'][0]))
                elif mode == 'false-skip': receipts['guards'][0]['executed'] = False
                elif mode == 'foreign-process': state['process_cases'][100,100]['section'] = 'public'
                elif mode == 'duplicate-process': receipts['guards'][0]['process'] = receipts['lifecycle'][0]['process']
                else: state['probes'][999,999] = next(iter(state['probes'].values()))
                with self.assertRaises((q.Rejected, q.InfrastructureFailure, ValueError, OSError)):
                    q._verify_diagnostic_payload(state, receipts, reports, generator, snapshot)



class _GuardNoLaunch(BaseException):
    pass


class GuardRegistrationActualFunctionControls(unittest.TestCase):
    """Separately released filesystem proof; synthetic owners, never a compiler."""
    FOCUSED_GUARD_FILESYSTEM = True
    REPORTS = []
    INDICES = (16,20,24,28,48,52,56,60,72,76)
    MODES = ('registration-entry','runtime-read','helper-read','sentinel-read',
             'target-read','before-create','after-create','verify-entry',
             'postverify-read','checkpoint','registration-return','registration-transition','all-reads')

    @contextlib.contextmanager
    def harness(self, authenticated, index=16, baseline=False):
        import os,sys,threading
        from unittest.mock import patch
        repo,cm,contracts=authenticated
        with tempfile.TemporaryDirectory(prefix='unit4-guard-source-') as temp,contextlib.ExitStack() as stack:
            parent=Path(temp).resolve();root=parent/'run'
            for name in ('operations','tmp','home','public-runtime/collection/guards'):
                (root/name).mkdir(parents=True,exist_ok=True)
            helper=parent/'contracts.py';helper.write_bytes((repo/runtime.PUBLIC/'contracts.py').read_bytes())
            base=ROOT.parent
            if baseline:
                base=ROOT.parents[4]/'package-unit4-consumers-v11/candidate/tests/qualification'
            source=base/'package_dependency_unit4_current/preparation.py'
            q=types.ModuleType('_oxid_package_dependency_unit4_preparation_v1')
            q.__file__=str(source);raw=source.read_bytes();q.__source_sha256__=runtime.sha(raw)
            exec(compile(raw,str(source),'exec'),q.__dict__)
            h=types.SimpleNamespace(q=q,root=root,index=index,clock=0.0,forced=[],reads=[],launches=0,baseline=baseline)
            q.time=types.SimpleNamespace(monotonic=lambda:h.clock)
            inv=object();h.inv=inv
            run={'root':root,'deadline':1000000000.0,'failed':False,'next':0};q._RUNS[inv]=run;h.run=run
            def association(value):
                q.need(value is inv,'synthetic original invocation')
                return {'run_root':str(root),'nonce':'synthetic-guard-owner'},{}
            context=types.SimpleNamespace(_invocation_association=association)
            stack.enter_context(patch.dict(sys.modules,{q.KEY:q,q.CONTEXT_KEY:context}))
            q._MODULES[q.CONTEXT_KEY]=context
            q._CHECKED.append(q._CheckedScope(inv,context,q._TOKEN))
            pins=[];binaries={}
            for i in range(8):
                path=parent/('inert-body-'+str(i));path.write_bytes(('never executable body '+str(i)).encode());pins.append(q.identity(path))
            for i,key in enumerate(('ordinary-debug','ordinary-release','lifecycle-debug','lifecycle-release')):binaries[key]=pins[i]
            permit=q.RuntimePermit({'kind':'public','root':str(root),'invocation_nonce':'synthetic-guard-owner','binaries':binaries,'body_pins':pins},q._TOKEN)
            q._RUNTIME[permit]={'invocation':inv,'probes':()};h.permit=permit;h.pins=pins;h.helper=helper
            rows=copy.deepcopy(contracts.roster('guards','Linux x86_64'))
            self.assertEqual(tuple(i for i,row in enumerate(rows) if row['scope']=='execute' and row['group']=='occupied' and row['case']['output_kind']=='symlink'),self.INDICES)
            for i,row in enumerate(rows):
                if i!=index:row['scope']='excluded-host'
            h.rows=rows;h.row=rows[index];h.contracts=contracts
            q._PUBLIC_CONTRACTS[permit]={'helper':q.identity(helper),'rosters':{'guards':rows}}
            (root/'public-runtime/collection/guards/roster.json').write_bytes(q.serial(rows))
            q.public_output_start(inv,permit);m=q._OUTPUT_MONITORS[inv];h.m=m
            # Synthetic fixture arms only this exact real tuple. No guard registry
            # is injected; actual case admission, registration, audit and sealing run.
            m['section']='guards';m['case_order']=(index,)
            rpath=base/'package_dependency_unit4_public_v1/runtime.py'
            r=types.ModuleType('guard_actual_consumer');r.__file__=str(rpath)
            exec(compile(rpath.read_bytes(),str(rpath),'exec'),r.__dict__);h.r=r
            retained=repo/runtime.PUBLIC/'runtime.py'
            self.assertEqual(runtime.sha(retained.read_bytes()),'866f839279f65ff4d8e57a49190202785aa2b4139097156a00aa1e952b3b9db8')
            ns={'Path':Path,'os':os,'sha':q.sha,'need':q.need}
            functions(retained,('materialize','file_identity','inventory'),ns)
            rt=types.SimpleNamespace(**{name:ns[name] for name in ('materialize','file_identity','inventory')},__file__=str(retained))
            r.rt=rt;r.receipt_identity=cm.receipt_identity;r.FREEZE_SHA=cm.FREEZE_SHA
            r._preparation=lambda:q;r._skipped=lambda row,cfg:{'executed':False,'key':row['key']}
            cfgs={}
            for role in ('ordinary','lifecycle'):
                selected={profile:binaries[role+'-'+profile] for profile in ('debug','release')}
                cfgs[role]={'role':role,'binaries':selected,'_binary_stats':{profile:rt.file_identity(row['path']) for profile,row in selected.items()},'_sha256':'synthetic','_consumer':{},'_execution_context':{}}
            h.cfgs=cfgs;h.owner={'invocation':inv,'runtime_permit':permit,'helpers':{'runtime':rt}}
            h.base=root/'public-runtime/collection/guards'/('%03d'%index)
            h.link=h.base/'occupied-output';h.target=h.base/'preserved-target'
            def no_launch(owner,argv,cwd,env,**kwargs):
                h.launches+=1
                self.assertIs(owner,h.owner);self.assertFalse(run['failed'])
                self.assertEqual(os.readlink(h.link),'preserved-target')
                self.assertEqual(h.target.read_bytes(),b'Oxid Unit3 no-clobber sentinel\x00\xff\n')
                self.assertNotIn(inv,q._GUARD_PROSPECTS)
                q._verify_link(q._GUARD_LINKS[inv][str(h.link)])
                q.public_output_tick(inv)
                self.assertFalse(run['failed']);self.assertTrue(m['last_attempt']['success'])
                raise _GuardNoLaunch()
            r._owned_process=no_launch
            try:yield h
            finally:
                self.REPORTS.append({'test':self._testMethodName,'index':index,'baseline':baseline,
                    'forced_boundaries':list(h.forced),'body_identity_paths':list(h.reads),
                    'no_launch_boundaries':h.launches,'invocation_failed':run['failed'],
                    'prospect_revoked':inv not in getattr(q,'_GUARD_PROSPECTS',{}),
                    'target_evidence_retained':h.target.is_file(),'link_evidence_present':os.path.lexists(h.link),
                    'audit_count':q._out_stats(m)['attempts'] if 'attempts' in q._out_stats(m) else len(m['audits']),
                    'first_failed_attempt':m.get('first_failed_attempt'),
                    'last_attempt':m.get('last_attempt')})
                q._CHECKED.clear()

    @contextlib.contextmanager
    def timing(self,h,mode,mutation=None):
        import os,sys
        from unittest.mock import patch
        q=h.q;original_symlink=os.symlink;old_trace=sys.gettrace()
        def force(label):
            h.forced.append(label);h.clock=h.m['last_audit']+q._OUT_DUE
            q.tick()
            self.assertTrue(h.m['last_attempt']['success'])
        def trace(frame,event,arg):
            if frame.f_globals is not q.__dict__ or event not in ('call','return','line'):
                return trace
            if h.m['auditing'] or h.m['sampling_live']:
                return trace
            if h.run['failed']:
                if event=='line' and frame.f_code.co_name=='register_public_guard':
                    with self.assertRaises(q.Rejected):q.public_output_tick(h.inv)
                    h.forced.append('poisoned-unwind-rejected-line-'+str(frame.f_lineno))
                return trace
            names=[];caller=frame
            while caller is not None:
                names.append(caller.f_code.co_name);caller=caller.f_back
            fn=frame.f_code.co_name
            if event=='line':
                if (fn=='register_public_guard' and mode in ('registration-transition','all-reads') and
                        (h.inv in getattr(q,'_GUARD_PROSPECTS',{}) or str(h.link) in q._GUARD_LINKS.get(h.inv,{}))):
                    if (mutation=='overlap-row' and h.inv in q._GUARD_PROSPECTS and str(h.link) in q._GUARD_LINKS.get(h.inv,{})):
                        q._GUARD_LINKS[h.inv][str(h.link)]=dict(q._GUARD_LINKS[h.inv][str(h.link)],link_text='different')
                    force('registration-transition-line-'+str(frame.f_lineno))
                return trace
            if ('register_public_guard' not in names and 'public_case_scope' in names and
                    fn in ('public_output_tick','_out_snapshot') and (h.base/'receipt.json').is_file()):
                if mode=='all-reads':force('scope-finalization-'+fn+'-'+event)
                return trace
            if 'register_public_guard' not in names:return trace
            if fn=='identity' and event=='call':h.reads.append(str(frame.f_locals['path']))
            label=None
            if fn=='register_public_guard':label='registration-entry' if event=='call' else 'registration-return'
            elif fn in ('body','sha'):
                if '_runtime_permit_full' in names:label='runtime-read'
                elif '_public_case_verified' in names:label='helper-read'
                elif '_verify_link' in names:label='postverify-read'
                elif 'identity' in names:label='target-read'
                else:label='sentinel-read'
            elif fn=='_verify_link' and event=='call':label='verify-entry'
            elif fn=='public_output_tick' and event=='call':label='checkpoint'
            if label is not None and (mode==label or mode=='all-reads'):
                force(label+'-'+event)
            return trace
        def create(text,path,*args,**kwargs):
            self.assertEqual((text,str(path)),('preserved-target',str(h.link)))
            if mode in ('before-create','all-reads'):force('before-create')
            if mutation=='creation-failure':raise OSError('injected exclusive creation failure')
            result=original_symlink(text,path,*args,**kwargs)
            if mode in ('after-create','all-reads') or mutation in ('observed-disappearance','observed-replacement'):
                force('after-create')
            if mutation=='observed-disappearance':h.link.unlink();q.public_output_tick(h.inv)
            if mutation=='observed-replacement':
                h.link.unlink();original_symlink('preserved-target',h.link);q.public_output_tick(h.inv)
            if mutation=='alternate-link':
                h.link.unlink();original_symlink('./preserved-target',h.link)
            if mutation=='postverify-target':h.target.write_bytes(b'changed after exclusive creation')
            if mutation=='postverify-missing':h.link.unlink()
            return result
        with patch.object(os,'symlink',side_effect=create):
            sys.settrace(trace)
            try:yield
            finally:sys.settrace(old_trace)

    def collect_to_boundary(self,h):
        return h.r._run_guards(h.rows,h.root/'public-runtime/collection/guards',h.cfgs['ordinary'],h.cfgs['lifecycle'],h.contracts,{},None,h.owner)

    @contextlib.contextmanager
    def case(self,h):
        q=h.q
        with q.public_case_scope(h.inv,h.permit,'guards',h.index):
            source=h.contracts.cases[h.row['case']['source_case_id']]
            h.r.rt.materialize(h.contracts.source_bytes(source),h.base/'fixtures'/source['id'])
            h.target.write_bytes(b'Oxid Unit3 no-clobber sentinel\x00\xff\n')
            yield {'section':'guards','index':h.index,'variant':'ordinary'}

    def test_all_ten_contract_tuples_all_read_and_creation_boundaries(self):
        with authenticated_contract_fixture(self) as authenticated:
            for index in self.INDICES:
                for mode in self.MODES:
                    with self.subTest(index=index,mode=mode),self.harness(authenticated,index) as h:
                        with self.timing(h,mode),self.assertRaises(_GuardNoLaunch):self.collect_to_boundary(h)
                        self.assertEqual(h.launches,1);self.assertTrue(h.forced)
                        for pin in h.pins:self.assertIn(pin['path'],h.reads)
                        self.assertNotIn(h.inv,h.q._GUARD_PROSPECTS)
                        self.assertTrue(h.link.is_symlink());self.assertTrue(h.target.is_file())
                        # The deliberate no-launch BaseException poisons the case
                        # only after the healthy boundary assertions above ran.
                        self.assertTrue(h.run['failed'])

    def test_original_order_reproduces_unregistered_link_rejection(self):
        with authenticated_contract_fixture(self) as authenticated,self.harness(authenticated,20,baseline=True) as h:
            with self.timing(h,'runtime-read'),self.assertRaisesRegex(h.q.Rejected,'unregistered public output link'):
                self.collect_to_boundary(h)
            self.assertEqual(h.launches,0);self.assertTrue(h.run['failed'])
            self.assertEqual(h.m['first_failed_attempt']['phase'],'snapshot')
            self.assertTrue(h.link.is_symlink());self.assertTrue(h.target.is_file())

    def test_owner_case_input_and_preexisting_rejections(self):
        import os
        from unittest.mock import patch
        controls=('foreign-invocation','foreign-permit','foreign-case','observer-case','wrong-section',
                  'inactive-scope','wrong-thread','wrong-process','preexisting-file','preexisting-link',
                  'changed-pin','changed-helper','changed-sentinel','changed-target','unrelated-link')
        with authenticated_contract_fixture(self) as authenticated:
            for control in controls:
                with self.subTest(control=control),self.harness(authenticated) as h:
                    q=h.q;inv,permit=h.inv,h.permit
                    with self.assertRaises((q.Rejected,OSError)),self.case(h) as case,contextlib.ExitStack() as stack:
                        if control=='foreign-invocation':inv=object()
                        elif control=='foreign-permit':permit=object()
                        elif control=='foreign-case':case=dict(case,index=20)
                        elif control=='observer-case':case=dict(case,variant='observer')
                        elif control=='wrong-section':case=dict(case,section='lifecycle')
                        elif control=='inactive-scope':h.m['case']=None
                        elif control=='wrong-thread':h.m['thread']=-1
                        elif control=='wrong-process':
                            original=q._GuardProspect.__init__
                            def altered(prospect,*args,**kwargs):
                                original(prospect,*args,**kwargs);object.__setattr__(prospect,'pid',-1)
                            stack.enter_context(patch.object(q._GuardProspect,'__init__',altered))
                        elif control=='preexisting-file':h.link.write_bytes(b'preserved preexisting output')
                        elif control=='preexisting-link':h.link.symlink_to('preserved-target')
                        elif control=='changed-pin':Path(h.pins[-1]['path']).write_bytes(b'changed pin')
                        elif control=='changed-helper':h.helper.write_bytes(b'changed helper')
                        elif control=='changed-sentinel':h.target.write_bytes(b'changed sentinel')
                        elif control=='changed-target':
                            old=q.identity
                            def changed(path):
                                result=old(path)
                                if Path(path)==h.target:h.target.write_bytes(b'changed during target identity')
                                return result
                            stack.enter_context(patch.object(q,'identity',side_effect=changed))
                        elif control=='unrelated-link':(h.base/'unrelated-link').symlink_to('preserved-target')
                        with self.timing(h,'all-reads'):q.register_public_guard(inv,permit,case)
                    self.assertTrue(h.run['failed']);self.assertEqual(h.launches,0)
                    self.assertNotIn(h.inv,q._GUARD_PROSPECTS);self.assertTrue(h.target.is_file())
                    with self.assertRaises(q.Rejected):q.register_public_guard(h.inv,h.permit,{'section':'guards','index':h.index,'variant':'ordinary'})

    def test_creation_postverification_and_observed_inode_failures(self):
        controls=('creation-failure','alternate-link','postverify-target','postverify-missing',
                  'observed-disappearance','observed-replacement','overlap-row')
        with authenticated_contract_fixture(self) as authenticated:
            for control in controls:
                with self.subTest(control=control),self.harness(authenticated) as h:
                    with self.assertRaises((h.q.Rejected,OSError)),self.case(h) as case,self.timing(h,'all-reads',control):
                        h.q.register_public_guard(h.inv,h.permit,case)
                    self.assertTrue(h.run['failed']);self.assertEqual(h.launches,0)
                    self.assertNotIn(h.inv,h.q._GUARD_PROSPECTS);self.assertTrue(h.target.is_file())
                    with self.assertRaises(h.q.Rejected):h.q.register_public_guard(h.inv,h.permit,{'section':'guards','index':h.index,'variant':'ordinary'})

    def test_actual_scope_finalization_and_no_reuse(self):
        with authenticated_contract_fixture(self) as authenticated,self.harness(authenticated) as h:
            with self.timing(h,'all-reads'),self.case(h) as case:
                row=h.q.register_public_guard(h.inv,h.permit,case)
                (h.base/'receipt.json').write_bytes(b'{}')
            self.assertFalse(h.run['failed']);self.assertIn(str(h.link),h.m['sealed'])
            self.assertTrue(any(label.startswith('scope-finalization-') for label in h.forced))
            self.assertIn(('guards',h.index),h.m['finished_cases']);self.assertNotIn(h.inv,h.q._GUARD_PROSPECTS)
            h.q._verify_link(row)
            with self.assertRaises(h.q.Rejected):h.q.register_public_guard(h.inv,h.permit,case)
            self.assertTrue(h.run['failed']);self.assertTrue(h.link.is_symlink())

    def test_finalizer_rejects_missing_materialized_link(self):
        with authenticated_contract_fixture(self) as authenticated,self.harness(authenticated) as h:
            with self.assertRaisesRegex(h.q.Rejected,'registered guard link disappeared'),self.timing(h,'all-reads'),self.case(h) as case:
                h.q.register_public_guard(h.inv,h.permit,case)
                (h.base/'receipt.json').write_bytes(b'{}');h.link.unlink()
            self.assertTrue(h.run['failed']);self.assertTrue(h.target.is_file())
            self.assertNotIn(h.inv,h.q._GUARD_PROSPECTS)


if __name__ == '__main__':
    unittest.main()
