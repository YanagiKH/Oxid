"""Deterministic source-only controls. No actual tool probe or build is run.

Pipeline tests replace only private I/O boundaries with labelled fixtures. Such
mocked admissions are confined to this test module and are not execution proof.
"""
import copy
import contextlib
import hashlib
import io
import json
import os
import subprocess
import sys
import tarfile
import tempfile
import time
import types
import unittest
from pathlib import Path
from unittest import mock

sys.dont_write_bytecode = True
PATH = Path(__file__).absolute().with_name('machine.py')
machine = types.ModuleType('machine_source_controls')
machine.__file__ = str(PATH)
exec(compile(PATH.read_bytes(), str(PATH), 'exec'), machine.__dict__)


class FakeProbe:
    def __init__(self, document, streams):
        self._document, self._streams = document, streams

    def document(self):
        return copy.deepcopy(self._document)

    def stdout(self):
        return self._streams['stdout']

    def stderr(self):
        return self._streams['stderr']


class FakeAPI:
    """Owned-probe test double. It never starts a process."""
    def __init__(self, root, expected_rust):
        self.root, self.expected_rust = root, expected_rust
        self.calls, self.records = [], []
        self.change = None
        self.read_scopes = 0
        self.scope_events = []
        self.scope_failed = False

    @contextlib.contextmanager
    def checked_scope(self, invocation):
        self.read_scopes += 1
        self.scope_events.append(('enter', invocation))
        try:
            yield
        except BaseException:
            self.scope_failed = True
            raise
        finally:
            self.scope_events.append(('exit', invocation))
            self.read_scopes -= 1

    def check_deadline(self, invocation):
        return True

    def owned_probe(self, invocation, argv, cwd, environment, timeout_seconds, stream_limit_bytes):
        if self.read_scopes:
            raise AssertionError('owned operation inside read-only checkpoint')
        self.calls.append(list(argv))
        if argv[1:4] == ['which', '--toolchain', '1.99.0']:
            stdout = str(self.root / 'bin' / argv[-1]) + '\n'
        elif Path(argv[0]).name == 'rustc' and argv[1:] == ['-Vv']:
            stdout = self.expected_rust
        elif Path(argv[0]).name == 'cargo':
            stdout = 'cargo 1.99.0 (fixture)\nrelease: 1.99.0\nhost: x86_64-unknown-linux-gnu\n'
        elif argv[1:] == ['--print', 'sysroot']:
            stdout = str(self.root) + '\n'
        else:
            stdout = 'fixture native compiler 1\n'
        now = time.time_ns()
        result = {'argv': list(argv), 'cwd': str(cwd), 'environment': dict(environment),
                  'pid': len(self.calls), 'status': 0, 'started_ns': now, 'completed_ns': now,
                  'timed_out': False, 'stream_limit_exceeded': False, 'cleanup_complete': True,
                  'timeout_seconds': timeout_seconds, 'stream_limit_bytes': stream_limit_bytes}
        streams = {'stdout': stdout.encode(), 'stderr': b''}
        for name, raw in streams.items():
            result[name] = {'path': '/mock-only/' + str(len(self.calls)) + '/' + name,
                            'bytes': len(raw), 'sha256': hashlib.sha256(raw).hexdigest()}
        if self.change:
            self.change(result, streams)
        record = FakeProbe(result, streams)
        self.records.append(record)
        return record

    def verify_probe(self, record, invocation, **expected):
        if record not in self.records:
            raise machine.Rejected('mock probe owner mismatch')
        for key, value in expected.items():
            if record._document[key] != value:
                raise machine.Rejected('mock probe context differs')
        return record.document()


