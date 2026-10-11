"""Source-only controls for the distinct, execution-held production owner.

Every test forbids subprocess creation.  Synthetic source capabilities and a
purposefully in-memory process tree exercise the retained supervisor and its
terminal evidence paths; they never establish production execution authority.
"""
import copy
import hashlib
from pathlib import Path
import stat
import subprocess
import sys
import tempfile
import types
import unittest
import weakref
from contextlib import ExitStack
from unittest.mock import Mock, patch

sys.dont_write_bytecode = True
HERE = Path(__file__).resolve().parent
MARKER = b"# BEGIN EXPLICIT PRODUCTION SUCCESSOR"
NAME = '_oxid_package_dependency_production_process_v1'
BASELINE_BYTES = 117696
BASELINE_SHA = '7c475867d93065b123f5c6f4acd12a7c622efb0c8392c0f7748d0637fa11e6e6'


def isolated_api():
    path = HERE / 'preparation.py'
    module = types.ModuleType(NAME)
    module.__file__ = str(path)
    raw = path.read_bytes()
    module.__source_sha256__ = hashlib.sha256(raw).hexdigest()
    exec(compile(raw, str(path), 'exec'), module.__dict__)
    return module


class FakePipe:
    """A numeric token only: it is never an OS descriptor or process pipe."""
    def __init__(self, number):
        self.number, self.closed = number, False

    def fileno(self):
        return self.number

    def close(self):
        self.closed = True


class InlineReader:
    """Run the original bounded reader against fake bytes without any thread."""
    def __init__(self, target, args, daemon):
        self.target, self.args = target, args
        self.daemon = daemon

    def start(self):
        self.target(*self.args)

    def is_alive(self):
        return False

    def join(self, timeout):
        pass


class FakeTree:
    def __init__(self, events, status=0, after_close=None):
        self.events, self.closed = events, False
        self.child = types.SimpleNamespace(
            pid=424242, stdout=FakePipe(101), stderr=FakePipe(102),
            poll=Mock(return_value=status), wait=Mock(return_value=status), kill=Mock())
        self.factory = Mock(side_effect=self.start)
        self.close = Mock(side_effect=self.finish)
        self.after_close = after_close

    def start(self, *args, **kwargs):
        self.events.append('fake-engine-start')
        return self

    def finish(self):
        self.events.append('tree-close')
        self.closed = True
        if self.after_close is not None:
            self.after_close()


class ObservedRegistry(weakref.WeakKeyDictionary):
    def __init__(self, events):
        super().__init__()
        self.events = events

    def __setitem__(self, key, value):
        self.events.append('record-registry-issue')
        super().__setitem__(key, value)


