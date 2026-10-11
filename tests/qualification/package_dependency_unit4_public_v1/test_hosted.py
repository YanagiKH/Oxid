"""Pure/mocked successor hosted controls. No processes or security setup run."""
import ast
import copy
import errno
import hashlib
import os
from pathlib import Path
import tempfile
import types
import unittest
from unittest import mock

path = Path(__file__).absolute().with_name('hosted.py')
hosted = types.ModuleType('public_hosted_under_test')
hosted.__file__ = str(path)
exec(compile(path.read_bytes(), str(path), 'exec'), hosted.__dict__)


def digest(value=b''):
    return hashlib.sha256(value).hexdigest()


def identity():
    return {'effective_uid': 101, 'effective_gid': 102, 'supplementary_groups': []}


def stream_process():
    return {'pid': 9001, 'status': 0, 'argv': ['worker'], 'cwd': '/fresh/output', 'environment': {},
            'started_ns': 110, 'completed_ns': 1000, 'timed_out': False, 'stream_limit_exceeded': False,
            'cleanup_complete': True, 'timeout_seconds': 600, 'stream_limit_bytes': hosted.STREAM_LIMIT,
            'stdout': '', 'stderr': '', 'stdout_sha256': digest(), 'stderr_sha256': digest()}


def roster():
    result = {}
    for section in hosted.SECTIONS:
        result[section] = [
            {'key': section + '/' + str(i), 'case_id': str(i // 2), 'profile': hosted.PROFILES[i % 2],
             'host': hosted.HOST, 'execution_host': hosted.HOST, 'scope': 'execute',
             'required_capabilities': [hosted.CAPABILITY], 'argv_template': ['{oxid}', 'check', 'main.ox']}
            for i in range(6)]
    return result


class HostedSourceControls(unittest.TestCase):
    def test_import_and_opaque_objects_never_issue_evidence(self):
        for cls in (hosted.TransportAdmission, hosted.WorkerAdmission, hosted.HostedResult):
            with self.assertRaises(TypeError):
                cls()
        for value, checker in (({}, hosted.verify_transport), ({}, hosted.verify_worker), ({}, hosted.verify_hosted), ({}, hosted.result_inputs)):
            with mock.patch.object(hosted, '_runtime', side_effect=AssertionError('must reject before loading')):
                with self.assertRaises(hosted.Rejected):
                    checker(value)
        self.assertEqual(len(hosted._TRANSPORTS), 0)
        self.assertEqual(len(hosted._WORKERS), 0)
        self.assertEqual(len(hosted._RESULTS), 0)

    def test_failed_owned_attempt_is_retained_without_qualifying(self):
        invocation = types.SimpleNamespace(document=lambda: {'started_ns': 100})
        process = stream_process()
        process['status'] = 71
        probe = types.SimpleNamespace(stdout=lambda: b'', stderr=lambda: b'')
        permit = object()
        api = types.SimpleNamespace(register_public_hosted=mock.Mock(return_value=permit),
                                    owned_probe=mock.Mock(return_value=probe),
                                    verify_probe=mock.Mock(return_value=process))
        runtime = types.SimpleNamespace(hosted_context=lambda admission: {'invocation': invocation, 'runtime_permit': 'parent-live'},
                                       _binding=lambda: types.SimpleNamespace(preparation_api=lambda: api))
        owned = {'admission': object(), 'started': False, 'probe': None, 'hosted_permit': None,
                 'binding': {'path': '/fresh/transport.json'}, 'argv': ['worker'], 'environment': {},
                 'ledger': {'output': '/fresh/output', 'identity': identity(),
                            'controller': {'setpriv': {'path': '/usr/bin/setpriv'}}}}
        with mock.patch.object(hosted, 'verify_transport', return_value=owned), \
             mock.patch.object(hosted, '_runtime', return_value=runtime), \
             mock.patch.object(hosted, '_read', side_effect=AssertionError('failed process must not load successful report')):
            with self.assertRaises(hosted.Rejected): hosted.execute_transport(object())
        self.assertIs(owned['probe'], probe)
        self.assertTrue(owned['started'])
        self.assertIs(owned['hosted_permit'], permit)
        self.assertIs(api.owned_probe.call_args.kwargs['runtime_permit'], permit)
        self.assertIs(api.verify_probe.call_args.kwargs['runtime_permit'], permit)
        self.assertEqual(api.owned_probe.call_args.kwargs['operation_kind'], 'public-hosted-worker')
        self.assertEqual(len(hosted._RESULTS), 0)

    def test_retained_hosted_four_file_pin_roster(self):
        self.assertEqual(set(hosted.RETAINED_HOSTED), {'README.md', 'hosted.py', 'integration_controls.py', 'selftest.py'})
        self.assertEqual(sum(row[0] for row in hosted.RETAINED_HOSTED.values()), 67930)
        self.assertTrue(all(len(row[1]) == 64 for row in hosted.RETAINED_HOSTED.values()))

    def test_only_explicit_process_route_no_historical_controller_or_patch(self):
        tree = ast.parse(path.read_bytes())
        calls = [node for node in ast.walk(tree) if isinstance(node, ast.Call)]
        process_calls = [node for node in calls if isinstance(node.func, ast.Attribute) and node.func.attr == 'process']
        direct = [node for node in process_calls if any(keyword.arg == 'new_session' for keyword in node.keywords)]
        self.assertEqual(len(direct), 1)
        self.assertIs(next(keyword.value.value for keyword in direct[0].keywords if keyword.arg == 'new_session'), False)
        owned = [node for node in calls if isinstance(node.func, ast.Attribute) and node.func.attr == 'owned_probe']
        self.assertEqual(len(owned), 1)
        self.assertEqual(next(keyword.value.value for keyword in owned[0].keywords if keyword.arg == 'operation_kind'),
                         'public-hosted-worker')
        for node in calls:
            if isinstance(node.func, ast.Attribute):
                self.assertNotIn(node.func.attr, ('candidate_binding', 'install_worker_group_policy', 'orchestrate'))
        for node in ast.walk(tree):
            if isinstance(node, (ast.Assign, ast.AnnAssign)):
                targets = node.targets if isinstance(node, ast.Assign) else [node.target]
                self.assertFalse(any(isinstance(target, ast.Attribute) and target.attr == 'process' for target in targets))

    def test_original_all_twelve_rows_derive_from_immutable_roster(self):
        rows = roster()
        cases = {str(i): {'filesystem_setup': {'temporary_mode_octal': '000'}} for i in range(3)}
        calls = []
        contracts = types.SimpleNamespace(cases=cases, roster=lambda section, host: calls.append((section, host)) or rows[section])
        self.assertEqual(hosted.selected_rosters(contracts), rows)
        self.assertEqual(calls, [(section, hosted.HOST) for section in hosted.SECTIONS])
        for change in ('missing', 'duplicate', 'wrong-host', 'wrong-profile', 'wrong-capability'):
            mutant = copy.deepcopy(rows)
            if change == 'missing': mutant['public'].pop()
            elif change == 'duplicate': mutant['public'][1] = copy.deepcopy(mutant['public'][0])
            elif change == 'wrong-host': mutant['public'][0]['host'] = 'macOS arm64'
            elif change == 'wrong-profile':
                for row in mutant['public']: row['profile'] = 'debug'
            else: mutant['public'][0]['required_capabilities'] = []
            contracts.roster = lambda section, host: mutant[section]
            with self.subTest(change=change), self.assertRaises(hosted.Rejected): hosted.selected_rosters(contracts)

    def test_exact_replacements_never_skip_failure_or_profile(self):
        keys = ['public/' + str(i) for i in range(6)]
        rows = [{'key': key, 'executed': True, 'status': 'pass'} for key in keys]
        self.assertEqual(list(hosted.replacement_map(keys, rows)), keys)
        mutants = [[], rows[:-1], rows + [rows[0]], [rows[0]] * 6]
        for field, value in (('executed', False), ('status', 'unsupported-capability'), ('status', 'fail'), ('key', 'extra')):
            changed = copy.deepcopy(rows); changed[0][field] = value; mutants.append(changed)
        for changed in mutants:
            with self.assertRaises(hosted.Rejected): hosted.replacement_map(keys, changed)
        with self.assertRaises(hosted.Rejected): hosted.replacement_map([], [])

    def test_denial_requires_actual_nonroot_errno_and_restoration(self):
        proof = {'read_open_succeeded': False, 'effective_uid': 101, 'effective_gid': 102, 'groups': [],
                 'error_errno': errno.EACCES, 'mode_before_candidate': 0, 'mode_after_candidate': 0, 'source_restored': True}
        hosted.require_denial_proof({'capability_proof': proof}, identity())
        for field, value in (('read_open_succeeded', True), ('effective_uid', 0), ('effective_uid', True),
                             ('effective_gid', 103), ('groups', [102]), ('error_errno', errno.ENOENT),
                             ('mode_before_candidate', 0o644), ('mode_after_candidate', 0o644), ('source_restored', False)):
            changed = {**proof, field: value}
            with self.subTest(field=field), self.assertRaises(hosted.Rejected):
                hosted.require_denial_proof({'capability_proof': changed}, identity())
        with self.assertRaises(hosted.Rejected): hosted.require_denial_proof({'capability_proof': proof}, {**identity(), 'effective_uid': 0})

    def test_outer_environment_is_finite_and_cannot_inherit_overrides(self):
        with mock.patch.dict(hosted.os.environ, {'LD_PRELOAD': '/injected', 'PYTHONPATH': '/injected'}, clear=True):
            env = hosted.outer_environment('/fresh/output')
        self.assertEqual(env, {'HOME': '/fresh/output', 'PATH': '/usr/bin:/bin', 'LANG': 'C.UTF-8', 'LC_ALL': 'C.UTF-8',
                               'PYTHONDONTWRITEBYTECODE': '1', 'PYTHONNOUSERSITE': '1'})
        child = hosted._expected_child_environment(env, '/fresh/output')
        self.assertEqual(child, {**env, 'OXID_LLVM_BIN': '/fresh/output/tool-traps'})

    def test_worker_argv_has_finite_privilege_drop_and_exact_context(self):
        args = hosted.worker_argv('/usr/bin/setpriv', '/usr/bin/python3', '/readonly/hosted.py', '/fresh/transport.json',
                                 'a' * 64, '/fresh/observations', 101, 102)
        self.assertEqual(args, ['/usr/bin/setpriv', '--reuid=101', '--regid=102', '--clear-groups', '--no-new-privs',
                                '/usr/bin/python3', '-B', '/readonly/hosted.py', 'worker', '--ledger', '/fresh/transport.json',
                                '--ledger-sha256', 'a' * 64, '--out', '/fresh/observations', '--expected-uid', '101', '--expected-gid', '102'])
        for uid, gid, sha in ((0, 102, 'a' * 64), (True, 102, 'a' * 64), (101, -1, 'a' * 64), (101, 102, 'x' * 64)):
            with self.assertRaises(hosted.Rejected): hosted.worker_argv('/s', '/p', '/h', '/l', sha, '/o', uid, gid)

    def test_owned_outer_process_window_streams_status_cleanup(self):
        process = stream_process()
        self.assertTrue(hosted.inspect_worker_process(process, ['worker'], '/fresh/output', {}, {'started_ns': 100}))
        for field, value in (('argv', ['other']), ('cwd', '/old'), ('environment', {'PYTHONPATH': 'inject'}),
                             ('pid', 0), ('status', False), ('status', 1), ('timed_out', True),
                             ('stream_limit_exceeded', True), ('cleanup_complete', False), ('started_ns', 99),
                             ('completed_ns', 90), ('timeout_seconds', 1200), ('stdout', 'fabricated'), ('stderr_sha256', '0' * 64)):
            with self.subTest(field=field), self.assertRaises(hosted.Rejected):
                hosted.inspect_worker_process({**process, field: value}, ['worker'], '/fresh/output', {}, {'started_ns': 100})

    def test_nested_process_owned_group_and_actual_window(self):
        outer = stream_process()
        observed = {**stream_process(), 'pid': 9002, 'started_ns': 150, 'completed_ns': 200, 'worker_pid': 9001,
                    'process_group': 9001, 'session_id': 9001, 'nested_new_session': False,
                    'pipe_capture_complete': True, 'outer_cleanup_required': True}
        observed.pop('cleanup_complete')
        who = {**identity(), 'process_group_policy': {'identity': 'outer-owned-session-shared-nested-v1',
               'worker_pid': 9001, 'process_group': 9001, 'session_id': 9001, 'nested_new_sessions': False}}
        self.assertTrue(hosted.inspect_nested_process(observed, outer, who))
        for field, value in (('worker_pid', 9002), ('process_group', 1), ('session_id', 1), ('nested_new_session', True),
                             ('pid', 9001), ('started_ns', 109), ('completed_ns', 1001), ('pipe_capture_complete', False), ('outer_cleanup_required', False),
                             ('stdout', 'fabricated')):
            with self.subTest(field=field), self.assertRaises(hosted.Rejected):
                hosted.inspect_nested_process({**observed, field: value}, outer, who)


class ReadonlyTransportControls(unittest.TestCase):
    def make_ledger(self, root):
        destination = hosted.namespace_path(root, '/original/input')
        destination.parent.mkdir(parents=True)
        destination.write_bytes(b'controlled source-only fixture, never executable')
        destination.chmod(0o444)
        for directory in (destination.parent, root / 'readonly'):
            directory.chmod(0o555)
        bound = hosted.file_identity(destination)
        return {'schema': 'package-dependency-unit4-public-hosted-transport-v1', 'status': 'NotReady',
                'execution_qualified': False, 'source_manifest_sha256': hosted.SOURCE_SHA, 'host': hosted.HOST,
                'root': str(root), 'output': str(root / 'observations'), 'entries': [
                    {'logical_path': '/original/input', 'physical_path': str(destination), 'bytes': bound['bytes'],
                     'sha256': bound['sha256'], 'mode': 0o444, 'roles': ['source']}]}

    def writable(self, root):
        for path in root.rglob('*'):
            if path.is_dir(): path.chmod(0o755)
            elif not path.is_symlink(): path.chmod(0o644)

    def test_exact_readonly_files_dirs_bytes_modes_and_current_source(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp).resolve()
            ledger = self.make_ledger(root)
            try:
                self.assertEqual(list(hosted.verify_ledger(ledger)), ['/original/input'])
                for field, value in (('source_manifest_sha256', '9432c61fc4f63b760e5f55599aedb24067a206e40e8b44b0911392c00cda7261'),
                                     ('execution_qualified', True), ('status', 'pass'), ('host', 'Windows x86_64'), ('output', '/elsewhere')):
                    with self.subTest(field=field), self.assertRaises(hosted.Rejected): hosted.verify_ledger({**ledger, field: value})
                for field, value in (('physical_path', '/outside'), ('sha256', '0' * 64), ('bytes', -1), ('mode', 0o644), ('roles', [])):
                    changed = copy.deepcopy(ledger); changed['entries'][0][field] = value
                    with self.subTest(field=field), self.assertRaises(hosted.Rejected): hosted.verify_ledger(changed)
                changed = copy.deepcopy(ledger); changed['entries'].append(changed['entries'][0])
                with self.assertRaises(hosted.Rejected): hosted.verify_ledger(changed)
                self.writable(root)
                extra = root / 'readonly/extra-directory'; extra.mkdir(); extra.chmod(0o555)
                for directory in (root / 'readonly', root / 'readonly/original'): directory.chmod(0o555)
                (root / 'readonly/original/input').chmod(0o444)
                with self.assertRaises(hosted.Rejected): hosted.verify_ledger(ledger)
            finally:
                self.writable(root)

    def test_path_traversal_symlink_and_unlisted_reads_reject(self):
        for logical in ('relative', '/x/../y', '/'):
            with self.assertRaises(hosted.Rejected): hosted.namespace_path('/transport', logical)
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp).resolve()
            ledger = self.make_ledger(root)
            try:
                resolve = hosted._resolver(ledger)
                self.assertEqual(resolve('/original/input'), str(root / 'readonly/original/input'))
                with self.assertRaises(hosted.Rejected): resolve('/unlisted')
                self.writable(root)
                target = root / 'readonly/original/input'
                target.unlink(); target.symlink_to('/etc/hostname')
                with self.assertRaises(hosted.Rejected): hosted.verify_ledger(ledger)
            finally:
                self.writable(root)

    def test_operation_roster_is_exact_three_plus_eighteen(self):
        rows = roster()
        cfgs = {role: {'binaries': {profile: {'path': '/' + role + '/' + profile} for profile in hosted.PROFILES}}
                for role in ('ordinary', 'lifecycle')}
        inputs = {'output': '/fresh/observations', 'environment': {'OXID_LLVM_BIN': '/fresh/observations/tool-traps'},
                  'helpers': {'runtime': types.SimpleNamespace(__file__='/readonly/unit4_public_v3/runtime.py')},
                  'cfgs': cfgs, 'capability_rows': rows}
        operations = hosted._worker_operations(inputs)
        self.assertEqual(len(operations), 21)
        self.assertEqual([row['kind'] for row in operations[:3]], ['trap-control'] * 3)
        self.assertEqual([row['kind'] for row in operations[3:]], ['capability-observation'] * 18)
        self.assertEqual(operations[3]['cwd'], '/fresh/observations/public/0/ordinary/fixture')
        self.assertEqual(operations[-1]['cwd'], '/fresh/observations/lifecycle/5/observer/fixture')
        self.assertNotIn('UNIT4_LIFECYCLE_LOG', operations[3]['environment'])
        self.assertIn('UNIT4_LIFECYCLE_LOG', operations[-1]['environment'])
        self.assertEqual({tuple(row['argv'][1:]) for row in operations[3:]}, {('check', 'main.ox')})


    def test_derived_output_roster_rejects_extra_programs_and_directories(self):
        rows = roster()
        sources = {row['key']: {'main.ox': b'controlled', 'lib/helper.ox': b'fixture'}
                   for values in rows.values() for row in values}
        required, optional, directories = hosted.expected_output_paths(rows, sources)
        inventory = sorted([{'path': name, 'kind': 'file', 'bytes': 0, 'sha256': digest()} for name in required] +
                           [{'path': name, 'kind': 'directory'} for name in directories], key=lambda row: row['path'])
        self.assertTrue(hosted.verify_output_roster(inventory, rows, sources))
        for extra in ({'path': 'unlisted', 'kind': 'directory'},
                      {'path': 'public/0/ordinary/program.ll', 'kind': 'file'},
                      {'path': 'public/0/ordinary/fixture/out.bin', 'kind': 'file'},
                      {'path': 'lifecycle/0/observer/native-source-free/program', 'kind': 'file'}):
            mutated = sorted(inventory + [extra], key=lambda row: row['path'])
            with self.assertRaises(hosted.Rejected): hosted.verify_output_roster(mutated, rows, sources)
        with self.assertRaises(hosted.Rejected): hosted.verify_output_roster(inventory[:-1], rows, sources)
        with self.assertRaises(hosted.Rejected): hosted.verify_output_roster(inventory + [inventory[0]], rows, sources)
        permitted = sorted(inventory + [{'path': sorted(optional)[0], 'kind': 'file'}], key=lambda row: row['path'])
        self.assertTrue(hosted.verify_output_roster(permitted, rows, sources))

    def test_mocked_nested_capture_preserves_session_and_cleanup_scope(self):
        runtime = types.SimpleNamespace(process=mock.Mock(return_value={
            'pid': 222, 'status': 1, 'started_ns': 1, 'completed_ns': 2,
            'timed_out': False, 'stream_limit_exceeded': False, 'stream_limit_bytes': hosted.STREAM_LIMIT,
            'stdout': '', 'stderr': '', 'stdout_sha256': digest(), 'stderr_sha256': digest()}))
        inputs = {'helpers': {'runtime': runtime}}
        allowed = {'argv': ['/readonly/compiler', 'check', 'main.ox'], 'cwd': '/output/fixture',
                   'environment': {'X': 'controlled'}, 'timeout_seconds': 120, 'kind': 'capability-observation'}
        worker = object.__new__(hosted.WorkerAdmission)
        hosted._WORKERS[worker] = {'inputs': inputs, 'deadline': 200, 'operations': [allowed], 'completed': {}}
        with mock.patch.object(hosted, 'verify_worker', return_value=inputs), \
             mock.patch.object(hosted.time, 'monotonic', return_value=100), \
             mock.patch.object(hosted.os, 'getpid', return_value=111), \
             mock.patch.object(hosted.os, 'getpgrp', return_value=111), \
             mock.patch.object(hosted.os, 'getsid', return_value=111):
            result = worker.process(allowed['argv'], allowed['cwd'], allowed['environment'])
            self.assertEqual(result['timeout_seconds'], 100)
            self.assertEqual(result['requested_timeout_seconds'], 120)
            self.assertNotIn('cleanup_complete', result)
            self.assertTrue(result['pipe_capture_complete'])
            self.assertTrue(result['outer_cleanup_required'])
            self.assertFalse(runtime.process.call_args.kwargs['new_session'])
            self.assertEqual(runtime.process.call_args.kwargs['timeout'], 100)
            with self.assertRaises(hosted.Rejected): worker.process(allowed['argv'], allowed['cwd'], allowed['environment'])
            with self.assertRaises(hosted.Rejected): worker.process(['/bin/sh'], allowed['cwd'], allowed['environment'])
        self.assertEqual(runtime.process.call_count, 1)
        del hosted._WORKERS[worker]

    def test_transported_binary_paths_truthfully_keep_logical_build_identity(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp).resolve()
            cfgs, mapping = {}, {}
            for role in ('ordinary', 'lifecycle'):
                binaries = {}
                for profile in hosted.PROFILES:
                    physical = root / (role + '-' + profile)
                    physical.write_bytes((role + profile + ' source-only fixture').encode())
                    logical = '/original/' + role + '/' + profile
                    mapping[logical] = str(physical)
                    binaries[profile] = {**hosted.file_identity(physical), 'path': logical}
                cfgs[role] = {'kind': 'package-dependency-unit4-public-' + role + '-v1', 'role': role,
                              '_sha256': digest(role.encode()), 'binaries': binaries, '_binary_stats': {},
                              'build_binding': {'role': role, 'host': hosted.HOST,
                                                'source_binding': {'source_manifest': {'sha256': hosted.SOURCE_SHA}}}}
            original = copy.deepcopy(cfgs)
            converted = hosted.transported_configs(cfgs, mapping.__getitem__, {'path': '/transport', 'sha256': 'a' * 64})
            self.assertEqual(cfgs, original)
            for role in cfgs:
                self.assertEqual(converted[role]['_sha256'], cfgs[role]['_sha256'])
                self.assertEqual(converted[role]['transport']['logical_binaries'], cfgs[role]['binaries'])
                for profile in hosted.PROFILES:
                    self.assertEqual(converted[role]['binaries'][profile]['path'], mapping[cfgs[role]['binaries'][profile]['path']])
            changed = copy.deepcopy(cfgs)
            changed['ordinary']['build_binding']['source_binding']['source_manifest']['sha256'] = '0' * 64
            with self.assertRaises(hosted.Rejected): hosted.transported_configs(changed, mapping.__getitem__, {})
            changed = copy.deepcopy(cfgs); changed['lifecycle']['kind'] = 'unit4-public-v3-lifecycle-observer'
            with self.assertRaises(hosted.Rejected): hosted.transported_configs(changed, mapping.__getitem__, {})


if __name__ == '__main__':
    unittest.main()
