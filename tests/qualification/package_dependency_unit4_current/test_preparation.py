"""Pure/mocked preparation controls; never execute preparation or native probes.

All process outcomes below come from in-memory fake process trees. Synthetic
registered records exercise verification only; they assert no genuine admission.
"""
import ast
import copy
import hashlib
import io
import os
from pathlib import Path
import stat
import subprocess
import sys
import tempfile
import types
import unittest
from contextlib import ExitStack
from unittest.mock import Mock, patch

sys.dont_write_bytecode = True
HERE = Path(__file__).resolve().parent
NAME = '_oxid_package_dependency_unit4_preparation_v1'


def isolated_api():
    path = HERE / 'preparation.py'
    raw = path.read_bytes()
    module = types.ModuleType(NAME)
    module.__file__ = str(path)
    module.__source_sha256__ = hashlib.sha256(raw).hexdigest()
    exec(compile(raw, str(path), 'exec'), module.__dict__)
    return module


class FakePipe:
    def __init__(self, number):
        self.number, self.closed = number, False

    def fileno(self):
        return self.number

    def close(self):
        self.closed = True


class InlineReader:
    """Deterministic bounded reader, with no operating-system thread or pipe."""
    def __init__(self, target, args, daemon):
        self.target, self.args = target, args
        self.daemon = daemon
        self.joined = False

    def start(self):
        self.target(*self.args)

    def is_alive(self):
        return False

    def join(self, timeout):
        self.joined = True


class FakeTree:
    def __init__(self):
        self.child = types.SimpleNamespace(
            pid=424242, stdout=FakePipe(101), stderr=FakePipe(102),
            poll=Mock(return_value=0), wait=Mock(return_value=0), kill=Mock())
        self.close = Mock()
        self.factory = Mock(return_value=self)


class FakeOutputRoot:
    def __init__(self, root, members):
        self.root, self.members = root, members

    def __truediv__(self, name):
        return self.root / name

    def rglob(self, pattern):
        return iter(self.members)