class ProductionControls(unittest.TestCase):
    def setUp(self):
        self.guard = ExitStack()
        self.addCleanup(self.guard.close)
        # Install guards before loading even the candidate's top-level source.
        self.popen = self.guard.enter_context(patch.object(
            subprocess, 'Popen', side_effect=AssertionError('actual process creation forbidden')))
        self.run = self.guard.enter_context(patch.object(
            subprocess, 'run', side_effect=AssertionError('actual process execution forbidden')))
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name).resolve()
        self.q = isolated_api()
        self.events = []
        self.guard.enter_context(patch.object(self.q, '_manifest',
            side_effect=AssertionError('legacy Unit4 source owner must never be consulted')))
        self.guard.enter_context(patch.object(self.q, '_worker',
            side_effect=AssertionError('legacy Unit4 preparation worker must never be consulted')))

    def tearDown(self):
        self.popen.assert_not_called()
        self.run.assert_not_called()

    def file(self, name, raw=b'synthetic nonexecutable source-only bytes'):
        path = self.root / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(raw)
        return path

    def source_fixture(self, *, host='linux-x86_64', nonce='source-only-fixture'):
        """Fake private owner boundary; no real source admission is asserted."""
        q = self.q
        self.repo = self.root / 'synthetic-checkout'
        self.runner = self.root / 'synthetic-runner-temp'
        self.repo.mkdir(exist_ok=True)
        self.runner.mkdir(exist_ok=True)
        self.context = object()
        self.recipe_id = 'quality-and-repository:release-production'
        suffix = '.exe' if host == 'windows-x86_64' else ''
        self.recipe = {
            'id': self.recipe_id, 'job_recipe_ids': ['synthetic-job'],
            'role': 'production', 'profile': 'release',
            'argv': ['cargo', 'build', '--release', '--locked'],
            'projection_id': 'current-all376', 'manifest_path': 'Cargo.toml',
            'package_name': 'oxid', 'target_kind': 'bin', 'target_name': 'oxid',
            'target_path': 'src/cli.rs', 'output_relative': 'target/release/oxid' + suffix,
            'artifact_key': self.recipe_id.replace(':', '-'), 'original_id': 'synthetic-original',
        }
        self.doc = {
            'job_recipe_id': 'synthetic-job', 'host': host, 'nonce': nonce,
            'repository_path': str(self.repo),
            'runner_temp': {'path': str(self.runner), 'identity': q._production_directory(self.runner)},
            'source_files': ['synthetic-source-%03d' % i for i in range(376)],
        }
        self.owner = types.ModuleType(q.SOURCE_OWNER_KEY)
        self.owner.owner = Mock()
        self.owner.owns_production_owner = Mock(side_effect=lambda module: module is q)
        self.owner.tick = Mock()

        def verify_job(context):
            q.need(context is self.context, 'synthetic original source context required')
            return copy.deepcopy(self.doc)

        self.owner.verify_job = Mock(side_effect=verify_job)
        self.owner.registry = Mock(side_effect=lambda context: {'build_recipes': {
            self.recipe_id: copy.deepcopy(self.recipe)}})
        self.owner.route = Mock(return_value={'original': {'command': 'cargo build --release --locked'}})
        self.owner.projection = Mock(side_effect=lambda *args: {'files': copy.deepcopy(self.doc['source_files'])})
        self.owner.require_operational = Mock()
        self.artifacts = types.SimpleNamespace(_policies=Mock(), _metadata_exact=Mock())
        self.owner.load_sibling = Mock(return_value=self.artifacts)
        self.owner.load_production_owner = Mock(side_effect=lambda name: self.machine)
        self.guard.enter_context(patch.object(q, '_SOURCE_OWNER', self.owner, create=True))
        self.guard.enter_context(patch.dict(sys.modules, {q.PRODUCTION_KEY: q, q.SOURCE_OWNER_KEY: self.owner}))
        self.machine = types.SimpleNamespace(production_operation_inputs=Mock(side_effect=self.descriptor),
                                             verify_production_for_operation=Mock())
        return self.context

    def descriptor(self, invocation, operation):
        if operation == 'build':
            argv, kind = ['cargo', 'build', '--release', '--locked'], 'ci-production-build'
        elif operation.startswith('metadata-'):
            argv = ['cargo', 'metadata', '--format-version', '1', '--locked', '--no-deps']
            kind = 'ci-production-metadata'
        else:
            argv, kind = ['cargo', '--version'], 'ci-production-probe'
        return {'argv': argv, 'cwd': str(self.repo), 'environment': {'PATH': str(self.root / 'synthetic-bin')},
                'timeout_seconds': 10, 'stream_limit_bytes': 64, 'kind': kind}

    def begin_fixture(self):
        self.source_fixture()
        self.guard.enter_context(patch.object(self.q, 'PRODUCTION_EXECUTION_REVIEWED', True))
        return self.q.begin_production(self.context, self.recipe_id)

    def fake_engine(self, invocation, *, status=0, chunks=None, after_close=None, live_group=False,
                    sampled_members=None, phase=False):
        """Replace only private launch/OS boundaries, retaining _supervise itself."""
        q = self.q
        # Generic supervisor tests bypass only the independent machine/metadata
        # policy boundary. Dedicated phase tests below exercise its real body.
        if not phase:
            self.guard.enter_context(patch.object(q, '_production_phase'))
        tree = FakeTree(self.events, status, after_close)
        self.tool = self.file('synthetic-bin/cargo')
        self.tree_path = self.file('synthetic-checkout/' + q.TREE_PATH)
        self.guard.enter_context(patch.object(q.shutil, 'which', return_value=str(self.tool)))
        self.module_loader = self.guard.enter_context(patch.object(q, '_module',
            return_value=types.SimpleNamespace(ProcessTree=tree.factory)))
        self.guard.enter_context(patch.object(q.threading, 'Thread', InlineReader))
        queues = copy.deepcopy(chunks or {101: [b'fake-engine stdout', b''], 102: [b'fake-engine stderr', b'']})

        def read(number, size):
            self.assertEqual(size, 65536)
            value = queues[number].pop(0)
            if isinstance(value, BaseException):
                raise value
            return value

        self.guard.enter_context(patch.object(q.os, 'read', side_effect=read))
        self.handlers = self.guard.enter_context(patch.object(q.signal, 'signal', return_value='synthetic-old-handler'))
        self.guard.enter_context(patch.object(q.sys, 'platform', 'linux'))

        def sample(pid):
            self.assertEqual(pid, tree.child.pid)
            if tree.closed:
                self.events.append('group-readback')
                return [{'pid': pid + 1, 'rss': 0, 'state': 'S'}] if live_group else []
            return sampled_members or []

        self.guard.enter_context(patch.object(q, '_sample_linux', side_effect=sample))
        self.guard.enter_context(patch.object(q, '_PRODUCTION_OPERATIONS', ObservedRegistry(self.events)))
        retain, emergency = q._retain_terminal, q._emergency_terminal

        def retain_terminal(*args):
            self.events.append('retain-terminal-start')
            result = retain(*args)
            self.events.append('retain-terminal-complete')
            return result

        def emergency_terminal(*args):
            self.events.append('emergency-terminal')
            return emergency(*args)

        self.guard.enter_context(patch.object(q, '_retain_terminal', side_effect=retain_terminal))
        self.guard.enter_context(patch.object(q, '_emergency_terminal', side_effect=emergency_terminal))
        self.real_supervise = self.guard.enter_context(patch.object(q, '_supervise', wraps=q._supervise))
        return tree

    def assert_cleanup(self, tree):
        tree.factory.assert_called_once()
        tree.close.assert_called_once_with()
        self.assertTrue(tree.child.stdout.closed)
        self.assertTrue(tree.child.stderr.closed)
        self.assertEqual(self.handlers.call_count, 4)
        self.assertLess(self.events.index('tree-close'), self.events.index('group-readback'))

    def assert_failed(self, invocation, tree, *, failure=None, timed_out=False, exceeded=False,
                      cleanup_complete=True, status=0):
        q = self.q
        state = q._PRODUCTION_INVOCATIONS[invocation]
        self.assert_cleanup(tree)
        self.assertTrue(state['failed'])
        self.assertIsNone(state['active'])
        self.assertEqual(len(q._PRODUCTION_OPERATIONS), 0)
        self.assertNotIn('record-registry-issue', self.events)
        self.assertIsNone(state['operations']['build']['record'])
        failed = state['failed_terminal']
        result = failed['outcome']
        self.assertIs(failed['success'], False)
        self.assertEqual(result['pid'], 424242)
        self.assertEqual(result['status'], status)
        self.assertEqual(result['timed_out'], timed_out)
        self.assertEqual(result['stream_limit_exceeded'], exceeded)
        self.assertEqual(result['cleanup_complete'], cleanup_complete)
        self.assertLessEqual(result['started_ns'], result['completed_ns'])
        if failure:
            self.assertIn(failure, result['failure'])
        self.assertLess(self.events.index('group-readback'), self.events.index('emergency-terminal'))
        directory = state['operations']['build']['directory']
        self.assertEqual(q.decode((directory / 'failed-outcome.json').read_bytes()), failed)
        self.assertEqual(q._PROBES, {})
        self.assertEqual(q._RECORDS, {})
        return result

    def test_exact_retained_baseline_has_only_six_named_substitutions(self):
        raw = (HERE / 'preparation.py').read_bytes()
        self.assertEqual(raw.count(MARKER), 1)
        retained, _ = raw.split(MARKER)
        # Two separator newlines belong to the appended branch, not the source.
        self.assertTrue(retained.endswith(b'\n\n\n'))
        retained = retained[:-2]
        substitutions = [
            (b" return 16 if not preparation and operation_kind in ('public-build','parser-build','parser-helper','runtime-trap-build','ci-production-build') else 4",
             b" return 16 if not preparation and operation_kind in ('public-build','parser-build','parser-helper','runtime-trap-build') else 4"),
            (b",output_monitor=None,production_invocation=None):", b",output_monitor=None):"),
            (b" compile_operation=operation_kind in ('public-build','parser-build','parser-helper','runtime-trap-build','ci-production-build')",
             b" compile_operation=operation_kind in ('public-build','parser-build','parser-helper','runtime-trap-build')"),
            (b"   if production_invocation is not None:production_tick(production_invocation)\n", b''),
            (b"  if production_invocation is not None:production_tick(production_invocation)\n", b''),
            (b"    roots=(output_root,) if production_invocation is None else _production_output_roots(production_invocation,output_root)\n    for p in (item for monitored in roots for item in _output_entries(monitored,native_registration,deadline,transient_roots)):",
             b"    for p in _output_entries(output_root,native_registration,deadline,transient_roots):"),
        ]
        for modified, original in substitutions:
            self.assertEqual(retained.count(modified), 1)
            retained = retained.replace(modified, original, 1)
        self.assertEqual(len(retained), BASELINE_BYTES)
        self.assertEqual(hashlib.sha256(retained).hexdigest(), BASELINE_SHA)
        self.assertEqual(self.q.BASELINE['bytes'], BASELINE_BYTES)
        self.assertEqual(self.q.BASELINE['sha256'], BASELINE_SHA)
        frozen = next((p / 'KH/package-unit4-consumers-v7/candidate' / self.q.BASELINE['path']
                       for p in HERE.parents
                       if (p / 'KH/package-unit4-consumers-v7/candidate' / self.q.BASELINE['path']).is_file()), None)
        if frozen is not None:
            self.assertEqual(retained, frozen.read_bytes())

    def test_every_unit4_public_entry_rejects_before_any_owner_lookup(self):
        with patch.object(self.q, '_production_source', side_effect=AssertionError('owner lookup forbidden')):
            for name in self.q.UNIT4_PUBLIC_ENTRIES:
                with self.subTest(name=name), self.assertRaisesRegex(self.q.Rejected, 'Unit4 API is unavailable'):
                    getattr(self.q, name)(object(), unexpected='inert')
        self.assertEqual(len(self.q.UNIT4_PUBLIC_ENTRIES), len(set(self.q.UNIT4_PUBLIC_ENTRIES)))

    def test_production_classes_cannot_be_constructed(self):
        for kind in (self.q.ProductionInvocation, self.q.ProductionOperationRecord):
            with self.subTest(kind=kind.__name__), self.assertRaises(self.q.Rejected):
                kind({})

    def test_forged_dict_unit4_and_cross_module_records_are_inert(self):
        q = self.q
        legacy = types.ModuleType('synthetic_legacy_no_owner')
        exec(compile((HERE / 'preparation.py').read_bytes().split(MARKER)[0], '<retained-source-only>', 'exec'), legacy.__dict__)
        other = isolated_api()
        values = [{}, object(), object.__new__(q.ProductionInvocation),
                  object.__new__(q.ProductionOperationRecord),
                  object.__new__(other.ProductionInvocation), object.__new__(other.ProductionOperationRecord),
                  legacy.ProbeRecord({}, legacy._TOKEN), legacy.PreparationRecord({}, legacy._TOKEN)]
        with patch.object(q, '_production_source', side_effect=AssertionError('pre-owner type rejection required')):
            for value in values:
                with self.subTest(type=type(value).__name__):
                    with self.assertRaises(q.Rejected):
                        q._production_entry(value)
                    with self.assertRaises(q.Rejected):
                        q.verify_production_operation(value, object(), 'build')

    def test_production_source_requires_original_canonical_and_issued_slots(self):
        q = self.q
        self.source_fixture()
        self.assertIs(q._production_source(), self.owner)
        for slot in (q.PRODUCTION_KEY, q.SOURCE_OWNER_KEY):
            with self.subTest(slot=slot), patch.dict(sys.modules, {slot: types.ModuleType(slot)}):
                with self.assertRaises(q.Rejected):
                    q._production_source()
        self.owner.owns_production_owner.return_value = False
        self.owner.owns_production_owner.side_effect = None
        with self.assertRaisesRegex(q.Rejected, 'unissued production process module'):
            q._production_source()

    def test_unregistered_invocation_subclass_cannot_gain_authority(self):
        invocation = self.begin_fixture()
        class Derived(self.q.ProductionInvocation):
            pass
        value = object.__new__(Derived)
        self.q._PRODUCTION_INVOCATIONS[value] = self.q._PRODUCTION_INVOCATIONS[invocation]
        with self.assertRaisesRegex(self.q.Rejected, 'original production invocation'):
            self.q.verify_production(value)

    def test_windows_and_execution_review_hold_precede_directory_effects(self):
        for host in ('linux-x86_64', 'windows-x86_64'):
            with self.subTest(host=host):
                self.source_fixture(host=host)
                with patch.object(Path, 'mkdir', side_effect=AssertionError('held path has no directory effects')):
                    with self.assertRaisesRegex(self.q.Rejected, 'NotReady'):
                        self.q.begin_production(self.context, self.recipe_id)
                self.owner.load_sibling.assert_not_called()
                self.assertEqual(len(self.q._PRODUCTION_INVOCATIONS), 0)

    def test_source_context_and_recipe_mismatches_precede_effects(self):
        self.source_fixture()
        with patch.object(Path, 'mkdir', side_effect=AssertionError('no effects for inert context')):
            for context, recipe in (({}, self.recipe_id), (object(), self.recipe_id),
                                    (self.context, 'arbitrary-recipe'), (self.context, 1)):
                with self.subTest(context=type(context).__name__, recipe=recipe), self.assertRaises(self.q.Rejected):
                    self.q.begin_production(context, recipe)

    def test_fixed_recipe_mismatch_roster_rejects_before_effects(self):
        self.source_fixture()
        original = copy.deepcopy(self.recipe)
        changes = {'id': 'other', 'job_recipe_ids': ['other'], 'role': 'test', 'profile': 'debug',
                   'argv': ['cargo', 'build'], 'projection_id': 'partial', 'manifest_path': 'elsewhere/Cargo.toml',
                   'package_name': 'other', 'target_kind': 'lib', 'target_name': 'other',
                   'target_path': 'src/elsewhere.rs', 'output_relative': 'target/debug/oxid', 'artifact_key': '../escape'}
        for key, value in changes.items():
            self.recipe = dict(original, **{key: value})
            with self.subTest(key=key), patch.object(Path, 'mkdir', side_effect=AssertionError('no effect')):
                with self.assertRaises(self.q.Rejected):
                    self.q.begin_production(self.context, self.recipe_id)
        self.recipe = original
        self.owner.route.return_value = {'original': {'command': 'cargo build --offline'}}
        with self.assertRaises(self.q.Rejected):
            self.q.begin_production(self.context, self.recipe_id)

    def test_projection_must_be_complete_and_exact(self):
        self.source_fixture()
        for files in (self.doc['source_files'][:-1], ['wrong'] * 376):
            self.owner.projection.side_effect = None
            self.owner.projection.return_value = {'files': files}
            with self.subTest(length=len(files)), self.assertRaisesRegex(self.q.Rejected, 'current-all376'):
                self.q.begin_production(self.context, self.recipe_id)

    def test_preexisting_selected_executable_is_neither_adopted_nor_deleted(self):
        self.source_fixture()
        selected = self.file('synthetic-checkout/target/release/oxid', b'untouched old cache')
        with patch.object(self.q, 'PRODUCTION_EXECUTION_REVIEWED', True):
            with self.assertRaisesRegex(self.q.Rejected, 'selected executable preexists'):
                self.q.begin_production(self.context, self.recipe_id)
        self.assertEqual(selected.read_bytes(), b'untouched old cache')
        self.assertEqual(list(self.runner.iterdir()), [])

    def test_preexisting_evidence_root_is_neither_adopted_nor_deleted(self):
        self.source_fixture()
        evidence = self.runner / ('package-ci-production-' + self.doc['nonce'] + '-' + self.recipe['artifact_key'])
        evidence.mkdir()
        (evidence / 'precious').write_bytes(b'untouched')
        with patch.object(self.q, 'PRODUCTION_EXECUTION_REVIEWED', True):
            with self.assertRaisesRegex(self.q.Rejected, 'evidence root must be fresh'):
                self.q.begin_production(self.context, self.recipe_id)
        self.assertEqual((evidence / 'precious').read_bytes(), b'untouched')

    def test_evidence_must_be_external_to_checkout_and_runner_identity_exact(self):
        self.source_fixture()
        for parent in (self.repo, self.repo / 'inside'):
            parent.mkdir(exist_ok=True)
            self.doc['runner_temp'] = {'path': str(parent), 'identity': self.q._production_directory(parent)}
            with self.subTest(parent=parent), self.assertRaisesRegex(self.q.Rejected, 'external to checkout'):
                self.q._production_paths(self.doc, self.recipe)
        self.doc['runner_temp'] = {'path': str(self.runner), 'identity': {'ino': -1}}
        with self.assertRaisesRegex(self.q.Rejected, 'identity drift'):
            self.q._production_paths(self.doc, self.recipe)

    def test_registered_source_nonce_host_and_recipe_drift_are_rejected(self):
        invocation = self.begin_fixture()
        for key, value in (('nonce', 'other-run'), ('host', 'macos-aarch64'), ('job_recipe_id', 'other-job'),
                           ('repository_path', str(self.root))):
            old = self.doc[key]
            self.doc[key] = value
            with self.subTest(key=key), self.assertRaises(self.q.Rejected):
                self.q.verify_production(invocation)
            self.doc[key] = old
        self.recipe['profile'] = 'debug'
        with self.assertRaises(self.q.Rejected):
            self.q.verify_production(invocation)

    def test_cross_invocation_scope_and_cancellation_reject(self):
        invocation = self.begin_fixture()
        q = self.q
        other = object.__new__(q.ProductionInvocation)
        q._PRODUCTION_INVOCATIONS[other] = copy.copy(q._PRODUCTION_INVOCATIONS[invocation])
        with q._production_scope(invocation):
            with self.assertRaisesRegex(q.Rejected, 'cross-invocation'):
                with q._production_scope(other):
                    self.fail('cross-invocation scope entered')
        q.cancel_production(invocation)
        with self.assertRaises(q.Cancelled):
            q.production_tick(invocation)
        self.assertFalse(q._PRODUCTION_INVOCATIONS[other]['cancelled'])

    def test_begin_same_context_recipe_is_single_use_even_after_root_removal(self):
        invocation = self.begin_fixture()
        state = self.q._PRODUCTION_INVOCATIONS[invocation]
        (state['root'] / 'operations').rmdir()
        state['root'].rmdir()
        with self.assertRaisesRegex(self.q.Rejected, 'already consumed'):
            self.q.begin_production(self.context, self.recipe_id)

    def test_descriptor_rejects_expansion_bounds_cwd_target_and_command_drift(self):
        invocation = self.begin_fixture()
        good = self.descriptor(invocation, 'build')
        changes = [{'extra': True}, {'argv': ['cargo', 'build', '--release']}, {'argv': ['cargo', '\0']},
                   {'cwd': str(self.root)}, {'environment': {'CARGO_TARGET_DIR': '/arbitrary'}},
                   {'environment': {'PATH': '\0'}}, {'timeout_seconds': 1801}, {'timeout_seconds': True},
                   {'stream_limit_bytes': 8 * 1024 ** 2 + 1}, {'stream_limit_bytes': True},
                   {'kind': 'public-build'}]
        for change in changes:
            self.machine.production_operation_inputs.side_effect = None
            self.machine.production_operation_inputs.return_value = dict(good, **change)
            with self.subTest(change=change), self.assertRaises(self.q.Rejected):
                self.q._production_descriptor(invocation, 'build')
        for operation in ('arbitrary', {}, 1):
            with self.subTest(operation=operation), self.assertRaises(self.q.Rejected):
                self.q._production_descriptor(invocation, operation)

    def test_success_uses_actual_retained_fake_engine_and_issues_only_after_terminal(self):
        invocation = self.begin_fixture()
        tree = self.fake_engine(invocation)
        q = self.q
        record = q.owned_production_operation(invocation, 'build')
        self.assert_cleanup(tree)
        self.assertIs(type(record), q.ProductionOperationRecord)
        self.assertEqual(len(q._PRODUCTION_OPERATIONS), 1)
        self.assertEqual(self.events.count('record-registry-issue'), 1)
        for earlier, later in (('fake-engine-start', 'tree-close'), ('group-readback', 'retain-terminal-start'),
                               ('retain-terminal-start', 'retain-terminal-complete'),
                               ('retain-terminal-complete', 'record-registry-issue')):
            self.assertLess(self.events.index(earlier), self.events.index(later))
        self.real_supervise.assert_called_once()
        self.assertIs(self.real_supervise.call_args.kwargs['production_invocation'], invocation)
        self.assertEqual(self.real_supervise.call_args.kwargs['transient_roots'], (self.repo / 'target',))
        self.assertEqual(record.stdout(), b'fake-engine stdout')
        self.assertEqual(record.stderr(), b'fake-engine stderr')
        document = q.verify_production_operation(record, invocation, 'build')
        self.assertFalse(document['cargo_offline'])
        self.assertFalse(document['network_isolated'])
        self.assertEqual(document['pid'], tree.child.pid)
        self.assertEqual(document['argv'], ['cargo', 'build', '--release', '--locked'])
        self.assertEqual(document['operation_policy']['processes'], 16)
        self.assertEqual(document['operation_policy']['rss_bytes'], 8 * 1024 ** 3)
        self.assertEqual(document['operation_policy']['output_bytes'], 6 * 1024 ** 3)
        self.assertEqual(document['operation_policy']['file_bytes'], 2 * 1024 ** 3)
        self.assertIsNone(document['preparation_resource_policy'])

    def test_operation_and_cross_invocation_replay_never_launch_again(self):
        invocation = self.begin_fixture()
        tree = self.fake_engine(invocation)
        q = self.q
        record = q.owned_production_operation(invocation, 'build')
        for other, operation in ((object(), 'build'), (invocation, 'metadata-before')):
            with self.subTest(operation=operation), self.assertRaisesRegex(q.Rejected, 'another invocation/operation'):
                q.verify_production_operation(record, other, operation)
        with self.assertRaisesRegex(q.Rejected, 'already attempted'):
            q.owned_production_operation(invocation, 'build')
        tree.factory.assert_called_once()

    def test_cross_registry_record_rejects_before_source_owner(self):
        invocation = self.begin_fixture()
        self.fake_engine(invocation)
        record = self.q.owned_production_operation(invocation, 'build')
        other = isolated_api()
        with patch.object(other, '_production_source', side_effect=AssertionError('must reject pre-owner')):
            with self.assertRaisesRegex(other.Rejected, 'original production operation'):
                other.verify_production_operation(record, invocation, 'build')

    def test_supervisor_scans_only_production_target_and_evidence_roots(self):
        invocation = self.begin_fixture()
        self.fake_engine(invocation)
        q = self.q
        state = q._PRODUCTION_INVOCATIONS[invocation]
        target = self.repo / 'target'
        self.file('synthetic-checkout/.git/immutable', b'not mutable output')
        self.file('synthetic-checkout/src/immutable', b'not mutable output')
        original = q._output_entries
        with patch.object(q, '_output_entries', wraps=original) as traversal:
            q.owned_production_operation(invocation, 'build')
        self.assertEqual({call.args[0] for call in traversal.call_args_list}, {target, state['root']})
        for call in traversal.call_args_list:
            self.assertEqual(call.args[3], (target,))
        for path in (self.repo, self.repo / '.git', self.repo / 'src', self.runner):
            with self.subTest(path=path), self.assertRaisesRegex(q.Rejected, 'output root mismatch'):
                q._production_output_roots(invocation, path)

    def test_cancelled_fake_engine_is_closed_and_failure_evidence_is_retained(self):
        invocation = self.begin_fixture()
        tree = self.fake_engine(invocation)
        tree.child.poll.side_effect = self.q.Cancelled('synthetic cancellation')
        with self.assertRaisesRegex(self.q.Rejected, 'terminal evidence failure'):
            self.q.owned_production_operation(invocation, 'build')
        self.assert_failed(invocation, tree, failure='synthetic cancellation')

    def test_production_checkpoint_cancellation_after_launch_issues_none(self):
        invocation = self.begin_fixture()
        tree = self.fake_engine(invocation)
        original_start = tree.start
        def start(*args, **kwargs):
            result = original_start(*args, **kwargs)
            self.q.cancel_production(invocation)
            return result
        tree.factory.side_effect = start
        with self.assertRaisesRegex(self.q.Rejected, 'terminal evidence failure'):
            self.q.owned_production_operation(invocation, 'build')
        self.assert_failed(invocation, tree, failure='cancelled')

    def test_timeout_preserves_process_cleanup_and_issues_none(self):
        invocation = self.begin_fixture()
        tree = self.fake_engine(invocation)
        original = self.q._supervise
        # Bound only the fake-engine clock; pre/post owner clocks remain real.
        def expired(*args, **kwargs):
            ticks = iter([0, 2])
            with patch.object(self.q.time, 'monotonic', side_effect=lambda: next(ticks, 2)):
                args = list(args)
                args[4] = 1
                return original(*args, **kwargs)
        with patch.object(self.q, '_supervise', side_effect=expired):
            with self.assertRaisesRegex(self.q.Rejected, 'terminal evidence failure'):
                self.q.owned_production_operation(invocation, 'build')
        self.assert_failed(invocation, tree, timed_out=True)

    def test_stream_overflow_preserves_bounded_raw_streams_and_issues_none(self):
        invocation = self.begin_fixture()
        tree = self.fake_engine(invocation, chunks={101: [b'x' * 65, b''], 102: [b'']})
        with self.assertRaisesRegex(self.q.Rejected, 'terminal evidence failure'):
            self.q.owned_production_operation(invocation, 'build')
        result = self.assert_failed(invocation, tree, exceeded=True)
        self.assertEqual(result['stdout']['bytes'], 64)

    def test_exited_leader_with_live_descendants_is_not_terminal_success(self):
        invocation = self.begin_fixture()
        tree = self.fake_engine(invocation, live_group=True)
        with self.assertRaisesRegex(self.q.Rejected, 'terminal evidence failure'):
            self.q.owned_production_operation(invocation, 'build')
        result = self.assert_failed(invocation, tree, cleanup_complete=False)
        self.assertIn('owned process group remains', result['cleanup_error'])

    def test_nonzero_child_retains_original_status_and_never_issues_record(self):
        invocation = self.begin_fixture()
        tree = self.fake_engine(invocation, status=7)
        with self.assertRaisesRegex(self.q.Rejected, 'terminal evidence failure'):
            self.q.owned_production_operation(invocation, 'build')
        self.assert_failed(invocation, tree, status=7, failure='nonzero status: 7')

    def test_postbuild_source_drift_preserves_original_process_facts(self):
        invocation = self.begin_fixture()
        tree = self.fake_engine(invocation, after_close=lambda: self.doc.update(nonce='postbuild-drift'))
        with self.assertRaisesRegex(self.q.Rejected, 'terminal evidence failure'):
            self.q.owned_production_operation(invocation, 'build')
        self.assert_failed(invocation, tree, failure='source/recipe drift')

    def test_postbuild_executable_drift_preserves_original_process_facts(self):
        invocation = self.begin_fixture()
        tree = self.fake_engine(invocation, after_close=lambda: self.tool.write_bytes(b'changed after fake close'))
        with self.assertRaisesRegex(self.q.Rejected, 'terminal evidence failure'):
            self.q.owned_production_operation(invocation, 'build')
        self.assert_failed(invocation, tree, failure='route/body drift')

    def test_terminal_result_write_failure_preserves_cleanup_and_issues_none(self):
        invocation = self.begin_fixture()
        tree = self.fake_engine(invocation)
        original_open = Path.open
        def broken(path, *args, **kwargs):
            if path.name == 'result.json' and args and args[0] == 'xb':
                raise OSError('synthetic terminal write failure')
            return original_open(path, *args, **kwargs)
        with patch.object(Path, 'open', broken):
            with self.assertRaisesRegex(self.q.Rejected, 'terminal evidence failure'):
                self.q.owned_production_operation(invocation, 'build')
        self.assert_failed(invocation, tree)
        self.assertIn('synthetic terminal write failure', self.q._PRODUCTION_INVOCATIONS[invocation]['failed_terminal']['evidence_error'])

    def test_terminal_hash_failure_preserves_cleanup_and_issues_none(self):
        invocation = self.begin_fixture()
        tree = self.fake_engine(invocation)
        original_identity = self.q.identity
        def broken(path):
            if Path(path).name == 'result.json':
                raise self.q.Rejected('synthetic terminal evidence identity failure')
            return original_identity(path)
        with patch.object(self.q, 'identity', side_effect=broken):
            with self.assertRaisesRegex(self.q.Rejected, 'terminal evidence failure'):
                self.q.owned_production_operation(invocation, 'build')
        self.assert_failed(invocation, tree)

    def test_process_and_rss_bounds_remain_original_compile_limits(self):
        invocation = self.begin_fixture()
        members = [{'pid': 424242 + i, 'rss': 0, 'state': 'S'} for i in range(17)]
        tree = self.fake_engine(invocation, sampled_members=members)
        with self.assertRaisesRegex(self.q.Rejected, 'terminal evidence failure'):
            self.q.owned_production_operation(invocation, 'build')
        self.assert_failed(invocation, tree, failure='process/RSS bound')
        self.assertEqual(self.q._operation_process_cap('ci-production-build', False, None), 16)
        self.assertEqual(self.q._operation_process_cap('ci-production-probe', False, None), 4)
        self.assertEqual(self.q._operation_process_cap('ci-production-build', True, None), 4)

    def test_selected_executable_appearing_before_launch_refuses_without_deletion(self):
        invocation = self.begin_fixture()
        tree = self.fake_engine(invocation)
        selected = self.file('synthetic-checkout/target/release/oxid', b'appeared cache')
        with self.assertRaisesRegex(self.q.Rejected, 'selected executable preexists'):
            self.q.owned_production_operation(invocation, 'build')
        tree.factory.assert_not_called()
        self.assertEqual(selected.read_bytes(), b'appeared cache')
        self.assertEqual(len(self.q._PRODUCTION_OPERATIONS), 0)

    def test_review_hold_reapplies_to_existing_invocation_before_launch(self):
        invocation = self.begin_fixture()
        tree = self.fake_engine(invocation)
        with patch.object(self.q, 'PRODUCTION_EXECUTION_REVIEWED', False):
            with self.assertRaisesRegex(self.q.Rejected, 'NotReady'):
                self.q.owned_production_operation(invocation, 'build')
        tree.factory.assert_not_called()
        self.assertEqual(self.q._PRODUCTION_INVOCATIONS[invocation]['operations'], {})

    def test_terminal_evidence_and_stream_replay_detect_body_mutations(self):
        invocation = self.begin_fixture()
        self.fake_engine(invocation)
        record = self.q.owned_production_operation(invocation, 'build')
        directory = self.q._PRODUCTION_OPERATIONS[record]['directory']
        (directory / 'stdout').write_bytes(b'forged replay')
        with self.assertRaisesRegex(self.q.Rejected, 'stream drift'):
            record.stdout()
        with self.assertRaises(self.q.Rejected):
            self.q.verify_production_operation(record, invocation, 'build')

    def test_final_verifier_failure_revokes_all_new_record_associations(self):
        invocation = self.begin_fixture()
        tree = self.fake_engine(invocation)
        q = self.q
        with patch.object(q, 'verify_production_operation', side_effect=q.Rejected('synthetic last boundary drift')):
            with self.assertRaisesRegex(q.Rejected, 'last boundary drift'):
                q.owned_production_operation(invocation, 'build')
        state = q._PRODUCTION_INVOCATIONS[invocation]
        self.assert_cleanup(tree)
        self.assertTrue(state['failed'])
        self.assertIsNone(state['active'])
        self.assertIsNone(state['operations']['build']['record'])
        self.assertEqual(len(q._PRODUCTION_OPERATIONS), 0)
        # The original retained result must survive revocation unchanged.
        result = q.decode((state['operations']['build']['directory'] / 'result.json').read_bytes())
        self.assertEqual(result['pid'], 424242)
        self.assertEqual(result['status'], 0)
        self.assertIs(result['cleanup_complete'], True)
        self.assertIsNone(result['cleanup_error'])

    def test_phase_requires_successful_metadata_before_build_and_after(self):
        invocation = self.begin_fixture()
        tree = self.fake_engine(invocation, phase=True)
        for operation in ('build', 'metadata-after'):
            with self.subTest(operation=operation), self.assertRaisesRegex(self.q.Rejected, 'successful metadata-before'):
                self.q._production_phase(invocation, operation)
        tree.factory.assert_not_called()
        self.module_loader.assert_not_called()
        self.assertEqual(self.q._PRODUCTION_INVOCATIONS[invocation]['operations'], {})

    def test_phase_verifies_original_metadata_record_and_exact_metadata_body(self):
        invocation = self.begin_fixture()
        tree = self.fake_engine(invocation, phase=True)
        q = self.q
        record = q.owned_production_operation(invocation, 'metadata-before')
        self.assertIs(type(record), q.ProductionOperationRecord)
        q._production_phase(invocation, 'build')
        self.artifacts._metadata_exact.assert_called_once_with(
            b'fake-engine stdout', self.repo, self.recipe, self.doc['source_files'])
        with self.assertRaisesRegex(q.Rejected, 'successful build before metadata-after'):
            q._production_phase(invocation, 'metadata-after')
        tree.factory.assert_called_once()

    def test_phase_rejects_non_original_metadata_record(self):
        invocation = self.begin_fixture()
        state = self.q._PRODUCTION_INVOCATIONS[invocation]
        state['operations']['metadata-before'] = {'record': {'status': 0}}
        with self.assertRaisesRegex(self.q.Rejected, 'original production operation'):
            self.q._production_phase(invocation, 'build')
        self.artifacts._metadata_exact.assert_not_called()

    def test_phase_does_not_accept_metadata_before_after_build_attempt(self):
        invocation = self.begin_fixture()
        self.q._PRODUCTION_INVOCATIONS[invocation]['operations']['build'] = {'record': None}
        with self.assertRaisesRegex(self.q.Rejected, 'metadata-before must precede build'):
            self.q._production_phase(invocation, 'metadata-before')

    def test_independent_machine_readmission_precedes_metadata_effects(self):
        invocation = self.begin_fixture()
        tree = self.fake_engine(invocation, phase=True)
        self.machine.verify_production_for_operation.side_effect = self.q.Rejected('synthetic machine drift')
        with self.assertRaisesRegex(self.q.Rejected, 'machine drift'):
            self.q.owned_production_operation(invocation, 'metadata-before')
        tree.factory.assert_not_called()
        self.module_loader.assert_not_called()
        self.assertEqual(self.q._PRODUCTION_INVOCATIONS[invocation]['operations'], {})

    def test_postbuild_independent_machine_drift_preserves_process_result(self):
        invocation = self.begin_fixture()
        tree = self.fake_engine(invocation)
        self.machine.verify_production_for_operation.side_effect = self.q.Rejected('synthetic postbuild machine drift')
        with self.assertRaisesRegex(self.q.Rejected, 'terminal evidence failure'):
            self.q.owned_production_operation(invocation, 'build')
        self.assert_failed(invocation, tree, failure='postbuild machine drift')

    def test_cancelled_checkpoint_before_tree_construction_has_no_launch(self):
        invocation = self.begin_fixture()
        tree = self.fake_engine(invocation)
        self.q.cancel_production(invocation)
        result, streams = self.q._supervise(tree.factory, ['synthetic-only'], str(self.repo), {}, 1, 64,
            output_root=self.repo / 'target', operation_kind='ci-production-build',
            transient_roots=(self.repo / 'target',), production_invocation=invocation)
        tree.factory.assert_not_called()
        tree.close.assert_not_called()
        self.assertIsNone(result['pid'])
        self.assertIsNone(result['status'])
        self.assertIn('cancelled', result['failure'])
        self.assertEqual(streams, {'stdout': b'', 'stderr': b''})
        self.assertEqual(len(self.q._PRODUCTION_OPERATIONS), 0)

    def test_invocation_deadline_expiry_precedes_launch(self):
        invocation = self.begin_fixture()
        tree = self.fake_engine(invocation)
        self.q._PRODUCTION_INVOCATIONS[invocation]['deadline'] = -1
        with self.assertRaisesRegex(self.q.Rejected, 'deadline expired'):
            self.q.owned_production_operation(invocation, 'build')
        tree.factory.assert_not_called()
        self.module_loader.assert_not_called()

    def test_rss_overflow_preserves_original_compile_memory_limit(self):
        invocation = self.begin_fixture()
        tree = self.fake_engine(invocation, sampled_members=[{
            'pid': 424242, 'rss': 8 * 1024 ** 3 + 1, 'state': 'S'}])
        with self.assertRaisesRegex(self.q.Rejected, 'terminal evidence failure'):
            self.q.owned_production_operation(invocation, 'build')
        result = self.assert_failed(invocation, tree, failure='process/RSS bound')
        self.assertEqual(result['operation_policy']['rss_bytes'], 8 * 1024 ** 3)

    def test_output_symlink_never_receives_a_transient_target_exception(self):
        invocation = self.begin_fixture()
        target = self.repo / 'target'
        target.mkdir()
        outside = self.file('outside-output')
        try:
            (target / 'unregistered-link').symlink_to(outside)
        except (OSError, NotImplementedError):
            self.skipTest('symbolic link creation unavailable')
        tree = self.fake_engine(invocation)
        with self.assertRaisesRegex(self.q.Rejected, 'terminal evidence failure'):
            self.q.owned_production_operation(invocation, 'build')
        self.assert_failed(invocation, tree, failure='unregistered output link')

    def test_special_output_never_receives_a_transient_target_exception(self):
        invocation = self.begin_fixture()
        tree = self.fake_engine(invocation)
        member = self.repo / 'target' / 'synthetic-fifo'
        original_lstat = Path.lstat
        def lstat(path, *args, **kwargs):
            if path == member:
                return types.SimpleNamespace(st_mode=stat.S_IFIFO, st_size=0)
            return original_lstat(path, *args, **kwargs)
        with patch.object(self.q, '_output_entries', return_value=iter([member])), patch.object(Path, 'lstat', lstat):
            with self.assertRaisesRegex(self.q.Rejected, 'terminal evidence failure'):
                self.q.owned_production_operation(invocation, 'build')
        self.assert_failed(invocation, tree, failure='special output member')

    def test_output_file_bound_uses_original_two_gib_limit(self):
        invocation = self.begin_fixture()
        tree = self.fake_engine(invocation)
        member = self.repo / 'target' / 'synthetic-large-file'
        original_lstat = Path.lstat
        def lstat(path, *args, **kwargs):
            if path == member:
                return types.SimpleNamespace(st_mode=stat.S_IFREG, st_size=2 * 1024 ** 3 + 1)
            return original_lstat(path, *args, **kwargs)
        with patch.object(self.q, '_output_entries', return_value=iter([member])), patch.object(Path, 'lstat', lstat):
            with self.assertRaisesRegex(self.q.Rejected, 'terminal evidence failure'):
                self.q.owned_production_operation(invocation, 'build')
        result = self.assert_failed(invocation, tree, failure='output file bound')
        self.assertEqual(result['operation_policy']['file_bytes'], 2 * 1024 ** 3)

    def test_failure_summary_is_bounded_and_never_replaces_terminal_evidence(self):
        invocation = self.begin_fixture()
        tree = self.fake_engine(invocation, status=3)
        with self.assertRaisesRegex(self.q.Rejected, 'terminal evidence failure'):
            self.q.owned_production_operation(invocation, 'build')
        state = self.q._PRODUCTION_INVOCATIONS[invocation]
        directory = state['operations']['build']['directory']
        before = {p.name: p.read_bytes() for p in directory.iterdir()}
        original_summary = copy.deepcopy(state['attempt_failure'])
        summary = self.q.retain_production_failure(invocation, RuntimeError('x' * 9000))
        self.assertIs(summary['success'], False)
        self.assertEqual(summary, original_summary)
        self.assertLessEqual(len(summary['error']), 8192)
        self.assertEqual(summary['source_readback'], 'not reverified after failure')
        self.assertEqual(summary['terminal_failure'], state['failed_terminal'])
        self.assertEqual(before, {p.name: p.read_bytes() for p in directory.iterdir()})
        self.assertEqual(self.q.decode((state['root'] / 'failed-attempt.json').read_bytes()), summary)
        self.assertEqual(len(self.q._PRODUCTION_OPERATIONS), 0)
        self.assert_cleanup(tree)

    def test_failure_summary_refuses_inert_invocation_and_nonexception(self):
        invocation = self.begin_fixture()
        with self.assertRaises(self.q.Rejected):
            self.q.retain_production_failure({}, RuntimeError('fixture'))
        with self.assertRaisesRegex(self.q.Rejected, 'actual production failure exception'):
            self.q.retain_production_failure(invocation, 'not an exception')

    def test_first_failure_summary_bounds_error_without_claiming_process_success(self):
        invocation = self.begin_fixture()
        summary = self.q.retain_production_failure(invocation, RuntimeError('x' * 9000))
        self.assertEqual(len(summary['error']), 8192)
        self.assertEqual(summary['operations'], [])
        self.assertIsNone(summary['terminal_failure'])
        self.assertIs(summary['success'], False)
        self.assertEqual(len(self.q._PRODUCTION_OPERATIONS), 0)

    def test_operation_directory_failure_is_registered_failed_without_launch(self):
        invocation = self.begin_fixture()
        tree = self.fake_engine(invocation)
        q = self.q
        state = q._PRODUCTION_INVOCATIONS[invocation]
        directory = state['root'] / 'operations' / '0001'
        original_mkdir = Path.mkdir
        def broken(path, *args, **kwargs):
            if path == directory:
                raise OSError('synthetic operation directory failure')
            return original_mkdir(path, *args, **kwargs)
        with patch.object(Path, 'mkdir', broken):
            with self.assertRaisesRegex(OSError, 'operation directory failure'):
                q.owned_production_operation(invocation, 'build')
        tree.factory.assert_not_called()
        self.assertTrue(state['failed'])
        self.assertIsNone(state['active'])
        self.assertEqual(state['operations']['build']['directory'], directory)
        self.assertIsNone(state['operations']['build']['record'])
        self.assertEqual(state['attempt_failure']['operations'], [
            {'id': 'build', 'evidence_directory': str(directory), 'completed_record': False}])
        self.assertIsNone(state['attempt_failure']['terminal_failure'])
        self.assertFalse(directory.exists())
        self.assertEqual(len(q._PRODUCTION_OPERATIONS), 0)

    def test_command_evidence_write_failure_is_registered_failed_without_launch(self):
        invocation = self.begin_fixture()
        tree = self.fake_engine(invocation)
        q = self.q
        state = q._PRODUCTION_INVOCATIONS[invocation]
        original_write = Path.write_bytes
        def broken(path, raw):
            if path.name == 'command.json':
                raise OSError('synthetic command evidence write failure')
            return original_write(path, raw)
        with patch.object(Path, 'write_bytes', broken):
            with self.assertRaisesRegex(OSError, 'command evidence write failure'):
                q.owned_production_operation(invocation, 'build')
        tree.factory.assert_not_called()
        self.assertTrue(state['failed'])
        self.assertIsNone(state['active'])
        self.assertIsNone(state['operations']['build']['record'])
        directory = state['operations']['build']['directory']
        self.assertTrue(directory.is_dir())
        self.assertEqual(list(directory.iterdir()), [])
        self.assertEqual(state['attempt_failure']['operations'], [
            {'id': 'build', 'evidence_directory': str(directory), 'completed_record': False}])
        self.assertIsNone(state['attempt_failure']['terminal_failure'])
        self.assertEqual(len(q._PRODUCTION_OPERATIONS), 0)

    def test_nonzero_child_and_terminal_write_failure_preserve_both_facts(self):
        invocation = self.begin_fixture()
        tree = self.fake_engine(invocation, status=23)
        original_open = Path.open
        def broken(path, *args, **kwargs):
            if path.name == 'result.json' and args and args[0] == 'xb':
                raise OSError('synthetic result write failure after nonzero child')
            return original_open(path, *args, **kwargs)
        with patch.object(Path, 'open', broken):
            with self.assertRaisesRegex(self.q.Rejected, 'terminal evidence failure'):
                self.q.owned_production_operation(invocation, 'build')
        result = self.assert_failed(invocation, tree, status=23, failure='nonzero status: 23')
        state = self.q._PRODUCTION_INVOCATIONS[invocation]
        self.assertIn('write failure after nonzero child', state['failed_terminal']['evidence_error'])
        self.assertEqual(state['attempt_failure']['terminal_failure']['outcome'], result)

    def test_supplementary_summary_failure_never_replaces_original_error(self):
        invocation = self.begin_fixture()
        tree = self.fake_engine(invocation)
        original = self.q.Rejected('synthetic original descriptor failure')
        with patch.object(self.q, '_production_descriptor', side_effect=original), \
                patch.object(self.q, 'retain_production_failure', side_effect=OSError('secondary summary failure')):
            with self.assertRaises(self.q.Rejected) as caught:
                self.q.owned_production_operation(invocation, 'build')
        self.assertIs(caught.exception, original)
        self.assertTrue(self.q._PRODUCTION_INVOCATIONS[invocation]['failed'])
        self.assertEqual(len(self.q._PRODUCTION_OPERATIONS), 0)
        tree.factory.assert_not_called()

    def test_phase_failure_poisons_invocation_and_cannot_be_replayed(self):
        invocation = self.begin_fixture()
        tree = self.fake_engine(invocation, phase=True)
        with self.assertRaisesRegex(self.q.Rejected, 'successful metadata-before'):
            self.q.owned_production_operation(invocation, 'build')
        state = self.q._PRODUCTION_INVOCATIONS[invocation]
        self.assertTrue(state['failed'])
        self.assertIs(state['attempt_failure']['success'], False)
        with self.assertRaisesRegex(self.q.Rejected, 'failed or deadline expired'):
            self.q.owned_production_operation(invocation, 'metadata-before')
        tree.factory.assert_not_called()

    def test_output_total_bound_combines_roots_without_expanding_mutable_scope(self):
        invocation = self.begin_fixture()
        tree = self.fake_engine(invocation)
        q = self.q
        state = q._PRODUCTION_INVOCATIONS[invocation]
        members = [self.repo / 'target' / ('file-%d' % i) for i in range(3)]
        members.append(state['root'] / 'synthetic-evidence-overflow')
        original_lstat = Path.lstat
        def lstat(path, *args, **kwargs):
            if path in members:
                return types.SimpleNamespace(st_mode=stat.S_IFREG, st_size=2 * 1024 ** 3)
            return original_lstat(path, *args, **kwargs)
        def entries(root, *args):
            if root == self.repo / 'target':
                return iter(members[:3])
            if root == state['root']:
                return iter(members[3:])
            self.fail('unexpected output root ' + str(root))
        with patch.object(q, '_output_entries', side_effect=entries), patch.object(Path, 'lstat', lstat):
            with self.assertRaisesRegex(q.Rejected, 'terminal evidence failure'):
                q.owned_production_operation(invocation, 'build')
        result = self.assert_failed(invocation, tree, failure='output byte bound')
        self.assertEqual(result['operation_policy']['output_bytes'], 6 * 1024 ** 3)

    def test_disappearance_exception_is_restricted_to_owned_target(self):
        invocation = self.begin_fixture()
        q = self.q
        state = q._PRODUCTION_INVOCATIONS[invocation]
        target = self.repo / 'target'
        q._scan_disappearance(target / 'transient.o', target, None, (target,))
        for missing in (self.repo / 'src' / 'source.rs', self.repo / '.git' / 'index',
                        state['root'] / 'operations' / 'result.json'):
            with self.subTest(path=missing), self.assertRaisesRegex(q.Rejected, 'vanished stable output'):
                q._scan_disappearance(missing, target, None, (target,))

    def test_final_scope_tick_failure_revokes_successful_provisional_record(self):
        invocation = self.begin_fixture()
        tree = self.fake_engine(invocation)
        q = self.q
        actual_tick, actual_verify = q.production_tick, q.verify_production_operation
        verified = False
        def verify(*args):
            nonlocal verified
            result = actual_verify(*args)
            verified = True
            return result
        def tick(value):
            if verified:
                raise q.Rejected('synthetic final scope tick expiry')
            return actual_tick(value)
        with patch.object(q, 'production_tick', side_effect=tick), \
                patch.object(q, 'verify_production_operation', side_effect=verify):
            with self.assertRaisesRegex(q.Rejected, 'final scope tick expiry'):
                q.owned_production_operation(invocation, 'build')
        self.assertTrue(verified)
        self.assert_cleanup(tree)
        state = q._PRODUCTION_INVOCATIONS[invocation]
        self.assertTrue(state['failed'])
        self.assertIsNone(state['active'])
        self.assertIsNone(q._PRODUCTION_ACTIVE.get())
        self.assertIsNone(state['operations']['build']['record'])
        self.assertEqual(len(q._PRODUCTION_OPERATIONS), 0)
        self.assertFalse(state['attempt_failure']['operations'][0]['completed_record'])
        result = q.decode((state['operations']['build']['directory'] / 'result.json').read_bytes())
        self.assertEqual(result['status'], 0)
        self.assertIs(result['cleanup_complete'], True)

    def test_cross_thread_invocation_rejects_without_real_thread_or_process(self):
        invocation = self.begin_fixture()
        tree = self.fake_engine(invocation)
        q = self.q
        state = q._PRODUCTION_INVOCATIONS[invocation]
        owner_thread = state['thread']
        with patch.object(q.threading, 'get_ident', return_value=owner_thread + 1):
            with self.assertRaisesRegex(q.Rejected, 'thread'):
                q.production_tick(invocation)
            with self.assertRaisesRegex(q.Rejected, 'thread'):
                q.owned_production_operation(invocation, 'build')
        self.assertEqual(state['thread'], owner_thread)
        self.assertTrue(state['failed'])
        self.assertEqual(len(q._PRODUCTION_OPERATIONS), 0)
        tree.factory.assert_not_called()
        self.module_loader.assert_not_called()


if __name__ == '__main__':
    unittest.main()
