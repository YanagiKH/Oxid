"""Pure and mocked source-context controls. No preparation or native probes."""
import copy
import json
import io
import os
from pathlib import Path
import sys
import tempfile
import types
import unittest
from unittest.mock import Mock, patch
from contextlib import nullcontext

sys.dont_write_bytecode = True
HERE = Path(__file__).resolve().parent
NAME = '_oxid_package_dependency_unit4_context_v1'
q = types.ModuleType(NAME)
q.__file__ = str(HERE / 'context.py')
exec(compile((HERE / 'context.py').read_bytes(), q.__file__, 'exec'), q.__dict__)
q.__source_sha256__ = q.sha((HERE / 'context.py').read_bytes())


class ContextControls(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.repo = self.root / 'repo'
        self.repo.mkdir()
        self.file = self.root / 'body'
        self.file.write_bytes(b'identity')
        self.capture_value = {'scope': 'public', 'checkout': {'path': str(self.repo),
                              'head': 'a' * 40, 'tree': 'b' * 40, 'event_sha': 'c' * 40}}
        self.snapshot = patch.object(q, '_snapshot', side_effect=lambda *args, **kwargs: copy.deepcopy(self.capture_value))
        self.snapshot.start()
        self.addCleanup(self.snapshot.stop)

    def capture(self):
        return q.capture_source(self.repo, 'a' * 40, 'c' * 40, self.capture_value['scope'])

    def invocation(self, name='run'):
        return q.begin_invocation(self.capture(), self.root / name)

    def test_immutable_capture_and_defensive_document(self):
        captured = self.capture()
        captured.document()['checkout']['head'] = 'wrong'
        self.assertEqual(captured.document(), self.capture_value)
        with self.assertRaises(TypeError):
            captured._raw = b'{}'

    def test_public_constructors_cannot_issue_authority(self):
        for cls in (q.CapturedSource, q.Invocation, q.AdmittedContext):
            for args in ((), ({'status': 'pass'},), (object(),)):
                with self.subTest(cls=cls.__name__, args=args), self.assertRaises(q.Rejected):
                    cls(*args)
            forged = object.__new__(cls)
            with self.assertRaises(q.Rejected):
                forged.document()
        with self.assertRaises(q.Rejected):
            q._invocation(types.SimpleNamespace(document=lambda: {}, captured=lambda: self.capture()))

    def test_association_is_live_registry_only_and_full_check_detects_drift(self):
        invocation = self.invocation()
        original = q._invocation_association(invocation)
        self.capture_value['checkout']['head'] = 'd' * 40
        self.assertEqual(q._invocation_association(invocation), original)
        with self.assertRaisesRegex(q.Rejected, 'drift'):
            q._invocation(invocation)
        original[1]['checkout']['head'] = 'mutated defensive copy'
        self.assertEqual(q._invocation_association(invocation)[1]['checkout']['head'], 'a' * 40)

    def test_association_rejects_forged_invocation_and_capture(self):
        with self.assertRaises(q.Rejected):
            q._invocation_association(object.__new__(q.Invocation))
        invocation = self.invocation()
        entry = q._INVOCATIONS[invocation]
        original = entry['capture']
        try:
            entry['capture'] = object.__new__(q.CapturedSource)
            with self.assertRaises(q.Rejected):q._invocation_association(invocation)
        finally:
            entry['capture'] = original

    def test_invocation_registry_and_defensive_document(self):
        captured = self.capture()
        invocation = q.begin_invocation(captured, self.root / 'future')
        self.assertIs(invocation.captured(), captured)
        invocation.document()['nonce'] = 'wrong'
        self.assertNotEqual(invocation.document()['nonce'], 'wrong')
        with self.assertRaises(TypeError):
            invocation._capture = captured

    def test_context_views_are_scoped_and_defensive(self):
        invocation = self.invocation()
        value = {'views': {'ordinary': {'files': [1]}}, 'preparation': {'root': str(self.root)}}
        with patch.object(q, '_admit', return_value=value):
            context = q.admit_prepared(invocation, self.root)
        context.view('ordinary')['files'].append(2)
        self.assertEqual(context.view('ordinary')['files'], [1])
        self.assertIs(context.invocation(), invocation)
        for role in ('parser_observed', 'old'):
            with self.assertRaises(q.Rejected):
                context.view(role)

    def test_nonfinite_and_duplicate_json_reject(self):
        for raw in (b'{"x":1,"x":2}', b'{"x":NaN}'):
            with self.assertRaises(q.Rejected):
                q.decode(raw)

    def test_closed_rows_and_portable_safe_names(self):
        good = q.row('src/a', b'a')
        self.assertEqual(q.rows([good]), {'src/a': good})
        bad = [[good, good], [dict(good, extra=1)], [dict(good, path='../a')],
               [dict(good, bytes=True)], [dict(good, sha256='0')],
               [good, q.row('src/a/b', b'b')]]
        for records in bad:
            with self.assertRaises(q.Rejected):
                q.rows(records)
        for name in ('C:relative', 'C:/absolute', 'x:y', 'x\\y', 'x//y', './x', '../x', '/x', ''):
            with self.subTest(name=name), self.assertRaises(q.Rejected):
                q.safe(name)

    def test_scan_posix_relative_paths(self):
        root = self.root / 'scan'
        (root / 'one/two').mkdir(parents=True)
        (root / 'one/two/body').write_bytes(b'body')
        files, dirs = q.scan(root)
        self.assertEqual(set(files), {'one/two/body'})
        self.assertEqual(dirs, {'one', 'one/two'})

    @unittest.skipUnless(hasattr(os, 'mkfifo'), 'FIFO is POSIX-specific')
    def test_special_member_rejects_before_read(self):
        os.mkfifo(self.root / 'pipe')
        with self.assertRaises(q.Rejected):
            q.scan(self.root)

    def test_symlink_and_ancestry_reject(self):
        try:
            (self.root / 'alias').symlink_to(self.root, target_is_directory=True)
        except (OSError, NotImplementedError):
            self.skipTest('symlink creation not available')
        with self.assertRaises(q.Rejected):
            q.body(self.root / 'alias/body')
        with self.assertRaises(q.Rejected):
            q.scan(self.root)

    def test_oversize_rejects_before_read(self):
        with self.assertRaises(q.Rejected):
            q.body(self.file, 2)

    def test_growing_file_rejects_before_buffer_accumulates(self):
        with patch.object(Path, 'open', return_value=io.BytesIO(b'x' * (1024 ** 2))):
            with self.assertRaisesRegex(q.Rejected, 'input grew beyond bounded size'):
                q.body(self.file)

    def test_capture_wrong_install_before_git(self):
        self.snapshot.stop()
        with patch.object(q.subprocess, 'run', side_effect=AssertionError('must not spawn')):
            with self.assertRaises(q.Rejected):
                q.capture_source(self.root, 'a' * 40, 'b' * 40)

    def test_begin_fresh_external_nonce_without_write(self):
        captured = self.capture()
        one = q.begin_invocation(captured, self.root / 'future')
        two = q.begin_invocation(captured, self.root / 'other')
        self.assertNotEqual(one.document()['nonce'], two.document()['nonce'])
        self.assertFalse((self.root / 'future').exists())
        for root in (self.repo / 'inside', self.repo, self.root):
            with self.assertRaises(q.Rejected):
                q.begin_invocation(captured, root)

    def test_invocation_requires_exact_registry_object(self):
        for value in ({'nonce': 'a' * 32}, object(), object.__new__(q.Invocation)):
            with self.assertRaises(q.Rejected):
                q._invocation(value)

    def test_capture_drift_rehashes_source(self):
        captured = self.capture()
        self.capture_value['checkout']['head'] = 'd' * 40
        with self.assertRaises(q.Rejected):
            q._capture_check(captured)

    def test_serialized_context_requires_registry_invocation_and_readmission(self):
        expected = {'preparation': {'root': str(self.root)}, 'status': 'NotReady'}
        self.file.write_bytes(q.serial(expected))
        invocation = self.invocation()
        with patch.object(q, '_admit', return_value=expected) as admit:
            context = q.load_context(self.file, invocation)
            self.assertEqual(context.document(), expected)
            admit.assert_called_once_with(invocation, str(self.root))
            with self.assertRaises(q.Rejected):
                q.load_context(self.file, object())
        with patch.object(q, '_admit', return_value=dict(expected, status='pass')):
            with self.assertRaises(q.Rejected):
                q.load_context(self.file, invocation)

    def test_verify_context_rereads_actual_inputs(self):
        invocation = self.invocation()
        value = {'preparation': {'root': str(self.root)}}
        with patch.object(q, '_admit', return_value=value):
            context = q.admit_prepared(invocation, self.root)
            self.assertTrue(q.verify_context(context))
        with patch.object(q, '_admit', return_value={}):
            with self.assertRaises(q.Rejected):
                q.verify_context(context)
        with self.assertRaises(q.Rejected):
            q.verify_context(types.SimpleNamespace(document=lambda: value, invocation=lambda: invocation))

    def test_unassociated_report_cannot_establish_provenance(self):
        invocation = self.invocation()
        root = self.root / 'run/source-preparation'
        root.mkdir(parents=True)
        (root / 'report.json').write_text('{"genuine_public_prepared_ns":1}')
        with patch.object(q, '_worker_api', side_effect=AssertionError('record must be checked first')):
            with self.assertRaisesRegex(q.Rejected, 'trusted preparation record missing'):
                q._admit(invocation, root)

    def test_preparation_wrong_root_rejects_before_worker(self):
        invocation = self.invocation()
        with patch.object(q, '_worker_api', side_effect=AssertionError('not reached')):
            with self.assertRaises(q.Rejected):
                q._admit(invocation, self.root)

    def test_producer_record_is_once_only_and_reverified(self):
        invocation, record, issuer = self.invocation(), object(), object()
        verified = {'config': {}, 'plan': {}, 'provenance': {'fresh': 'trusted mock'}}
        producer = types.SimpleNamespace(verify_record=Mock(return_value=verified))
        with patch.object(q, '_producer', return_value=producer):
            q._record_preparation(invocation, record, issuer)
            self.assertEqual(q._preparation(invocation), verified)
            self.assertEqual(producer.verify_record.call_count, 2)
            producer.verify_record.assert_called_with(record, invocation)
            with self.assertRaises(q.Rejected):
                q._record_preparation(invocation, record, issuer)
            producer.verify_record.return_value = dict(verified, extra=True)
            with self.assertRaises(q.Rejected):
                q._preparation(invocation)
            producer.verify_record.side_effect = q.Rejected('captured input changed')
            with self.assertRaisesRegex(q.Rejected, 'captured input changed'):
                q._preparation(invocation)

    def test_unknown_producer_token_rejects(self):
        with self.assertRaises(q.Rejected):
            q._record_preparation(self.invocation(), object(), object())

    def shared_fixture(self):
        root = self.repo / q.SELF
        root.mkdir(parents=True)
        for name in q.SHARED_FILES:
            (root / name).write_bytes(('fixture ' + name + '\n').encode())
        records = [q.row(name, (root / name).read_bytes()) for name in q.SHARED_FILES]
        (root / 'package-manifest.json').write_bytes(q.serial({
            'schema': 'oxid-package-dependency-unit4-current-package-v1', 'files': records}))
        return root, {r['path']: r for r in records}

    def test_git_identity_uses_canonical_tool_and_rejects_drift(self):
        executable = self.root / 'git-tool'
        executable.write_bytes(b'mocked executable body')
        result = types.SimpleNamespace(returncode=0, stdout=b'identity\n', stderr=b'')
        with patch.object(q.shutil, 'which', return_value=str(executable)), \
             patch.object(q.subprocess, 'run', return_value=result) as run:
            self.assertEqual(q._git(self.repo, 'rev-parse', 'HEAD'), 'identity')
            command = run.call_args.args[0]
            self.assertEqual(command[0], str(executable))
            self.assertIn('--no-optional-locks', command)
            self.assertIn('core.hooksPath=' + os.devnull, command)
            self.assertIn('core.fsmonitor=false', command)
            self.assertEqual(run.call_args.kwargs['timeout'], 10)
            self.assertEqual(run.call_args.kwargs['env']['GIT_CONFIG_GLOBAL'], os.devnull)
            def drift(*args, **kwargs):
                executable.write_bytes(b'changed tool')
                return result
            run.side_effect = drift
            with self.assertRaisesRegex(q.Rejected, 'Git executable identity drift'):
                q._git(self.repo, 'rev-parse', 'HEAD')

    def test_expired_producer_tick_aborts_before_source_read(self):
        root, expected = self.shared_fixture()
        producer = types.ModuleType(q.PRODUCER_NAME)
        producer.__file__ = str(root / 'preparation.py')
        producer.__source_sha256__ = expected['preparation.py']['sha256']
        producer.tick = Mock(side_effect=q.Rejected('expired phase'))
        producer.remaining_seconds = lambda: 0.0
        with patch.object(q, '__file__', str(root / 'context.py')), \
             patch.dict(sys.modules, {q.PRODUCER_NAME: producer}):
            with self.assertRaisesRegex(q.Rejected, 'expired phase'):
                q.body(self.file)
            with self.assertRaisesRegex(q.Rejected, 'expired phase'):
                q.scan(self.root)
            producer.tick.assert_called()

    def test_body_ticks_each_chunk_and_git_uses_shared_remaining_budget(self):
        self.file.write_bytes(b'x' * (2 * 1024 ** 2 + 1))
        with patch.object(q, '_tick') as tick:
            self.assertEqual(len(q.body(self.file)), 2 * 1024 ** 2 + 1)
            self.assertGreaterEqual(tick.call_count, 5)
        executable = self.root / 'git-tool'
        executable.write_bytes(b'mocked tool body')
        result = types.SimpleNamespace(returncode=0, stdout=b'identity\n', stderr=b'')
        owner = types.SimpleNamespace(tick=lambda: None, remaining_seconds=lambda: 0.25)
        with patch.object(q, '_clock_owner', return_value=owner), \
             patch.object(q.shutil, 'which', return_value=str(executable)), \
             patch.object(q.subprocess, 'run', return_value=result) as run:
            q._git(self.repo, 'rev-parse', 'HEAD')
            self.assertEqual(run.call_args.kwargs['timeout'], 0.25)
        with patch.object(q, '_LOCAL_DEADLINE', 0):
            with self.assertRaisesRegex(q.Rejected, 'source capture deadline'):
                q.body(self.file)

    def test_shared_requires_all_five_and_closed_directory(self):
        root, expected = self.shared_fixture()
        self.assertEqual(q._shared(self.repo)[1], expected)
        (root / 'extra').write_bytes(b'extra')
        with self.assertRaises(q.Rejected):
            q._shared(self.repo)
        (root / 'extra').unlink()
        (root / 'preparation.py').unlink()
        with self.assertRaises(q.Rejected):
            q._shared(self.repo)

    def test_producer_registration_checks_canonical_owner_path_hash(self):
        root, expected = self.shared_fixture()
        producer = types.ModuleType(q.PRODUCER_NAME)
        producer.__file__ = str(root / 'preparation.py')
        producer.__source_sha256__ = expected['preparation.py']['sha256']
        producer.tick = lambda: None
        producer.remaining_seconds = lambda: 7200.0
        with patch.object(q, '__file__', str(root / 'context.py')), \
             patch.object(q, '__source_sha256__', expected['context.py']['sha256']), \
             patch.dict(sys.modules, {q.CONTEXT_NAME: q, q.PRODUCER_NAME: producer}):
            token = q._register_producer(producer)
            self.assertIs(q._register_producer(producer), token)
            with patch.object(producer, '__source_sha256__', '0' * 64):
                with self.assertRaises(q.Rejected):
                    q._register_producer(producer)
            with patch.object(producer, '__file__', str(root / 'wrong.py')):
                with self.assertRaises(q.Rejected):
                    q._register_producer(producer)
            other = types.ModuleType(q.PRODUCER_NAME)
            other.__file__, other.__source_sha256__ = producer.__file__, producer.__source_sha256__
            with self.assertRaises(q.Rejected):
                q._register_producer(other)
            (root / 'preparation.py').write_bytes(b'drift')
            with self.assertRaises(q.Rejected):
                q._register_producer(producer)

    def test_scoped_admission_uses_trusted_worker_and_provenance(self):
        for scope in q.SCOPES:
            with self.subTest(scope=scope):
                self.capture_value = {'scope': scope, 'checkout': {'path': str(self.repo),
                    'head': 'a' * 40, 'tree': 'b' * 40, 'event_sha': 'c' * 40},
                    'current_source_manifest': q.row('new.json', b'new'),
                    'predecessor_source_manifest': q.row('old.json', b'old'),
                    'source_checkpoint': {'head': 'd' * 40, 'tree': 'e' * 40},
                    'maps': {role: [q.row('src/m%04d.rs' % n, b'source') for n in range(q.ROLES[role][3])]
                             for role in q.SCOPES[scope]}, 'shared_helper': {}, 'shared_package': {}}
                invocation = self.invocation(scope)
                root = self.root / scope / 'source-preparation'
                (root / 'predecessor').mkdir(parents=True)
                (root / 'report.json').write_bytes(b'report fixture')
                proof = 'public-admission.json' if scope == 'public' else 'session.json'
                (root / 'predecessor' / proof).write_bytes(b'proof fixture')
                config = {'scope': scope, 'current_repo': str(self.repo), 'stage1_root': str(self.repo),
                          'output': str(root), 'run_root': str(root.parent)}
                overlays = [q.row(q.ROLES[role][0] + '/package-preparation-overlay.json', b'overlay')
                            for role in q.SCOPES[scope]]
                proposed = {'views': {q.ROLES[role][0]: self.capture_value['maps'][role]
                                     for role in q.SCOPES[scope]}, 'expected_files': overlays}
                report = {'scope': scope, 'status': 'NotReady', 'execution_qualified': False,
                          'old_source': self.capture_value['predecessor_source_manifest'],
                          'new_source': self.capture_value['current_source_manifest'],
                          'source_checkpoint': self.capture_value['source_checkpoint'], 'files': overlays}
                proof_key = 'genuine_public_admission' if scope == 'public' else 'genuine_predecessor_session'
                report[proof_key] = q.row('predecessor/' + proof, b'proof fixture')
                (root / 'report.json').write_bytes(q.serial(report))
                provenance = {'supervisor': 'mocked control only', 'nonce': invocation.document()['nonce']}
                verified = {'config': config, 'plan': proposed, 'provenance': provenance}
                worker = types.SimpleNamespace(plan=Mock(return_value=proposed), verify_output=Mock(return_value=report))
                entry=q._INVOCATIONS[invocation];entry['record']=object();entry['issuer']=object()
                producer=types.SimpleNamespace(checked_scope=Mock(side_effect=lambda _:nullcontext()))
                with patch.object(q, '_producer',return_value=producer), patch.object(q, '_preparation', return_value=verified), patch.object(q, '_worker_api', return_value=worker), patch.object(q,'_invocation',wraps=q._invocation) as full:
                    result = q._admit(invocation, root)
                    self.assertEqual(full.call_count,2)
                    producer.checked_scope.assert_called_once_with(invocation)
                    self.assertEqual(set(result['views']), set(q.SCOPES[scope]))
                    self.assertEqual(result['preparation']['provenance_sha256'], q.sha(q.canonical(provenance)))
                    self.assertEqual(result['preparation']['scope'], scope)
                    self.assertEqual(result['preparation']['report'], q.identity(root / 'report.json'))
                    worker.plan.assert_called_once_with(config, tick=q._tick)
                    worker.verify_output.assert_called_once_with(config, proposed, tick=q._tick)
                    worker.plan.return_value = dict(proposed, drift=True)
                    with self.assertRaisesRegex(q.Rejected, 'sealed preparation plan differs'):
                        q._admit(invocation, root)

    def test_full_admission_snapshot_brackets_pure_read_body(self):
        invocation=self.invocation();root=self.root/'run/source-preparation';root.mkdir(parents=True)
        entry=q._INVOCATIONS[invocation];entry['record']=object();entry['issuer']=object()
        events=[]
        from contextlib import contextmanager
        @contextmanager
        def scope(_):
            events.append('scope-entry')
            yield
            events.append('scope-exit')
        producer=types.SimpleNamespace(checked_scope=scope)
        original=q._invocation
        def full(value):events.append('full-snapshot');return original(value)
        def body(*_):events.append('all-admission-body-checks');return {'mocked':True}
        with patch.object(q,'_producer',return_value=producer),patch.object(q,'_invocation',side_effect=full),patch.object(q,'_admit_readonly',side_effect=body):
            self.assertEqual(q._admit(invocation,root),{'mocked':True})
        self.assertEqual(events,['full-snapshot','scope-entry','all-admission-body-checks','scope-exit','full-snapshot'])

    def test_execution_stays_notready(self):
        for function in (q.build, q.runtime_admission):
            with self.assertRaises(q.NotReady):
                function({'execution_qualified': True})

    def test_installed_stage1_scope_load_is_pure(self):
        for scope, wanted in [('public', {'source', 'public_maps'}),
                              ('parser', {'source', 'parser_maps'}),
                              ('combined', {'source', 'public_maps', 'parser_maps'})]:
            with patch.object(q.subprocess, 'Popen', side_effect=AssertionError('subprocess forbidden')):
                _, mods, records = q._stage(HERE.parents[2], scope)
            self.assertEqual(len(records), 10)
            self.assertEqual(set(mods), wanted)


if __name__ == '__main__':
    unittest.main()