class PreparationControls(unittest.TestCase):
    def setUp(self):
        self.q = isolated_api()
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name).resolve()
        self.guard = ExitStack()
        self.addCleanup(self.guard.close)
        self.guard.enter_context(patch.object(subprocess, 'Popen', side_effect=AssertionError('no subprocess permitted')))
        self.guard.enter_context(patch.object(subprocess, 'run', side_effect=AssertionError('no subprocess permitted')))
        self.guard.enter_context(patch.object(self.q, 'owned_probe', side_effect=AssertionError('no actual owned probe permitted')))
        self.guard.enter_context(patch.object(self.q, 'produce', side_effect=AssertionError('no actual preparation permitted')))
        self.guard.enter_context(patch.object(self.q, 'verify_invocation', return_value=({}, {})))

    def file(self, name, raw=b'bounded fixture'):
        path = self.root / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(raw)
        return path

    def shared_fixture(self):
        q = self.q
        root = self.root / 'helpers'
        root.mkdir()
        for name in sorted(q.FILES):
            (root / name).write_bytes(('fixture ' + name + '\n').encode())
        rows = [dict(q.identity(root / name), path=name) for name in sorted(q.FILES)]
        manifest = {'schema': 'oxid-package-dependency-unit4-current-package-v1', 'files': rows}
        (root / 'package-manifest.json').write_bytes(q.serial(manifest))
        self.guard.enter_context(patch.object(q, '__file__', str(root / 'preparation.py')))
        self.guard.enter_context(patch.object(q, '__source_sha256__', next(r['sha256'] for r in rows if r['path'] == 'preparation.py')))
        self.guard.enter_context(patch.dict(sys.modules, {q.KEY: q}))
        return root, manifest

    def synthetic_probe(self):
        """Fixture-issued evidence only; no producer operation is invoked."""
        q = self.q
        invocation = object()
        directory = self.root / 'operation'
        directory.mkdir()
        command = {'argv': ['/fixture/tool', '--source-only'], 'cwd': str(self.root),
                   'environment': {'PATH': '/fixture'}, 'timeout_seconds': 3,
                   'stream_limit_bytes': 128, 'operation_kind': 'probe'}
        (directory / 'command.json').write_bytes(q.serial(command))
        tool = self.file('tool', b'not executable: fixture only')
        helper = self.file('process-helper', b'not executable: fixture helper')
        document = dict(command, schema='oxid-owned-probe-v1', invocation_nonce='fixture',
                        pid=424242, started_ns=10, completed_ns=20, status=0,
                        timed_out=False, stream_limit_exceeded=False,
                        cleanup_complete=True, cleanup_error=None, failure=None,
                        executable=q.identity(tool), process_tree=q.identity(helper),
                        executable_route={'path': str(tool), 'target': str(tool)})
        for name, raw in [('stdout', b'fixture output'), ('stderr', b'')]:
            (directory / name).write_bytes(raw)
            document[name] = q.identity(directory / name)
        (directory / 'result.json').write_bytes(q.serial(document))
        record = q.ProbeRecord(document, q._TOKEN)
        q._PROBES[record] = {'invocation': invocation, 'directory': directory,
                           'command': q.identity(directory / 'command.json'),
                           'result': q.identity(directory / 'result.json')}
        return invocation, record, command

    def check_probe(self, invocation, record, command):
        return self.q.verify_probe(record, invocation, **command)

    def synthetic_preparation(self):
        """In-memory verifier fixture, never an actual preparation result."""
        q = self.q
        invocation, probe, _ = self.synthetic_probe()
        config = {'scope': 'public', 'input_files': [], 'input_routes': [],
                  'absent_paths': [], 'closed_roots': []}
        plan = {'fixture_only': ['no genuine preparation was performed']}
        report = self.file('report.json', q.serial({'genuine_public_prepared_ns': 15}))
        config_file = self.file('config.json', q.serial(config))
        document = {'schema': 'oxid-live-preparation-record-v1', 'scope': 'public',
                    'config_sha256': q.sha(q.canonical(config)),
                    'plan_sha256': q.sha(q.canonical(plan)), 'operation': probe.document(),
                    'report': q.identity(report), 'expected': {},
                    'status': 'NotReady', 'execution_qualified': False}
        record = q.PreparationRecord(document, q._TOKEN)
        q._RECORDS[record] = {'invocation': invocation, 'config': config, 'plan': plan,
                            'probe': probe, 'config_file': q.identity(config_file)}
        return invocation, record

    def terminal_fixture(self, name='terminal'):
        directory = self.root / name
        directory.mkdir()
        (directory / 'command.json').write_bytes(self.q.serial({'fixture_only': True}))
        result = {'pid': 424242, 'status': 0, 'started_ns': 10, 'completed_ns': 20,
                  'cleanup_complete': True, 'cleanup_error': None, 'failure': None,
                  'timed_out': False, 'stream_limit_exceeded': False}
        streams = {'stdout': b'fixture terminal stdout', 'stderr': b'fixture terminal stderr'}
        state = {'failed': False}
        return directory, result, streams, state

    def assert_failed_terminal(self, directory, result, streams, state, *, on_disk=True):
        self.assertIs(state['failed'], True)
        failure = state['failed_terminal']
        self.assertIs(failure['success'], False)
        self.assertEqual(failure['schema'], 'oxid-failed-terminal-capture-v1')
        for key in ('pid', 'status', 'started_ns', 'completed_ns', 'cleanup_complete', 'cleanup_error'):
            self.assertEqual(failure['outcome'][key], result[key])
        self.assertEqual(failure['captured_stream_lengths'], {name: len(raw) for name, raw in streams.items()})
        self.assertIsInstance(failure['preservation_errors'], list)
        if on_disk:
            self.assertEqual(self.q.decode((directory / 'failed-outcome.json').read_bytes()), failure)
        self.assertEqual(self.q._PROBES, {})
        self.assertEqual(self.q._RECORDS, {})
        return failure

    def dummy_executable_view(self, *, control=False, profile='debug'):
        """Register bounded dummy bytes using a mocked fixed-authority boundary.

        These files are not real compiler executables and are never executed.
        """
        q = self.q
        invocation = object()
        run = self.root / 'view-run'
        toolchain = self.root / 'dummy-toolchain'
        targets = {name: self.file('dummy-toolchain/bin/' + name,
                                  ('nonexecutable fixture ' + name).encode())
                   for name in ('cargo', 'rustc')}
        view = run / 'parser' / (('build-control-' if control else 'build-') + profile) / 'rust-bin'
        view.mkdir(parents=True)
        try:
            for name, target in targets.items():
                (view / name).symlink_to(target)
        except (OSError, NotImplementedError):
            self.skipTest('symlink creation unavailable')
        captured = {'scope': 'parser', 'checkout': {'path': str(self.root / 'fixture-checkout')}}
        state = {'root': run, 'failed': False}
        authority_path = Path(captured['checkout']['path']) / 'tests/fixtures/typed_project_unit4_parser_portable/frozen/v3/authority.json'
        authority = q.serial({'compiler_files': [dict(q.identity(target), path='bin/' + name)
                                                for name, target in targets.items()]})
        actual_body, actual_sha = q.body, q.sha
        self.guard.enter_context(patch.object(q, '_run_state', return_value=({}, captured, state)))
        self.guard.enter_context(patch.object(q, 'body', side_effect=lambda path, *args:
            authority if Path(path) == authority_path else actual_body(path, *args)))
        self.guard.enter_context(patch.object(q, 'sha', side_effect=lambda raw:
            '02b72b3dcf45c695e5c523d71bb1c83e15c082556cf36029829fefc7a71571b0'
            if raw == authority else actual_sha(raw)))
        rows = q.register_executable_view(invocation, view, toolchain)
        return invocation, view, toolchain, targets, rows, captured, state

    def supervise(self, *, chunks=None, tree=None, timeout=10, limit=64,
                  samples=None, output=None, clock=None, preparation=False,
                  reader_class=InlineReader, operation_kind='probe', transient_roots=(),
                  argv=None, allowed_links=None, native_plan=None):
        q = self.q
        tree = tree or FakeTree()
        argv = ['/fixture/tool'] if argv is None else argv
        chunks = chunks or {101: [b'out', b''], 102: [b'err', b'']}
        queues = {key: list(value) for key, value in chunks.items()}
        def read(number, size):
            self.assertEqual(size, 65536)
            value = queues[number].pop(0)
            if isinstance(value, BaseException):
                raise value
            return value
        with ExitStack() as stack:
            stack.enter_context(patch.object(q.threading, 'Thread', reader_class))
            stack.enter_context(patch.object(q.os, 'read', side_effect=read))
            handlers = stack.enter_context(patch.object(q.signal, 'signal', return_value='old handler'))
            stack.enter_context(patch.object(q.sys, 'platform', 'linux'))
            stack.enter_context(patch.object(q, '_sample_linux', side_effect=samples) if samples is not None
                                else patch.object(q, '_sample_linux', return_value=[]))
            if clock is not None:
                stack.enter_context(patch.object(q.time, 'monotonic', side_effect=clock))
            result, streams = q._supervise(tree.factory, argv, str(self.root),
                {'PATH': '/fixture'}, timeout, limit, preparation=preparation,
                output_root=output, operation_kind=operation_kind, transient_roots=transient_roots,
                allowed_links=allowed_links,native_plan=native_plan)
            self.assertEqual(handlers.call_count, 4)
            self.assertEqual(handlers.call_args_list[-2].args, (q.signal.SIGINT, 'old handler'))
            self.assertEqual(handlers.call_args_list[-1].args, (q.signal.SIGTERM, 'old handler'))
        tree.factory.assert_called_once_with(argv, str(self.root),
                                            {'PATH': '/fixture'}, new_session=True)
        tree.close.assert_called_once_with()
        self.assertTrue(tree.child.stdout.closed)
        self.assertTrue(tree.child.stderr.closed)
        return result, streams, tree

    def test_exact_path_import_is_bytecode_disabled_and_spawn_free(self):
        self.assertEqual(self.q.__file__, str(HERE / 'preparation.py'))
        self.assertTrue(sys.dont_write_bytecode)
        self.assertEqual(self.q._PROBES, {})
        self.assertEqual(self.q._RECORDS, {})
        self.assertEqual(self.q._RUNS, {})

    def test_shared_manifest_has_exact_five_helper_closure(self):
        root, manifest = self.shared_fixture()
        self.assertEqual(set(self.q._manifest()[1]), self.q.FILES)
        (root / 'extra').write_bytes(b'extra')
        with self.assertRaises(self.q.Rejected):
            self.q._manifest()
        (root / 'extra').unlink()
        (root / 'test_preparation.py').unlink()
        with self.assertRaises((self.q.Rejected, FileNotFoundError)):
            self.q._manifest()

    def test_manifest_rejects_wrong_schema_duplicate_rows_and_extra_row_fields(self):
        root, manifest = self.shared_fixture()
        bad = [dict(manifest, schema='other'),
               dict(manifest, files=manifest['files'] + [manifest['files'][0]]),
               dict(manifest, files=[dict(manifest['files'][0], extra=True)] + manifest['files'][1:])]
        for value in bad:
            with self.subTest(value=value):
                (root / 'package-manifest.json').write_bytes(self.q.serial(value))
                with self.assertRaises(self.q.Rejected):
                    self.q._manifest()

    def test_manifest_rejects_helper_body_drift(self):
        root, _ = self.shared_fixture()
        (root / 'prepare_worker.py').write_bytes(b'drift')
        with self.assertRaisesRegex(self.q.Rejected, 'pin'):
            self.q._manifest()

    def test_manifest_requires_exact_canonical_producer_object(self):
        self.shared_fixture()
        other = types.ModuleType(self.q.KEY)
        other.__file__ = self.q.__file__
        other.__source_sha256__ = self.q.__source_sha256__
        with patch.dict(sys.modules, {self.q.KEY: other}):
            with self.assertRaises(self.q.Rejected):
                self.q._manifest()

    def test_manifest_rejects_canonical_owner_path_or_digest_drift(self):
        self.shared_fixture()
        for field, value in [('__file__', str(self.root / 'wrong.py')),
                             ('__source_sha256__', '0' * 64)]:
            other = types.SimpleNamespace(__file__=self.q.__file__,
                                          __source_sha256__=self.q.__source_sha256__)
            setattr(other, field, value)
            with patch.dict(sys.modules, {self.q.KEY: other}), self.assertRaises(self.q.Rejected):
                self.q._manifest()

    def test_exact_path_module_loader_pins_and_reuses_identity(self):
        path = self.file('pure-helper.py', b'VALUE = 41\n')
        name = '_test_preparation_pure_helper'
        with patch.dict(sys.modules):
            sys.modules.pop(name, None)
            module = self.q._module(path, name, self.q.sha(path.read_bytes()))
            self.assertEqual(module.VALUE, 41)
            self.assertIs(self.q._module(path, name, module.__source_sha256__), module)
            path.write_bytes(b'VALUE = 42\n')
            with self.assertRaises(self.q.Rejected):
                self.q._module(path, name, module.__source_sha256__)

    def test_module_loader_rejects_occupied_wrong_metadata(self):
        path = self.file('pure-helper.py', b'VALUE = 41\n')
        name = '_test_preparation_occupied_helper'
        for owner in [object(), types.SimpleNamespace(__file__=str(path), __source_sha256__='0' * 64)]:
            with patch.dict(sys.modules, {name: owner}), self.assertRaises(self.q.Rejected):
                self.q._module(path, name, self.q.sha(path.read_bytes()))

    def test_module_loader_rejects_matching_metadata_registry_substitution(self):
        path = self.file('pure-helper.py', b'VALUE = 41\n')
        name = '_test_preparation_substituted_helper'
        digest = self.q.sha(path.read_bytes())
        with patch.dict(sys.modules):
            sys.modules.pop(name, None)
            module = self.q._module(path, name, digest)
            substitute = types.ModuleType(name)
            substitute.__file__, substitute.__source_sha256__ = str(path), digest
            sys.modules[name] = substitute
            with self.assertRaisesRegex(self.q.Rejected, 'unissued cached helper'):
                self.q._module(path, name, digest)
            sys.modules[name] = module
            self.assertIs(self.q._module(path, name, digest), module)

    def test_preparation_first_context_bootstrap_is_pure_and_identity_stable(self):
        root, manifest = self.shared_fixture()
        path = root / 'context.py'
        path.write_bytes((HERE / 'context.py').read_bytes())
        manifest['files'] = [dict(self.q.identity(root / name), path=name)
                             for name in sorted(self.q.FILES)]
        (root / 'package-manifest.json').write_bytes(self.q.serial(manifest))
        with patch.dict(sys.modules):
            sys.modules.pop(self.q.CONTEXT_KEY, None)
            module = self.q.context_api()
            self.assertIs(sys.modules[self.q.CONTEXT_KEY], module)
            self.assertIs(self.q.context_api(), module)
            self.assertEqual(module.__file__, str(path))
            self.assertEqual(module.__source_sha256__, self.q.sha(path.read_bytes()))

    def test_module_loader_removes_failed_exec_registration(self):
        path = self.file('pure-helper.py', b'raise RuntimeError("fixture import failure")\n')
        name = '_test_preparation_failed_helper'
        with patch.dict(sys.modules):
            sys.modules.pop(name, None)
            with self.assertRaisesRegex(RuntimeError, 'fixture import failure'):
                self.q._module(path, name, self.q.sha(path.read_bytes()))
            self.assertNotIn(name, sys.modules)

    def test_record_constructors_cannot_issue_authority(self):
        for cls in (self.q.ProbeRecord, self.q.PreparationRecord):
            with self.subTest(cls=cls.__name__), self.assertRaises(self.q.Rejected):
                cls({'status': 'fixture'})
            with self.subTest(cls=cls.__name__), self.assertRaises(self.q.Rejected):
                cls({}, object())

    def test_records_are_immutable_and_documents_defensive(self):
        for cls in (self.q.ProbeRecord, self.q.PreparationRecord):
            record = cls({'nested': {'values': [1]}}, self.q._TOKEN)
            record.document()['nested']['values'].append(2)
            self.assertEqual(record.document(), {'nested': {'values': [1]}})
            with self.assertRaises(TypeError):
                record._document = b'{}'

    def test_forged_and_copied_probe_records_are_not_live_evidence(self):
        invocation, record, command = self.synthetic_probe()
        for forged in (object.__new__(self.q.ProbeRecord),
                       self.q.ProbeRecord(record.document(), self.q._TOKEN),
                       types.SimpleNamespace(document=record.document)):
            with self.subTest(forged=type(forged).__name__), self.assertRaises((self.q.Rejected, TypeError)):
                self.check_probe(invocation, forged, command)
        with self.assertRaises(self.q.Rejected):
            self.check_probe(object(), record, command)

    def test_forged_preparation_record_rejects_before_probe_or_report(self):
        for forged in (object.__new__(self.q.PreparationRecord),
                       self.q.PreparationRecord({'status': 'NotReady'}, self.q._TOKEN), object()):
            with patch.object(self.q, 'verify_probe', side_effect=AssertionError('must reject first')):
                with self.assertRaises(self.q.Rejected):
                    self.q.verify_record(forged, object())

    def test_probe_verifier_checks_exact_command_and_context(self):
        invocation, record, command = self.synthetic_probe()
        self.assertEqual(self.check_probe(invocation, record, command), record.document())
        changes = {'argv': ['/fixture/tool', '--other'], 'cwd': str(self.root / 'elsewhere'),
                   'environment': {'PATH': '/other'}, 'timeout_seconds': 2,
                   'stream_limit_bytes': 127, 'operation_kind': 'public-build'}
        for key, value in changes.items():
            with self.subTest(key=key), self.assertRaisesRegex(self.q.Rejected, 'command/context'):
                self.check_probe(invocation, record, dict(command, **{key: value}))

    def test_probe_result_and_command_bodies_are_rehashed(self):
        invocation, record, command = self.synthetic_probe()
        directory = self.q._PROBES[record]['directory']
        for name in ('command.json', 'result.json'):
            path = directory / name
            before = path.read_bytes()
            path.write_bytes(before + b' ')
            with self.subTest(name=name), self.assertRaisesRegex(self.q.Rejected, 'evidence/tool drift'):
                self.check_probe(invocation, record, command)
            path.write_bytes(before)

    def test_probe_tool_and_process_helper_bodies_are_rehashed(self):
        invocation, record, command = self.synthetic_probe()
        for key in ('executable', 'process_tree'):
            path = Path(record.document()[key]['path'])
            before = path.read_bytes()
            path.write_bytes(b'drift')
            with self.subTest(key=key), self.assertRaisesRegex(self.q.Rejected, 'evidence/tool drift'):
                self.check_probe(invocation, record, command)
            path.write_bytes(before)

    def test_probe_closed_evidence_directory_rejects_extra_member(self):
        invocation, record, command = self.synthetic_probe()
        (self.q._PROBES[record]['directory'] / 'extra').write_bytes(b'extra')
        with self.assertRaisesRegex(self.q.Rejected, 'closed operation'):
            self.check_probe(invocation, record, command)

    def test_probe_stream_identity_and_defensive_reads(self):
        invocation, record, command = self.synthetic_probe()
        self.assertEqual(record.stdout(), b'fixture output')
        self.assertEqual(record.stderr(), b'')
        Path(record.document()['stdout']['path']).write_bytes(b'drift')
        with self.assertRaisesRegex(self.q.Rejected, 'stream drift'):
            record.stdout()
        with self.assertRaisesRegex(self.q.Rejected, 'stream drift'):
            self.check_probe(invocation, record, command)

    def test_probe_execution_failure_flags_reject(self):
        invocation, original, command = self.synthetic_probe()
        owner = self.q._PROBES[original]
        changes = [('cleanup_complete', False), ('cleanup_error', 'fixture error'),
                   ('failure', 'fixture failure'), ('timed_out', True), ('stream_limit_exceeded', True)]
        for key, value in changes:
            record = self.q.ProbeRecord(dict(original.document(), **{key: value}), self.q._TOKEN)
            self.q._PROBES[record] = owner
            with self.subTest(key=key), self.assertRaisesRegex(self.q.Rejected, 'bounded completion'):
                self.check_probe(invocation, record, command)

    def test_probe_executable_route_retargeting_rejects(self):
        invocation, original, command = self.synthetic_probe()
        alias = self.root / 'tool-route'
        target = Path(original.document()['executable']['path'])
        try:
            alias.symlink_to(target)
        except (OSError, NotImplementedError):
            self.skipTest('symlink creation unavailable')
        document = original.document()
        document['executable_route'] = {'path': str(alias), 'target': str(target)}
        record = self.q.ProbeRecord(document, self.q._TOKEN)
        owner = dict(self.q._PROBES[original])
        result = owner['directory'] / 'result.json'
        result.write_bytes(self.q.serial(document))
        owner['result'] = self.q.identity(result)
        self.q._PROBES[record] = owner
        self.check_probe(invocation, record, command)
        other = self.file('other-tool', b'other fixture tool')
        alias.unlink()
        alias.symlink_to(other)
        with self.assertRaisesRegex(self.q.Rejected, 'route changed'):
            self.check_probe(invocation, record, command)

    def test_probe_optional_parser_helper_body_is_rehashed(self):
        invocation, original, command = self.synthetic_probe()
        helper = self.file('parser-helper-fixture.py', b'fixture only; never executed')
        document = dict(original.document(), helper=self.q.identity(helper))
        record = self.q.ProbeRecord(document, self.q._TOKEN)
        owner = dict(self.q._PROBES[original])
        result = owner['directory'] / 'result.json'
        result.write_bytes(self.q.serial(document))
        owner['result'] = self.q.identity(result)
        self.q._PROBES[record] = owner
        self.check_probe(invocation, record, command)
        helper.write_bytes(b'changed fixture')
        with self.assertRaisesRegex(self.q.Rejected, 'parser helper changed'):
            self.check_probe(invocation, record, command)

    def test_synthetic_preparation_verifier_returns_defensive_source_only_views(self):
        invocation, record = self.synthetic_preparation()
        verified = self.q.verify_record(record, invocation)
        self.assertEqual(verified['provenance']['status'], 'NotReady')
        self.assertIs(verified['provenance']['execution_qualified'], False)
        verified['config']['scope'] = 'changed'
        verified['plan']['fixture_only'].append('changed')
        again = self.q.verify_record(record, invocation)
        self.assertEqual(again['config']['scope'], 'public')
        self.assertEqual(len(again['plan']['fixture_only']), 1)

    def test_preparation_registry_requires_exact_invocation_identity(self):
        invocation, record = self.synthetic_preparation()
        with self.assertRaisesRegex(self.q.Rejected, 'live preparation producer'):
            self.q.verify_record(record, object())

    def test_preparation_config_and_report_evidence_are_rehashed(self):
        invocation, record = self.synthetic_preparation()
        rows = [self.q._RECORDS[record]['config_file'], record.document()['report']]
        for row in rows:
            path = Path(row['path'])
            raw = path.read_bytes()
            path.write_bytes(raw + b' ')
            with self.subTest(path=path.name), self.assertRaisesRegex(self.q.Rejected, 'preparation evidence changed'):
                self.q.verify_record(record, invocation)
            path.write_bytes(raw)

    def test_preparation_config_and_plan_associations_are_sealed(self):
        invocation, record = self.synthetic_preparation()
        stored = self.q._RECORDS[record]
        for key in ('config', 'plan'):
            saved = copy.deepcopy(stored[key])
            stored[key]['unexpected_fixture_field'] = True
            with self.subTest(key=key), self.assertRaisesRegex(self.q.Rejected, 'producer record association'):
                self.q.verify_record(record, invocation)
            stored[key] = saved

    def test_preparation_operation_must_equal_exact_registered_probe(self):
        invocation, original = self.synthetic_preparation()
        document = original.document()
        document['operation']['started_ns'] += 1
        record = self.q.PreparationRecord(document, self.q._TOKEN)
        self.q._RECORDS[record] = self.q._RECORDS[original]
        with self.assertRaisesRegex(self.q.Rejected, 'actual successful preparation record'):
            self.q.verify_record(record, invocation)

    def test_terminal_retention_success_requires_all_evidence_hashes(self):
        directory, result, streams, state = self.terminal_fixture()
        with patch.object(self.q, 'identity', wraps=self.q.identity) as identity:
            rows = self.q._retain_terminal(directory, result, streams, state)
        self.assertEqual([call.args[0].name for call in identity.call_args_list],
                         ['stdout', 'stderr', 'result.json', 'command.json'])
        self.assertEqual(rows, {'result': self.q.identity(directory / 'result.json'),
                                'command': self.q.identity(directory / 'command.json')})
        self.assertEqual(self.q.decode((directory / 'result.json').read_bytes()), result)
        for name, raw in streams.items():
            self.assertEqual((directory / name).read_bytes(), raw)
            self.assertEqual(result[name], self.q.identity(directory / name))
        self.assertIs(state['failed'], False)
        self.assertNotIn('failed_terminal', state)
        self.assertFalse((directory / 'failed-outcome.json').exists())
        self.assertEqual(self.q._PROBES, {})

    def test_terminal_deadline_exactly_reached_preserves_observed_outcome(self):
        directory, result, streams, state = self.terminal_fixture()
        with patch.object(self.q, '_DEADLINE', 10), \
             patch.object(self.q.time, 'monotonic', return_value=10):
            with self.assertRaisesRegex(self.q.Rejected, 'terminal evidence failure'):
                self.q._retain_terminal(directory, result, streams, state)
        failure = self.assert_failed_terminal(directory, result, streams, state)
        self.assertIn('preparation total deadline', failure['evidence_error'])
        self.assertNotIn('stdout', result)
        self.assertFalse((directory / 'result.json').exists())
        for name, raw in streams.items():
            self.assertEqual((directory / name).read_bytes(), raw)

    def test_terminal_any_required_hash_failure_poison_state_and_preserve_outcome(self):
        for failed_name in ('stdout', 'stderr', 'result.json', 'command.json'):
            directory, result, streams, state = self.terminal_fixture('hash-failure-' + failed_name)
            actual_identity = self.q.identity
            def identity(path):
                if Path(path).name == failed_name:
                    raise OSError('fixture hash failure: ' + failed_name)
                return actual_identity(path)
            with self.subTest(failed_name=failed_name), \
                 patch.object(self.q, 'identity', side_effect=identity):
                with self.assertRaisesRegex(self.q.Rejected, 'terminal evidence failure'):
                    self.q._retain_terminal(directory, result, streams, state)
            failure = self.assert_failed_terminal(directory, result, streams, state)
            self.assertIn('fixture hash failure: ' + failed_name, failure['evidence_error'])
            for name, raw in streams.items():
                self.assertEqual((directory / name).read_bytes(), raw)

    def test_terminal_partial_stream_write_is_preserved_without_overwrite(self):
        directory, result, streams, state = self.terminal_fixture()
        original_open = Path.open
        class PartialWriter:
            def __init__(self, path, *args, **kwargs):
                self.stream = original_open(path, *args, **kwargs)
            def __enter__(self): return self
            def __exit__(self, *args):
                self.stream.close()
                return False
            def write(self, raw):
                self.stream.write(raw[:3])
                raise OSError('fixture partial stream write')
        def opening(path, *args, **kwargs):
            if path == directory / 'stdout' and args and args[0] == 'xb':
                return PartialWriter(path, *args, **kwargs)
            return original_open(path, *args, **kwargs)
        with patch.object(Path, 'open', opening):
            with self.assertRaisesRegex(self.q.Rejected, 'terminal evidence failure'):
                self.q._retain_terminal(directory, result, streams, state)
        failure = self.assert_failed_terminal(directory, result, streams, state)
        self.assertIn('fixture partial stream write', failure['evidence_error'])
        self.assertEqual((directory / 'stdout').read_bytes(), streams['stdout'][:3])
        self.assertEqual((directory / 'stderr').read_bytes(), streams['stderr'])
        self.assertNotIn('stdout', result)

    def test_terminal_stream_write_failure_records_failed_preservation(self):
        directory, result, streams, state = self.terminal_fixture()
        original_open = Path.open
        def opening(path, *args, **kwargs):
            if path == directory / 'stderr' and args and args[0] == 'xb':
                raise OSError('fixture unwritable stderr')
            return original_open(path, *args, **kwargs)
        with patch.object(Path, 'open', opening):
            with self.assertRaisesRegex(self.q.Rejected, 'terminal evidence failure'):
                self.q._retain_terminal(directory, result, streams, state)
        failure = self.assert_failed_terminal(directory, result, streams, state)
        self.assertEqual([row['path'] for row in failure['preservation_errors']], [str(directory / 'stderr')])
        self.assertIn('fixture unwritable stderr', failure['preservation_errors'][0]['error'])
        self.assertEqual((directory / 'stdout').read_bytes(), streams['stdout'])
        self.assertFalse((directory / 'stderr').exists())

    def test_terminal_result_write_failure_preserves_streams_and_observed_outcome(self):
        directory, result, streams, state = self.terminal_fixture()
        original_open = Path.open
        def opening(path, *args, **kwargs):
            if path == directory / 'result.json' and args and args[0] == 'xb':
                raise OSError('fixture unwritable result')
            return original_open(path, *args, **kwargs)
        with patch.object(Path, 'open', opening):
            with self.assertRaisesRegex(self.q.Rejected, 'terminal evidence failure'):
                self.q._retain_terminal(directory, result, streams, state)
        failure = self.assert_failed_terminal(directory, result, streams, state)
        self.assertIn('fixture unwritable result', failure['evidence_error'])
        for name, raw in streams.items():
            self.assertEqual((directory / name).read_bytes(), raw)

    def test_emergency_metadata_write_failure_retains_observed_outcome_in_state(self):
        directory, result, streams, state = self.terminal_fixture()
        original_open = Path.open
        def opening(path, *args, **kwargs):
            if path == directory / 'failed-outcome.json':
                raise OSError('fixture unwritable failure metadata')
            return original_open(path, *args, **kwargs)
        with patch.object(Path, 'open', opening), \
             patch.object(self.q, 'identity', side_effect=AssertionError('emergency must not rehash')):
            with self.assertRaisesRegex(self.q.Rejected, 'terminal evidence failure') as caught:
                self.q._emergency_terminal(directory, result, streams, OSError('fixture capture error'), state)
        failure = self.assert_failed_terminal(directory, result, streams, state, on_disk=False)
        self.assertEqual([row['path'] for row in failure['preservation_errors']],
                         [str(directory / 'failed-outcome.json')])
        self.assertIn('fixture unwritable failure metadata', str(caught.exception))
        self.assertIn('424242', str(caught.exception))
        self.assertFalse((directory / 'failed-outcome.json').exists())

    def test_emergency_capture_grace_is_bounded_without_source_or_tool_hashing(self):
        directory, result, streams, state = self.terminal_fixture()
        with patch.object(self.q.time, 'monotonic', side_effect=[10, 12, 12, 12]), \
             patch.object(self.q, '_DEADLINE', 10), \
             patch.object(self.q, 'identity', side_effect=AssertionError('emergency must not rehash')):
            with self.assertRaisesRegex(self.q.Rejected, 'terminal evidence failure'):
                self.q._emergency_terminal(directory, result, streams, TimeoutError('fixture deadline'), state)
        failure = self.assert_failed_terminal(directory, result, streams, state, on_disk=False)
        self.assertEqual(len(failure['preservation_errors']), 3)
        self.assertTrue(all('emergency capture deadline' in row['error'] for row in failure['preservation_errors']))
        self.assertEqual({path.name for path in directory.iterdir()}, {'command.json'})

    def test_terminal_noncompletion_flags_never_return_successful_evidence(self):
        for field, value in [('failure', 'fixture cancellation'), ('cleanup_complete', False),
                             ('timed_out', True), ('stream_limit_exceeded', True)]:
            directory, result, streams, state = self.terminal_fixture('terminal-' + field)
            result[field] = value
            with self.subTest(field=field), self.assertRaisesRegex(self.q.Rejected, 'terminal evidence failure'):
                self.q._retain_terminal(directory, result, streams, state)
            self.assert_failed_terminal(directory, result, streams, state)

    def test_fake_supervisor_success_captures_streams_and_closes_tree(self):
        result, streams, tree = self.supervise()
        self.assertEqual(streams, {'stdout': b'out', 'stderr': b'err'})
        self.assertEqual(result['status'], 0)
        self.assertTrue(result['cleanup_complete'])
        self.assertIsNone(result['failure'])
        self.assertFalse(result['timed_out'])
        self.assertFalse(result['stream_limit_exceeded'])
        tree.child.kill.assert_not_called()

    def test_fake_supervisor_stream_limit_always_cleans_owned_tree(self):
        result, streams, _ = self.supervise(chunks={101: [b'abcdef', b''], 102: [b'']}, limit=3)
        self.assertTrue(result['stream_limit_exceeded'])
        self.assertEqual(streams['stdout'], b'abc')
        self.assertTrue(result['cleanup_complete'])

    def test_fake_supervisor_timeout_always_cleans_owned_tree(self):
        result, _, _ = self.supervise(timeout=1, clock=iter([0, 2, 2]).__next__)
        self.assertTrue(result['timed_out'])
        self.assertTrue(result['cleanup_complete'])

    def test_fake_supervisor_cancellation_always_cleans_owned_tree(self):
        tree = FakeTree()
        tree.child.poll.side_effect = self.q.Cancelled('fixture cancellation')
        result, _, _ = self.supervise(tree=tree)
        self.assertIn('Cancelled', result['failure'])
        self.assertTrue(result['cleanup_complete'])

    def test_fake_supervisor_reader_error_is_failure_with_cleanup(self):
        result, _, _ = self.supervise(chunks={101: [OSError('fixture read failure')], 102: [b'']})
        self.assertIn('stream reader failed', result['failure'])
        self.assertTrue(result['cleanup_complete'])

    def test_fake_supervisor_cleanup_error_never_reports_completion(self):
        tree = FakeTree()
        tree.close.side_effect = RuntimeError('fixture close failure')
        result, _, _ = self.supervise(tree=tree)
        self.assertFalse(result['cleanup_complete'])
        self.assertIn('fixture close failure', result['cleanup_error'])

    def test_fake_supervisor_unfinished_readers_fail_cleanup_with_bounded_grace(self):
        class UnfinishedReader(InlineReader):
            def is_alive(self):
                return True
        result, _, _ = self.supervise(timeout=1, clock=iter([0, 2, 2, 4]).__next__,
                                     reader_class=UnfinishedReader)
        self.assertTrue(result['timed_out'])
        self.assertFalse(result['cleanup_complete'])
        self.assertIn('reader cleanup incomplete', result['cleanup_error'])

    def test_fake_supervisor_wait_failure_kills_and_reaps(self):
        tree = FakeTree()
        tree.child.wait.side_effect = [0, TimeoutError('fixture wait'), -9]
        result, _, _ = self.supervise(tree=tree)
        tree.child.kill.assert_called_once_with()
        self.assertEqual(result['status'], -9)
        self.assertTrue(result['cleanup_complete'])

    def test_fake_supervisor_remaining_live_group_rejects_cleanup(self):
        result, _, _ = self.supervise(samples=lambda _: [{'pid': 424242, 'rss': 0, 'state': 'S'}])
        self.assertFalse(result['cleanup_complete'])
        self.assertIn('process group remains', result['cleanup_error'])

    def test_fake_supervisor_zombie_is_not_a_live_owned_group(self):
        result, _, _ = self.supervise(samples=lambda _: [{'pid': 424242, 'rss': 0, 'state': 'Z'}])
        self.assertTrue(result['cleanup_complete'])

    def test_fake_supervisor_process_and_rss_limits_fail_closed(self):
        cases = [[{'pid': i, 'rss': 0, 'state': 'S'} for i in range(5)],
                 [{'pid': 424242, 'rss': 2 * 1024 ** 3 + 1, 'state': 'S'}]]
        for members in cases:
            with self.subTest(members=members):
                result, _, _ = self.supervise(preparation=True, samples=[members, []])
                self.assertIn('process/RSS bound', result['failure'])
                self.assertTrue(result['cleanup_complete'])

    def test_only_source_authenticated_direct_public_compile_gets_five_processes(self):
        plan={'source':{'sha256':self.q.NATIVE_SOURCE_SHA}}
        self.assertEqual(self.q._operation_process_cap('public-runtime',False,plan),5)
        for kind in ('probe','public-native','public-hosted-worker','generator','parser-collect','parser-phase'):
            self.assertEqual(self.q._operation_process_cap(kind,False,None),4)
        self.assertEqual(self.q._operation_process_cap('public-runtime',False,None),4)
        self.assertEqual(self.q._operation_process_cap('public-runtime',True,plan),4)
        with self.assertRaises(self.q.Rejected):self.q._operation_process_cap('public-runtime',False,{'source':{'sha256':'0'*64}})

    def test_exact_five_processes_accepted_and_six_rejected_under_native_policy(self):
        plan={'source':{'sha256':self.q.NATIVE_SOURCE_SHA}}
        for count in (5,6):
            members=[{'pid':i,'rss':0,'state':'S'} for i in range(count)]
            samples=iter([members])
            with patch.object(self.q,'_native_register',return_value=None),patch.object(self.q,'_native_terminal'):
                result,_,_=self.supervise(operation_kind='public-runtime',native_plan=plan,samples=lambda _:next(samples,[]))
            self.assertEqual(result['operation_policy']['processes'],5)
            self.assertEqual(result['operation_policy']['rss_bytes'],2*1024**3)
            self.assertEqual(result['failure'] is None,count==5)
            self.assertTrue(result['cleanup_complete'])

    def test_fake_supervisor_records_finite_operation_specific_limits(self):
        for kind in ('public-build', 'parser-build', 'parser-helper'):
            with self.subTest(kind=kind):
                result, _, _ = self.supervise(operation_kind=kind)
                policy = result['operation_policy']
                self.assertEqual(policy['processes'], 16)
                self.assertEqual(policy['rss_bytes'], 8 * 1024 ** 3)
                self.assertEqual(policy['output_bytes'], 6 * 1024 ** 3)
                self.assertEqual(policy['file_bytes'], 2 * 1024 ** 3)
        result, _, _ = self.supervise(preparation=True)
        self.assertEqual(result['operation_policy']['processes'], 4)
        self.assertEqual(result['operation_policy']['rss_bytes'], 2 * 1024 ** 3)
        self.assertEqual(result['preparation_resource_policy']['linux_child_cpu_seconds'], 120)
        self.assertEqual(result['preparation_resource_policy']['linux_child_address_space_bytes'], 1024 ** 3)

    def test_fake_supervisor_disappearance_allowed_only_under_dedicated_tmp(self):
        for relative, allowed in [('tmp/gone', True), ('tmp/nested/gone', True),
                                  ('source-preparation/gone', False), ('tmp-extra/gone', False),
                                  ('source-preparation/tmp/gone', False)]:
            output = FakeOutputRoot(self.root, [self.root / relative])
            with self.subTest(relative=relative):
                result, _, _ = self.supervise(preparation=True, output=output)
                if allowed:
                    self.assertIsNone(result['failure'])
                else:
                    self.assertIn('vanished stable output', result['failure'])
                self.assertTrue(result['cleanup_complete'])

    def test_fake_supervisor_only_allows_disappearance_below_exact_transient_target(self):
        target = self.root / 'targets/public-ordinary'
        cases = [(target / 'debug/incremental/gone', True),
                 (target, False), (target.parent, False),
                 (self.root / 'targets/public-lifecycle/debug/gone', False),
                 (self.root / 'operations/0001/result.json', False),
                 (self.root / 'source-preparation/public/ordinary/source.rs', False),
                 (self.root / 'home/gone', False), (self.root / 'parser/source/gone', False)]
        for path, allowed in cases:
            with self.subTest(path=path):
                result, _, _ = self.supervise(operation_kind='public-build',
                    transient_roots=(target,), output=FakeOutputRoot(self.root, [path]))
                self.assertEqual(result['operation_policy']['transient_roots'], [str(target)])
                if allowed:
                    self.assertIsNone(result['failure'])
                else:
                    self.assertIn('vanished stable output', result['failure'])
                self.assertTrue(result['cleanup_complete'])

    def test_fake_supervisor_transient_special_member_still_rejects(self):
        target = self.root / 'targets/public-ordinary'
        target.mkdir(parents=True)
        link = target / 'special-member'
        try:
            link.symlink_to(self.root / 'missing-target')
        except (OSError, NotImplementedError):
            self.skipTest('symlink creation unavailable')
        result, _, _ = self.supervise(operation_kind='public-build', transient_roots=(target,),
            output=FakeOutputRoot(self.root, [link]))
        self.assertIn('unregistered output link', result['failure'])
        self.assertTrue(result['cleanup_complete'])

    def test_dummy_executable_view_registers_exact_pair_and_returns_defensive_rows(self):
        invocation, view, toolchain, targets, rows, _, _ = self.dummy_executable_view()
        self.assertEqual({Path(row['path']).name for row in rows}, {'cargo', 'rustc'})
        for row in rows:
            self.assertEqual(row['target_identity'], self.q.identity(row['target']))
            self.assertEqual(row['target_stat'], self.q._target_stat(row['target']))
        self.assertEqual(self.q._verify_view_rows(rows), {row['path']: row for row in rows})
        rows[0]['target_identity']['sha256'] = 'changed fixture copy'
        again = self.q.register_executable_view(invocation, view, toolchain)
        self.assertNotEqual(again[0]['target_identity']['sha256'], 'changed fixture copy')
        self.assertEqual(set(self.q._VIEWS[invocation]), {str(view / 'cargo'), str(view / 'rustc')})

    def test_dummy_executable_view_accepts_only_four_fixed_build_locations(self):
        invocation, original, toolchain, targets, _, _, state = self.dummy_executable_view()
        for control in (False, True):
            for profile in ('debug', 'release'):
                view = state['root'] / 'parser' / (('build-control-' if control else 'build-') + profile) / 'rust-bin'
                if view != original:
                    view.mkdir(parents=True)
                    for name, target in targets.items():
                        (view / name).symlink_to(target)
                with self.subTest(view=view):
                    self.assertEqual(len(self.q.register_executable_view(invocation, view, toolchain)), 2)
        self.assertEqual(len(self.q._VIEWS[invocation]), 8)
        outside = state['root'] / 'parser/source/rust-bin'
        outside.mkdir(parents=True)
        for name, target in targets.items():
            (outside / name).symlink_to(target)
        with self.assertRaisesRegex(self.q.Rejected, 'fixed parser executable view location'):
            self.q.register_executable_view(invocation, outside, toolchain)

    def test_dummy_executable_view_rejects_nonparser_scope(self):
        invocation, view, toolchain, _, _, captured, state = self.dummy_executable_view()
        captured = dict(captured, scope='public')
        with patch.object(self.q, '_run_state', return_value=({}, captured, state)), \
             self.assertRaisesRegex(self.q.Rejected, 'parser view scope'):
            self.q.register_executable_view(invocation, view, toolchain)

    def test_dummy_executable_view_requires_fixed_authority_body_pin(self):
        invocation, view, toolchain, _, _, _, _ = self.dummy_executable_view()
        with patch.object(self.q, 'sha', return_value='0' * 64), \
             self.assertRaisesRegex(self.q.Rejected, 'fixed compiler pair authority'):
            self.q.register_executable_view(invocation, view, toolchain)

    def test_dummy_executable_view_rejects_compiler_body_outside_mock_authority(self):
        invocation, view, toolchain, targets, _, _, _ = self.dummy_executable_view()
        targets['cargo'].write_bytes(b'changed dummy bytes outside mocked fixed authority')
        with self.assertRaisesRegex(self.q.Rejected, 'fixed original compiler body'):
            self.q.register_executable_view(invocation, view, toolchain)

    def test_registered_view_rejects_extra_entry_or_missing_pair_member(self):
        invocation, view, toolchain, _, rows, _, _ = self.dummy_executable_view()
        extra = view / 'extra'
        extra.write_bytes(b'fixture extra entry')
        with self.assertRaisesRegex(self.q.Rejected, 'closed registered executable view'):
            self.q._verify_view_rows(rows)
        with self.assertRaisesRegex(self.q.Rejected, 'exact two executable links'):
            self.q.register_executable_view(invocation, view, toolchain)
        extra.unlink()
        with self.assertRaisesRegex(self.q.Rejected, 'closed registered executable view'):
            self.q._verify_view_rows(rows[:1])
        (view / 'rustc').unlink()
        with self.assertRaisesRegex(self.q.Rejected, 'exact two executable links'):
            self.q.register_executable_view(invocation, view, toolchain)

    def test_registered_link_rejects_changed_target_and_equivalent_relative_spelling(self):
        _, view, _, targets, rows, _, _ = self.dummy_executable_view()
        row = next(row for row in rows if Path(row['path']).name == 'cargo')
        path = view / 'cargo'
        path.unlink()
        path.symlink_to(targets['rustc'])
        with self.assertRaisesRegex(self.q.Rejected, 'registered executable link drift'):
            self.q._verify_link(row)
        path.unlink()
        path.symlink_to(os.path.relpath(targets['cargo'], view))
        self.assertEqual(path.resolve(), targets['cargo'])
        with self.assertRaisesRegex(self.q.Rejected, 'registered executable link drift'):
            self.q._verify_link(row)

    def test_registered_link_rejects_regular_file_substitution(self):
        _, view, _, _, rows, _, _ = self.dummy_executable_view()
        path = view / 'cargo'
        row = next(row for row in rows if row['path'] == str(path))
        path.unlink()
        path.write_bytes(b'fixture file substituted for exact link')
        with self.assertRaisesRegex(self.q.Rejected, 'registered executable link drift'):
            self.q._verify_link(row)

    def test_registered_target_stat_drift_rejects_before_body_read(self):
        _, _, _, targets, rows, _, _ = self.dummy_executable_view()
        row = next(row for row in rows if row['target'] == str(targets['cargo']))
        targets['cargo'].write_bytes(b'changed fixture body and size')
        with patch.object(self.q, 'identity', side_effect=AssertionError('must reject stat first')), \
             self.assertRaisesRegex(self.q.Rejected, 'registered target stat drift'):
            self.q._verify_link(row)

    def test_registered_target_body_is_hashed_independently_of_mocked_stat(self):
        _, _, _, targets, rows, _, _ = self.dummy_executable_view()
        row = next(row for row in rows if row['target'] == str(targets['cargo']))
        targets['cargo'].write_bytes(b'changed fixture body')
        with patch.object(self.q, '_target_stat', return_value=row['target_stat']), \
             self.assertRaisesRegex(self.q.Rejected, 'registered executable body drift'):
            self.q._verify_link(row)

    def test_fake_supervisor_accepts_only_exact_registered_dummy_links(self):
        _, view, _, _, rows, _, _ = self.dummy_executable_view()
        allowed = self.q._verify_view_rows(rows)
        with patch.object(self.q, 'identity', side_effect=AssertionError('live scan must use pinned target stats')):
            result, _, _ = self.supervise(operation_kind='parser-helper', allowed_links=allowed,
                output=FakeOutputRoot(self.root, [view / 'cargo', view / 'rustc']))
        self.assertIsNone(result['failure'])
        self.assertTrue(result['cleanup_complete'])
        result, _, _ = self.supervise(operation_kind='parser-helper',
            output=FakeOutputRoot(self.root, [view / 'cargo']))
        self.assertIn('unregistered output link', result['failure'])
        self.assertTrue(result['cleanup_complete'])

    def test_fake_supervisor_rejects_unknown_or_retargeted_link_despite_other_registered_rows(self):
        _, view, _, targets, rows, _, _ = self.dummy_executable_view()
        allowed = self.q._verify_view_rows(rows)
        unknown = self.root / 'unregistered-cargo'
        unknown.symlink_to(targets['cargo'])
        result, _, _ = self.supervise(allowed_links=allowed,
            output=FakeOutputRoot(self.root, [unknown]))
        self.assertIn('unregistered output link', result['failure'])
        path = view / 'cargo'
        path.unlink()
        path.symlink_to(targets['rustc'])
        result, _, _ = self.supervise(allowed_links=allowed,
            output=FakeOutputRoot(self.root, [path]))
        self.assertIn('registered executable link drift', result['failure'])
        self.assertTrue(result['cleanup_complete'])

    def test_fake_supervisor_parser_helper_target_disappearance_is_narrow(self):
        result_root = self.root / 'parser/build-debug/result'
        target = result_root / 'target'
        for path, allowed in ((target / 'debug/gone', True), (target, False),
                              (result_root / 'stdout.jsonl', False),
                              (result_root / 'stderr.txt', False),
                              (self.root / 'parser/source/gone', False)):
            with self.subTest(path=path):
                result, _, _ = self.supervise(operation_kind='parser-helper',
                    argv=['/fixture/python', '--output', str(result_root)],
                    transient_roots=(target,), output=FakeOutputRoot(self.root, [path]))
                if allowed:
                    self.assertIsNone(result['failure'])
                else:
                    self.assertIn('vanished stable output', result['failure'])
                self.assertTrue(result['cleanup_complete'])

    def test_fake_supervisor_file_and_total_output_bounds(self):
        def member(size, mode=stat.S_IFREG):
            return types.SimpleNamespace(lstat=lambda: types.SimpleNamespace(st_mode=mode, st_size=size))
        cases = [([member(64 * 1024 ** 2 + 1)], 'output file bound'),
                 ([member(64 * 1024 ** 2)] * 3, 'output byte bound'),
                 ([member(0, stat.S_IFIFO)], 'special output member')]
        for members, expected in cases:
            with self.subTest(expected=expected):
                result, _, _ = self.supervise(preparation=True, output=FakeOutputRoot(self.root, members))
                self.assertIn(expected, result['failure'])
                self.assertTrue(result['cleanup_complete'])

    def test_snapshot_deduplicates_exact_input_files_and_preserves_empty_directories(self):
        root = self.root / 'source'
        (root / 'empty').mkdir(parents=True)
        path = self.file('source/code', b'code')
        rows, closures = self.q._snapshot_files([path, path], [root, root])
        self.assertEqual(rows, [self.q.identity(path)])
        self.assertEqual(closures, [{'path': str(root), 'files': ['code'], 'directories': ['empty']}])
        self.assertTrue(self.q._verify_inputs({'input_files': rows, 'input_routes': [],
            'absent_paths': [], 'closed_roots': closures}))

    def test_snapshot_input_count_and_total_bytes_are_bounded(self):
        cases = [(5001, 1), (1, 256 * 1024 ** 2 + 1)]
        for count, size in cases:
            paths = [self.root / ('fixture-%05d' % i) for i in range(count)]
            def row(path):
                return {'path': str(path), 'bytes': size, 'sha256': 'a' * 64}
            with self.subTest(count=count, size=size), \
                 patch.object(self.q, 'real', side_effect=Path), \
                 patch.object(self.q, 'identity', side_effect=row), \
                 self.assertRaisesRegex(self.q.Rejected, 'finite preparation input closure'):
                self.q._snapshot_files(paths, [])

    def input_config(self):
        root = self.root / 'source'
        root.mkdir()
        path = self.file('source/code', b'original')
        rows, closures = self.q._snapshot_files([], [root])
        return {'input_files': rows, 'input_routes': [], 'absent_paths': [], 'closed_roots': closures}, path

    def test_input_body_drift_rejects(self):
        config, path = self.input_config()
        path.write_bytes(b'changed!')
        with self.assertRaisesRegex(self.q.Rejected, 'input drift'):
            self.q._verify_inputs(config)

    def test_input_closure_rejects_extra_file_and_empty_directory(self):
        config, path = self.input_config()
        extra = path.parent / 'extra'
        for directory in (False, True):
            if directory:
                extra.mkdir()
            else:
                extra.write_bytes(b'extra')
            with self.subTest(directory=directory), self.assertRaisesRegex(self.q.Rejected, 'closure drift'):
                self.q._verify_inputs(config)
            extra.rmdir() if directory else extra.unlink()

    def test_tool_route_drift_rejects(self):
        config, path = self.input_config()
        config['input_routes'] = [{'path': str(path), 'target': str(path.parent / 'other')}]
        with self.assertRaisesRegex(self.q.Rejected, 'tool route drift'):
            self.q._verify_inputs(config)

    def test_absent_configuration_rejects_file_and_broken_symlink(self):
        config, _ = self.input_config()
        path = self.root / 'unexpected-config'
        config['absent_paths'] = [str(path)]
        self.assertTrue(self.q._verify_inputs(config))
        path.write_bytes(b'config')
        with self.assertRaisesRegex(self.q.Rejected, 'unexpected preparation configuration'):
            self.q._verify_inputs(config)
        path.unlink()
        try:
            path.symlink_to(self.root / 'missing-target')
        except (OSError, NotImplementedError):
            self.skipTest('symlink creation unavailable')
        with self.assertRaisesRegex(self.q.Rejected, 'unexpected preparation configuration'):
            self.q._verify_inputs(config)

    def test_symlink_input_rejects(self):
        path = self.file('source', b'source')
        alias = self.root / 'alias'
        try:
            alias.symlink_to(path)
        except (OSError, NotImplementedError):
            self.skipTest('symlink creation unavailable')
        with self.assertRaises(self.q.Rejected):
            self.q.identity(alias)

    @unittest.skipUnless(hasattr(os, 'mkfifo'), 'FIFO is POSIX-specific')
    def test_special_input_rejects_before_read(self):
        root = self.root / 'special'
        root.mkdir()
        os.mkfifo(root / 'pipe')
        with self.assertRaisesRegex(self.q.Rejected, 'special preserved member'):
            self.q._snapshot_files([], [root])

    def test_body_rejects_oversize_before_open(self):
        path = self.file('bounded', b'1234')
        with patch.object(Path, 'open', side_effect=AssertionError('must reject before read')):
            with self.assertRaisesRegex(self.q.Rejected, 'regular bounded body'):
                self.q.body(path, 3)

    def test_body_checks_deadline_between_bounded_reads(self):
        path = self.file('chunked', b'x' * (2 * 1024 ** 2 + 1))
        stream = io.BytesIO(path.read_bytes())
        reads = []
        class Reader:
            def __enter__(self): return self
            def __exit__(self, *args): return False
            def read(self, size):
                reads.append(size)
                return stream.read(size)
        with patch.object(self.q, '_DEADLINE', 1), \
             patch.object(self.q.time, 'monotonic', side_effect=[0, 0, 2]), \
             patch.object(Path, 'open', return_value=Reader()):
            with self.assertRaisesRegex(self.q.Rejected, 'deadline'):
                self.q.body(path)
        self.assertEqual(reads, [1024 ** 2])

    def test_hash_checks_deadline_between_bounded_chunks(self):
        raw = b'x' * (2 * 1024 ** 2 + 1)
        with patch.object(self.q, '_DEADLINE', 1), \
             patch.object(self.q.time, 'monotonic', side_effect=[0, 2]):
            with self.assertRaisesRegex(self.q.Rejected, 'deadline'):
                self.q.sha(raw)

    def test_chunked_hash_matches_standard_digest_including_empty_body(self):
        for raw in (b'', b'small', b'x' * (1024 ** 2 + 1)):
            with self.subTest(length=len(raw)):
                self.assertEqual(self.q.sha(raw), hashlib.sha256(raw).hexdigest())
        with patch.object(self.q, '_DEADLINE', 1), \
             patch.object(self.q.time, 'monotonic', return_value=2):
            with self.assertRaisesRegex(self.q.Rejected, 'deadline'):
                self.q.sha(b'')

    def test_body_rejects_growth_during_read_without_consuming_more(self):
        path = self.file('growth', b'x')
        stream = Mock()
        stream.read.side_effect = [b'xx', AssertionError('must not continue growing read')]
        manager = Mock()
        manager.__enter__ = Mock(return_value=stream)
        manager.__exit__ = Mock(return_value=False)
        with patch.object(Path, 'open', return_value=manager):
            with self.assertRaisesRegex(self.q.Rejected, 'growing bounded input'):
                self.q.body(path)
        stream.read.assert_called_once_with(1024 ** 2)

    def test_body_rejects_shrink_during_read(self):
        path = self.file('shrink', b'xx')
        with patch.object(Path, 'open', return_value=io.BytesIO(b'x')):
            with self.assertRaisesRegex(self.q.Rejected, 'body changed'):
                self.q.body(path)

    def test_scope_and_host_rejections_happen_before_any_probe(self):
        q = self.q
        state = {'root': self.root}
        worker = types.SimpleNamespace(SCOPES={'public': (), 'parser': (), 'combined': ()},
            REQUIRED_HOSTS={('linux', 'x86_64'), ('macos', 'aarch64')}, measured_host=Mock())
        cases = [('unknown', 'unknown', ('linux', 'x86_64'), 'exact preparation scope'),
                 ('public', 'parser', ('linux', 'x86_64'), 'captured scope mismatch'),
                 ('public', 'public', ('other', 'other'), 'qualified host'),
                 ('parser', 'parser', ('macos', 'aarch64'), 'parser host applicability')]
        for scope, captured_scope, host, reason in cases:
            captured = {'scope': captured_scope, 'checkout': {'path': str(self.root)}}
            worker.measured_host.return_value = {'os': host[0], 'architecture': host[1]}
            with self.subTest(scope=scope, reason=reason), \
                 patch.object(q, '_run_state', return_value=({}, captured, state)), \
                 patch.object(q, '_worker', return_value=worker), \
                 self.assertRaisesRegex(q.Rejected, reason):
                q._config(object(), scope, None, None)
        q.owned_probe.assert_not_called()

    def test_operation_kind_is_closed(self):
        for kind in ('build', 'PUBLIC-BUILD', '', None):
            with self.subTest(kind=kind), self.assertRaisesRegex(self.q.Rejected, 'fixed operation kind'):
                self.q._operation_recipe(kind, ['fixture'], self.root / 'tool', self.root)

    def test_operation_recipe_rejects_relative_or_traversal_route(self):
        for route in (Path('relative-tool'), self.root / '../outside-tool'):
            with self.subTest(route=route), self.assertRaisesRegex(self.q.Rejected, 'normalized executable route'):
                self.q._operation_recipe('probe', ['fixture'], route, self.root)

    def test_public_build_recipe_allows_only_exact_debug_and_release_vectors(self):
        vector = ['cargo', 'build', '--bin', 'oxid', '--locked', '--offline']
        for suffix in ([], ['--release']):
            self.assertTrue(self.q._operation_recipe('public-build', vector + suffix,
                                                     self.root / 'tool', self.root))
        variants = [vector[:-1], vector + ['--features', 'extra'],
                    ['/usr/bin/cargo'] + vector[1:], vector + ['--release', '--release'],
                    ['cargo', 'build', '--offline', '--locked', '--bin', 'oxid'],
                    vector[:-2] + ['--offline', '--locked']]
        for argv in variants:
            with self.subTest(argv=argv), self.assertRaisesRegex(self.q.Rejected, 'exact public build recipe'):
                self.q._operation_recipe('public-build', argv, self.root / 'tool', self.root)

    def test_parser_build_recipe_requires_exact_vector_and_owned_tool_route(self):
        route = self.root / 'owned/tool'
        tail = ['test', '--offline', '--locked', '--no-run', '--bin', 'oxid', '--jobs', '2',
                '--target', 'x86_64-unknown-linux-gnu', '--message-format=json']
        for suffix in ([], ['--release']):
            self.assertTrue(self.q._operation_recipe('parser-build', [str(route)] + tail + suffix,
                                                     route, self.root))
        variants = [[str(route)] + tail + ['--features', 'extra'],
                    [str(route)] + [arg for arg in tail if arg != '--no-run'],
                    [str(route)] + [arg if arg != '2' else '8' for arg in tail],
                    [str(route)] + tail + ['--release', '--release']]
        for argv in variants:
            with self.subTest(argv=argv), self.assertRaisesRegex(self.q.Rejected, 'exact parser build recipe'):
                self.q._operation_recipe('parser-build', argv, route, self.root)
        for outside in (self.root, self.root.parent / 'outside-tool',
                        self.root / '../outside-tool',
                        self.root.with_name(self.root.name + '-sibling') / 'tool'):
            with self.subTest(route=outside), self.assertRaises(self.q.Rejected):
                self.q._operation_recipe('parser-build', [str(outside)] + tail, outside, self.root)

    def test_parser_helper_recipe_binds_role_profile_and_output(self):
        q = self.q
        root = self.root / 'parser'
        route = Path(sys.executable)
        vectors = []
        for control in (False, True):
            for profile in ('debug', 'release'):
                argv = [sys.executable, '-B', str(root / 'helpers/build.py'), '--overlay',
                        str(root / ('control-source' if control else 'source') / 'overlay-manifest.json'),
                        '--profile', profile, '--output',
                        str(root / (('build-control-' if control else 'build-') + profile) / 'result')]
                vectors.append(argv)
                self.assertTrue(q._operation_recipe('parser-helper', argv, route, self.root))
        argv = vectors[0]
        changes = {0: '/wrong/python', 1: '-E', 2: str(root / 'helpers/other.py'),
                   3: '--source', 4: str(root / 'other-source/overlay-manifest.json'),
                   5: '--mode', 6: 'development', 7: '--out',
                   8: str(root / 'build-release/result')}
        for index, value in changes.items():
            changed = list(argv)
            changed[index] = value
            with self.subTest(index=index), self.assertRaises(q.Rejected):
                q._operation_recipe('parser-helper', changed, route, self.root)
        for changed in (argv[:-1], argv + ['--extra'],
                        argv[:4] + [str(root / 'control-source/overlay-manifest.json')] + argv[5:],
                        argv[:8] + [str(root / 'build-control-debug/result')]):
            with self.subTest(argv=changed), self.assertRaises(q.Rejected):
                q._operation_recipe('parser-helper', changed, route, self.root)

    def test_public_transient_target_is_fixed_per_exact_prepared_source_role(self):
        argv = ['cargo', 'build', '--bin', 'oxid', '--locked', '--offline']
        for role in ('ordinary', 'lifecycle'):
            cwd = self.root / 'source-preparation/public' / role
            target = self.root / 'targets' / ('public-' + role)
            environment = {'CARGO_TARGET_DIR': str(target)}
            with self.subTest(role=role):
                self.assertEqual(self.q._transient_target('public-build', argv, cwd, environment, self.root), target)
        cwd = self.root / 'source-preparation/public/ordinary'
        forbidden = [cwd, cwd.parent, self.root, self.root.parent, self.root / 'operations',
                     self.root / 'home', self.root / 'tmp', self.root / 'parser/source',
                     self.root / 'targets', self.root / 'targets/public-lifecycle',
                     self.root / 'targets/public-ordinary/../public-ordinary']
        for target in forbidden:
            with self.subTest(target=target), self.assertRaisesRegex(self.q.Rejected, 'fixed owned Cargo target'):
                self.q._transient_target('public-build', argv, cwd,
                                        {'CARGO_TARGET_DIR': str(target)}, self.root)
        with self.assertRaisesRegex(self.q.Rejected, 'fixed owned Cargo target'):
            self.q._transient_target('public-build', argv, cwd, {}, self.root)

    def test_public_transient_target_rejects_nonexact_source_cwd(self):
        target = self.root / 'targets/public-ordinary'
        for cwd in (self.root, self.root / 'source-preparation/public',
                    self.root / 'source-preparation/public/ordinary/src',
                    self.root / 'parser/source', self.root / 'home', self.root / 'tmp'):
            with self.subTest(cwd=cwd), self.assertRaisesRegex(self.q.Rejected, 'exact prepared source cwd'):
                self.q._transient_target('public-build', [], cwd,
                                        {'CARGO_TARGET_DIR': str(target)}, self.root)

    def test_parser_build_transient_targets_bind_source_role_and_profile(self):
        for control in (False, True):
            cwd = self.root / 'parser' / ('control-source' if control else 'source')
            for profile in ('debug', 'release'):
                argv = ['fixture', 'test'] + (['--release'] if profile == 'release' else [])
                target = self.root / 'parser-targets' / ('control-' if control else 'observed-') / profile
                with self.subTest(control=control, profile=profile):
                    self.assertEqual(self.q._transient_target('parser-build', argv, cwd,
                        {'CARGO_TARGET_DIR': str(target)}, self.root), target)
                for forbidden in (cwd, cwd.parent, self.root, self.root / 'operations',
                                  self.root / 'home', self.root / 'tmp', self.root / 'parser-targets'):
                    with self.subTest(target=forbidden), self.assertRaises(self.q.Rejected):
                        self.q._transient_target('parser-build', argv, cwd,
                            {'CARGO_TARGET_DIR': str(forbidden)}, self.root)

    def test_parser_helper_transient_target_follows_validated_role_and_profile(self):
        root = self.root / 'parser'
        for control in (False, True):
            for profile in ('debug', 'release'):
                result = root / (('build-control-' if control else 'build-') + profile) / 'result'
                argv = [sys.executable, '-B', str(root / 'helpers/build.py'), '--overlay',
                        str(root / ('control-source' if control else 'source') / 'overlay-manifest.json'),
                        '--profile', profile, '--output', str(result)]
                self.q._operation_recipe('parser-helper', argv, Path(sys.executable), self.root)
                self.assertEqual(self.q._transient_target('parser-helper', argv, root,
                    {'CARGO_TARGET_DIR': str(self.root)}, self.root), result / 'target')
        self.assertIsNone(self.q._transient_target('probe', [], self.root,
                                                 {'CARGO_TARGET_DIR': str(self.root)}, self.root))

    def test_limit_launcher_declares_cpu_address_space_and_file_size_bounds(self):
        """Inspect source only: do not execute resource changes or runpy."""
        tree = ast.parse(self.q._LIMIT_LAUNCHER)
        calls = [node for node in ast.walk(tree) if isinstance(node, ast.Call)
                 and isinstance(node.func, ast.Attribute) and node.func.attr == 'setrlimit']
        actual = {call.args[0].attr: ast.literal_eval(call.args[1]) for call in calls}
        self.assertEqual(actual, {'RLIMIT_CPU': (120, 120),
                                 'RLIMIT_AS': (1073741824, 1073741824),
                                 'RLIMIT_FSIZE': (67108864, 67108864)})

    def test_public_worker_window_rejects_timestamp_outside_operation(self):
        for value in (9, 21):
            with self.subTest(timestamp=value), self.assertRaisesRegex(self.q.Rejected, 'outside actual worker'):
                self.q._windows({'scope': 'public'}, {}, {'started_ns': 10, 'completed_ns': 20},
                                {'genuine_public_prepared_ns': value})

    def test_historical_worker_windows_bind_expected_host_tools_and_environment(self):
        q = self.q
        output = self.root / 'historical-window-fixture'
        expected = {'child_environment': {'PATH': '/fixture'},
                    'child_tools': {'python': 'fixture-only'},
                    'child_host': {'os': 'linux', 'architecture': 'x86_64'},
                    'host': {'os': 'linux', 'architecture': 'intentionally-distinct-fixture'}}
        child = {'started_ns': 11, 'finished_ns': 16,
                 'environment': expected['child_environment'],
                 'tools': expected['child_tools'], 'host': expected['child_host']}
        self.file('historical-window-fixture/predecessor/session.json', q.serial({'prepared_ns': 18}))
        for name in ('prepare', 'prepare-control'):
            self.file('historical-window-fixture/predecessor/' + name + '/invocation.json', q.serial(child))
        config, operation = {'scope': 'parser', 'output': str(output)}, {'started_ns': 10, 'completed_ns': 20}
        q._windows(config, expected, operation, {})
        path = output / 'predecessor/prepare-control/invocation.json'
        for key in ('environment', 'tools', 'host'):
            path.write_bytes(q.serial(dict(child, **{key: {'drift': 'fixture'}})))
            with self.subTest(key=key), self.assertRaisesRegex(q.Rejected, 'producer-captured context'):
                q._windows(config, expected, operation, {})
        path.write_bytes(q.serial(dict(child, started_ns=9)))
        with self.assertRaisesRegex(q.Rejected, 'historical child outside actual worker'):
            q._windows(config, expected, operation, {})
        path.write_bytes(q.serial(child))
        (output / 'predecessor/session.json').write_bytes(q.serial({'prepared_ns': 21}))
        with self.assertRaisesRegex(q.Rejected, 'old preparation outside actual worker'):
            q._windows(config, expected, operation, {})

    def test_nonfinite_and_duplicate_json_are_rejected(self):
        for raw in (b'{"x":1,"x":2}', b'{"x":NaN}', b'{"x":Infinity}'):
            with self.subTest(raw=raw), self.assertRaises(self.q.Rejected):
                self.q.decode(raw)


