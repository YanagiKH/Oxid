"""Pure/mock artifact controls; never invokes a compiler, tool probe or CI."""
import base64
import copy
import hashlib
import importlib.util
import json
import os
import time
from pathlib import Path
import tempfile
import subprocess
import unittest
from unittest import mock

HERE = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location('_pure_package_ci_artifacts', HERE / 'artifacts.py')
artifacts = importlib.util.module_from_spec(spec)
spec.loader.exec_module(artifacts)


def recipe():
    return {'id': 'release-quality', 'original_id': 'ci.yml:L80', 'job_recipe_ids': ['quality-linux'],
            'role': 'production', 'profile': 'release', 'argv': ['cargo', 'build', '--release', '--locked'],
            'projection_id': 'production-main', 'package_name': 'oxid', 'manifest_path': 'Cargo.toml',
            'target_name': 'oxid', 'target_kind': 'bin', 'target_path': 'src/cli.rs',
            'output_relative': 'target/release/oxid', 'operational': False,
            'blockers': ['Pure source-only fixture; execution and transport are not reviewed'],
            'tool_policy': {'reviewed': False, 'cargo_version': None, 'rustc_version': None, 'rustc_host': None},
            'environment_policy': {'reviewed': False, 'inherit': [], 'fixed': {}, 'forbidden': []},
            'config_policy': {'reviewed': False, 'files': [], 'absent': []},
            'resource_policy': {'reviewed': False, 'timeout_seconds': None, 'stream_limit_bytes': None}}


def policies():
    result = recipe()
    result['tool_policy'] = {'reviewed': True, 'cargo_version': 'cargo fixture\n',
                             'rustc_version': 'rustc fixture\nhost: synthetic-host\n', 'rustc_host': 'synthetic-host'}
    result['environment_policy'] = {'reviewed': True, 'inherit': ['PATH', 'HOME'], 'fixed': {},
                                    'forbidden': list(artifacts._FORBIDDEN_ENV)}
    result['config_policy'] = {'reviewed': True, 'files': [], 'absent': []}
    result['resource_policy'] = {'reviewed': True, 'timeout_seconds': 10, 'stream_limit_bytes': 1024}
    return result


def metadata(repository):
    return {'version': 1, 'workspace_root': str(repository), 'target_directory': str(repository / 'target'),
            'workspace_members': ['fixture-id'], 'packages': [{'name': 'oxid', 'id': 'fixture-id', 'source': None,
             'manifest_path': str(repository / 'Cargo.toml'),
             'targets': [{'name': 'oxid', 'kind': ['bin'], 'src_path': str(repository / 'src/cli.rs')}]}]}


def stream(raw):
    return {'base64': base64.b64encode(raw).decode(), 'bytes': len(raw), 'sha256': hashlib.sha256(raw).hexdigest()}


class PureSource:
    def __init__(self, selected=None):
        self.recipe = selected or recipe()
        self.value = {'schema': 'synthetic-inert-source', 'job_recipe_id': 'quality-linux',
                      'repository_path': '/fixture', 'activation': {'approved': False}}
    def verify_job(self, context):
        return copy.deepcopy(self.value)
    def registry(self, context):
        return {'build_recipes': {self.recipe['id']: self.recipe}}
    def projection(self, context, projection_id):
        if projection_id != 'production-main':
            raise artifacts.Rejected('unknown projection')
        return {'src/main.rs': {'bytes': 1, 'sha256': 'a' * 64}}
    def route(self, context, original_id):
        return {'original_record': {'original': {'command': 'cargo build --release --locked'}}}
    def require_operational(self, context, selected=None):
        raise artifacts.Rejected('operational producer remains held in pure tests')
    def identity(self, path):
        raw = path.read_bytes()
        return {'path': str(path), 'bytes': len(raw), 'sha256': hashlib.sha256(raw).hexdigest()}
    def tick(self):
        pass