class SourceControls(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name).resolve()

    def test_import_and_unsealed_capture_launch_nothing(self):
        fresh = types.ModuleType('inert_machine')
        fresh.__file__ = str(PATH)
        with mock.patch.object(subprocess, 'Popen', side_effect=AssertionError('actual process forbidden')):
            exec(compile(PATH.read_bytes(), str(PATH), 'exec'), fresh.__dict__)
            with mock.patch.object(fresh, 'SHARED_MANIFEST_SHA256', None), \
                 mock.patch.object(fresh, '_body', side_effect=AssertionError('unsealed loader must fail before reading')):
                with self.assertRaises(fresh.NotReady):
                    fresh.capture_machine(object())

    def test_sealed_shared_loader_reuses_authenticated_owner_without_execution(self):
        shared = PATH.parent.parent / 'package_dependency_unit4_current'
        self.assertEqual(hashlib.sha256((shared / 'package-manifest.json').read_bytes()).hexdigest(),
                         machine.SHARED_MANIFEST_SHA256)
        with mock.patch.object(subprocess, 'Popen', side_effect=AssertionError('shared loading cannot execute a probe')):
            api = machine._shared()
            self.assertIs(api, machine._shared())
        self.assertIs(api, sys.modules[machine.SHARED_KEY])
        self.assertEqual(api.__file__, str(shared / 'preparation.py'))
        self.assertEqual(api.__source_sha256__, hashlib.sha256((shared / 'preparation.py').read_bytes()).hexdigest())
        self.assertTrue(callable(api.owned_probe))
        self.assertTrue(callable(api.verify_probe))

    def test_no_dictionary_boolean_or_constructor_issues_authority(self):
        for value in ({'admitted': True}, {'execution_qualified': True}, True, object()):
            with self.subTest(value=value), self.assertRaises(machine.Rejected), \
                 mock.patch.object(machine, '_shared', side_effect=AssertionError('forged admission reached shared loader')):
                machine.verify_machine(value, object())
        for args in ((), ({'status': 'pass'},), ({}, True)):
            with self.assertRaises(TypeError):
                machine.ToolAdmission(*args)
        forged = object.__new__(machine.ToolAdmission)
        with self.assertRaises(machine.Rejected):
            forged.document()
        with self.assertRaises(machine.Rejected), \
             mock.patch.object(machine, '_shared', side_effect=AssertionError('forged admission reached shared loader')):
            machine.verify_machine(forged, object())
        with self.assertRaises(machine.NotReady):
            machine.load_machine({'status': 'pass'})

    def test_clean_environment_drops_all_unreviewed_overrides(self):
        for path in ('cargo', 'rustup'):
            (self.root / path).mkdir()
        inputs = {'HOME': str(self.root), 'CARGO_HOME': str(self.root / 'cargo'),
                  'RUSTUP_HOME': str(self.root / 'rustup'), 'PATH': '/evil:/usr/bin',
                  'RUSTFLAGS': '--cfg hostile', 'RUSTC_WRAPPER': '/evil/wrapper',
                  'CARGO_PROFILE_RELEASE_OPT_LEVEL': '0', 'GITHUB_TOKEN': 'secret',
                  'LD_PRELOAD': '/evil/inject.so', 'RUSTUP_TOOLCHAIN': 'nightly'}
        env = machine._clean_environment(inputs, {'name': 'Linux x86_64'})
        self.assertEqual(env['RUSTUP_TOOLCHAIN'], '1.99.0')
        self.assertEqual(env['PATH'], '/usr/bin:/bin')
        self.assertEqual(env['CARGO_BUILD_JOBS'], '2')
        self.assertEqual(env['CARGO_INCREMENTAL'], '0')
        for key in ('RUSTFLAGS', 'RUSTC_WRAPPER', 'CARGO_PROFILE_RELEASE_OPT_LEVEL', 'GITHUB_TOKEN', 'LD_PRELOAD'):
            self.assertNotIn(key, env)

    def test_actual_host_measurement_covers_exact_four_hosts(self):
        aliases = [('Linux', 'x86_64', 'Linux x86_64'), ('Darwin', 'x86_64', 'macOS x86_64'),
                   ('Darwin', 'arm64', 'macOS arm64'), ('Windows', 'AMD64', 'Windows x86_64')]
        for system, arch, wanted in aliases:
            uname = types.SimpleNamespace(system=system, machine=arch, release='fixture', version='fixture')
            with mock.patch.object(machine.platform, 'uname', return_value=uname), mock.patch.object(machine, '_route', return_value={}):
                self.assertEqual(machine.measured_host()['target'], machine.HOSTS[wanted])
        uname = types.SimpleNamespace(system='Linux', machine='aarch64', release='fixture', version='fixture')
        with mock.patch.object(machine.platform, 'uname', return_value=uname), self.assertRaises(machine.Rejected):
            machine.measured_host()

    def test_linux_helper_which_route_is_captured_without_probe(self):
        seen = []
        def selected(name, path):
            seen.append(name)
            binary = {'cc': 'x86_64-linux-gnu-gcc-12', 'c++': 'x86_64-linux-gnu-g++-12'}.get(name, name)
            return {'selected': '/fixture/' + name, 'resolved': '/fixture/' + binary,
                    'body': {'sha256': hashlib.sha256(binary.encode()).hexdigest()}}
        env = {'PATH': '/fixture'}
        with mock.patch.object(machine, '_which', side_effect=selected), \
             mock.patch.object(machine, '_tree', side_effect=lambda path: {'root': str(path), 'rows': []}), \
             mock.patch.object(machine, '_route', side_effect=lambda path: {'selected': str(path)}), \
             mock.patch.object(Path, 'glob', return_value=[Path('/fixture/libc.so')]), \
             mock.patch.object(Path, 'is_file', return_value=False), \
             mock.patch.object(Path, 'is_dir', return_value=False), \
             mock.patch.object(machine, '_probe', side_effect=AssertionError('which capture must not probe')):
            record = machine._native_inputs({'name': 'Linux x86_64', 'target': 'x86_64-unknown-linux-gnu'}, env)
        self.assertEqual(seen.count('which'), 1)
        self.assertEqual(record['tools']['which']['resolved'], '/fixture/which')
        self.assertEqual(record['tools']['which']['body']['sha256'], hashlib.sha256(b'which').hexdigest())

    def test_config_absence_covers_actual_views_and_all_ancestors(self):
        repo, run = self.root / 'repo', self.root / 'run'
        cargo = self.root / 'cargo'
        for path in (repo, run, cargo):
            path.mkdir()
        env = {'HOME': str(self.root), 'CARGO_HOME': str(cargo)}
        rows = machine._config_absence(repo, run, env)
        paths = {r['path'] for r in rows}
        self.assertIn(str(run / '.cargo/config.toml'), paths)
        self.assertIn(str(run / 'source-preparation/public/ordinary/.cargo/config'), paths)
        self.assertIn(str(run / 'source-preparation/public/lifecycle/.cargo/config.toml'), paths)
        self.assertIn(str(cargo / 'config.toml'), paths)
        (run / '.cargo').mkdir()
        (run / '.cargo/config.toml').write_text('[build]\nrustc="attacker"\n')
        with self.assertRaises(machine.Rejected):
            machine._config_absence(repo, run, env)

    def test_route_records_alias_and_detects_body_and_link_replacement(self):
        body, alias = self.root / 'body', self.root / 'tool'
        body.write_bytes(b'fixture executable')
        alias.symlink_to(body)
        first = machine._route(alias)
        self.assertEqual(first['selected'], str(alias))
        self.assertEqual(first['resolved'], str(body))
        body.write_bytes(b'changed executable')
        self.assertNotEqual(first, machine._route(alias))
        other = self.root / 'other'
        other.write_bytes(b'fixture executable')
        alias.unlink(); alias.symlink_to(other)
        self.assertNotEqual(first, machine._route(alias))

    def test_tree_captures_membership_and_rejects_link_cycles(self):
        root = self.root / 'tree'; root.mkdir()
        (root / 'body').write_bytes(b'one')
        first = machine._tree(root)
        (root / 'extra').write_bytes(b'two')
        self.assertNotEqual(first, machine._tree(root))
        (root / 'cycle').symlink_to(root, target_is_directory=True)
        with self.assertRaises(machine.Rejected):
            machine._tree(root)

    def _crate_fixture(self):
        repo = self.root / 'repo'; repo.mkdir()
        cargo = self.root / 'cargo'
        registry = cargo / 'registry'
        registry_name = 'index.crates.io-1949cf8c6b5b557f'
        source = registry / 'src' / registry_name / 'fixture-1.0.0'
        source.mkdir(parents=True)
        files = {'Cargo.toml': b'[package]\nname="fixture"\nversion="1.0.0"\n',
                 'src/lib.rs': b'pub fn fixture() {}\n', 'build.rs': b'fn main() {}\n'}
        output = io.BytesIO()
        with tarfile.open(fileobj=output, mode='w:gz') as tar:
            for name, raw in files.items():
                info = tarfile.TarInfo('fixture-1.0.0/' + name)
                info.size = len(raw)
                tar.addfile(info, io.BytesIO(raw))
                path = source / name; path.parent.mkdir(exist_ok=True)
                path.write_bytes(raw)
        archive = registry / 'cache' / registry_name / 'fixture-1.0.0.crate'
        archive.parent.mkdir(parents=True)
        archive.write_bytes(output.getvalue())
        checksum = hashlib.sha256(output.getvalue()).hexdigest()
        (source / '.cargo-ok').write_bytes(b'{"v":1}')
        (source / '.cargo-checksum.json').write_text(json.dumps({'package': checksum,
            'files': {n: hashlib.sha256(b).hexdigest() for n, b in files.items()}}))
        index = registry / 'index' / registry_name
        (index / '.cache/fi/xt').mkdir(parents=True)
        (index / '.cache/fi/xt/fixture').write_bytes(b'fixture index')
        (index / 'config.json').write_bytes(b'{}')
        (repo / 'Cargo.lock').write_text('version = 4\n[[package]]\nname = "oxid"\nversion = "0.9.0"\n'
            '[[package]]\nname = "fixture"\nversion = "1.0.0"\n'
            'source = "registry+https://github.com/rust-lang/crates.io-index"\nchecksum = "' + checksum + '"\n')
        return repo, cargo, source, archive

    def test_locked_archive_authenticates_full_source_and_build_script(self):
        repo, cargo, source, archive = self._crate_fixture()
        closure = machine._dependency_closure(repo, cargo)
        self.assertEqual(len(closure), 1)
        (source / 'build.rs').write_bytes(b'fn main() { hostile(); }\n')
        metadata = json.loads((source / '.cargo-checksum.json').read_text())
        metadata['files']['build.rs'] = hashlib.sha256((source / 'build.rs').read_bytes()).hexdigest()
        (source / '.cargo-checksum.json').write_text(json.dumps(metadata))
        with self.assertRaises(machine.Rejected):
            machine._dependency_closure(repo, cargo)

    def test_locked_archive_and_extra_source_mutations_reject(self):
        repo, cargo, source, archive = self._crate_fixture()
        original = archive.read_bytes()
        archive.write_bytes(original + b'changed')
        with self.assertRaises(machine.Rejected):
            machine._dependency_closure(repo, cargo)
        archive.write_bytes(original)
        (source / 'extra.rs').write_bytes(b'extra')
        with self.assertRaises(machine.Rejected):
            machine._dependency_closure(repo, cargo)

    def test_linux_original_compiler_map_never_becomes_four_host_authority(self):
        root = self.root / 'toolchain'
        for name in ('bin/cargo', 'bin/rustc', 'lib/libLLVM.so',
                     'lib/rustlib/x86_64-unknown-linux-gnu/lib/libstd.rlib'):
            p = root / name; p.parent.mkdir(parents=True, exist_ok=True); p.write_bytes(name.encode())
        files = [{'path': p.relative_to(root).as_posix(), 'bytes': p.stat().st_size,
                  'sha256': hashlib.sha256(p.read_bytes()).hexdigest()}
                 for p in sorted(root.rglob('*')) if p.is_file()]
        frozen = {'compiler_files': files, 'recipe': {'target': machine.HOSTS['Linux x86_64']}}
        host = {'name': 'Linux x86_64', 'target': machine.HOSTS['Linux x86_64']}
        record = machine._rust_inputs(root, host, frozen, parser_required=True)
        self.assertEqual(record['parser_original_authority']['scope'], 'Linux x86_64 parser only')
        (root / 'lib/rustlib/x86_64-unknown-linux-gnu/lib/extra.rlib').write_bytes(b'changed')
        with self.assertRaises(machine.Rejected):
            machine._rust_inputs(root, host, frozen, parser_required=True)
        mac = {'name': 'macOS arm64', 'target': machine.HOSTS['macOS arm64']}
        (root / 'lib/rustlib' / mac['target'] / 'lib').mkdir(parents=True)
        record = machine._rust_inputs(root, mac, frozen)
        self.assertIsNone(record['parser_original_authority'])
        with self.assertRaises(machine.Rejected):
            machine._rust_inputs(root, mac, frozen, parser_required=True)

    def test_optional_checksum_metadata_does_not_replace_archive_verification(self):
        repo, cargo, source, archive = self._crate_fixture()
        (source / '.cargo-checksum.json').unlink()
        self.assertEqual(len(machine._dependency_closure(repo, cargo)), 1)
        (source / '.cargo-ok').write_bytes(b'forged marker')
        with self.assertRaises(machine.Rejected):
            machine._dependency_closure(repo, cargo)

    def test_scope_derived_source_closure_accepts_parser_only(self):
        repo = self.root / 'repo'; repo.mkdir()
        rows = []
        for name in ('Cargo.toml', 'Cargo.lock', 'build.rs', 'native/fixture.c'):
            path = repo / name; path.parent.mkdir(exist_ok=True)
            path.write_bytes(('fixture ' + name).encode())
            rows.append({'path': name, 'bytes': path.stat().st_size,
                         'sha256': hashlib.sha256(path.read_bytes()).hexdigest()})
        frozen_path = repo / machine.FROZEN_PATH
        frozen_path.parent.mkdir(parents=True)
        frozen_path.write_bytes(b'{"fixture":true}')
        captured = {'current_source_manifest': {'sha256': machine.SOURCE_SHA256},
                    'checkout': {'path': str(repo)}, 'maps': {
                        'parser_observed': copy.deepcopy(rows), 'parser_control': copy.deepcopy(rows)}}
        with mock.patch.object(machine, 'FROZEN_SHA256', hashlib.sha256(frozen_path.read_bytes()).hexdigest()):
            actual, closure, frozen = machine._source_authority(captured)
        self.assertEqual(actual, repo)
        self.assertIn(str(repo / 'build.rs'), [r['path'] for r in closure])
        changed = copy.deepcopy(captured)
        row = next(r for r in changed['maps']['parser_control'] if r['path'] == 'Cargo.lock')
        row['sha256'] = '0' * 64
        with self.assertRaises(machine.Rejected):
            machine._source_authority(changed)

    def test_ancestor_workspace_rejects_before_any_probe(self):
        root = self.root / 'run'
        source = root / 'source-preparation/public/ordinary'
        source.mkdir(parents=True)
        (source / 'Cargo.toml').write_bytes(b'approved own package')
        rows = machine._workspace_absence(root)
        self.assertIn(str(root / 'Cargo.toml'), {r['path'] for r in rows})
        (root / 'source-preparation/Cargo.toml').write_bytes(b'[workspace]\nmembers=["public/ordinary"]')
        with self.assertRaises(machine.Rejected):
            machine._workspace_absence(root)

    def test_file_chunks_and_archive_reads_check_deadline(self):
        called = []
        def stop():
            called.append(True)
            raise machine.Rejected('fixture deadline')
        token = machine._CHECKPOINT.set(stop)
        try:
            path = self.root / 'file'; path.write_bytes(b'fixture')
            with self.assertRaises(machine.Rejected):
                machine._file(path)
            with self.assertRaises(machine.Rejected):
                machine._ArchiveReader(b'fixture').read(1)
            with self.assertRaises(machine.Rejected):
                machine._tree(self.root)
        finally:
            machine._CHECKPOINT.reset(token)
        self.assertEqual(len(called), 3)
        api = types.SimpleNamespace(check_deadline=lambda invocation: None)
        with mock.patch.object(machine.time, 'monotonic', side_effect=(0, 301)):
            with self.assertRaises(machine.Rejected), machine._deadline(api, object(), 300):
                self.fail('expired deadline admitted')

    def test_windows_1100_is_only_valid_exact_linker_help(self):
        text = ('Microsoft (R) Incremental Linker Version 14.44.35219.0\n'
                'Copyright (C) Microsoft Corporation.  All rights reserved.\n\n'
                ' usage: LINK [options] [files] [@commandfile]\n\n   options:\n\n'
                '      /OUT:filename\n      /MACHINE:machine\n')
        record = {'status': 1100, 'stdout': text, 'stderr': ''}
        machine._windows_help(record)
        for key, value in (('status', 1), ('stderr', 'error'), ('stdout', text + 'fatal error LNK1000\n'),
                           ('stdout', text.replace('Incremental Linker', 'Other Tool'))):
            changed = dict(record, **{key: value})
            with self.subTest(key=key), self.assertRaises(machine.Rejected):
                machine._windows_help(changed)