class RuntimePolicyControls(unittest.TestCase):
    def setUp(self):
        self.q=isolated_api()
        self.temp=tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root=Path(self.temp.name)

    def test_runtime_tool_registration_rejects_mixed_purpose_and_unsafe_env(self):
        invocation=object();permit=object();probes=[object(),object(),object()]
        root=self.root/'public-runtime/tools/main'
        docs=[{'cwd':str(root),'environment':{},'operation_kind':'runtime-tool-version','status':0} for _ in probes]
        with patch.object(self.q,'_runtime_permit',return_value={'root':str(self.root)}):
            for bad in ('LD_PRELOAD','PYTHONPATH','DYLD_INSERT_LIBRARIES'):
                mutated=copy.deepcopy(docs);mutated[0]['environment'][bad]='/untrusted'
                with patch.object(self.q,'_reverify_issued_probe',side_effect=mutated),self.assertRaises(self.q.Rejected):
                    self.q.register_public_tools(invocation,permit,probes)
            mutated=copy.deepcopy(docs);mutated[1]['cwd']=str(self.root/'public-runtime/tools/capability')
            with patch.object(self.q,'_reverify_issued_probe',side_effect=mutated),self.assertRaises(self.q.Rejected):
                self.q.register_public_tools(invocation,permit,probes)

    def test_public_environment_is_derived_per_tuple(self):
        permit=object();invocation=object();base={'HOME':'/home','OXID_LLVM_BIN':'/fixed/wrappers','UNIT4_ADAPTER_ROOT':'/fixed/helper'}
        self.q._PUBLIC_TOOLS[permit]={'environment':base,'pins':[]}
        case={'section':'guards','index':0,'variant':'observer'};fixture=self.root/'fixture';evidence=self.root/'observer'
        with patch.object(self.q,'_public_case',return_value=({}, {'group':'race'},[],fixture,evidence)):
            got=self.q._public_environment(permit,invocation,'public-runtime',case)
        self.assertEqual(got,dict(base,UNIT4_TOOL_LOG=str(evidence/'tools.jsonl'),UNIT4_TOOL_EVIDENCE=str(evidence),UNIT4_LIFECYCLE_LOG=str(evidence/'lifecycle.jsonl'),UNIT4_RACE_OUTPUT=str(fixture/'out.bin')))
        with patch.object(self.q,'_runtime_permit',return_value={}),patch.object(self.q,'_public_environment',return_value=got):
            for bad in (dict(got,PYTHONPATH='/other'),dict(got,UNIT4_TOOL_LOG='/wrong'),dict(got,UNIT4_LIFECYCLE_LOG='/wrong')):
                with self.assertRaises(self.q.Rejected):self.q._public_runtime_recipe(permit,None,invocation,'public-runtime',case,[],fixture,bad,120,8*1024**2)

    def test_hosted_environment_is_closed(self):
        invocation=object();parent=object();permit=self.q.RuntimePermit({'argv':['setpriv'],'cwd':str(self.root),'body_pins':[]},self.q._TOKEN)
        self.q._HOSTED[permit]={'invocation':invocation,'runtime_permit':parent}
        with patch.object(self.q,'_runtime_permit',return_value={}):
            for env in ({},{'HOME':str(self.root),'PATH':'/usr/bin:/bin','LANG':'C.UTF-8','LC_ALL':'C.UTF-8','PYTHONDONTWRITEBYTECODE':'1','PYTHONNOUSERSITE':'1','LD_PRELOAD':'/bad'}):
                with self.assertRaises(self.q.Rejected):self.q._hosted_runtime_recipe(permit,invocation,['setpriv'],self.root,env,600,8*1024**2)

    def test_public_tuple_policy_derives_fixed_current_binary_and_fixture(self):
        invocation=object();root=self.root
        doc={'kind':'public','root':str(root),'binaries':{'ordinary-debug':{'path':'/actual/ordinary'},'lifecycle-debug':{'path':'/actual/observer'}},'body_pins':[]}
        permit=self.q.RuntimePermit(doc,self.q._TOKEN);self.q._RUNTIME[permit]={'invocation':invocation}
        helper=root/'contracts.py';helper.write_bytes(b'fixed')
        row={'scope':'execute','profile':'debug','argv_template':['{oxid}','check','main.ox','--edition','typed-preview'],'case':{}}
        self.q._PUBLIC_CONTRACTS[permit]={'helper':self.q.identity(helper),'rosters':{'public':[row]}}
        case={'section':'public','index':0,'variant':'ordinary'}
        got=self.q._public_case(permit,invocation,case)
        self.assertEqual(got[2],['/actual/ordinary','check','main.ox','--edition','typed-preview'])
        self.assertEqual(got[3],root/'public-runtime/collection/public/00000/ordinary/fixture')
        for bad in (dict(case,index=-1),dict(case,index=True),dict(case,index=1),dict(case,variant='other'),dict(case,extra=1)):
            with self.assertRaises(self.q.Rejected):self.q._public_case(permit,invocation,bad)
        row['scope']='unavailable-host'
        with self.assertRaises(self.q.Rejected):self.q._public_case(permit,invocation,case)

    def test_hosted_arbitrary_receipt_is_not_live_permit(self):
        with self.assertRaises(self.q.Rejected):
            self.q._hosted_runtime_recipe({},object(),[],self.root,{},600,8*1024**2)

    def test_registered_relative_guard_link_preserves_target(self):
        target=self.root/'preserved-target';target.write_bytes(b'fixed');link=self.root/'occupied-output';link.symlink_to('preserved-target')
        row={'path':str(link),'target':str(target),'link_text':'preserved-target','target_stat':self.q._target_stat(target),'target_identity':self.q.identity(target)}
        self.q._verify_link(row)
        target.write_bytes(b'drift')
        with self.assertRaises(self.q.Rejected):self.q._verify_link(row)

    def test_runtime_permit_cannot_be_constructed_from_document(self):
        with self.assertRaises(self.q.Rejected):
            self.q.RuntimePermit({'kind':'parser'})
        with self.assertRaises(self.q.Rejected):
            self.q._runtime_permit({},object(),'parser')

    def test_public_registration_requires_exact_probe_roster(self):
        inv=object();state={'root':self.root}
        with patch.object(self.q,'_run_state',return_value=({}, {'scope':'public'},state)), patch.object(self.q,'check_deadline'):
            for probes in ([],[object()]*4,[object() for _ in range(5)]):
                with self.subTest(count=len(probes)),self.assertRaises(self.q.Rejected):
                    self.q.register_public_runtime(inv,probes,[object(),object()])

    def test_parser_registration_requires_exact_probe_roster(self):
        with patch.object(self.q,'_run_state',return_value=({}, {'scope':'parser'},{'root':self.root})),patch.object(self.q,'check_deadline'):
            with self.assertRaises(self.q.Rejected):
                self.q.register_parser_runtime(object(),self.root/'parser/package-source-session.json',[],self.root)

    def test_setup_version_fixed_recipe_and_limits(self):
        cwd=self.root/'public-runtime/tools/main'
        args=['/tools/clang','--version']
        self.q._operation_recipe('runtime-tool-version',args,Path(args[0]),self.root)
        self.q._runtime_setup_recipe('runtime-tool-version',args,cwd,{},self.root,self.root,30,8*1024**2)
        for bad in (['/tools/clang','--help'],['/tools/python','--version']):
            with self.assertRaises(self.q.Rejected):self.q._operation_recipe('runtime-tool-version',bad,Path(bad[0]),self.root)
        with self.assertRaises(self.q.Rejected):self.q._runtime_setup_recipe('runtime-tool-version',args,cwd,{},self.root,self.root,31,8*1024**2)
        with self.assertRaises(self.q.Rejected):self.q._runtime_setup_recipe('runtime-tool-version',args,self.root,{},self.root,self.root,30,8*1024**2)

    def test_trap_control_exact_log_and_path(self):
        cwd=self.root/'public-runtime/tools/main';argv=[str(cwd/'no-tool-traps/clang'),'--version']
        env={'UNIT4_TOOL_LOG':str(cwd/'clang-trap-control.jsonl'),'OXID_LLVM_BIN':str(cwd/'no-tool-traps')}
        self.q._runtime_setup_recipe('runtime-trap-control',argv,cwd,env,self.root,self.root,10,8*1024**2)
        for bad in ({},dict(env,UNIT4_TOOL_LOG=str(self.root/'escape'))):
            with self.assertRaises(self.q.Rejected):self.q._runtime_setup_recipe('runtime-trap-control',argv,cwd,bad,self.root,self.root,10,8*1024**2)

    def test_parser_helper_recipe_forbids_subset_and_wrong_profile(self):
        root=self.root/'parser';d={'root':str(root),'contract_root':str(self.root/'contract'),'checkpoint':str(root/'package-source-session.json')}
        env={'HOME':str(root/'home'),'PATH':'/usr/bin:/bin','LANG':'C.UTF-8','LC_ALL':'C.UTF-8','PYTHONDONTWRITEBYTECODE':'1','PYTHONNOUSERSITE':'1'}
        args=[sys.executable,'-B',str(root/'helpers/run.py'),'--build-receipt',str(root/'build-debug/result/build-receipt.json'),'--contract-dir',d['contract_root'],'--checkpoint',d['checkpoint'],'--output',str(root/'collect-debug/result')]
        with patch.object(self.q,'_runtime_permit',return_value=d),patch.object(self.q,'identity',return_value={}):
            self.q._parser_runtime_recipe(object(),object(),'parser-collect',args,root,env,1800,8*1024**2)
            for bad in (args+['--case','one'],[s.replace('build-debug','build-release') for s in args]):
                with self.assertRaises(self.q.Rejected):self.q._parser_runtime_recipe(object(),object(),'parser-collect',bad,root,env,1800,8*1024**2)
            with self.assertRaises(self.q.Rejected):self.q._parser_runtime_recipe(object(),object(),'parser-collect',args,root,dict(env,EXTRA='1'),1800,8*1024**2)

    def test_same_invocation_and_runtime_domain_are_required(self):
        invocation=object();permit=self.q.RuntimePermit({'kind':'parser','body_pins':[]},self.q._TOKEN)
        self.q._RUNTIME[permit]={'invocation':invocation}
        self.q._runtime_permit(permit,invocation,'parser')
        with self.assertRaises(self.q.Rejected):self.q._runtime_permit(permit,object(),'parser')
        with self.assertRaises(self.q.Rejected):self.q._runtime_permit(permit,invocation,'public')

    def test_unchanged_runtime_body_is_required(self):
        invocation=object();path=self.root/'binary';path.write_bytes(b'reviewed')
        permit=self.q.RuntimePermit({'kind':'parser','body_pins':[self.q.identity(path)]},self.q._TOKEN)
        self.q._RUNTIME[permit]={'invocation':invocation}
        with patch.object(self.q,'check_deadline'):
            self.q._runtime_permit(permit,invocation,'parser');path.write_bytes(b'changed')
            with self.assertRaises(self.q.Rejected):self.q._runtime_permit(permit,invocation,'parser')