class ArtifactControls(unittest.TestCase):
    def test_detached_owner_cannot_plan_or_produce(self):
        with self.assertRaises(artifacts.Rejected):
            artifacts.plan_build(object(), 'release-quality')
        with self.assertRaises(artifacts.Rejected):
            artifacts.produce_build(object(), 'release-quality')

    def test_no_constructor_json_or_hash_can_issue_live_artifact(self):
        with self.assertRaises(artifacts.Rejected):
            artifacts.BuildArtifact({'path': '/fixture/oxid', 'sha256': 'a' * 64})
        forged = object.__new__(artifacts.BuildArtifact)
        with mock.patch.object(artifacts, '_source', return_value=PureSource()):
            for value in (forged, {'success': True}, {'artifact': '/fixture/oxid'}):
                with self.assertRaises(artifacts.Rejected):
                    artifacts.verify_artifact(value, role='production', profile='release')

    def test_original_three_unchanged_build_commands_plan_as_inert(self):
        for original_id in artifacts.ORIGINAL_BUILDS:
            value = recipe(); value['original_id'] = original_id
            with mock.patch.object(artifacts, '_source', return_value=PureSource(value)):
                plan = artifacts.plan_build(object(), value['id'])
            self.assertTrue(plan['supported'])
            self.assertFalse(plan['operational'])
            self.assertEqual(plan['authority'], 'inert-plan')
            self.assertEqual(plan['recipe']['argv'], ['cargo', 'build', '--release', '--locked'])
            self.assertNotIsInstance(plan, artifacts.BuildArtifact)

    def test_closed_registry_preserves_three_original_build_bodies(self):
        fixed = json.loads((HERE / 'routes.json').read_text())
        original_ids = set()
        for recipe_id, selected in fixed['build_recipes'].items():
            original_ids.add(selected['original_id'])
            source = PureSource(selected)
            source.value['job_recipe_id'] = selected['job_recipe_ids'][0]
            original = next(row for row in fixed['original_entries'] if row['id'] == selected['original_id'])
            with mock.patch.object(artifacts, '_source', return_value=source), \
                 mock.patch.object(source, 'projection', return_value={'authority': 'inert-fixture'}), \
                 mock.patch.object(source, 'route', return_value=original):
                plan = artifacts.plan_build(object(), recipe_id)
                self.assertTrue(plan['supported'])
                self.assertFalse(plan['operational'])
                self.assertEqual(plan['recipe']['argv'], artifacts.ORIGINAL_ARGV)
        self.assertEqual(original_ids, artifacts.ORIGINAL_BUILDS)

    def test_roles_profiles_and_commands_are_not_interchangeable(self):
        variants = []
        for role in ('test', 'observer', 'container-image'):
            value = recipe(); value['role'] = role; variants.append(value)
        value = recipe(); value['profile'] = 'debug'; variants.append(value)
        value = recipe(); value['argv'] += ['--message-format=json']; variants.append(value)
        value = recipe(); value['original_id'] = 'ci.yml:L517'; variants.append(value)
        for value in variants:
            with mock.patch.object(artifacts, '_source', return_value=PureSource(value)):
                plan = artifacts.plan_build(object(), value['id'])
                self.assertFalse(plan['supported'])
                self.assertFalse(plan['operational'])

    def test_wrong_job_unknown_recipe_and_target_reject(self):
        source = PureSource()
        with mock.patch.object(artifacts, '_source', return_value=source):
            with self.assertRaises(artifacts.Rejected):
                artifacts.plan_build(object(), 'invented')
            source.value['job_recipe_id'] = 'other-job'
            with self.assertRaises(artifacts.Rejected):
                artifacts.plan_build(object(), 'release-quality')
        for key, changed in [('target_path', 'src/main.rs'), ('manifest_path', 'other/Cargo.toml'),
                             ('output_relative', 'target/debug/oxid'), ('package_name', 'other')]:
            value = recipe(); value[key] = changed
            with mock.patch.object(artifacts, '_source', return_value=PureSource(value)), self.assertRaises(artifacts.Rejected):
                artifacts.plan_build(object(), value['id'])

    def test_changed_original_command_rejects(self):
        source = PureSource()
        with mock.patch.object(source, 'route', return_value={'original_record': {'original': {'command': 'cargo build'}}}), \
             mock.patch.object(artifacts, '_source', return_value=source), self.assertRaises(artifacts.Rejected):
            artifacts.plan_build(object(), 'release-quality')

    def test_gate_precedes_every_effect(self):
        with mock.patch.object(artifacts, '_source', return_value=PureSource()), \
             mock.patch.object(subprocess, 'Popen') as process:
            with self.assertRaises(artifacts.Rejected):
                artifacts.produce_build(object(), 'release-quality')
            process.assert_not_called()

    def test_registry_flags_cannot_enable_unreviewed_producer(self):
        source = PureSource(policies())
        source.recipe['operational'] = True
        source.value['activation']['approved'] = True
        with mock.patch.object(artifacts, '_source', return_value=source), \
             mock.patch.object(source, 'require_operational', return_value=None), \
             mock.patch.object(subprocess, 'Popen') as process:
            with self.assertRaisesRegex(artifacts.Rejected, 'authenticated supervisor/machine'):
                artifacts.produce_build(object(), source.recipe['id'])
            process.assert_not_called()
        self.assertEqual(len(artifacts._LIVE), 0)

    def test_pending_and_incomplete_policies_refuse_before_any_probe(self):
        with self.assertRaises(artifacts.Rejected):
            artifacts._policies(recipe())
        good = policies()
        self.assertEqual(artifacts._policies(good)[3]['timeout_seconds'], 10)
        for key in ('tool_policy', 'environment_policy', 'config_policy', 'resource_policy'):
            value = copy.deepcopy(good); value[key]['reviewed'] = False
            with self.subTest(key=key), self.assertRaises(artifacts.Rejected):
                artifacts._policies(value)
        for value in (True, 0, artifacts._MAX_PROCESS_SECONDS + 1):
            changed = copy.deepcopy(good); changed['resource_policy']['timeout_seconds'] = value
            with self.assertRaises(artifacts.Rejected):
                artifacts._policies(changed)
        changed = copy.deepcopy(good); changed['environment_policy']['inherit'] += ['GH_TOKEN']
        with self.assertRaises(artifacts.Rejected):
            artifacts._policies(changed)

    def test_override_environment_cannot_change_the_command(self):
        policy = policies()['environment_policy']
        with mock.patch.dict(artifacts.os.environ, {'PATH': '/fixture/bin', 'HOME': '/fixture/home'}, clear=True):
            self.assertEqual(artifacts._environment(policy), {'PATH': '/fixture/bin', 'HOME': '/fixture/home'})
        for key in ('RUSTFLAGS', 'RUSTC_WRAPPER', 'CARGO_TARGET_DIR', 'CARGO_PROFILE_RELEASE_DEBUG'):
            with mock.patch.dict(artifacts.os.environ, {'PATH': '/fixture/bin', 'HOME': '/fixture/home', key: 'evil'}, clear=True), \
                 self.assertRaises(artifacts.Rejected):
                artifacts._environment(policy)

    def test_safe_relative_paths(self):
        for path in ('../oxid', '/tmp/oxid', 'a/../oxid', 'a//oxid', 'C:oxid', 'a\\oxid', 'a\x00b'):
            with self.subTest(path=path), self.assertRaises(artifacts.Rejected):
                artifacts._relative(path)

    def test_preexisting_output_never_adopted(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory).resolve()
            output = root / 'target/release/oxid'
            self.assertEqual(artifacts._assert_output_absent(root, recipe()), output)
            output.parent.mkdir(parents=True); output.write_bytes(b'inert-test-fixture')
            with self.assertRaises(artifacts.Rejected):
                artifacts._assert_output_absent(root, recipe())
            self.assertEqual(output.read_bytes(), b'inert-test-fixture')

    def test_symlink_output_is_not_fresh(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory).resolve(); output = root / 'target/release/oxid'
            output.parent.mkdir(parents=True); output.symlink_to(root / 'missing')
            with self.assertRaises(artifacts.Rejected):
                artifacts._assert_output_absent(root, recipe())

    def test_metadata_requires_exact_package_target_and_output_directory(self):
        root = Path('/fixture'); value = metadata(root)
        parsed = artifacts._metadata(artifacts.canonical(value), root, recipe())
        self.assertEqual(parsed['target']['src_path'], '/fixture/src/cli.rs')
        variants = []
        for key, replacement in [('workspace_root', '/other'), ('target_directory', '/other/target'), ('version', 2)]:
            changed = copy.deepcopy(value); changed[key] = replacement; variants.append(changed)
        changed = copy.deepcopy(value); changed['packages'][0]['targets'][0]['src_path'] = '/fixture/src/main.rs'; variants.append(changed)
        changed = copy.deepcopy(value); changed['packages'][0]['source'] = 'registry+foreign'; variants.append(changed)
        changed = copy.deepcopy(value); changed['packages'].append(copy.deepcopy(changed['packages'][0])); variants.append(changed)
        changed = copy.deepcopy(value); changed['workspace_members'] = []; variants.append(changed)
        for changed in variants:
            with self.assertRaises(artifacts.Rejected):
                artifacts._metadata(artifacts.canonical(changed), root, recipe())

    def test_raw_stream_binds_exact_bytes(self):
        value = stream(b'actual uninterpreted stdout\xff')
        self.assertEqual(artifacts._stream(value), b'actual uninterpreted stdout\xff')
        for key, replacement in [('bytes', True), ('sha256', 'f' * 64), ('base64', 'not-base64!')]:
            changed = copy.deepcopy(value); changed[key] = replacement
            with self.assertRaises(artifacts.Rejected):
                artifacts._stream(changed)

    def test_dependency_cache_inventory_is_separate_and_bounded(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory).resolve(); source = PureSource()
            selected = root / 'target/release/oxid'
            self.assertFalse(artifacts._cache_inventory(source, root, selected)['existed'])
            dependency = root / 'target/release/deps/inert'; dependency.parent.mkdir(parents=True)
            dependency.write_bytes(b'not-executable-cache-fixture')
            result = artifacts._cache_inventory(source, root, selected)
            self.assertEqual(result['files'][0]['path'], 'target/release/deps/inert')
            self.assertNotIsInstance(result, artifacts.BuildArtifact)
            with mock.patch.object(artifacts, 'MAX_CACHE_BYTES', 1), self.assertRaises(artifacts.Rejected):
                artifacts._cache_inventory(source, root, selected)

    def test_unreviewed_config_body_rejects(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory).resolve(); home = root / 'home'; home.mkdir(); (home / '.cargo').mkdir()
            repo = root / 'repo'; repo.mkdir(); (repo / '.cargo').mkdir(); config = repo / '.cargo/config.toml'
            config.write_text('[build]\nrustc-wrapper="other"\n')
            with self.assertRaises(artifacts.Rejected):
                artifacts._config(PureSource(), repo, {'HOME': str(home)}, {'files': [], 'absent': []})

    def test_failed_source_readback_rejects(self):
        source = PureSource()
        before = source.verify_job(object()); source.value['job_recipe_id'] = 'other'
        with self.assertRaises(artifacts.Rejected):
            artifacts._same_source(source, object(), before)

    def test_serialized_build_transport_remains_blocked(self):
        source = mock.Mock()
        source.verify_join.return_value = {'schema': 'synthetic-inert-join'}
        source.read.return_value = artifacts.canonical({'schema': artifacts.TRANSPORT_SCHEMA, 'success': True})
        with mock.patch.object(artifacts, '_source', return_value=source), self.assertRaisesRegex(artifacts.Rejected, 'transport awaits'):
            artifacts.read_artifact_evidence('/fixture/report.json', object(), object())
        self.assertEqual(len(artifacts._LIVE), 0)