class MockPipelineControls(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name).resolve()
        self.toolchain = self.root / 'toolchain'
        (self.toolchain / 'bin').mkdir(parents=True)
        for name in ('cargo', 'rustc'):
            (self.toolchain / 'bin' / name).write_bytes(('fixture-' + name).encode())
        self.rust = ('rustc 1.99.0 (b940084d7 2026-09-28)\nbinary: rustc\n'
                     'commit-hash: b940084d7eb6a299eb4bfeb8e34901bc051e7ac4\n'
                     'commit-date: 2026-09-28\nhost: x86_64-unknown-linux-gnu\nrelease: 1.99.0\nLLVM version: 23.1.1\n')
        self.api = FakeAPI(self.toolchain, self.rust)
        self.invocation = object()
        self.resolver = {'selected': '/fixture/rustup', 'resolved': '/fixture/rustup'}
        self.native = {name: {'resolved': '/fixture/' + name} for name in ('cc', 'cxx', 'ar', 'linker', 'which')}
        self.environment = {'PATH': str(self.toolchain / 'bin') + ':/usr/bin:/bin',
                            'CARGO_BUILD_JOBS': '2', 'CARGO_INCREMENTAL': '0'}
        self.frozen = {'recipe': {'rustc_verbose': self.rust, 'target': 'x86_64-unknown-linux-gnu'}}
        self.snapshot_calls = []
        self.drift = False
        self.addCleanup(mock.patch.stopall)
        mock.patch.object(machine, '_shared', return_value=self.api).start()
        mock.patch.object(machine, '_which', return_value=self.resolver).start()
        mock.patch.object(machine, '_snapshot', side_effect=self.snapshot).start()

    def snapshot(self, api, invocation, toolchain=None, resolver=None, direct_discovery=None):
        self.snapshot_calls.append((toolchain, resolver))
        rust = None if toolchain is None else {
            'root': str(self.toolchain),
            'tools': {n: machine._route(self.toolchain / 'bin' / n) for n in ('cargo', 'rustc')},
            'parser_original_authority': {'recipe': {'build_environment': {'CARGO_PROFILE_DEV_DEBUG': '0'}}}}
        state = {'invocation': {'run_root': str(self.root / 'run'), 'nonce': 'fixture'},
                 'checkout': {'path': str(self.root / 'repo')},
                 'environment': dict(self.environment), 'host': {'name': 'Linux x86_64', 'target': 'x86_64-unknown-linux-gnu',
                    'python': {'selected': '/fixture/python3', 'resolved': '/fixture/python3'}},
                 'producer_environment': {'PATH': '/fixture'}, 'native': {'tools': self.native},
                 'resolver': resolver, 'rust': rust, 'fixture_drift': self.drift,
                 'direct_discovery': direct_discovery,
                 'source_scope': 'combined', 'source_roles': list(machine.ROLES)}
        return state, self.frozen

    def test_capture_order_direct_probes_and_terminal_reread(self):
        with mock.patch.object(subprocess, 'Popen', side_effect=AssertionError('actual probe forbidden')):
            admission = machine.capture_machine(self.invocation)
            self.assertTrue(machine.verify_machine(admission, self.invocation))
        self.assertEqual(self.api.calls[:2], [['/fixture/rustup', 'which', '--toolchain', '1.99.0', 'cargo'],
                                             ['/fixture/rustup', 'which', '--toolchain', '1.99.0', 'rustc']])
        self.assertEqual(self.api.calls[2], [str(self.toolchain / 'bin/rustc'), '-Vv'])
        self.assertEqual(self.api.calls[4], [str(self.toolchain / 'bin/rustc'), '--print', 'sysroot'])
        self.assertEqual(len(self.api.calls), 8)
        self.assertGreaterEqual(len(self.snapshot_calls), 6)
        d = admission.document()
        self.assertFalse(d['hermetic']); self.assertFalse(d['network_isolated'])
        self.assertFalse(d['execution_qualified']); self.assertFalse(d['runtime_native_admitted'])
        self.assertEqual(d['rustc_verbose'], self.rust)
        d['snapshot']['host']['name'] = 'forged'
        self.assertEqual(admission.document()['snapshot']['host']['name'], 'Linux x86_64')
        self.assertIs(admission.invocation(), self.invocation)
        self.assertEqual(admission.tool('cargo'), str(self.toolchain / 'bin/cargo'))
        self.assertEqual(admission.tool('which'), '/fixture/which')
        with self.assertRaises(machine.Rejected):
            machine.verify_machine(admission, object())
        self.drift = True
        with self.assertRaises(machine.Rejected):
            machine.verify_machine(admission, self.invocation)

    def test_direct_producer_tools_do_not_require_rustup(self):
        def discovery(name, path):
            if name not in ('cargo', 'rustc'):
                raise AssertionError('direct tool capture must not use rustup')
            return machine._route(self.toolchain / 'bin' / name)
        with mock.patch.object(machine, '_which', side_effect=discovery):
            admission = machine.capture_machine(self.invocation)
            self.assertTrue(machine.verify_machine(admission, self.invocation))
        self.assertEqual(admission.document()['tool_discovery'], 'producer-direct')
        self.assertIsNone(admission.document()['snapshot']['resolver'])
        self.assertEqual(len(self.api.calls), 6)
        self.assertTrue(all('which' not in argv for argv in self.api.calls))

    def test_environment_is_defensive_and_target_cannot_overlap_source(self):
        admission = machine.capture_machine(self.invocation)
        target = self.root / 'run/build-ordinary/target'
        env = admission.environment('ordinary', 'debug', target)
        self.assertEqual(env['CARGO_TARGET_DIR'], str(target))
        self.assertNotIn('CARGO_PROFILE_DEV_DEBUG', env)
        env['PATH'] = 'forged'
        self.assertNotEqual(admission.environment('ordinary', 'debug', target)['PATH'], 'forged')
        parser = admission.environment('parser_observed', 'debug', self.root / 'run/build-parser/target')
        self.assertEqual(parser['CARGO_PROFILE_DEV_DEBUG'], '0')
        for bad in (self.root / 'elsewhere', self.root / 'run/source-preparation',
                    self.root / 'run/source-preparation/public/ordinary/target'):
            with self.assertRaises(machine.Rejected):
                admission.environment('ordinary', 'debug', bad)

    def test_failed_incomplete_or_wrong_version_probe_never_issues(self):
        before = len(machine._ISSUED)
        for field, value in (('status', 1), ('status', False), ('pid', 0), ('timed_out', True),
                             ('stream_limit_exceeded', True), ('cleanup_complete', False),
                             ('started_ns', 0), ('stream_limit_bytes', 1)):
            self.api.change = lambda d, s, f=field, v=value: d.update({f: v})
            with self.subTest(field=field), self.assertRaises(machine.Rejected):
                machine.capture_machine(self.invocation)
        self.api.change = None
        self.api.expected_rust = self.rust.replace('1.99.0', '1.98.0')
        with self.assertRaises(machine.Rejected):
            machine.capture_machine(self.invocation)
        self.assertEqual(len(machine._ISSUED), before)

    def test_full_machine_rederives_projection_even_if_its_local_digest_was_changed(self):
        admission = machine.capture_machine(self.invocation)
        record = machine._ISSUED[admission]
        state = machine.decode(record['runtime_projection'])
        state['host']['name'] = 'forged host'
        record['runtime_projection'] = machine.canonical(state)
        record['runtime_projection_sha256'] = machine.sha(record['runtime_projection'])
        with self.assertRaisesRegex(machine.Rejected, 'projection differs from original full machine carrier'):
            machine.verify_machine(admission, self.invocation)

    def test_terminal_drift_and_probe_evidence_drift_reject(self):
        def mutate_on_version(document, streams):
            if document['argv'][1:] == ['--print', 'sysroot']:
                self.drift = True
        self.api.change = mutate_on_version
        with self.assertRaises(machine.Rejected):
            machine.capture_machine(self.invocation)
        self.drift = False; self.api.change = None
        admission = machine.capture_machine(self.invocation)
        self.api.records[-1]._streams['stdout'] = b'changed evidence'
        with self.assertRaises(machine.Rejected):
            machine.verify_machine(admission, self.invocation)