class NativeWorkspaceControls(unittest.TestCase):
    """Synthetic filesystem/scan events only; no compiler or target starts."""
    def setUp(self):
        self.q=isolated_api();self.temp=tempfile.TemporaryDirectory();self.addCleanup(self.temp.cleanup)
        self.root=Path(self.temp.name);self.parent=self.root/'fixture';self.parent.mkdir()
        self.source=self.root/'native.rs';self.source.write_bytes(b'fixed source')
        self.plan={'parent':self.q._directory_identity(self.parent),'source':self.q.identity(self.source),'compiler':{'path':'/not-executed'},'output':str(self.parent/'out.bin'),'invocation_nonce':'synthetic'}
        self.reg=self.q._native_register(self.plan,424242);self.workspace=Path(self.reg['workspace'])

    def test_exact_six_disappearances_only(self):
        for p in [self.workspace,*[self.workspace/n for n in sorted(self.q.NATIVE_MEMBERS)]]:
            self.q._native_enoent(p,self.reg)
        self.assertEqual(len(self.reg['tolerated_disappearances']),6)
        self.q._native_enoent(self.workspace,self.reg)
        self.assertEqual(len(self.reg['tolerated_disappearances']),6)

    def test_llvm_exact_temp_families_and_ten_disappearances(self):
        self.workspace.mkdir(mode=0o700)
        names=['program-0123abcd.o.tmp','runtime-aac21d1e.o.tmp','executable.tmp123abcd','executable.tmp456efab']
        for name in names:
            p=self.workspace/name;p.write_bytes(b'growing output'*1024)
            self.q._native_observed(p,p.lstat(),self.reg)
            self.q._native_observed(p,p.lstat(),self.reg)
        for p in [self.workspace,*[self.workspace/n for n in sorted(self.q.NATIVE_MEMBERS)],*[self.workspace/n for n in names]]:
            self.q._native_enoent(p,self.reg)
        self.assertEqual(len(self.reg['tolerated_disappearances']),10)
        self.assertEqual({k:len(v) for k,v in self.reg['temporary_members'].items()},{'program':1,'runtime':1,'executable':2})
        with self.assertRaises(self.q.Rejected):self.q._native_terminal(self.reg)

    def test_llvm_near_miss_names_and_locations_rejected(self):
        names=['program-abcdefg0.o.tmp','program-ABCDEF01.o.tmp','program-1234567.o.tmp','program-123456789.o.tmp',
               'program-12345678.o.tmp.extra','runtime-12345678.obj.tmp','other-12345678.o.tmp','executable.tmp123456',
               'executable.tmp12345678','executable.tmp123abcG','nested/runtime-12345678.o.tmp']
        for name in names:
            with self.subTest(name=name),self.assertRaises(self.q.Rejected):self.q._native_enoent(self.workspace/name,self.reg)
        for p in [self.parent/'program-12345678.o.tmp',self.parent/'.oxid-native-424243-0/runtime-12345678.o.tmp']:
            with self.assertRaises(self.q.Rejected):self.q._native_enoent(p,self.reg)

    def test_llvm_distinct_temporary_counts_not_collision_retries(self):
        for family,names in [('program',['program-00000000.o.tmp','program-00000001.o.tmp']),
                             ('runtime',['runtime-00000000.o.tmp','runtime-00000001.o.tmp']),
                             ('executable',['executable.tmp0000000','executable.tmp0000001','executable.tmp0000002'])]:
            registration=self.q._native_register(self.plan,424242)
            for name in names[:-1]:self.q._native_enoent(self.workspace/name,registration)
            with self.subTest(family=family),self.assertRaises(self.q.Rejected):self.q._native_enoent(self.workspace/names[-1],registration)

    def test_llvm_temp_symlink_fifo_directory_rejected(self):
        self.workspace.mkdir(mode=0o700);p=self.workspace/'runtime-12345678.o.tmp'
        p.symlink_to(self.source)
        with self.assertRaises(self.q.Rejected):self.q._native_observed(p,p.lstat(),self.reg)
        p.unlink();os.mkfifo(p)
        with self.assertRaises(self.q.Rejected):self.q._native_observed(p,p.lstat(),self.reg)
        p.unlink();p.mkdir()
        with self.assertRaises(self.q.Rejected):self.q._native_observed(p,p.lstat(),self.reg)

    def test_llvm_growing_regular_members_remain_byte_accounted(self):
        self.workspace.mkdir(mode=0o700);p=self.workspace/'executable.tmp123abcd';p.write_bytes(b'x'*65537)
        entries=list(self.q._output_entries(self.root,self.reg,self.q.time.monotonic()+10))
        self.assertIn(p,entries);self.assertEqual(p.lstat().st_size,65537)
        p.unlink();self.workspace.rmdir();self.q._native_terminal(self.reg)

    def test_wrong_pid_counter_parent_and_depth_rejected(self):
        bad=[self.parent/'.oxid-native-424243-0',self.parent/'.oxid-native-424242-1',self.root/self.workspace.name,
             self.workspace/'unknown',self.workspace/'nested/program.ll',self.parent/'out.bin',self.parent/'main.ox',self.parent]
        for p in bad:
            with self.subTest(path=str(p)),self.assertRaises(self.q.Rejected):self.q._native_enoent(p,self.reg)

    def test_known_members_count_and_types_stay_strict(self):
        self.workspace.mkdir(mode=0o700);self.q._native_observed(self.workspace,self.workspace.lstat(),self.reg)
        for name in self.q.NATIVE_MEMBERS:
            p=self.workspace/name;p.write_bytes(b'counted');self.q._native_observed(p,p.lstat(),self.reg)
        unknown=self.workspace/'unknown';unknown.write_bytes(b'bad')
        with self.assertRaises(self.q.Rejected):list(self.q._output_entries(self.root,self.reg,self.q.time.monotonic()+10))

    def test_nested_special_and_symlink_rejected(self):
        self.workspace.mkdir(mode=0o700)
        p=self.workspace/'program.ll';p.symlink_to(self.source)
        with self.assertRaises(self.q.Rejected):self.q._native_observed(p,p.lstat(),self.reg)
        p.unlink();os.mkfifo(p)
        with self.assertRaises(self.q.Rejected):self.q._native_observed(p,p.lstat(),self.reg)
        p.unlink();p.mkdir()
        with self.assertRaises(self.q.Rejected):self.q._native_observed(p,p.lstat(),self.reg)

    def test_exact_workspace_enumeration_enoent_is_recorded(self):
        self.workspace.mkdir(mode=0o700);original=os.scandir
        def scan(path):
            if Path(path)==self.workspace:raise FileNotFoundError(str(path))
            return original(path)
        with patch.object(self.q.os,'scandir',side_effect=scan):
            list(self.q._output_entries(self.root,self.reg,self.q.time.monotonic()+10))
        self.assertEqual(self.reg['tolerated_disappearances'],[str(self.workspace)])

    def test_stable_parent_enumeration_enoent_rejected(self):
        with patch.object(self.q.os,'scandir',side_effect=FileNotFoundError('stable')):
            with self.assertRaises(self.q.Rejected):list(self.q._output_entries(self.root,self.reg,self.q.time.monotonic()+10))

    def test_non_enoent_errors_not_suppressed(self):
        for error in (PermissionError('denied'),NotADirectoryError('notdir'),OSError(5,'io')):
            with patch.object(self.q.os,'scandir',side_effect=error):
                with self.assertRaises(type(error)):list(self.q._output_entries(self.root,self.reg,self.q.time.monotonic()+10))

    def test_terminal_residue_or_changed_parent_rejected(self):
        self.q._native_terminal(self.reg)
        self.workspace.mkdir(mode=0o700)
        with self.assertRaises(self.q.Rejected):self.q._native_terminal(self.reg)
        self.workspace.rmdir();self.parent.chmod(0o700 if self.parent.stat().st_mode&0o777!=0o700 else 0o755)
        with self.assertRaises(self.q.Rejected):self.q._native_terminal(self.reg)

    def test_publication_hardlink_never_transient(self):
        self.workspace.mkdir(mode=0o700);p=self.workspace/'executable';p.write_bytes(b'elf');output=self.parent/'out.bin';os.link(p,output)
        self.q._native_observed(p,p.lstat(),self.reg);self.q._native_observed(output,output.lstat(),self.reg)
        with self.assertRaises(self.q.Rejected):self.q._native_enoent(output,self.reg)
        p.unlink();self.workspace.rmdir();self.q._native_terminal(self.reg)

    def test_no_hosted_or_generic_operation_registration(self):
        for kind in ('public-hosted-worker','public-native','generator','probe','parser-collect','public-build'):
            self.assertIsNone(self.q._native_plan(object(),kind,['/noexec','compile','--backend','llvm','--output','out.bin'],self.parent))
        self.assertIsNone(self.q._native_plan(object(),'public-runtime',['/noexec','check','--backend','llvm','--output','out.bin'],self.parent))

    def test_preexisting_native_prefix_rejected(self):
        (self.parent/'.oxid-native-unrelated').mkdir()
        args=['/noexec','compile','--backend','llvm','--output','out.bin'];captured={'checkout':{'path':str(self.root)}}
        with patch.object(self.q,'verify_invocation',return_value=({'run_root':str(self.root),'nonce':'synthetic'},captured)),patch.object(self.q,'identity',return_value={'sha256':self.q.NATIVE_SOURCE_SHA}),patch.object(self.q,'check_deadline'):
            with self.assertRaises(self.q.Rejected):self.q._native_plan(object(),'public-runtime',args,self.parent)

    def test_existing_output_has_no_workspace_exception(self):
        (self.parent/'out.bin').write_bytes(b'occupied')
        self.assertIsNone(self.q._native_plan(object(),'public-runtime',['/noexec','compile','--backend','llvm','--output','out.bin'],self.parent))

    def test_scanner_keeps_existing_tmp_policy_but_not_root(self):
        p=self.root/'tmp'/'gone';self.q._scan_disappearance(p,self.root,self.reg,())
        with self.assertRaises(self.q.Rejected):self.q._scan_disappearance(self.root/'tmp',self.root,self.reg,())

    def test_native_terminal_expired_deadline_rejected(self):
        with patch.object(self.q,'_DEADLINE',self.q.time.monotonic()-1):
            with self.assertRaises(self.q.Rejected):self.q._native_terminal(self.reg)

    def test_native_scan_deadline_is_enforced(self):
        with self.assertRaises(self.q.Rejected):list(self.q._output_entries(self.root,self.reg,self.q.time.monotonic()-1))