class ProductionAdapterControls(unittest.TestCase):
    """Inert source/byte fixtures. No fixture creates a live BuildArtifact."""
    def setUp(self):
        self.process_guard = mock.patch.object(subprocess, 'Popen', side_effect=AssertionError('actual process forbidden'))
        self.run_guard = mock.patch.object(subprocess, 'run', side_effect=AssertionError('actual process forbidden'))
        self.process_guard.start(); self.run_guard.start()
        self.addCleanup(self.process_guard.stop); self.addCleanup(self.run_guard.stop)

    @staticmethod
    def exact_metadata(root):
        package_id = 'path+' + root.as_uri() + '#oxid@0.9.0'
        targets = [
            {'kind': ['bin'], 'crate_types': ['bin'], 'name': 'oxid', 'src_path': str(root / 'src/cli.rs'),
             'edition': '2021', 'doc': True, 'doctest': False, 'test': True},
            {'kind': ['custom-build'], 'crate_types': ['bin'], 'name': 'build-script-build',
             'src_path': str(root / 'build.rs'), 'edition': '2021', 'doc': False, 'doctest': False, 'test': False},
            {'kind': ['test'], 'crate_types': ['bin'], 'name': 'inert', 'src_path': str(root / 'tests/inert.rs'),
             'edition': '2021', 'doc': False, 'doctest': False, 'test': True}]
        package = {'id': package_id, 'name': 'oxid', 'version': '0.9.0', 'edition': '2021', 'source': None,
                   'manifest_path': str(root / 'Cargo.toml'), 'targets': targets}
        value = {'version': 1, 'resolve': None, 'packages': [package], 'workspace_members': [package_id],
                 'workspace_default_members': [package_id], 'workspace_root': str(root),
                 'target_directory': str(root / 'target')}
        files = [{'path': name, 'bytes': 1, 'sha256': '0' * 64} for name in
                 ['Cargo.toml', 'Cargo.lock', 'build.rs', 'src/cli.rs', 'tests/inert.rs'] +
                 ['fixtures/inert-' + str(index) for index in range(371)]]
        return value, files

    def test_dormant_source_barrier_remains_independent_of_flags(self):
        self.assertIs(artifacts.PRODUCTION_ADAPTER_REVIEWED, False)
        source = PureSource(policies())
        source.recipe['operational'] = True; source.value['activation']['approved'] = True
        source.load_production_owner = mock.Mock(side_effect=AssertionError('must not load effects before source review'))
        with mock.patch.object(artifacts, '_source', return_value=source), \
             mock.patch.object(source, 'require_operational', return_value=None), \
             self.assertRaisesRegex(artifacts.Rejected, 'independent execution review'):
            artifacts.produce_build(object(), source.recipe['id'])
        source.load_production_owner.assert_not_called()
        self.assertFalse(artifacts._LIVE)

    def test_exact_metadata_checks_complete_default_workspace_target_roster(self):
        root = Path('/inert-fixture'); value, files = self.exact_metadata(root)
        self.assertEqual(artifacts._metadata_exact(artifacts.canonical(value), root, recipe(), files)['package_id'],
                         value['packages'][0]['id'])
        changes = []
        altered = copy.deepcopy(value); altered['workspace_default_members'] = []; changes.append(altered)
        altered = copy.deepcopy(value); altered['packages'][0]['targets'].pop(); changes.append(altered)
        altered = copy.deepcopy(value); altered['packages'][0]['targets'].append(copy.deepcopy(altered['packages'][0]['targets'][0])); changes.append(altered)
        altered = copy.deepcopy(value); altered['packages'][0]['version'] = '0.9.1'; changes.append(altered)
        altered = copy.deepcopy(value); altered['resolve'] = {'nodes': []}; changes.append(altered)
        for altered in changes:
            with self.assertRaises(artifacts.Rejected):
                artifacts._metadata_exact(artifacts.canonical(altered), root, recipe(), files)
        for bad_files in (files[:-1], files[:-1] + [files[0]]):
            with self.assertRaises(artifacts.Rejected):
                artifacts._metadata_exact(artifacts.canonical(value), root, recipe(), bad_files)

    @staticmethod
    def inert_elf():
        value = bytearray(64); value[:7] = b'\x7fELF\x02\x01\x01'
        value[16:18] = (2).to_bytes(2, 'little'); value[18:20] = (62).to_bytes(2, 'little')
        value[20:24] = (1).to_bytes(4, 'little'); value[52:54] = (64).to_bytes(2, 'little')
        return bytes(value)

    def test_selected_byte_inspection_is_inert_and_checks_emission_host_permission(self):
        with tempfile.TemporaryDirectory() as name:
            path = Path(name).resolve() / 'inert-elf-header'
            path.write_bytes(self.inert_elf()); path.chmod(0o700)
            now = path.stat().st_mtime_ns
            window = {'started_ns': now - 1, 'completed_ns': now + 1}
            value = artifacts._emitted_artifact(path, 'linux-x86_64', window, lambda: None)
            self.assertIsInstance(value, dict); self.assertEqual(value['sha256'], hashlib.sha256(self.inert_elf()).hexdigest())
            self.assertFalse(artifacts._LIVE)
            for host, times in [('linux-x86_64', {'started_ns': now + 1, 'completed_ns': now + 2}),
                                ('macos-x86_64', window), ('windows-x86_64', window)]:
                with self.assertRaises(artifacts.Rejected):
                    artifacts._emitted_artifact(path, host, times, lambda: None)
            path.chmod(0o600)
            with self.assertRaises(artifacts.Rejected):
                artifacts._emitted_artifact(path, 'linux-x86_64', window, lambda: None)

    def test_selected_read_cancellation_never_becomes_artifact(self):
        with tempfile.TemporaryDirectory() as name:
            path = Path(name).resolve() / 'inert'; path.write_bytes(self.inert_elf()); path.chmod(0o700)
            checkpoint = mock.Mock(side_effect=RuntimeError('inert read cancelled'))
            with self.assertRaisesRegex(RuntimeError, 'read cancelled'):
                artifacts._emitted_artifact(path, 'linux-x86_64', {'started_ns': 0, 'completed_ns': time.time_ns()}, checkpoint)
            self.assertFalse(artifacts._LIVE)

    def test_pipeline_failure_retains_original_error_and_never_issues(self):
        with tempfile.TemporaryDirectory() as name:
            root = Path(name).resolve(); context = object(); invocation = object()
            selected = recipe(); selected['id'] = 'synthetic-inert'
            source = mock.Mock()
            source.verify_job.return_value = {'repository_path': str(root), 'host': 'linux-x86_64'}
            process, machine = mock.Mock(), mock.Mock()
            process.begin_production.return_value = invocation
            machine.capture_production_machine.side_effect = RuntimeError('inert original capture failure')
            process.retain_production_failure.side_effect = OSError('inert preservation failure')
            source.load_production_owner.side_effect = lambda name: {'process': process, 'machine': machine}[name]
            with self.assertRaisesRegex(RuntimeError, 'original capture failure'):
                artifacts._produce_build(source, context, {'recipe': selected})
            process.retain_production_failure.assert_called_once()
            process.cancel_production.assert_called_once_with(invocation)
            self.assertFalse(artifacts._LIVE)

    def test_private_producer_refuses_stale_selected_before_owner_loading(self):
        with tempfile.TemporaryDirectory() as name:
            root = Path(name).resolve(); stale = root / 'target/release/oxid'; stale.parent.mkdir(parents=True)
            stale.write_bytes(b'inert stale cache output')
            source = mock.Mock(); source.verify_job.return_value = {'repository_path': str(root)}
            with self.assertRaisesRegex(artifacts.Rejected, 'absent'):
                artifacts._produce_build(source, object(), {'recipe': recipe()})
            source.load_production_owner.assert_not_called()
            self.assertEqual(stale.read_bytes(), b'inert stale cache output')
            self.assertFalse(artifacts._LIVE)



    def test_late_pipeline_failure_keeps_original_error_and_retention(self):
        # Deliberately inert owner doubles drive only failure paths. No fixture
        # crosses final BuildArtifact issuance or authentic owner admission.
        for fail_at in ('emitted-file', 'final-machine'):
            with self.subTest(fail_at=fail_at), tempfile.TemporaryDirectory() as name:
                root = Path(name).resolve(); context = object(); invocation = mock.Mock()
                invocation.document.return_value = {'started_ns': 1}
                selected = recipe(); selected['id'] = 'synthetic-inert'
                source = mock.Mock()
                source.verify_job.return_value = {'repository_path': str(root), 'host': 'linux-x86_64', 'source_files': []}
                process, machine = mock.Mock(), mock.Mock()
                process.begin_production.return_value = invocation
                machine.capture_production_machine.return_value = object()
                machine.verify_production_machine.return_value = {'inert-machine': True}
                if fail_at == 'final-machine':
                    machine.verify_production_machine.side_effect = [{'inert-machine': True}] * 7 + [RuntimeError('inert final-machine failure')]
                records, documents = {}, {}
                for index, operation_id in enumerate(('metadata-before', 'build', 'metadata-after')):
                    raw = b'inert metadata' if operation_id != 'build' else b''
                    record = mock.Mock(); record.stdout.return_value = raw; record.stderr.return_value = b''
                    documents[operation_id] = {'started_ns': 2 + index * 2, 'completed_ns': 3 + index * 2,
                        'stdout': {'bytes': len(raw), 'sha256': hashlib.sha256(raw).hexdigest()},
                        'stderr': {'bytes': 0, 'sha256': hashlib.sha256(b'').hexdigest()}}
                    records[operation_id] = record
                process.owned_production_operation.side_effect = lambda inv, operation_id: records[operation_id]
                process.verify_production_operation.side_effect = lambda rec, inv, operation_id: documents[operation_id]
                source.load_production_owner.side_effect = lambda owner: {'process': process, 'machine': machine}[owner]
                emitted = mock.Mock(return_value={'inert-selected-body': True})
                if fail_at == 'emitted-file':
                    emitted.side_effect = RuntimeError('inert emitted-file failure')
                with mock.patch.object(artifacts, '_metadata_exact'), \
                     mock.patch.object(artifacts, '_emitted_artifact', emitted), \
                     self.assertRaisesRegex(RuntimeError, 'inert ' + fail_at + ' failure'):
                    artifacts._produce_build(source, context, {'recipe': selected})
                process.retain_production_failure.assert_called_once()
                process.cancel_production.assert_called_once_with(invocation)
                self.assertFalse(artifacts._LIVE)



if __name__ == '__main__':
    unittest.main()