class RuntimeAPI(FakeAPI):
    """Creates tiny inert fixture bytes, never a tool process or compilation."""
    def __init__(self, root, expected_rust):
        super().__init__(root, expected_rust)
        self.host = 'Linux x86_64'
        self.runtime_change = None

    @staticmethod
    def artifact(host):
        if host == 'Linux x86_64':
            raw = bytearray(64)
            raw[:7] = b'\x7fELF\x02\x01\x01'
            raw[16:18], raw[18:20], raw[20:24], raw[52:54] = (2).to_bytes(2, 'little'), (62).to_bytes(2, 'little'), (1).to_bytes(4, 'little'), (64).to_bytes(2, 'little')
        elif host == 'Windows x86_64':
            raw = bytearray(154)
            raw[:2], raw[60:64] = b'MZ', (128).to_bytes(4, 'little')
            raw[128:132], raw[132:134], raw[152:154] = b'PE\0\0', b'\x64\x86', b'\x0b\x02'
        else:
            raw = bytearray(32)
            raw[:4] = b'\xcf\xfa\xed\xfe'
            raw[4:8] = (0x01000007 if host == 'macOS x86_64' else 0x0100000c).to_bytes(4, 'little')
            raw[12:16] = (2).to_bytes(4, 'little')
        return bytes(raw)

    def owned_probe(self, invocation, argv, cwd, environment, timeout_seconds, stream_limit_bytes,
                    operation_kind=None):
        if operation_kind is None:
            return super().owned_probe(invocation, argv, cwd, environment, timeout_seconds, stream_limit_bytes)
        if self.read_scopes:
            raise AssertionError('owned operation inside read-only checkpoint')
        self.calls.append(list(argv))
        started = time.time_ns()
        stdout, stderr, status = b'', b'', 0
        if operation_kind == 'runtime-trap-build':
            output = Path(argv[-1])
            output.write_bytes(self.artifact(self.host))
            output.chmod(0o755)
            # Coarse backing filesystem times are irrelevant to a source-only
            # fixture; explicitly put this mock output inside its mock window.
            now = time.time_ns()
            os.utime(output, ns=(now, now))
        elif operation_kind == 'runtime-trap-control':
            Path(environment['UNIT4_TOOL_LOG']).write_bytes(b'{"unexpected_native_tool":true}\n')
            status, stderr = 121, b'unexpected native tool invocation\n'
        elif operation_kind == 'runtime-tool-version':
            marker = dict(zip(machine.LLVM_TOOLS, machine.LLVM_MARKERS))[Path(argv[0]).name]
            stdout = (marker + '\n').encode()
        else:
            raise AssertionError('unreviewed runtime fixture operation')
        result = {'argv': list(argv), 'cwd': str(cwd), 'environment': dict(environment),
                  'pid': len(self.calls), 'status': status, 'started_ns': started,
                  'completed_ns': time.time_ns(), 'timed_out': False, 'failure': None,
                  'stream_limit_exceeded': False, 'cleanup_complete': True,
                  'timeout_seconds': timeout_seconds, 'stream_limit_bytes': stream_limit_bytes,
                  'operation_kind': operation_kind}
        streams = {'stdout': stdout, 'stderr': stderr}
        for name, raw in streams.items():
            result[name] = {'path': '/runtime-mock/' + str(len(self.calls)) + '/' + name,
                            'bytes': len(raw), 'sha256': machine.sha(raw)}
        if self.runtime_change:
            self.runtime_change(result, streams)
        record = FakeProbe(result, streams)
        self.records.append(record)
        return record


_REAL_VERIFY_MACHINE = machine._verify_machine