class CheckedOwnerScopeControls(unittest.TestCase):
    """Synthetic owner registries only; no captured source or actual operation."""
    def setUp(self):
        self.q=isolated_api();self.inv=object();self.other=object()
        self.context=types.ModuleType(self.q.CONTEXT_KEY)
        self.context._invocation_association=Mock(return_value=({'nonce':'synthetic'}, {'scope':'combined'}))
        self.context._invocation=Mock(return_value=({'nonce':'synthetic'}, {'scope':'combined'}))
        self.q._RUNS[self.inv]={'deadline':self.q.time.monotonic()+60,'failed':False}
        self.q._RUNS[self.other]={'deadline':self.q.time.monotonic()+60,'failed':False}
        self.stack=ExitStack();self.addCleanup(self.stack.close)
        self.stack.enter_context(patch.dict(sys.modules,{NAME:self.q,self.q.CONTEXT_KEY:self.context}))
        self.manifest=self.stack.enter_context(patch.object(self.q,'_manifest',return_value=(HERE,{'context.py':{'sha256':'synthetic'}})))
        self.stack.enter_context(patch.object(self.q,'_module',return_value=self.context))

    def test_read_scope_exposes_no_authority_and_uses_association(self):
        with self.q.checked_scope(self.inv) as handle:
            self.assertIsNone(handle)
            self.assertIs(self.q.context_api(),self.context)
            self.q.verify_invocation(self.inv)
            self.assertFalse(self.context._invocation.called)
        self.assertFalse(self.q._CHECKED)
        self.assertEqual(self.manifest.call_count,3)
        self.q.verify_invocation(self.inv)
        self.context._invocation.assert_called_once_with(self.inv)

    def test_full_verification_never_downgrades_inside_scope(self):
        with self.q.checked_scope(self.inv):self.q.full_verify_invocation(self.inv)
        self.context._invocation.assert_called_once_with(self.inv)

    def test_entry_helper_drift_poisons(self):
        self.manifest.side_effect=ValueError('entry drift')
        with self.assertRaises(ValueError):
            with self.q.checked_scope(self.inv):pass
        self.assertTrue(self.q._RUNS[self.inv]['failed'])
        self.assertFalse(self.q._CHECKED)

    def test_bounded_scope_depth(self):
        with self.assertRaises(self.q.Rejected):
            with ExitStack() as stack:
                for _ in range(17):stack.enter_context(self.q.checked_scope(self.inv))
        self.assertFalse(self.q._CHECKED)
        self.assertTrue(self.q._RUNS[self.inv]['failed'])

    def test_nested_same_owner_and_scope_expiry(self):
        with self.q.checked_scope(self.inv):
            outer=self.q._CHECKED[-1]
            with self.q.checked_scope(self.inv):self.q.verify_invocation(self.inv)
            self.assertTrue(outer.active)
        self.assertFalse(outer.active)
        with self.assertRaises(TypeError):outer.active=True

    def test_cross_invocation_nesting_poisons_outer(self):
        with self.assertRaises(self.q.Rejected):
            with self.q.checked_scope(self.inv):
                with self.q.checked_scope(self.other):pass
        self.assertTrue(self.q._RUNS[self.inv]['failed'])

    def test_exception_poisons_session_and_expires_scope(self):
        with self.assertRaisesRegex(ValueError,'synthetic'):
            with self.q.checked_scope(self.inv):raise ValueError('synthetic')
        self.assertTrue(self.q._RUNS[self.inv]['failed'])
        self.assertFalse(self.q._CHECKED)
        with self.assertRaises(self.q.Rejected):self.q.check_deadline(self.inv)

    def test_exit_helper_drift_poisons(self):
        with self.assertRaisesRegex(ValueError,'drift'):
            with self.q.checked_scope(self.inv):self.manifest.side_effect=ValueError('drift')
        self.assertTrue(self.q._RUNS[self.inv]['failed'])

    def test_owner_substitution_rejected(self):
        with self.assertRaises(self.q.Rejected):
            with self.q.checked_scope(self.inv):
                with patch.dict(sys.modules,{self.q.CONTEXT_KEY:types.ModuleType('forged')}):self.q.verify_invocation(self.inv)
        self.assertTrue(self.q._RUNS[self.inv]['failed'])

    def test_cross_thread_lookup_rejected(self):
        with self.assertRaises(self.q.Rejected):
            with self.q.checked_scope(self.inv):
                with patch.object(self.q.threading,'get_ident',return_value=-1):self.q.verify_invocation(self.inv)

    def test_expired_deadline_rejected(self):
        with self.assertRaises(self.q.Rejected):
            with self.q.checked_scope(self.inv):
                self.q._RUNS[self.inv]['deadline']=0
                self.q.verify_invocation(self.inv)

    def test_owned_launch_forbidden_inside_read_scope(self):
        with patch.object(self.q,'_owned_probe') as child:
            with self.assertRaises(self.q.Rejected):
                with self.q.checked_scope(self.inv):self.q.owned_probe(self.inv,['/synthetic'],HERE,{})
            child.assert_not_called()

    def permit(self):
        value=self.q.RuntimePermit({'kind':'public'},self.q._TOKEN)
        self.q._RUNTIME[value]={'invocation':self.inv}
        return value

    def test_public_launch_has_private_owner_without_read_scope(self):
        permit=self.permit()
        def fake(*args):
            self.assertFalse(self.q._CHECKED)
            self.assertIsNotNone(self.q._LAUNCH)
            self.q.verify_invocation(self.inv)
            return 'synthetic outcome'
        with patch.object(self.q,'_owned_probe',side_effect=fake):
            self.assertEqual(self.q.owned_probe(self.inv,['/synthetic'],HERE,{},operation_kind='public-runtime',runtime_permit=permit),'synthetic outcome')
        self.assertIsNone(self.q._LAUNCH)
        self.context._invocation.assert_not_called()

    def test_generic_probe_retains_full_association_route(self):
        def fake(*args):self.q.verify_invocation(self.inv);return None
        with patch.object(self.q,'_owned_probe',side_effect=fake):self.q.owned_probe(self.inv,['/synthetic'],HERE,{})
        self.context._invocation.assert_called_once_with(self.inv)

    def test_wrong_or_forged_public_permit_never_launches(self):
        permit=self.permit();self.q._RUNTIME[permit]['invocation']=self.other
        with patch.object(self.q,'_owned_probe') as child:
            for value in (permit,object(),None):
                with self.assertRaises(self.q.Rejected):self.q.owned_probe(self.inv,['/synthetic'],HERE,{},operation_kind='public-native',runtime_permit=value)
            child.assert_not_called()

    def test_launch_entry_helper_drift_poisons_before_child(self):
        permit=self.permit();self.manifest.side_effect=ValueError('entry drift')
        with patch.object(self.q,'_owned_probe') as child:
            with self.assertRaises(ValueError):self.q.owned_probe(self.inv,['/synthetic'],HERE,{},operation_kind='public-runtime',runtime_permit=permit)
            child.assert_not_called()
        self.assertIsNone(self.q._LAUNCH)
        self.assertTrue(self.q._RUNS[self.inv]['failed'])

    def test_launch_exception_poisons_and_closes_owner(self):
        with patch.object(self.q,'_owned_probe',side_effect=ValueError('synthetic')):
            with self.assertRaises(ValueError):self.q.owned_probe(self.inv,['/synthetic'],HERE,{},operation_kind='public-runtime',runtime_permit=self.permit())
        self.assertIsNone(self.q._LAUNCH)
        self.assertTrue(self.q._RUNS[self.inv]['failed'])

class PublicRecipeReadDedupControls(unittest.TestCase):
    """Actual finite recipe predicates with synthetic immutable authority rows."""
    def setUp(self):
        self.q=isolated_api();self.inv=object();self.root=Path('/synthetic-owned-root')
        self.q._RUNS[self.inv]={'deadline':self.q.time.monotonic()+60,'failed':False}
        self.pins=[];self.binaries={}
        for i,(role,profile) in enumerate((('ordinary','debug'),('ordinary','release'),('lifecycle','debug'),('lifecycle','release'))):
            emitted={'path':str(self.root/'targets'/('public-'+role)/profile/('oxid.exe' if os.name=='nt' else 'oxid')),'bytes':i+1,'sha256':str(i)*64}
            copied=dict(emitted,path=str(self.root/'public-build'/role/('oxid-'+profile+('.exe' if os.name=='nt' else ''))))
            self.pins.extend((emitted,copied));self.binaries[role+'-'+profile]=copied
        self.helper={'path':str(self.root/'contracts.py'),'bytes':9,'sha256':'a'*64}
        self.wrapper={'path':str(self.root/'wrapper'),'bytes':10,'sha256':'b'*64}
        self.case={'section':'public','index':0,'variant':'ordinary'}
        self.permit=self.q.RuntimePermit({'kind':'public','root':str(self.root),'binaries':self.binaries,'body_pins':self.pins},self.q._TOKEN)
        self.q._RUNTIME[self.permit]={'invocation':self.inv}
        self.q._PUBLIC_CONTRACTS[self.permit]={'helper':self.helper,'rosters':{'public':[{'scope':'execute','profile':'debug','argv_template':['oxid','check','source.ox']}]}}
        self.q._PUBLIC_TOOLS[self.permit]={'environment':{'HOME':str(self.root/'home')},'pins':[self.wrapper]}
        self.rows={x['path']:x for x in self.pins+[self.helper,self.wrapper]}
        self.identity=patch.object(self.q,'identity',side_effect=lambda p:dict(self.rows[str(p)]));self.reader=self.identity.start();self.addCleanup(self.identity.stop)
        self.selected=self.q._public_case(self.permit,self.inv,self.case)
        self.env=self.q._public_environment(self.permit,self.inv,'public-runtime',self.case)
        self.reader.reset_mock()

    def recipe(self,env=None):
        return self.q._public_runtime_recipe(self.permit,None,self.inv,'public-runtime',self.case,self.selected[2],self.selected[3],self.env if env is None else env,120,8*1024**2)

    def replay_stack(self):
        stack=ExitStack();self.addCleanup(stack.close)
        context=types.ModuleType(self.q.CONTEXT_KEY)
        context._invocation_association=Mock(return_value=({},{}))
        context._invocation=Mock(return_value=({},{}))
        stack.enter_context(patch.dict(sys.modules,{NAME:self.q,self.q.CONTEXT_KEY:context}))
        stack.enter_context(patch.object(self.q,'_manifest',return_value=(HERE,{'context.py':{'sha256':'synthetic'}})))
        stack.enter_context(patch.object(self.q,'_module',return_value=context))
        return stack

    def test_replay_checks_all_bodies_at_entry_exit_but_not_per_record(self):
        self.replay_stack()
        with self.q.public_replay(self.inv,self.permit) as value:
            self.assertIsNone(value)
            for _ in range(3):self.recipe()
        paths=[str(call.args[0]) for call in self.reader.call_args_list]
        for row in self.pins:self.assertEqual(paths.count(row['path']),2)
        self.assertIsNone(self.q._PUBLIC_REPLAY)
        self.assertFalse(self.q._CHECKED)

    def test_replay_exit_detects_inactive_binary_drift(self):
        self.replay_stack()
        with self.assertRaisesRegex(self.q.Rejected,'runtime input body drift'):
            with self.q.public_replay(self.inv,self.permit):
                self.rows[self.pins[7]['path']]=dict(self.pins[7],sha256='f'*64)
                self.recipe()
        self.assertTrue(self.q._RUNS[self.inv]['failed'])
        self.assertIsNone(self.q._PUBLIC_REPLAY)

    def test_replay_prohibits_any_launch(self):
        self.replay_stack()
        with patch.object(self.q,'_owned_probe') as child:
            with self.assertRaises(self.q.Rejected):
                with self.q.public_replay(self.inv,self.permit):self.q.owned_probe(self.inv,['/synthetic'],self.root,{})
            child.assert_not_called()

    def test_replay_does_not_cache_stream_or_evidence_bodies(self):
        self.replay_stack();record=object();self.q._PROBES[record]={'runtime_permit':self.permit}
        with self.q.public_replay(self.inv,self.permit):
            self.reader.reset_mock()
            self.assertEqual(self.q._probe_body_identity(self.pins[1],record,self.inv),self.pins[1])
            self.reader.assert_not_called()
            self.q._probe_body_identity(self.wrapper,record,self.inv)
            self.reader.assert_called_once_with(self.wrapper['path'])

    def test_replay_wrong_owner_and_overlap_rejected(self):
        self.replay_stack()
        with self.assertRaises(self.q.Rejected):
            with self.q.public_replay(self.inv,self.permit):
                self.q._runtime_permit(object(),self.inv,'public')
        self.assertTrue(self.q._RUNS[self.inv]['failed'])

    def test_exact_recipe_reads_only_frozen_selected_pair_once(self):
        self.assertIsNone(self.recipe())
        paths=[str(call.args[0]) for call in self.reader.call_args_list]
        for row in self.pins[:2]+[self.helper,self.wrapper]:self.assertEqual(paths.count(row['path']),1)
        for row in self.pins[2:]:self.assertNotIn(row['path'],paths)
        self.assertEqual(len(paths),4)

    def test_next_recipe_rereads_bodies_and_rejects_drift(self):
        self.recipe();self.reader.reset_mock()
        self.rows[self.pins[1]['path']]=dict(self.pins[1],sha256='f'*64)
        with self.assertRaisesRegex(self.q.Rejected,'selected public compiled body drift'):self.recipe()
        self.assertTrue(self.reader.called)

    def test_inactive_body_drift_rejected_at_full_boundary(self):
        self.rows[self.pins[7]['path']]=dict(self.pins[7],sha256='f'*64)
        self.assertIsNone(self.recipe())
        with self.assertRaisesRegex(self.q.Rejected,'runtime input body drift'):
            self.q._runtime_permit(self.permit,self.inv,'public')

    def test_wrong_profile_or_observer_role_rejected(self):
        original=self.q._PUBLIC_CONTRACTS[self.permit]['rosters']['public'][0]
        original['profile']='release'
        with self.assertRaises(self.q.Rejected):self.recipe()
        original['profile']='debug';self.case['variant']='observer'
        with self.assertRaises(self.q.Rejected):self.recipe()

    def test_existing_environment_predicates_remain_exact(self):
        for change in ({'LD_PRELOAD':'/injected'},{'PYTHONPATH':'/injected'},{'UNIT4_TOOL_LOG':'/wrong'}):
            with self.subTest(change=change),self.assertRaisesRegex(self.q.Rejected,'exact producer runtime environment'):
                self.recipe(dict(self.env,**change))

    def test_selected_tuple_derivation_matches_original_entrypoint(self):
        d=self.q._runtime_permit(self.permit,self.inv,'public')
        selected=self.q._public_case_verified(d,self.permit,self.case)
        self.assertEqual(selected,self.selected)
        self.assertEqual(self.q._public_environment_verified(self.permit,self.inv,'public-runtime',self.case,selected),self.env)