class RuntimeSourceControls(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name).resolve()
        self.run = self.root / 'run'
        self.run.mkdir()
        self.repo = self.root / 'repo'
        self.helper = self.repo / machine.PUBLIC_RUNTIME_PATH
        self.helper.mkdir(parents=True)
        # Labelled inert bodies exist only at the private I/O test boundary.
        # No test depends on another checkout, imports a helper, or executes it.
        self.fixture_helpers = {}
        for name in machine.RUNTIME_HELPERS:
            raw = ('inert source-only runtime helper fixture: ' + name + '\n').encode()
            (self.helper / name).write_bytes(raw)
            self.fixture_helpers[name] = machine.sha(raw)
        self.real_runtime_helpers = machine._runtime_helpers
        self.toolchain = self.root / 'rust'
        (self.toolchain / 'bin').mkdir(parents=True)
        (self.toolchain / 'lib').mkdir()
        for name in ('rustc', 'cargo'):
            (self.toolchain / 'bin' / name).write_bytes(b'inert fixture, never execute')
        self.invocation = object()
        self.rust_version = 'rustc 1.99.0 fixture identity\n'
        self.api = RuntimeAPI(self.toolchain, self.rust_version)
        self.admission = object.__new__(machine.ToolAdmission)
        self.state = {'invocation': {'run_root': str(self.run), 'nonce': 'source-only-runtime-fixture'},
                      'checkout': {'path': str(self.repo)}, 'source_roles': list(machine.ROLES),
                      'source_scope': 'combined', 'producer_environment': {},
                      'configuration_absence': [], 'ancestor_workspace_absence': [],
                      'resolver': None, 'resolver_settings': [], 'direct_discovery': None,
                      'environment': {'PATH': '/usr/bin:/bin', 'RUSTC': str(self.toolchain / 'bin/rustc')},
                      'host': {'name': 'Linux x86_64', 'python': machine._route(Path(sys.executable).absolute())},
                      'native': {'tools': {}, 'files': [], 'trees': []},
                      'rust': {'root': str(self.toolchain), 'shared_libraries': [], 'sysroot_trees': [],
                               'tools': {n: machine._route(self.toolchain / 'bin' / n) for n in ('rustc', 'cargo')},
                               'parser_original_authority': {'scope': 'source-only fixture', 'compiler_files': [
                                   {'path': 'bin/' + n, **{k: machine._file(self.toolchain / 'bin' / n)[k]
                                                          for k in ('bytes', 'sha256')}} for n in ('cargo', 'rustc')]}}}
        self.set_admission()
        self.addCleanup(lambda: machine._ISSUED.pop(self.admission, None))
        self.patches = [mock.patch.object(machine, '_shared', return_value=self.api),
                        mock.patch.object(machine, '_verify_machine', return_value=True),
                        mock.patch.object(machine, '_runtime_helpers', side_effect=self.mock_helpers),
                        mock.patch.object(subprocess, 'Popen', side_effect=AssertionError('real execution forbidden'))]
        for patch in self.patches:
            patch.start()
            self.addCleanup(patch.stop)

    def mock_helpers(self, state):
        machine.need({'ordinary', 'lifecycle'} <= set(state['source_roles']),
                     'mock public source roles missing')
        rows = {}
        for name, expected in sorted(self.fixture_helpers.items()):
            row = machine._file(self.helper / name)
            machine.need(row['sha256'] == expected, 'mock helper body changed')
            rows[name] = row
        return {'root': str(self.helper), 'files': rows, 'machine_consumer': machine._file(PATH)}

    def set_admission(self):
        raw = machine.canonical({'snapshot': self.state, 'rustc_verbose': self.rust_version,
                                 'runtime_native_admitted': False, 'execution_qualified': False,
                                 'probes': [], 'probe_records': []})
        projection = machine._runtime_projection(self.state)
        machine._ISSUED[self.admission] = {'raw': raw, 'raw_sha256': machine.sha(raw),
            'runtime_projection': projection, 'runtime_projection_sha256': machine.sha(projection),
            'invocation': self.invocation, 'probes': []}

    def native_inputs(self):
        llvm = self.root / 'llvm'
        llvm.mkdir()
        for name in machine.LLVM_TOOLS:
            (llvm / name).write_bytes(('inert mock ' + name).encode())
        libs = self.root / 'libraries'
        libs.mkdir()
        receipt = self.root / 'stage.json'
        receipt.write_bytes(b'{}')
        selected = {'carrier': {'receipt': machine._binding(receipt), 'library_directory': str(libs),
                                'content_sha256': machine.LLVM_CONTENT_SHA256, 'selected_libraries': [],
                                'dependency_scope': 'source-only fixture'},
                    'receipt_file': machine._file(receipt), 'library_routes': [],
                    'receipt_producer_authenticated': False, 'hosted_staging_admitted': False}
        return llvm, libs, receipt, selected

    def test_runtime_constructor_dictionary_and_cross_owner_fail_before_io(self):
        for cls, verifier in ((machine.RuntimeTools, machine.verify_runtime_tools),
                              (machine.ParserRuntimeInputs, machine.verify_parser_runtime)):
            with self.assertRaises(TypeError):
                cls({'admitted': True})
            for bad in ({'admitted': True}, True, object(), object.__new__(cls)):
                with self.subTest(cls=cls, bad=bad), self.assertRaises(machine.Rejected):
                    verifier(bad, self.admission, self.invocation)
        with self.assertRaises(machine.Rejected):
            machine.capture_runtime_tools({'status': 'pass'})
        self.assertEqual(self.api.calls, [])

    def test_capability_traps_exact_source_recipe_controls_and_ownership(self):
        value = machine.capture_capability_traps(self.admission)
        self.assertTrue(machine.verify_runtime_tools(value, self.admission, self.invocation))
        root = self.run / 'public-runtime/tools/capability'
        self.assertEqual(self.api.calls[0], [str(self.toolchain / 'bin/rustc'), '--edition=2021', '-C',
                                            'opt-level=0', str(self.helper / 'no_tool_trap.rs'), '-o',
                                            str(root / 'no-tool-trap')])
        self.assertEqual(len(self.api.calls), 4)
        for index, name in enumerate(machine.LLVM_TOOLS, 1):
            self.assertEqual(self.api.calls[index], [str(root / 'no-tool-traps' / name), '--version'])
            row = value.tools()[name]
            self.assertEqual(row['control']['status'], 121)
            self.assertEqual(row['control']['operation_kind'], 'runtime-trap-control')
            self.assertEqual(row['control']['environment']['UNIT4_TOOL_LOG'], str(root / (name + '-trap-control.jsonl')))
        self.assertEqual(value.purpose(), 'capability')
        self.assertEqual(value.mode(), 'no-native-traps')
        self.assertIsNone(value.selected_runtime())
        self.assertIs(value.machine(), self.admission)
        self.assertIs(value.invocation(), self.invocation)
        for key in ('execution_qualified', 'runtime_native_admitted', 'qualification_admitted',
                    'receipt_producer_authenticated', 'hosted_staging_admitted'):
            self.assertIs(value.document()[key], False)
        changed = value.document()
        changed['tools']['clang']['executable']['sha256'] = 'forged'
        self.assertNotEqual(value.tools()['clang']['executable']['sha256'], 'forged')
        with self.assertRaises(machine.Rejected):
            machine.verify_runtime_tools(value, self.admission, object())
        with self.assertRaises(machine.Rejected):
            machine.capture_capability_traps(self.admission)

    def test_trap_controls_fail_status_log_stream_cleanup_and_timeout(self):
        variants = [('status', 0), ('timed_out', True), ('cleanup_complete', False), ('failure', 'failure'),
                    ('timeout_seconds', 11), ('stream_limit_bytes', 1), ('pid', 0)]
        for key, value in variants:
            def mutate(doc, streams, key=key, value=value):
                if doc['operation_kind'] == 'runtime-trap-control':
                    doc[key] = value
            self.api.runtime_change = mutate
            with self.subTest(key=key), self.assertRaises(machine.Rejected):
                machine.capture_capability_traps(self.admission)
            import shutil
            shutil.rmtree(self.run / 'public-runtime')
        def bad_log(doc, streams):
            if doc['operation_kind'] == 'runtime-trap-control':
                Path(doc['environment']['UNIT4_TOOL_LOG']).write_bytes(b'')
        self.api.runtime_change = bad_log
        with self.assertRaises(machine.Rejected):
            machine.capture_capability_traps(self.admission)
        self.assertFalse(any(r['machine'] is self.admission for r in machine._RUNTIME_TOOLS.values()))

    def test_trap_evidence_file_and_stream_drift_fail_readmission(self):
        value = machine.capture_capability_traps(self.admission)
        log = Path(value.tools()['clang']['control_log']['path'])
        log.write_bytes(log.read_bytes() + b'{}\n')
        with self.assertRaises(machine.Rejected):
            machine.verify_runtime_tools(value, self.admission, self.invocation)

    def test_trap_live_probe_drift_rejects_even_if_files_are_unchanged(self):
        value = machine.capture_capability_traps(self.admission)
        self.api.records[-1]._streams['stderr'] = b'forged'
        with self.assertRaises(machine.Rejected):
            machine.verify_runtime_tools(value, self.admission, self.invocation)

    def test_all_three_non_linux_hosts_use_original_traps_and_windows_aliases(self):
        import shutil
        for host in ('macOS x86_64', 'macOS arm64', 'Windows x86_64'):
            self.state['host']['name'] = self.api.host = host
            self.set_admission()
            value = machine.capture_runtime_tools(self.admission)
            self.assertEqual(value.mode(), 'no-native-traps')
            self.assertEqual(value.purpose(), 'main')
            self.assertTrue(machine.verify_runtime_tools(value, self.admission, self.invocation))
            for tool in value.tools().values():
                self.assertEqual('windows_alias' in tool, host.startswith('Windows'))
            self.assertEqual(Path(value.document()['trap_build']['binary']['path']).suffix == '.exe',
                             host.startswith('Windows'))
            shutil.rmtree(self.run / 'public-runtime')

    def test_linux_main_never_silently_falls_back_to_traps(self):
        with self.assertRaises(machine.Rejected):
            machine.capture_runtime_tools(self.admission)
        self.assertEqual(self.api.calls, [])
        self.assertFalse((self.run / 'public-runtime').exists())

    def test_linux_llvm_route_versions_wrappers_and_local_only_provenance(self):
        llvm, libs, receipt, selected = self.native_inputs()
        with mock.patch.object(machine, '_selected_runtime', return_value=selected):
            value = machine.capture_runtime_tools(self.admission, llvm_root=llvm,
                                                  library_root=libs, runtime_receipt=receipt)
            self.assertTrue(machine.verify_runtime_tools(value, self.admission, self.invocation))
        self.assertEqual(self.api.calls, [[str(llvm / name), '--version'] for name in machine.LLVM_TOOLS])
        self.assertEqual(value.mode(), 'real-native')
        self.assertEqual(value.environment()['LD_LIBRARY_PATH'], str(libs))
        self.assertEqual(value.environment()['UNIT4_REAL_LLVM'], str(llvm))
        self.assertEqual(value.environment()['UNIT4_ADAPTER_ROOT'], str(self.helper))
        for name, marker in zip(machine.LLVM_TOOLS, machine.LLVM_MARKERS):
            row = value.tools()[name]
            self.assertEqual(row['version']['stdout'], marker + '\n')
            self.assertEqual(Path(row['wrapper']['path']).read_bytes(), (self.helper / 'tool_wrapper.py').read_bytes())
        for key in ('runtime_native_admitted', 'qualification_admitted', 'receipt_producer_authenticated',
                    'hosted_staging_admitted'):
            self.assertIs(value.document()[key], False)
        self.assertFalse(self.admission.document()['runtime_native_admitted'])

    def test_wrong_llvm_version_helper_source_and_native_mode_paths_reject(self):
        llvm, libs, receipt, selected = self.native_inputs()
        def wrong_version(doc, streams):
            streams['stdout'] = b'clang version 19.1.8\n'
            doc['stdout'].update(bytes=len(streams['stdout']), sha256=machine.sha(streams['stdout']))
        self.api.runtime_change = wrong_version
        with mock.patch.object(machine, '_selected_runtime', return_value=selected), self.assertRaises(machine.Rejected):
            machine.capture_runtime_tools(self.admission, llvm_root=llvm, library_root=libs, runtime_receipt=receipt)
        self.assertFalse(any(r['machine'] is self.admission for r in machine._RUNTIME_TOOLS.values()))
        self.api.runtime_change = None
        (self.helper / 'no_tool_trap.rs').write_bytes(b'changed historical body')
        with self.assertRaises(machine.Rejected):
            machine.capture_capability_traps(self.admission)

    def test_parser_permit_is_narrow_and_retains_exact_machine_invocation(self):
        value = machine.capture_parser_runtime(self.admission, self.invocation)
        self.assertTrue(machine.verify_parser_runtime(value, self.admission, self.invocation))
        self.assertEqual(self.api.calls, [])
        self.assertIs(value.machine(), self.admission)
        self.assertIs(value.invocation(), self.invocation)
        for key in ('binary_or_case_admission', 'execution_qualified', 'runtime_native_admitted',
                    'native_llvm_qualified', 'hosted_staging_admitted'):
            self.assertFalse(value.document()[key])
        env = value.environment()
        env['PATH'] = 'forged'
        self.assertNotEqual(value.environment()['PATH'], 'forged')
        with self.assertRaises(machine.Rejected):
            machine.verify_parser_runtime(value, self.admission, object())
        self.state['source_roles'] = ['ordinary', 'lifecycle']
        self.set_admission()
        with self.assertRaises(machine.Rejected):
            machine.verify_parser_runtime(value, self.admission, self.invocation)
        with self.assertRaises(machine.Rejected):
            machine.capture_parser_runtime(self.admission, self.invocation)

    def test_parser_readmission_cannot_generalize_original_linux_machine_authority(self):
        for host in ('macOS x86_64', 'macOS arm64', 'Windows x86_64'):
            self.state['host']['name'] = host
            self.set_admission()
            with self.subTest(host=host), self.assertRaises(machine.Rejected):
                machine.capture_parser_runtime(self.admission, self.invocation)
        self.state['host']['name'] = 'Linux x86_64'
        self.state['rust']['parser_original_authority'] = None
        self.set_admission()
        with self.assertRaises(machine.Rejected):
            machine.capture_parser_runtime(self.admission, self.invocation)
        self.assertEqual(self.api.calls, [])

    def test_selected_libraries_check_exact_content_closed_members_and_symlinks(self):
        root = self.root / 'selected'
        root.mkdir()
        (root / 'libfixture.so').write_bytes(b'inert selected-library fixture')
        (root / 'libfixture-alias.so').symlink_to('libfixture.so')
        content = {'entries': [{'path': 'libfixture.so', 'kind': 'file', 'soname': 'libfixture.so',
                                'bytes': (root / 'libfixture.so').stat().st_size,
                                'sha256': machine.sha((root / 'libfixture.so').read_bytes())},
                               {'path': 'libfixture-alias.so', 'kind': 'symlink', 'link': 'libfixture.so'}]}
        receipt = self.root / 'selected.json'
        data = {'schema': 'oxid-unit3-portable-v1-llvm-runtime-stage', 'content': content,
                'content_sha256': machine.sha(machine.canonical(content)), 'library_directory': str(root),
                'dependency_scope': 'source-only fixture'}
        receipt.write_bytes(machine.canonical(data))
        with self.assertRaises(machine.Rejected):
            machine._selected_runtime(receipt, root)
        # Only this source-only content-reader test swaps a pure fixed hash. It
        # cannot issue RuntimeTools, and the installed production pin is asserted.
        self.assertEqual(machine.LLVM_CONTENT_SHA256,
                         '31f2db375a0008638deb261dd8789c5d16a797a2ecd9e7074a7d0752922a5597')
        with mock.patch.object(machine, 'LLVM_CONTENT_SHA256', data['content_sha256']):
            result = machine._selected_runtime(receipt, root)
            self.assertFalse(result['receipt_producer_authenticated'])
            self.assertFalse(result['hosted_staging_admitted'])
            (root / 'unapproved.so').write_bytes(b'extra')
            with self.assertRaises(machine.Rejected):
                machine._selected_runtime(receipt, root)
            (root / 'unapproved.so').unlink()
            (root / 'libfixture-alias.so').unlink()
            (root / 'libfixture-alias.so').symlink_to('../selected.json')
            with self.assertRaises(machine.Rejected):
                machine._selected_runtime(receipt, root)

    def test_selected_library_path_cannot_expand_into_a_search_list(self):
        if os.name == 'nt':
            self.skipTest('Windows does not use the Linux native library route')
        library = self.root / 'override:extra'
        library.mkdir()
        receipt = self.root / 'receipt.json'
        receipt.write_bytes(b'{}')
        with self.assertRaisesRegex(machine.Rejected, 'one explicit library search directory'):
            machine._selected_runtime(receipt, library)

    def test_production_helper_pin_rejects_the_labelled_fixture_bodies(self):
        self.assertEqual(machine.RUNTIME_HELPERS['no_tool_trap.rs'],
                         '6493bd391eb3d8647dd3c33f27b75ff614024070942715bc20acc9732632bda2')
        self.assertEqual(machine.RUNTIME_HELPERS['tool_wrapper.py'],
                         '60382c17f7c51217d2e53354a423666aa644ed678fcce21b7d1507abfd7f16ed')
        with self.assertRaises(machine.Rejected):
            self.real_runtime_helpers(self.admission.document()['snapshot'])

    def test_exact_recipe_roster_rejects_added_setup_files_and_directories(self):
        def extra_output(doc, streams):
            if doc['operation_kind'] == 'runtime-trap-control' and Path(doc['argv'][0]).name == 'ld.lld':
                (Path(doc['cwd']) / 'unexpected-directory').mkdir()
                (Path(doc['cwd']) / 'unexpected-file').write_bytes(b'not part of the recipe')
        self.api.runtime_change = extra_output
        with self.assertRaisesRegex(machine.Rejected, 'extra or missing'):
            machine.capture_capability_traps(self.admission)
        receipt = self.run / 'public-runtime/tools/capability/runtime-setup-failure.json'
        failure = machine.decode(receipt.read_bytes())
        self.assertEqual(failure['status'], 'NotReady')
        self.assertEqual(failure['preservation']['status'], 'preserved')
        self.assertEqual(len(failure['completed_owned_probe_records']), 4)
        self.assertFalse(any(r['machine'] is self.admission for r in machine._RUNTIME_TOOLS.values()))

    def test_failure_keeps_original_error_and_unverified_preservation(self):
        def break_control_and_input(doc, streams):
            if doc['operation_kind'] == 'runtime-trap-control':
                doc['status'] = 0
                (self.helper / 'no_tool_trap.rs').write_bytes(b'changed after actual mock operation')
        self.api.runtime_change = break_control_and_input
        with self.assertRaisesRegex(machine.Rejected, '^runtime setup operation failed$'):
            machine.capture_capability_traps(self.admission)
        record = machine._RUNTIME_FAILURES[self.admission][-1]
        self.assertTrue(record['receipt_persisted'])
        self.assertIs(record['invocation'], self.invocation)
        self.assertEqual(tuple(self.api.records), record['probes'])
        failure = machine.decode(record['raw'])
        self.assertEqual(failure['preservation']['status'], 'unverified')
        self.assertIn('mock helper body changed', failure['preservation']['error'])
        self.assertEqual(len(failure['completed_owned_probe_records']), 2)
        self.assertEqual(failure['completed_owned_probe_records'][-1]['status'], 0)
        self.assertEqual(failure['original_error'], "Rejected('runtime setup operation failed')")

    def test_runtime_inputs_uses_immutable_projection_without_decoding_full_carrier(self):
        original = machine.ToolAdmission.document
        calls = []
        def counted(owner):
            calls.append(owner)
            return original(owner)
        before = self.admission.document()
        with mock.patch.object(machine.ToolAdmission, 'document', counted):
            inputs = machine._runtime_inputs(self.admission, 'capability', None, None, None)
        self.assertEqual(calls, [])
        self.assertEqual(self.admission.document(), before)
        inputs['environment']['PATH'] = 'changed returned copy'
        self.assertEqual(self.admission.document(), before)

    def test_runtime_projection_is_minimal_equivalent_and_defensive(self):
        projection = machine._runtime_state(self.admission)
        self.assertEqual(set(projection), {'checkout', 'source_roles', 'environment', 'host'})
        self.assertEqual(set(projection['checkout']), {'path'})
        self.assertEqual(set(projection['host']), {'name', 'python'})
        self.assertEqual(set(projection['host']['python']), {'selected', 'resolved'})
        actual = machine._runtime_inputs(self.admission, 'capability', None, None, None)
        with mock.patch.object(machine, '_runtime_state', side_effect=lambda value: value.document()['snapshot']):
            original = machine._runtime_inputs(self.admission, 'capability', None, None, None)
        self.assertEqual(actual, original)
        projection['environment']['PATH'] = 'forged returned data'
        self.assertNotEqual(machine._runtime_state(self.admission)['environment']['PATH'], 'forged returned data')

    def test_runtime_projection_rejects_forged_owner_or_mutated_carriers(self):
        for forged in ({'runtime_projection': b'{}'}, object(), object.__new__(machine.ToolAdmission)):
            with self.assertRaises(machine.Rejected):
                machine._runtime_state(forged)
        owner = machine._ISSUED[self.admission]
        before = dict(owner)
        for key, changed in (('runtime_projection', b'{}'), ('raw', owner['raw'] + b' '),
                             ('runtime_projection', '{}'), ('raw_sha256', '0' * 64)):
            owner.clear(); owner.update(before); owner[key] = changed
            with self.subTest(key=key), self.assertRaises(machine.Rejected):
                machine._runtime_state(self.admission)
        owner.clear(); owner.update(before)
        del owner['runtime_projection']
        with self.assertRaisesRegex(machine.Rejected, 'original producer runtime projection required'):
            machine._runtime_state(self.admission)
        owner.clear(); owner.update(before)

    def test_selected_readmission_has_one_checked_lifetime_and_fixed_owner(self):
        value = machine.capture_capability_traps(self.admission)
        self.api.scope_events.clear()
        self.assertTrue(machine.verify_runtime_inputs(value, self.admission, self.invocation))
        self.assertEqual(self.api.scope_events, [('enter', self.invocation), ('exit', self.invocation)])
        self.assertEqual(self.api.read_scopes, 0)
        with mock.patch.object(machine, '_shared', side_effect=[self.api, object()]):
            with self.assertRaisesRegex(machine.Rejected, 'canonical shared owner changed'):
                machine.verify_runtime_inputs(value, self.admission, self.invocation)
        self.assertTrue(self.api.scope_failed)
        self.assertEqual(self.api.read_scopes, 0)

    def test_full_machine_readmission_precedes_local_checked_scope(self):
        value = machine.capture_capability_traps(self.admission)
        observed_depths = []
        with mock.patch.object(machine, '_verify_machine', side_effect=lambda *args: observed_depths.append(self.api.read_scopes)):
            self.assertTrue(machine.verify_runtime_tools(value, self.admission, self.invocation))
        self.assertEqual(observed_depths, [0])

    def test_setup_probes_keep_original_opaque_owners_and_require_readmission(self):
        value = machine.capture_capability_traps(self.admission)
        records = value.setup_probes()
        self.assertIsInstance(records, tuple)
        self.assertEqual(len(records), 4)
        self.assertTrue(all(got is original for got, original in zip(records, self.api.records)))
        Path(value.tools()['clang']['wrapper']['path']).write_bytes(b'changed wrapper')
        with self.assertRaises(machine.Rejected):
            value.setup_probes()

    def test_selected_readmission_skips_only_full_machine_scan(self):
        value = machine.capture_capability_traps(self.admission)
        with mock.patch.object(machine, '_verify_machine', side_effect=AssertionError('full scan')):
            self.assertTrue(machine.verify_runtime_inputs(value, self.admission, self.invocation))
            with self.assertRaisesRegex(AssertionError, 'full scan'):
                machine.verify_runtime_tools(value, self.admission, self.invocation)
        path = Path(value.tools()['clang']['wrapper']['path'])
        path.write_bytes(path.read_bytes() + b'drift')
        with self.assertRaises(machine.Rejected):
            machine.verify_runtime_inputs(value, self.admission, self.invocation)

    def test_import_runtime_surface_is_inert(self):
        fresh = types.ModuleType('inert_runtime_source_controls')
        fresh.__file__ = str(PATH)
        with mock.patch.object(subprocess, 'Popen', side_effect=AssertionError('runtime import launched process')):
            exec(compile(PATH.read_bytes(), str(PATH), 'exec'), fresh.__dict__)
        self.assertEqual(len(fresh._RUNTIME_TOOLS), 0)
        self.assertEqual(len(fresh._PARSER_RUNTIME_INPUTS), 0)