class ExecutableViewReadDedupControls(unittest.TestCase):
    """Temporary ordinary files/links; no tool or process execution."""
    def setUp(self):
        self.q=isolated_api();self.temp=tempfile.TemporaryDirectory();self.addCleanup(self.temp.cleanup)
        self.root=Path(self.temp.name);self.rows=[];self.targets=[]
        for name in ('cargo','rustc'):
            path=self.root/name;path.write_bytes(('synthetic '+name).encode());self.targets.append(path)
        for index in range(4):
            view=self.root/('view-'+str(index));view.mkdir()
            for target in self.targets:
                path=view/target.name;path.symlink_to(target)
                self.rows.append({'path':str(path),'target':str(target),'link_text':str(target),'target_identity':self.q.identity(target),'target_stat':self.q._target_stat(target)})

    def test_four_views_hash_each_actual_target_once_per_call(self):
        with patch.object(self.q,'identity',wraps=self.q.identity) as reader:
            self.q._verify_view_rows(self.rows)
            self.assertEqual(reader.call_count,2)
            self.q._verify_view_rows(self.rows)
            self.assertEqual(reader.call_count,4)

    def test_alias_authority_cannot_differ(self):
        rows=copy.deepcopy(self.rows);rows[-1]['target_identity']['sha256']='0'*64
        with self.assertRaises(self.q.Rejected):self.q._verify_view_rows(rows)

    def test_target_change_during_call_is_rejected(self):
        original=self.q.identity
        def mutate(path):
            value=original(path)
            if Path(path)==self.targets[0]:Path(path).write_bytes(b'changed while reading')
            return value
        with patch.object(self.q,'identity',side_effect=mutate),self.assertRaises(self.q.Rejected):self.q._verify_view_rows(self.rows)

    def replay(self):
        from contextlib import nullcontext
        inv=object();self.q._RUNS[inv]={'deadline':self.q.time.monotonic()+60,'failed':False}
        self.q._VIEWS[inv]={r['path']:r for r in self.rows}
        permit=self.q.RuntimePermit({'kind':'public','body_pins':[]},self.q._TOKEN);self.q._RUNTIME[permit]={'invocation':inv}
        stack=ExitStack();self.addCleanup(stack.close)
        stack.enter_context(patch.object(self.q,'checked_scope',side_effect=lambda _:nullcontext()))
        stack.enter_context(patch.object(self.q,'_active_owner',return_value=object()))
        return inv,permit

    def test_read_only_replay_checks_routes_each_time_and_hashes_at_boundaries(self):
        inv,permit=self.replay()
        with patch.object(self.q,'identity',wraps=self.q.identity) as reader:
            with self.q.public_replay(inv,permit):
                for _ in range(5):self.q._verify_view_rows(self.rows)
            self.assertEqual(reader.call_count,4)
        self.assertIsNone(self.q._PUBLIC_REPLAY)

    def test_public_inactive_tool_stat_policy_and_full_override(self):
        inv,permit=self.replay()
        with patch.object(self.q,'identity',wraps=self.q.identity) as reader:
            self.q._verify_view_rows(self.rows,_stat_owner=(inv,permit))
            self.assertEqual(reader.call_count,0)
            self.q._verify_view_rows(self.rows,_force_full=True,_stat_owner=(inv,permit))
            self.assertEqual(reader.call_count,2)
            self.q._verify_view_rows(self.rows)
            self.assertEqual(reader.call_count,4)
        self.targets[0].write_bytes(b'changed inactive tool')
        with self.assertRaises(self.q.Rejected):self.q._verify_view_rows(self.rows,_stat_owner=(inv,permit))

    def test_public_input_scope_only_authorizes_original_setup_records(self):
        inv,permit=self.replay();record=object();other=object()
        self.q._PUBLIC_TOOLS[permit]={'probes':(record,)}
        with self.q.public_inputs(inv,permit) as exposed:
            self.assertIsNone(exposed)
            self.assertEqual(self.q._public_probe_view_owner(record,inv,'runtime-tool-version',None),(inv,permit))
            self.assertIsNone(self.q._public_probe_view_owner(other,inv,'probe',None))
            with self.assertRaises(self.q.Rejected):self.q._public_probe_view_owner(record,object(),'probe',None)
        self.assertEqual(self.q._PUBLIC_INPUTS,[])
        self.assertIsNone(self.q._public_probe_view_owner(record,inv,'probe',None))

    def test_public_record_outside_scope_keeps_full_body_policy(self):
        inv,permit=self.replay();record=object()
        self.q._PROBES[record]={'runtime_permit':permit}
        with patch.object(self.q,'_active_owner',return_value=None):
            self.assertIsNone(self.q._public_probe_view_owner(record,inv,'public-runtime',permit))

    def test_cancellation_only_poison_original_registered_invocation(self):
        inv=object();other=object();context=types.SimpleNamespace(_invocation_association=lambda value:self.q.need(value is inv,'original invocation'))
        self.q._MODULES[self.q.CONTEXT_KEY]=context;self.q._RUNS[inv]={'failed':False}
        with patch.dict(sys.modules,{self.q.CONTEXT_KEY:context}):
            with self.assertRaises(self.q.Rejected):self.q.cancel_invocation(other)
            self.assertFalse(self.q._RUNS[inv]['failed'])
            self.q.cancel_invocation(inv);self.assertTrue(self.q._RUNS[inv]['failed'])

    def test_public_input_scope_exception_discards_lifetime(self):
        inv,permit=self.replay();self.q._PUBLIC_TOOLS[permit]={'probes':()}
        with self.assertRaisesRegex(ValueError,'stop'):
            with self.q.public_inputs(inv,permit):raise ValueError('stop')
        self.assertEqual(self.q._PUBLIC_INPUTS,[])

    def test_read_only_replay_target_drift_poisons_result(self):
        inv,permit=self.replay()
        with self.assertRaises(self.q.Rejected):
            with self.q.public_replay(inv,permit):self.targets[0].write_bytes(b'changed after entry')
        self.assertTrue(self.q._RUNS[inv]['failed'])
        self.assertIsNone(self.q._PUBLIC_REPLAY)

class OutputMonitorControls(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name).resolve()
        for name in ('operations', 'tmp', 'home', 'public-runtime/collection'):
            (self.root / name).mkdir(parents=True, exist_ok=True)
        self.q = types.ModuleType('isolated_output_monitor_draft')
        path = Path(__file__).with_name('preparation.py')
        exec(compile(path.read_bytes(), str(path), 'exec'), self.q.__dict__)
        self.clock = 0.0
        self.q.time = types.SimpleNamespace(monotonic=lambda: self.clock)
        self.inv, self.permit = object(), object()
        self.run = {'root': self.root, 'deadline': 1000.0, 'failed': False, 'next': 0}
        self.q.need = self.need
        self.q._DEADLINE = None
        self.q._run_state = lambda inv: ({}, {}, self.run) if inv is self.inv else self.need(False, 'invocation')
        self.q._runtime_permit = lambda permit, inv, kind: self.runtime_permit(permit, inv, kind)
        self.q._runtime_permit_owner = self.q._runtime_permit
        self.q._PUBLIC_CONTRACTS = {self.permit: {'rosters': {
            section: [{'scope': 'execute', 'group': 'public'}] for section in self.q._OUT_SECTIONS}}}
        self.q._public_case_verified = self.public_case
        self.q._VIEWS, self.q._GUARD_LINKS = {}, {}
        self.q._native_observed = Mock()
        self.q._scan_disappearance = self.disappearance
        self.q._verify_link = self.verify_link
        self.q.public_output_start(self.inv, self.permit)

    @staticmethod
    def need(value, message):
        if not value:
            raise ValueError(message)

    def runtime_permit(self, permit, inv, kind):
        self.need(permit is self.permit and inv is self.inv and kind == 'public', 'wrong live capability')
        return {'root': str(self.root)}

    def public_case(self, d, permit, case):
        self.need(permit is self.permit and set(case) == {'section', 'index', 'variant'} and
                  case['section'] in self.q._OUT_SECTIONS and type(case['index']) is int and
                  case['index'] == 0 and case['variant'] == 'ordinary', 'wrong frozen case')
        section = case['section']
        base = self.root / 'public-runtime/collection' / section / ('000' if section == 'guards' else '00000')
        evidence = base / 'ordinary'
        fixture = base / 'fixtures/frozen' if section == 'guards' else evidence / 'fixture'
        return d, {'scope': 'execute'}, ['/never-executed', 'check'], fixture, evidence

    def disappearance(self, path, root, registration, transient):
        self.need(root / 'tmp' in Path(path).parents, 'vanished stable output')

    def verify_link(self, row, hash_body=False):
        path = Path(row['path'])
        self.need(path.is_symlink() and os.readlink(path) == row['link_text'] and
                  str(path.resolve(strict=True)) == row['target'], 'changed registered link')

    def open_section(self, section='public'):
        with self.q.public_output_boundary(self.inv, self.permit, 'section-open', section):
            root = self.root / 'public-runtime/collection' / section
            root.mkdir()
            (root / 'roster.json').write_bytes(b'[]')
        return root

    def create_case(self, section='public'):
        base = self.root / 'public-runtime/collection' / section / ('000' if section == 'guards' else '00000')
        source = base / 'ordinary/fixture/main.ox'
        source.parent.mkdir(parents=True)
        source.write_bytes(b'source')
        (base / 'receipt.json').write_bytes(b'{}')
        return base, source

    def monitor(self):
        return self.q._OUTPUT_MONITORS[self.inv]

    def test_initial_ledger_counts_real_bytes(self):
        self.assertEqual(self.monitor()['sealed_bytes'], 0)
        self.assertIn(str(self.root / 'tmp'), self.monitor()['sealed'])

    def test_start_cannot_reuse_live_invocation(self):
        with self.assertRaises(ValueError):
            self.q.public_output_start(self.inv, self.permit)

    def test_scope_spans_source_delete_restore_and_receipt_writes(self):
        self.open_section()
        with self.q.public_case_scope(self.inv, self.permit, 'public', 0):
            base, source = self.create_case()
            original = source.read_bytes()
            source.unlink()
            self.q.public_output_tick(self.inv)
            source.write_bytes(original)
            (base / 'ordinary/process.json').write_bytes(b'{}')
            (base / 'receipt.json').write_bytes(b'{"done":true}')
            self.assertNotIn(str(source), self.monitor()['sealed'])
        self.assertIn(str(source), self.monitor()['sealed'])
        self.assertEqual(self.monitor()['sealed'][str(source)]['sha256'], self.q.hashlib.sha256(original).hexdigest())
        self.assertIsNone(self.monitor()['case'])

    def test_nested_case_scope_poisoned(self):
        self.open_section()
        with self.assertRaises(ValueError):
            with self.q.public_case_scope(self.inv, self.permit, 'public', 0):
                with self.q.public_case_scope(self.inv, self.permit, 'public', 0):
                    pass
        self.assertTrue(self.run['failed'])

    def test_case_exception_does_not_seal_partial_root(self):
        self.open_section()
        with self.assertRaisesRegex(RuntimeError, 'cancelled'):
            with self.q.public_case_scope(self.inv, self.permit, 'public', 0):
                base, source = self.create_case()
                raise RuntimeError('cancelled')
        self.assertTrue(source.exists())
        self.assertNotIn(str(source), self.monitor()['sealed'])
        self.assertTrue(self.run['failed'])

    def test_missing_final_receipt_rejects(self):
        self.open_section()
        with self.assertRaises(ValueError):
            with self.q.public_case_scope(self.inv, self.permit, 'public', 0):
                base, source = self.create_case()
                (base / 'receipt.json').unlink()

    def test_old_metadata_same_size_ctime_mutation_rejects(self):
        self.open_section()
        m = self.monitor()
        actual = copy.deepcopy(m['sealed'])
        p = self.root / 'public-runtime/collection/public/roster.json'
        actual[str(p)]['stat']['st_ctime_ns'] += 1
        with self.assertRaises(ValueError):
            self.q._out_compare(m, actual)

    def test_sealed_file_disappearance_rejects(self):
        self.open_section()
        p = self.root / 'public-runtime/collection/public/roster.json'
        p.unlink()
        with self.assertRaises(ValueError):
            self.q._out_audit(self.monitor())
        self.assertTrue(self.run['failed'])

    def test_unregistered_sibling_file_rejects(self):
        (self.root / 'hidden-output').write_bytes(b'x')
        with self.assertRaises(ValueError):
            self.q._out_audit(self.monitor())

    def test_unregistered_symlink_rejects_without_following(self):
        (self.root / 'bad-link').symlink_to(self.root / 'home')
        with self.assertRaises(ValueError):
            self.q._out_audit(self.monitor())

    def test_special_file_rejects(self):
        if not hasattr(os, 'mkfifo'):
            self.skipTest('FIFO unavailable')
        os.mkfifo(self.root / 'fifo')
        with self.assertRaises(ValueError):
            self.q._out_audit(self.monitor())

    def test_full_audit_due_at_four_seconds(self):
        self.clock = 4.0
        self.q.public_output_tick(self.inv)
        self.assertEqual(self.monitor()['last_audit'], 4.0)

    def test_active_full_audit_late_fails_closed(self):
        self.open_section()
        with self.assertRaises(ValueError):
            with self.q.public_case_scope(self.inv, self.permit, 'public', 0):
                self.create_case()
                self.clock = 5.01
                self.q.public_output_tick(self.inv)
        self.assertTrue(self.run['failed'])

    def test_recursive_audit_rejects(self):
        self.monitor()['auditing'] = True
        with self.assertRaises(ValueError):
            self.q.public_output_tick(self.inv)

    def test_accounting_transfers_case_once(self):
        self.open_section()
        with self.q.public_case_scope(self.inv, self.permit, 'public', 0):
            self.create_case()
            during = self.q.public_output_tick(self.inv)['active_accounted_bytes']
        after = self.q.public_output_tick(self.inv)['active_accounted_bytes']
        self.assertEqual(during, after)
        with self.assertRaises(ValueError):
            with self.q.public_case_scope(self.inv, self.permit, 'public', 0):
                pass

    def test_hardlinks_count_per_path(self):
        a, b = self.root / 'tmp/a', self.root / 'tmp/b'
        a.write_bytes(b'123')
        os.link(a, b)
        sample = self.q.public_output_tick(self.inv)
        self.assertEqual(sample['active_accounted_bytes'], 6)

    def test_tmp_remains_live_not_sealed(self):
        p = self.root / 'tmp/current'
        p.write_bytes(b'123')
        self.q._out_audit(self.monitor())
        self.assertNotIn(str(p), self.monitor()['sealed'])
        self.assertEqual(self.q.public_output_tick(self.inv)['active_accounted_bytes'], 3)
        p.unlink()
        self.assertEqual(self.q.public_output_tick(self.inv)['active_accounted_bytes'], 0)

    def test_per_file_limit_is_inclusive(self):
        self.q._OUT_FILE = 3
        p = self.root / 'tmp/current'
        p.write_bytes(b'123')
        self.q.public_output_tick(self.inv)
        p.write_bytes(b'1234')
        with self.assertRaises(ValueError):
            self.q.public_output_tick(self.inv)

    def test_aggregate_limit_is_inclusive(self):
        self.q._OUT_TOTAL = 3
        p = self.root / 'tmp/current'
        p.write_bytes(b'123')
        self.q.public_output_tick(self.inv)
        (self.root / 'tmp/extra').write_bytes(b'x')
        with self.assertRaises(ValueError):
            self.q.public_output_tick(self.inv)

    def test_transition_cannot_authorize_old_roster_rewrite(self):
        section = self.open_section()
        with self.q.public_case_scope(self.inv, self.permit, 'public', 0):
            self.create_case()
        with self.assertRaises(ValueError):
            with self.q.public_output_boundary(self.inv, self.permit, 'section-close', 'public'):
                (section / 'roster.json').write_bytes(b'changed')
                (section / 'observations.jsonl.gz').write_bytes(b'gzip')
                (section / 'comparison.json').write_bytes(b'{}')

    def test_transition_rejects_unknown_new_member(self):
        with self.assertRaises(ValueError):
            with self.q.public_output_boundary(self.inv, self.permit, 'section-open', 'public'):
                section = self.root / 'public-runtime/collection/public'
                section.mkdir()
                (section / 'roster.json').write_bytes(b'[]')
                (section / 'unexpected').write_bytes(b'x')

    def test_section_close_requires_completed_cases(self):
        self.open_section()
        with self.assertRaises(ValueError):
            with self.q.public_output_boundary(self.inv, self.permit, 'section-close', 'public'):
                pass

    def test_operation_seals_four_files_but_keeps_case_active(self):
        self.open_section()
        with self.q.public_case_scope(self.inv, self.permit, 'public', 0):
            self.create_case()
            self.run['next'] = 1
            directory = self.root / 'operations/0001'
            m = self.q._public_output_operation_begin(self.inv, self.permit, 'public-runtime',
                    {'section': 'public', 'index': 0, 'variant': 'ordinary'}, directory)
            directory.mkdir()
            for name in ('command.json', 'stdout', 'stderr', 'result.json'):
                (directory / name).write_bytes(b'{}')
            self.q._public_output_operation_seal(m)
            self.assertIsNone(m['operation'])
            self.assertIsNotNone(m['case'])
            self.assertIn(str(directory / 'result.json'), m['sealed'])

    def test_operation_rejects_missing_or_extra_terminal_members(self):
        self.open_section()
        with self.assertRaises(ValueError):
            with self.q.public_case_scope(self.inv, self.permit, 'public', 0):
                self.create_case()
                self.run['next'] = 1
                directory = self.root / 'operations/0001'
                m = self.q._public_output_operation_begin(self.inv, self.permit, 'public-runtime',
                        {'section': 'public', 'index': 0, 'variant': 'ordinary'}, directory)
                directory.mkdir()
                (directory / 'failed-outcome.json').write_bytes(b'{}')
                self.q._public_output_operation_seal(m)

    def test_nonpublic_operation_not_optimized(self):
        for kind in ('generator', 'public-build', 'public-hosted-worker', 'parser-build', 'probe'):
            self.assertIsNone(self.q._public_output_operation_begin(
                self.inv, self.permit, kind, None, self.root / 'arbitrary'))

    def test_equal_stat_does_not_replace_consumer_body_identity(self):
        self.open_section()
        with self.q.public_case_scope(self.inv, self.permit, 'public', 0):
            base, source = self.create_case()
        wanted = self.monitor()['sealed'][str(source)]['sha256']
        source.write_bytes(b'xxxxxx')
        self.assertNotEqual(wanted, self.q.hashlib.sha256(source.read_bytes()).hexdigest())
        # A mocked equal-stat metadata sample could not prove this body equal;
        # existing authenticated consumer body verification remains mandatory.

    def test_due_audit_inside_transition_accepts_only_registered_additions(self):
        with self.q.public_output_boundary(self.inv, self.permit, 'section-open', 'public'):
            section = self.root / 'public-runtime/collection/public'
            section.mkdir()
            self.clock = 4.0
            self.q.public_output_tick(self.inv)
            (section / 'roster.json').write_bytes(b'[]')
        self.assertEqual(self.monitor()['last_audit'], 4.0)

    def test_body_sealing_services_due_metadata_audit(self):
        self.open_section()
        with self.q.public_case_scope(self.inv, self.permit, 'public', 0):
            base, source = self.create_case()
            before = self.q._out_stat(source.lstat())
            self.clock = 4.0
            digest = self.q._out_hash(source, before, self.monitor(), 100.0)
            self.assertEqual(digest, self.q.hashlib.sha256(b'source').hexdigest())
            self.assertEqual(self.monitor()['last_audit'], 4.0)

    def test_sibling_created_during_case_never_normalized_into_ledger(self):
        self.open_section()
        with self.q.public_case_scope(self.inv, self.permit, 'public', 0):
            self.create_case()
            sibling = self.root / 'public-runtime/collection/public/unregistered-case'
            sibling.mkdir()
        self.assertNotIn(str(sibling), self.monitor()['sealed'])
        self.clock = 4.0
        with self.assertRaises(ValueError):
            self.q.public_output_tick(self.inv)
        self.assertTrue(self.run['failed'])

    def test_seen_active_case_root_disappearance_rejects(self):
        self.open_section()
        with self.assertRaises(ValueError):
            with self.q.public_case_scope(self.inv, self.permit, 'public', 0):
                base, source = self.create_case()
                self.q.public_output_tick(self.inv)
                source.unlink()
                source.parent.rmdir()
                source.parent.parent.rmdir()
                (base / 'receipt.json').unlink()
                base.rmdir()
                self.q.public_output_tick(self.inv)

    def test_failure_audit_does_not_unpoison_or_promote(self):
        self.run['failed'] = True
        before = copy.deepcopy(self.monitor()['sealed'])
        result = self.q._public_output_failure_audit(self.monitor(), deadline=10.0)
        self.assertTrue(result['verified'])
        self.assertTrue(self.run['failed'])
        self.assertEqual(self.monitor()['sealed'], before)

    def test_failure_accessor_retains_actual_audit_without_repeating_it(self):
        first=self.q.public_output_failure(self.inv,self.permit)
        self.assertTrue(first['verified'])
        self.assertTrue(self.run['failed'])
        self.q._out_audit=Mock(side_effect=AssertionError('must retain first audit'))
        self.assertEqual(self.q.public_output_failure(self.inv,self.permit),first)
        with self.assertRaises(ValueError):self.q.public_output_failure(self.inv,object())

    def test_expired_failure_audit_reports_unverified(self):
        self.clock = 11.0
        result = self.q._public_output_failure_audit(self.monitor(), deadline=10.0)
        self.assertFalse(result['verified'])
        self.assertTrue(self.run['failed'])

    def test_three_second_due_keeps_five_second_hard_cap(self):
        self.clock = 2.9
        self.q.public_output_tick(self.inv)
        self.assertEqual(self.monitor()['last_audit'], 0.0)
        self.clock = 3.0
        self.q.public_output_tick(self.inv)
        self.assertEqual(self.monitor()['last_audit'], 3.0)

    def test_canonical_tick_hook_is_due_only_and_not_recursive(self):
        m = self.monitor()
        self.clock = 3.0
        m['auditing'] = True
        self.q._public_output_tick_active()
        self.assertEqual(m['last_audit'], 0.0)
        m['auditing'] = False
        self.q._public_output_tick_active()
        self.assertEqual(m['last_audit'], 3.0)
        m['closed'] = True
        self.clock = 20.0
        self.q._public_output_tick_active()
        self.assertEqual(m['last_audit'], 3.0)

    def test_auth_uses_owner_association_not_all_body_rehash(self):
        self.q._runtime_permit = Mock(side_effect=AssertionError('no all-body rehash at each transition'))
        self.open_section()
        with self.q.public_case_scope(self.inv, self.permit, 'public', 0):
            self.create_case()
        self.q._runtime_permit.assert_not_called()

    def test_canonical_prefix_keeps_component_boundary(self):
        self.assertTrue(self.q._out_under('/a/b/c', '/a/b'))
        self.assertTrue(self.q._out_under('/a/b', '/a/b'))
        self.assertFalse(self.q._out_under('/a/b-extra/c', '/a/b'))

    def test_native_prefilter_visits_exact_workspace_and_wrong_sibling(self):
        parent = self.root / 'tmp'
        workspace = parent / '.oxid-native-123-0'
        workspace.mkdir(mode=0o700)
        (workspace / 'program.ll').write_bytes(b'ir')
        wrong = parent / '.oxid-native-124-0'
        wrong.mkdir(mode=0o700)
        (parent / 'ordinary-file').write_bytes(b'x')
        reg = {'parent': {'path': str(parent)}, 'workspace': str(workspace)}
        self.q._out_snapshot(self.monitor(), [parent], deadline=10, registration=reg)
        visited = {call.args[0] for call in self.q._native_observed.call_args_list}
        self.assertEqual(visited, {workspace, workspace / 'program.ll', wrong})

    def test_failed_invocation_cannot_resume_after_partial_case(self):
        self.open_section()
        with self.assertRaises(RuntimeError):
            with self.q.public_case_scope(self.inv, self.permit, 'public', 0):
                self.create_case()
                raise RuntimeError('failed')
        with self.assertRaises(ValueError):
            self.q.public_output_tick(self.inv)

    def test_closed_collection_monitor_does_not_watch_later_phase_writes(self):
        self.monitor()['closed'] = True
        (self.root / 'later-authorized-parser-phase').mkdir()
        self.assertIsNone(self.q.public_output_tick(self.inv))
        self.assertFalse(self.run['failed'])



class IntegratedOutputHookControls(unittest.TestCase):
    def test_fresh_total_deadline_contract_is_fixed_without_changing_child_caps(self):
        text=(HERE/'preparation.py').read_text()
        self.assertIn("'deadline':time.monotonic()+36000",text)
        self.assertIn('0<timeout_seconds<=1800',text)
        self.assertIn('0<stream_limit_bytes<=8*1024**2',text)
        self.assertIn("'output_bytes':128*1024**2 if preparation else 6*1024**3",text)

    def test_monitor_hooks_bracket_actual_producer_writes_and_issue(self):
        text=(HERE/'preparation.py').read_text()
        owned=text[text.index('def _owned_probe('):text.index('def verify_probe(')]
        self.assertLess(owned.index('_public_output_operation_begin('),owned.index('directory.mkdir()'))
        self.assertLess(owned.index('_retain_terminal('),owned.index('_public_output_operation_seal('))
        self.assertLess(owned.index('_public_output_operation_seal('),owned.index('record=ProbeRecord('))
        supervise=text[text.index('def _supervise('):text.index('def _target_stat(')]
        self.assertLess(supervise.index('tree.close()'),supervise.index('_public_output_failure_audit('))
        self.assertIn("output_monitor['operation']['deadline']=deadline",supervise)
        self.assertIn("'output_bytes':None if output_monitor is not None else 0",supervise)
        self.assertIn("'public_output_ledger':_public_output_report(output_monitor)",supervise)

if __name__ == '__main__':
    unittest.main()