class ReadCheckpointControls(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name).resolve()
        self.body = self.root / 'library.so'
        self.body.write_bytes(b'genuine test fixture body')
        self.alias = self.root / 'library-alias.so'
        self.alias.symlink_to(self.body.name)

    def test_aliases_deduplicate_only_the_actual_body_within_one_call(self):
        with mock.patch.object(machine, '_file', wraps=machine._file) as reader:
            with machine._RuntimeReadCheckpoint() as checkpoint:
                original = checkpoint.binding(self.body)
                alias = checkpoint.route(self.alias)
                direct = checkpoint.route(self.body)
                again = checkpoint.file(self.body)
                self.assertEqual(reader.call_count, 1)
                self.assertEqual(alias['body'], direct['body'])
                self.assertEqual(alias['body']['sha256'], original['sha256'])
                again['stat']['st_size'] = -1
                self.assertNotEqual(checkpoint.file(self.body)['stat']['st_size'], -1)
                self.assertEqual(alias, machine._route(self.alias))
            self.assertFalse(checkpoint._active)
            self.assertTrue(checkpoint._closed)
            self.assertEqual(checkpoint._bodies, {})
            self.assertEqual(checkpoint._routes, {})

    def test_new_checkpoint_always_rehashes_and_old_checkpoint_cannot_return(self):
        with mock.patch.object(machine, '_file', wraps=machine._file) as reader:
            with machine._RuntimeReadCheckpoint() as first:
                first.route(self.alias)
            with machine._RuntimeReadCheckpoint() as second:
                second.route(self.alias)
            self.assertEqual(reader.call_count, 2)
        with self.assertRaises(machine.Rejected):
            first.file(self.body)
        with self.assertRaises(machine.Rejected):
            with first:
                pass

    def test_same_size_body_change_with_restored_mtime_rejects_a_cache_hit(self):
        with self.assertRaisesRegex(machine.Rejected, 'stat changed'):
            with machine._RuntimeReadCheckpoint() as checkpoint:
                row = checkpoint.file(self.body)
                raw = self.body.read_bytes()
                self.body.write_bytes(b'x' * len(raw))
                os.utime(self.body, ns=(self.body.stat().st_atime_ns, row['stat']['st_mtime_ns']))
                checkpoint.file(self.body)
        self.assertEqual(checkpoint._bodies, {})
        self.assertTrue(checkpoint._closed)

    def test_exit_rejects_late_body_drift_without_a_second_lookup(self):
        with self.assertRaises(machine.Rejected):
            with machine._RuntimeReadCheckpoint() as checkpoint:
                checkpoint.route(self.alias)
                self.body.write_bytes(b'changed after final lookup')
        self.assertEqual(checkpoint._routes, {})

    def test_alias_target_change_is_not_hidden_by_identical_bytes(self):
        other = self.root / 'same-bytes.so'
        other.write_bytes(self.body.read_bytes())
        with self.assertRaises(machine.Rejected):
            with machine._RuntimeReadCheckpoint() as checkpoint:
                checkpoint.route(self.alias)
                self.alias.unlink()
                self.alias.symlink_to(other.name)
                checkpoint.route(self.alias)

    def test_replaced_then_restored_alias_is_detected_by_full_route_stat(self):
        with self.assertRaisesRegex(machine.Rejected, 'route changed'):
            with machine._RuntimeReadCheckpoint() as checkpoint:
                checkpoint.route(self.alias)
                self.alias.unlink()
                self.alias.symlink_to(self.body.name)

    def test_exception_clears_checkpoint_and_preserves_original_error(self):
        with self.assertRaisesRegex(RuntimeError, '^original failure$'):
            with machine._RuntimeReadCheckpoint() as checkpoint:
                checkpoint.route(self.alias)
                self.body.write_bytes(b'late drift')
                raise RuntimeError('original failure')
        self.assertEqual(checkpoint._bodies, {})
        self.assertEqual(checkpoint._routes, {})
        self.assertTrue(checkpoint._closed)

    def test_checkpoint_cannot_be_read_from_another_thread(self):
        import threading
        errors = []
        with machine._RuntimeReadCheckpoint() as checkpoint:
            checkpoint.route(self.alias)
            def attempted_read():
                try:
                    checkpoint.file(self.body)
                except BaseException as error:
                    errors.append(error)
            worker = threading.Thread(target=attempted_read)
            worker.start()
            worker.join(timeout=2)
            self.assertFalse(worker.is_alive())
        self.assertEqual(len(errors), 1)
        self.assertIsInstance(errors[0], machine.Rejected)
        self.assertIn('same-thread', str(errors[0]))

    def test_receipt_drift_during_library_read_cannot_replace_its_entry_identity(self):
        libraries = self.root / 'libraries'
        libraries.mkdir()
        library = libraries / 'fixture.so'
        library.write_bytes(b'local test library')
        content = {'entries': [{'path': library.name, 'kind': 'file', 'soname': library.name,
                                'bytes': library.stat().st_size, 'sha256': machine.sha(library.read_bytes())}]}
        content_sha = machine.sha(machine.canonical(content))
        receipt = self.root / 'receipt.json'
        receipt.write_bytes(machine.canonical({'schema': 'oxid-unit3-portable-v1-llvm-runtime-stage',
                          'content': content, 'content_sha256': content_sha,
                          'library_directory': str(libraries), 'dependency_scope': 'pure test fixture'}))
        original = machine._file
        def mutate_receipt_after_library_read(path):
            row = original(path)
            if Path(path) == library:
                receipt.write_bytes(receipt.read_bytes() + b' ')
            return row
        with mock.patch.object(machine, 'LLVM_CONTENT_SHA256', content_sha), \
             mock.patch.object(machine, '_file', side_effect=mutate_receipt_after_library_read):
            with self.assertRaisesRegex(machine.Rejected, 'stat changed'):
                machine._selected_runtime(receipt, libraries)

    def test_full_machine_snapshot_uses_explicit_full_source_readmission(self):
        invocation = object()
        api = types.SimpleNamespace(
            full_verify_invocation=mock.Mock(return_value=({'run_root': '/unused'}, {'capture': 'fixture'})),
            verify_invocation=mock.Mock(side_effect=AssertionError('cheap association cannot replace full snapshot')))
        with mock.patch.object(machine, '_source_authority', side_effect=RuntimeError('stop before file capture')):
            with self.assertRaisesRegex(RuntimeError, 'stop before file capture'):
                machine._snapshot(api, invocation)
        api.full_verify_invocation.assert_called_once_with(invocation)
        api.verify_invocation.assert_not_called()


class ParserSelectedControls(unittest.TestCase):
    """Tiny inert files and private I/O mocks, never a real parser admission."""
    set_admission = RuntimeSourceControls.set_admission
    mock_helpers = RuntimeSourceControls.mock_helpers

    def setUp(self):
        RuntimeSourceControls.setUp(self)
        self.native = self.root / 'native-fixture'
        self.native.mkdir()
        for name in ('cc', 'c++', 'ar', 'as', 'ld', 'which', 'python3'):
            (self.native / name).write_bytes(('inert native fixture ' + name).encode())
        self.library = self.native / 'libfixture.so'
        self.library.write_bytes(b'inert runtime library')
        self.alias = self.native / 'libfixture-alias.so'
        self.alias.symlink_to(self.library.name)
        self.config = self.root / 'loader-config-fixture'
        self.config.mkdir()
        (self.config / 'fixture.conf').write_bytes(b'/fixture/loader/config\n')
        self.rlib = self.toolchain / 'lib/rustlib/x86_64-unknown-linux-gnu/lib/libfixture.rlib'
        self.rlib.parent.mkdir(parents=True)
        self.rlib.write_bytes(b'inert original compiler rlib')
        shared = self.toolchain / 'lib/librustc_driver-fixture.so'
        shared.write_bytes(b'inert original compiler shared library')
        self.state['source_roles'] = sorted(machine.ROLES)
        self.state['host'] = {'name': 'Linux x86_64', 'target': 'x86_64-unknown-linux-gnu',
                              'python': machine._route(self.native / 'python3'), 'fixture': 'not actual host'}
        self.base_env = {'PATH': str(self.native), 'HOME': str(self.root), 'CARGO_HOME': str(self.root / 'cargo')}
        self.state['producer_environment'] = {'PATH': str(self.native), 'HOME': str(self.root)}
        self.state['native'] = {'tools': {key: machine._route(self.native / name) for key, name in
                                         (('cc', 'cc'), ('cxx', 'c++'), ('ar', 'ar'), ('linker', 'cc'),
                                          ('as', 'as'), ('ld', 'ld'), ('which', 'which'))},
                                'files': [machine._route(self.library), machine._route(self.alias)],
                                'trees': [self.loader_tree(), {'root': '/usr/include', 'rows': [], 'bytes': 0}]}
        self.state['environment'] = dict(self.base_env,
            PATH=str(self.toolchain / 'bin') + os.pathsep + str(self.native),
            RUSTC=str(self.toolchain / 'bin/rustc'), CC=str(self.native / 'cc'),
            CXX=str(self.native / 'c++'), AR=str(self.native / 'ar'),
            CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER=str(self.native / 'cc'))
        rust = self.state['rust']
        rust['shared_libraries'] = [machine._route(shared)]
        rust['sysroot_trees'] = [machine._tree(self.toolchain / 'lib/rustlib/x86_64-unknown-linux-gnu')]
        rust['parser_original_authority']['compiler_files'] = [
            {'path': path.relative_to(self.toolchain).as_posix(),
             **{key: machine._file(path)[key] for key in ('bytes', 'sha256')}}
            for path in sorted([self.toolchain / 'bin/cargo', self.toolchain / 'bin/rustc', shared, self.rlib])]
        self.set_admission()
        self.api.verify_invocation = mock.Mock(side_effect=lambda invocation: (
            copy.deepcopy(self.state['invocation']),
            {'checkout': copy.deepcopy(self.state['checkout']), 'scope': self.state['source_scope'],
             'maps': {key: [] for key in self.state['source_roles']}}))
        patches = [mock.patch.object(machine, 'measured_host', side_effect=lambda: copy.deepcopy(self.state['host'])),
                   mock.patch.object(machine, '_producer_environment', side_effect=lambda: dict(self.state['producer_environment'])),
                   mock.patch.object(machine, '_clean_environment', side_effect=lambda *a: dict(self.base_env)),
                   mock.patch.object(machine, '_config_absence', return_value=[]),
                   mock.patch.object(machine, '_workspace_absence', return_value=[]),
                   mock.patch.object(machine, '_parser_native_runtime_paths', side_effect=lambda: [str(self.library), str(self.alias)]),
                   mock.patch.object(machine, '_parser_loader_trees', side_effect=lambda: [self.loader_tree()]),
                   mock.patch.object(machine.shutil, 'which', side_effect=lambda name, **kw:
                                     str((self.toolchain / 'bin' if name in ('cargo', 'rustc') else self.native) / name))]
        for patch in patches:
            patch.start()
            self.addCleanup(patch.stop)
        self.value = machine.capture_parser_runtime(self.admission, self.invocation)
        self.addCleanup(lambda: machine._PARSER_RUNTIME_INPUTS.pop(self.value, None))

    def loader_tree(self):
        return dict(machine._tree(self.config), root='/etc/ld.so.conf.d')

    def selected(self):
        return machine.verify_parser_runtime_selected(self.value, self.admission, self.invocation)

    def test_selected_equal_to_issuer_projection_and_never_calls_full_or_launch(self):
        original = machine._PARSER_RUNTIME_INPUTS[self.value]['selected_raw']
        self.assertEqual(original, machine._parser_runtime_projection(self.admission.document()))
        with mock.patch.object(machine, '_verify_machine', side_effect=AssertionError('full boundary only')), \
             mock.patch.object(machine, '_snapshot', side_effect=AssertionError('no full capture in selected check')), \
             mock.patch.object(machine, '_dependency_closure', side_effect=AssertionError('no dependency reconstruction')):
            self.assertTrue(self.selected())
        self.assertEqual(self.api.calls, [])
        self.assertEqual(self.api.read_scopes, 0)
        self.api.verify_invocation.assert_called_once_with(self.invocation)
        self.assertFalse(self.value.document()['binary_or_case_admission'])

    def test_selected_rehashes_every_operation_but_deduplicates_aliases_in_call(self):
        real_file, counts = machine._file, {}
        def counted(path):
            key = str(path)
            counts[key] = counts.get(key, 0) + 1
            return real_file(path)
        with mock.patch.object(machine, '_file', side_effect=counted):
            self.assertTrue(self.selected())
            self.assertEqual(counts[str(self.library)], 1)
            self.assertEqual(counts[str(self.rlib)], 1)
            self.assertTrue(self.selected())
            self.assertEqual(counts[str(self.library)], 2)
            self.assertEqual(counts[str(self.rlib)], 2)

    def test_selected_rejects_original_compiler_body_and_extra_roster(self):
        self.rlib.write_bytes(b'changed compiler archive')
        with self.assertRaises(machine.Rejected):
            self.selected()
        self.assertTrue(self.api.scope_failed)

    def test_selected_rejects_added_compiler_or_runtime_library(self):
        added = self.toolchain / 'lib/libLLVM-extra.so'
        added.write_bytes(b'new unrecorded compiler library')
        with self.assertRaisesRegex(machine.Rejected, 'library roster drift'):
            self.selected()
        added.unlink()
        extra = self.rlib.with_name('extra.rlib')
        extra.write_bytes(b'new unrecorded sysroot archive')
        with self.assertRaisesRegex(machine.Rejected, 'compiler roster drift'):
            self.selected()

    def test_selected_rejects_native_alias_and_loader_configuration_drift(self):
        other = self.native / 'same-body.so'
        other.write_bytes(self.library.read_bytes())
        self.alias.unlink()
        self.alias.symlink_to(other.name)
        with self.assertRaisesRegex(machine.Rejected, 'route or body drift'):
            self.selected()

    def test_selected_rejects_new_loader_config_member(self):
        (self.config / 'extra.conf').write_bytes(b'/unrecorded/library\n')
        with self.assertRaisesRegex(machine.Rejected, 'configuration tree drift'):
            self.selected()

    def test_selected_rejects_host_producer_environment_and_absence_drift(self):
        for name, replacement in (
            ('measured_host', {}), ('_producer_environment', {}), ('_config_absence', [{'unexpected': True}]),
            ('_workspace_absence', [{'unexpected': True}]), ('_parser_native_runtime_paths', [])):
            with self.subTest(name=name), mock.patch.object(machine, name, return_value=replacement):
                with self.assertRaises(machine.Rejected):
                    self.selected()

    def test_selected_original_ownership_and_context_are_required(self):
        for value, admission, invocation in (({}, self.admission, self.invocation),
                                            (object.__new__(machine.ParserRuntimeInputs), self.admission, self.invocation),
                                            (self.value, object.__new__(machine.ToolAdmission), self.invocation),
                                            (self.value, self.admission, object())):
            with self.assertRaises(machine.Rejected):
                machine.verify_parser_runtime_selected(value, admission, invocation)
        self.api.verify_invocation.return_value = ({'wrong': True}, {})
        self.api.verify_invocation.side_effect = None
        with self.assertRaisesRegex(machine.Rejected, 'context association differs'):
            self.selected()

    def test_selected_projection_carrier_changes_and_defensive_copies(self):
        record = machine._PARSER_RUNTIME_INPUTS[self.value]
        for key in ('raw', 'selected_raw'):
            saved = record[key]
            record[key] += b' '
            with self.subTest(key=key), self.assertRaisesRegex(machine.Rejected, 'carrier/projection changed'):
                self.selected()
            record[key] = saved
        document, projected = machine._parser_selected_state(record, self.admission)
        projected['environment']['PATH'] = 'forged decoded copy'
        document['machine_sha256'] = 'forged decoded copy'
        self.assertTrue(self.selected())

    def test_selected_second_observation_rejects_mid_checkpoint_roster_drift(self):
        with mock.patch.object(machine, '_parser_native_runtime_paths', side_effect=(
                [str(self.library), str(self.alias)], [])):
            with self.assertRaisesRegex(machine.Rejected, 'runtime input roster drift'):
                self.selected()
        self.assertTrue(self.api.scope_failed)

    def test_selected_replays_original_opaque_machine_probe_and_rejects_body_drift(self):
        # FakeAPI is an inert in-memory supervisor; Popen remains forbidden.
        live, observed = machine._probe(self.api, self.invocation,
                                       [str(self.toolchain / 'bin/rustc'), '-Vv'], self.run,
                                       dict(self.state['environment']))
        owner = machine._ISSUED[self.admission]
        document = self.admission.document()
        document['probes'], document['probe_records'] = [observed], [live.document()]
        owner['raw'] = machine.canonical(document)
        owner['raw_sha256'] = machine.sha(owner['raw'])
        owner['probes'] = [live]
        self.value = machine.capture_parser_runtime(self.admission, self.invocation)
        calls = list(self.api.calls)
        self.assertTrue(self.selected())
        self.assertEqual(self.api.calls, calls)
        live._streams['stdout'] = b'changed retained setup probe bytes'
        with self.assertRaisesRegex(machine.Rejected, 'retained machine probe body drift'):
            self.selected()
        self.assertTrue(self.api.scope_failed)

    def test_selected_exit_rejects_late_file_drift_and_shared_owner_replacement(self):
        def shared():
            self.library.write_bytes(b'late changed runtime body')
            return self.api
        with mock.patch.object(machine, '_shared', side_effect=(self.api, object())):
            with self.assertRaisesRegex(machine.Rejected, 'canonical shared owner changed'):
                self.selected()
        with mock.patch.object(machine, '_shared', side_effect=lambda: self.api if not self.api.read_scopes else shared()):
            with self.assertRaisesRegex(machine.Rejected, 'stat changed'):
                self.selected()

    def test_full_boundary_rederives_projection_and_rechecks_omitted_machine_trees(self):
        record = machine._PARSER_RUNTIME_INPUTS[self.value]
        projected = machine.decode(record['selected_raw'])
        projected['environment']['PATH'] = 'forged projection with recomputed local digest'
        record['selected_raw'] = machine.canonical(projected)
        record['selected_sha256'] = machine.sha(record['selected_raw'])
        with self.assertRaisesRegex(machine.Rejected, 'projection differs from full machine carrier'):
            machine.verify_parser_runtime(self.value, self.admission, self.invocation)
        record['selected_raw'] = machine._parser_runtime_projection(self.admission.document())
        record['selected_sha256'] = machine.sha(record['selected_raw'])
        changed = copy.deepcopy(self.state)
        changed['native']['trees'][-1]['bytes'] = 1
        with mock.patch.object(machine, '_snapshot', return_value=(changed, {})) as snapshot:
            self.assertTrue(self.selected())
            snapshot.assert_not_called()
            with mock.patch.object(machine, '_verify_machine', side_effect=_REAL_VERIFY_MACHINE):
                with self.assertRaisesRegex(machine.Rejected, 'captured local machine inputs drifted'):
                    machine.verify_parser_runtime(self.value, self.admission, self.invocation)
            snapshot.assert_called_once()


if __name__ == '__main__':
    unittest.main()
