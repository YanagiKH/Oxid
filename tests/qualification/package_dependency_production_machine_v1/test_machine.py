"""Pure/inert production-machine controls. No compiler/probe/build/network runs.

Synthetic dictionaries below exercise pure parsing and rejection boundaries.
They are NOT live source, process, machine, build or CI evidence. No test inserts
an admission into the production registry or claims a successful live capture.
"""
import ast
import contextlib
import copy
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import types
import unittest
from unittest import mock

HERE = Path(__file__).resolve().parent
MARKER = b'\n\n# BEGIN SOURCE-ONLY PRODUCTION SUCCESSOR V1\n'
BASELINE_PATH = 'KH/package-unit4-consumers-v7/candidate/tests/qualification/package_dependency_unit4_machine_v1/machine.py'
BASELINE_SHA256 = '19a6c3bd82e119b501bc1f6910fa96ad8d2b86f50beac4b4bae3b7a8de0a3056'
BASELINE_BYTES = 94821


def load(path, key):
    spec = importlib.util.spec_from_file_location(key, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


machine = load(HERE / 'machine.py', '_inert_production_machine_controls')
artifacts = load(HERE.parent / 'package_dependency_ci_v1/artifacts.py', '_inert_production_artifact_helpers')


def inert_policy(recipe_id='quality-and-repository:release-production'):
    """Unissued data for pure shape controls, never admitted to an owner."""
    fixed = json.loads((HERE.parent / 'package_dependency_ci_v1/routes.json').read_text())
    recipe = copy.deepcopy(fixed['build_recipes'][recipe_id])
    recipe['tool_policy'] = {'reviewed': True, 'cargo_version': 'inert cargo\n',
                             'rustc_version': 'inert rustc\n', 'rustc_host': 'x86_64-unknown-linux-gnu'}
    recipe['environment_policy'] = {'reviewed': True, 'inherit': ['PATH', 'HOME', 'CARGO_TERM_COLOR'],
                                    'fixed': {}, 'forbidden': list(artifacts._FORBIDDEN_ENV)}
    recipe['config_policy'] = {'reviewed': True, 'files': [], 'absent': []}
    recipe['resource_policy'] = {'reviewed': True, 'timeout_seconds': 123, 'stream_limit_bytes': 2048}
    recipe['machine_policy'] = {'schema': 'oxid-package-production-machine-policy-v1', 'reviewed': True,
        'host': 'linux-x86_64', 'discovery': 'direct',
        'toolchain_selector': 'stable' if recipe_id.startswith('cross-platform:') else '1.99.0',
        'inputs': {'discovery': {}, 'rustup': None, 'resolver_settings': [], 'native': {}, 'workspace': [],
                   'configuration': {}, 'rust': {'root': '/inert', 'tools': {}, 'shared_libraries': [],
                   'sysroot_trees': [], 'parser_original_authority': None}}}
    return recipe


class InertPolicySource:
    def load_sibling(self, name):
        if name != 'artifacts':
            raise AssertionError('unexpected policy sibling')
        return artifacts


@contextlib.contextmanager
def inert_snapshot_inputs():
    """Fake read-only boundaries for a detached snapshot, never any issuance."""
    with tempfile.TemporaryDirectory(prefix='inert-production-readbacks-') as name, contextlib.ExitStack() as stack:
        root = Path(name).resolve()
        recipe = inert_policy()
        original = {'repository_path': str(root), 'host': 'linux-x86_64',
                    'source_files': [{'path': 'inert-source', 'bytes': 1, 'sha256': 'a' * 64}]}
        document = {'source': original, 'recipe': recipe, 'authority': 'synthetic-inert-data'}
        projection = {'id': 'current-all376', 'files': copy.deepcopy(original['source_files'])}
        source = InertPolicySource()
        source.verify_job = lambda context: copy.deepcopy(original)
        source.projection = lambda context, name: copy.deepcopy(projection)
        environment = {'HOME': str(root), 'PATH': str(root / 'bin'), 'CARGO_TERM_COLOR': 'always'}
        host = {'name': 'Linux x86_64', 'target': 'x86_64-unknown-linux-gnu'}
        discovery = {name: {'selected': str(root / 'toolchain/bin' / name),
                           'resolved': str(root / 'toolchain/bin' / name), 'authority': 'inert-route'}
                     for name in ('cargo', 'rustc')}
        settings = [{'path': str(root / '.rustup' / relative), 'absent': True} for relative in
                    ('settings.toml', 'update-hashes/1.99.0-x86_64-unknown-linux-gnu')]
        rust = {'root': str(root / 'toolchain'), 'tools': copy.deepcopy(discovery), 'shared_libraries': ['inert libraries'],
                'sysroot_trees': ['inert full sysroot'], 'parser_original_authority': None}
        native = {'tools': {'cc': {'resolved': '/inert/cc'}}, 'trees': ['inert native trees'], 'files': ['inert native files']}
        workspace = [{'path': str(root / 'Cargo.toml'), 'bytes': 1, 'sha256': 'b' * 64}]
        configuration = {'declared': [], 'search_locations': [{'path': str(root / '.cargo/config'), 'absent': True}]}
        recipe['machine_policy']['inputs'] = {'discovery': copy.deepcopy(discovery), 'rustup': None,
            'resolver_settings': settings, 'native': native, 'rust': rust,
            'workspace': workspace, 'configuration': configuration}
        for function, value in (('_production_host', host), ('_production_environment', environment),
                                ('_production_configuration', (configuration, root)), ('_production_native', native),
                                ('_production_workspace', workspace), ('_dependency_closure', ['inert locked archive closure'])):
            stack.enter_context(mock.patch.object(machine, function, return_value=copy.deepcopy(value)))
        stack.enter_context(mock.patch.object(machine, '_production_discover', side_effect=lambda name, env: copy.deepcopy(discovery[name])))
        rust_reader = stack.enter_context(mock.patch.object(machine, '_rust_inputs', return_value=copy.deepcopy(rust)))
        policies = machine._production_policy(source, recipe)
        yield source, object(), document, projection, policies, root / 'toolchain', rust_reader


class ProductionMachineControls(unittest.TestCase):
    def setUp(self):
        self.effects = mock.patch.object(subprocess, 'Popen', side_effect=AssertionError('no subprocess is authorized'))
        self.effects.start()
        self.addCleanup(self.effects.stop)
        self.assertEqual(len(machine._PRODUCTION_ISSUED), 0)
        self.assertEqual(len(machine._PRODUCTION_CAPTURES), 0)

    def tearDown(self):
        self.assertEqual(len(machine._PRODUCTION_ISSUED), 0, 'pure fixture must never issue a live machine')
        self.assertEqual(len(machine._PRODUCTION_CAPTURES), 0, 'pure fixture must never begin a live capture')

    def test_complete_baseline_prefix_is_byte_identical(self):
        raw = (HERE / 'machine.py').read_bytes()
        prefix, appended = raw.split(MARKER, 1)
        self.assertEqual(len(prefix), BASELINE_BYTES)
        self.assertEqual(hashlib.sha256(prefix).hexdigest(), BASELINE_SHA256)
        self.assertEqual(machine.PRODUCTION_BASELINE_PATH, BASELINE_PATH)
        self.assertEqual(machine.PRODUCTION_BASELINE_SHA256, BASELINE_SHA256)
        self.assertEqual(machine.PRODUCTION_BASELINE_BYTES, BASELINE_BYTES)
        self.assertIn(b'PRODUCTION_EXECUTION_REVIEWED = False', appended)
        roots = [parent for parent in HERE.parents if (parent / BASELINE_PATH).is_file()]
        self.assertEqual(len(roots), 1, 'the exact retained source baseline must be available')
        self.assertEqual((roots[0] / BASELINE_PATH).read_bytes(), prefix)

    def test_neutral_reused_helpers_have_exact_source_diff_proof(self):
        raw = (HERE / 'machine.py').read_text()
        prefix, appended = raw.split(MARKER.decode(), 1)
        baseline = ast.parse(prefix)
        successor = ast.parse(raw)
        names = ('_file', '_body', '_route', '_tree', '_dependency_closure', '_rust_inputs', '_native_inputs')
        for name in names:
            old = [node for node in baseline.body if isinstance(node, ast.FunctionDef) and node.name == name]
            new = [node for node in successor.body if isinstance(node, ast.FunctionDef) and node.name == name]
            self.assertEqual(len(old), 1)
            self.assertEqual(len(new), 1)
            self.assertEqual(ast.dump(old[0], include_attributes=False), ast.dump(new[0], include_attributes=False))
        forbidden = {'_snapshot', '_source_authority', '_clean_environment', '_workspace_absence',
                     '_config_absence', '_capture_machine', '_shared', '_probe', 'measured_host', '_producer_environment'}
        calls = {node.func.id for node in ast.walk(ast.parse(appended))
                 if isinstance(node, ast.Call) and isinstance(node.func, ast.Name)}
        self.assertFalse(calls & forbidden)
        inherited_definitions = {node.name for node in baseline.body if isinstance(node, (ast.FunctionDef, ast.ClassDef))}
        self.assertFalse((calls & inherited_definitions) - machine.PRODUCTION_NEUTRAL_REUSE)
        self.assertTrue(set(names) <= machine.PRODUCTION_NEUTRAL_REUSE)

    def test_inherited_public_effect_apis_all_reject_immediately(self):
        expected = {'measured_host', 'capture_machine', 'verify_machine', 'supervisor', 'load_machine',
                    'capture_runtime_tools', 'capture_capability_traps', 'verify_runtime_tools',
                    'verify_runtime_inputs', 'capture_parser_runtime', 'verify_parser_runtime',
                    'verify_parser_runtime_selected'}
        self.assertEqual(set(machine._PRODUCTION_DISABLED_APIS), expected)
        with mock.patch.object(machine, '_shared', side_effect=AssertionError('frozen owner must not load')):
            for name in expected:
                with self.subTest(name=name), self.assertRaisesRegex(machine.Rejected, 'disabled'):
                    getattr(machine, name)(object(), arbitrary='inert adversarial arguments')

    def test_inherited_capability_constructors_and_methods_are_sentinels(self):
        for cls in (machine.ToolAdmission, machine.RuntimeTools, machine.ParserRuntimeInputs):
            with self.assertRaisesRegex(machine.Rejected, 'disabled'):
                cls()
            forged = object.__new__(cls)
            for name, member in vars(cls).items():
                if not name.startswith('_') and callable(member):
                    with self.subTest(cls=cls.__name__, method=name), self.assertRaisesRegex(machine.Rejected, 'disabled'):
                        getattr(forged, name)()

    def test_production_constructor_rejects_data_and_callbacks(self):
        for value in ({'authority': 'inert'}, lambda: True, object()):
            with self.assertRaises(machine.Rejected):
                machine.ProductionToolAdmission(value)

    def test_detached_module_and_forged_admission_reject_before_reads(self):
        forged = object.__new__(machine.ProductionToolAdmission)
        with mock.patch.object(machine, '_file', side_effect=AssertionError('no tool read')):
            for action in (lambda: machine.capture_production_machine({}),
                           lambda: machine.verify_production_machine(forged, object()),
                           lambda: machine.production_operation_inputs(object(), 'build'),
                           lambda: machine.verify_production_for_operation(object(), 'build'),
                           lambda: forged.document()):
                with self.assertRaises(machine.Rejected):
                    action()

    def test_ambient_matching_module_is_not_source_authority(self):
        fake = types.ModuleType(machine.PRODUCTION_SOURCE_KEY)
        fake.owner = lambda: fake
        fake.owns_production_owner = lambda module: False
        with mock.patch.dict(sys.modules, {machine.PRODUCTION_SOURCE_KEY: fake, machine.PRODUCTION_KEY: machine}), \
             mock.patch.object(machine, '_SOURCE_OWNER', fake, create=True), \
             self.assertRaisesRegex(machine.Rejected, 'did not issue'):
            machine._production_source()

    def test_module_name_path_or_hash_cannot_replace_exact_association(self):
        fake = types.ModuleType(machine.PRODUCTION_SOURCE_KEY)
        fake.owner = lambda: fake
        fake.owns_production_owner = lambda module: True
        other = types.ModuleType(machine.PRODUCTION_KEY)
        other.__file__ = str(HERE / 'machine.py')
        other.__source_sha256__ = hashlib.sha256((HERE / 'machine.py').read_bytes()).hexdigest()
        with mock.patch.dict(sys.modules, {machine.PRODUCTION_SOURCE_KEY: fake, machine.PRODUCTION_KEY: other}), \
             mock.patch.object(machine, '_SOURCE_OWNER', fake, create=True), \
             self.assertRaisesRegex(machine.Rejected, 'canonical module'):
            machine._production_source()

    def test_source_only_barrier_is_not_a_registry_flag(self):
        source = mock.Mock()
        doc = {'source': {'host': 'linux-x86_64', 'activation': {'approved': True}},
               'recipe': {'operational': True, 'reviewed': True}}
        with self.assertRaisesRegex(machine.NotReady, 'source-only'):
            machine._production_gate(source, object(), doc)
        source.require_operational.assert_not_called()
        self.assertIs(machine.PRODUCTION_EXECUTION_REVIEWED, False)

    def test_windows_containment_gate_precedes_every_effect_even_with_flags(self):
        source = mock.Mock()
        doc = {'source': {'host': 'windows-x86_64'}, 'recipe': {'operational': True}}
        with mock.patch.object(machine, 'PRODUCTION_EXECUTION_REVIEWED', True), \
             self.assertRaisesRegex(machine.Rejected, 'descendant containment'):
            machine._production_gate(source, object(), doc)
        source.require_operational.assert_not_called()

    def test_capture_terminal_checkpoint_revokes_already_issued_inert_placeholder(self):
        # Replace both registries with isolated ordinary dictionaries. This is
        # cleanup control flow only; the placeholder is not a live capability.
        invocation, placeholder = object(), object()
        state = {'invocation': invocation, 'complete': True, 'invalidated': False,
                 'authority': 'synthetic-inert-cleanup-fixture'}
        issued, captures = {}, {}
        process = mock.Mock()
        process.production_tick.side_effect = [None, machine.Rejected('inert terminal checkpoint failure')]
        def inert_capture(*args):
            issued[placeholder] = state
            captures[invocation] = state
            return placeholder
        with mock.patch.object(machine, '_PRODUCTION_ISSUED', issued), \
             mock.patch.object(machine, '_PRODUCTION_CAPTURES', captures), \
             mock.patch.object(machine, '_production_bound', return_value=(object(), process, object(), {}, {})), \
             mock.patch.object(machine, '_production_gate'), \
             mock.patch.object(machine, '_capture_production_machine', side_effect=inert_capture), \
             self.assertRaisesRegex(machine.Rejected, 'terminal checkpoint'):
            machine.capture_production_machine(invocation)
        self.assertEqual(issued, {})
        self.assertEqual(captures, {})
        self.assertTrue(state['invalidated'])
        self.assertFalse(state['complete'])
        self.assertEqual(process.production_tick.call_count, 2)

    def test_capture_owner_failure_revokes_existing_inert_state(self):
        invocation, placeholder = object(), object()
        state = {'invocation': invocation, 'complete': True, 'invalidated': False}
        issued, captures = {placeholder: state}, {invocation: state}
        with mock.patch.object(machine, '_PRODUCTION_ISSUED', issued), \
             mock.patch.object(machine, '_PRODUCTION_CAPTURES', captures), \
             mock.patch.object(machine, '_production_bound', side_effect=machine.Rejected('inert owner drift')), \
             self.assertRaisesRegex(machine.Rejected, 'owner drift'):
            machine.capture_production_machine(invocation)
        self.assertEqual(issued, {})
        self.assertEqual(captures, {})
        self.assertTrue(state['invalidated'])

    def test_readmission_owner_and_source_failures_invalidate_inert_original_state(self):
        for failure in ('initial-owner', 'source', 'wrong-invocation', 'gate'):
            invocation, placeholder = object(), object()
            state = {'invocation': invocation, 'complete': True, 'invalidated': False}
            with contextlib.ExitStack() as stack:
                stack.enter_context(mock.patch.object(machine, '_PRODUCTION_ISSUED', {placeholder: state}))
                stack.enter_context(mock.patch.object(machine, '_production_issued',
                    side_effect=machine.Rejected('inert initial owner failure') if failure == 'initial-owner' else None,
                    return_value=state))
                stack.enter_context(mock.patch.object(machine, '_production_bound',
                    side_effect=machine.Rejected('inert source failure') if failure == 'source' else None,
                    return_value=(object(), object(), object(), {}, {})))
                stack.enter_context(mock.patch.object(machine, '_production_gate',
                    side_effect=machine.Rejected('inert gate failure')))
                with self.subTest(failure=failure), self.assertRaises(machine.Rejected):
                    machine.verify_production_machine(placeholder, object() if failure == 'wrong-invocation' else invocation)
            self.assertTrue(state['invalidated'])

    def test_dormant_recipes_keep_exact_original_build_and_network_behavior(self):
        registry = json.loads((HERE.parent / 'package_dependency_ci_v1/routes.json').read_text())
        self.assertEqual(set(registry['build_recipes']), machine._PRODUCTION_BUILD_IDS)
        self.assertIs(registry['activation']['approved'], False)
        for recipe in registry['build_recipes'].values():
            self.assertEqual(recipe['argv'], ['cargo', 'build', '--release', '--locked'])
            self.assertEqual(recipe['projection_id'], 'current-all376')
            self.assertIs(recipe['operational'], False)
            for name in ('tool_policy', 'environment_policy', 'config_policy', 'resource_policy'):
                self.assertIs(recipe[name]['reviewed'], False)
            with self.assertRaises((machine.Rejected, artifacts.Rejected)):
                machine._production_policy(InertPolicySource(), recipe)

    def test_machine_policy_requires_full_structural_inputs(self):
        recipe = inert_policy()
        self.assertEqual(machine._production_policy(InertPolicySource(), recipe)[5], recipe['machine_policy'])
        for replacement in ({'sha256': 'a' * 64}, {'version': '1.99.0'}, None):
            bad = copy.deepcopy(recipe)
            bad['machine_policy']['inputs']['rust'] = replacement
            with self.assertRaises(machine.Rejected):
                machine._production_policy(InertPolicySource(), bad)

    def test_machine_policy_rejects_missing_unknown_or_unreviewed_inputs(self):
        for mutation in ('missing', 'unknown', 'unreviewed', 'parser'):
            recipe = inert_policy()
            if mutation == 'missing':
                del recipe['machine_policy']['inputs']['native']
            elif mutation == 'unknown':
                recipe['machine_policy']['inputs']['callback'] = 'caller protocol'
            elif mutation == 'unreviewed':
                recipe['machine_policy']['reviewed'] = False
            else:
                recipe['machine_policy']['inputs']['rust']['parser_original_authority'] = {'inert': True}
            with self.subTest(mutation=mutation), self.assertRaises(machine.Rejected):
                machine._production_policy(InertPolicySource(), recipe)

    def test_stable_cross_platform_and_pinned_quality_native_stay_distinct(self):
        for recipe_id in machine._PRODUCTION_BUILD_IDS:
            recipe = inert_policy(recipe_id)
            expected = 'stable' if recipe_id.startswith('cross-platform:') else '1.99.0'
            self.assertEqual(machine._production_policy(InertPolicySource(), recipe)[5]['toolchain_selector'], expected)
            recipe['machine_policy']['toolchain_selector'] = '1.99.0' if expected == 'stable' else 'stable'
            with self.assertRaises(machine.Rejected):
                machine._production_policy(InertPolicySource(), recipe)

    def test_original_environment_is_preserved_without_unit4_overrides(self):
        policy = inert_policy()['environment_policy']
        original = {'PATH': '/inert/bin', 'HOME': '/inert/home', 'CARGO_TERM_COLOR': 'always'}
        with mock.patch.dict(os.environ, original, clear=True):
            observed = machine._production_environment(artifacts, policy)
            self.assertEqual(observed, original)
            self.assertEqual(dict(os.environ), original)
            self.assertFalse(set(observed) & {'CC', 'CXX', 'AR', 'RUSTC', 'CARGO_TARGET_DIR', 'CARGO_NET_OFFLINE'})

    def test_injected_color_and_compiler_overrides_reject(self):
        policy = inert_policy()['environment_policy']
        original = {'PATH': '/inert/bin', 'HOME': '/inert/home', 'CARGO_TERM_COLOR': 'always'}
        for key, value in (('CC', '/inert/cc'), ('CXX', '/inert/cxx'), ('AR', '/inert/ar'),
                           ('CARGO_TARGET_DIR', '/inert/target'), ('CARGO_NET_OFFLINE', 'true'),
                           ('RUSTDOC', '/inert/rustdoc'), ('CRATE_CC_NO_DEFAULTS', '1'),
                           ('CARGO_TERM_COLOR', 'never'), ('CFLAGS_x86_64_unknown_linux_gnu', '-bad')):
            with mock.patch.dict(os.environ, dict(original, **{key: value}), clear=True), \
                 self.subTest(key=key), self.assertRaises((machine.Rejected, artifacts.Rejected)):
                machine._production_environment(artifacts, policy)

    def test_policy_fixed_value_cannot_change_original_environment(self):
        policy = inert_policy()['environment_policy']
        policy['fixed'] = {'PATH': '/injected/bin'}
        with mock.patch.dict(os.environ, {'PATH': '/inert/bin', 'HOME': '/inert/home', 'CARGO_TERM_COLOR': 'always'}, clear=True), \
             self.assertRaisesRegex(machine.Rejected, 'preserve'):
            machine._production_environment(artifacts, policy)

    def test_native_search_and_loader_overrides_cannot_be_silently_omitted(self):
        policy = inert_policy()['environment_policy']
        original = {'PATH': '/inert/bin', 'HOME': '/inert/home', 'CARGO_TERM_COLOR': 'always'}
        names = ('ARFLAGS', 'ARFLAGS_x86_64_unknown_linux_gnu', 'HOST_ARFLAGS', 'TARGET_ARFLAGS',
                 'HOST_CFLAGS', 'TARGET_CFLAGS', 'HOST_CXXFLAGS', 'TARGET_CXXFLAGS',
                 'HOST_CPPFLAGS', 'TARGET_CPPFLAGS', 'HOST_LDFLAGS', 'TARGET_LDFLAGS',
                 'HOST_RANLIB', 'TARGET_RANLIB', 'HOST_LD', 'TARGET_LD',
                 'LIBRARY_PATH', 'CPATH', 'C_INCLUDE_PATH', 'CPLUS_INCLUDE_PATH',
                 'COMPILER_PATH', 'GCC_EXEC_PREFIX', 'LD_LIBRARY_PATH', 'LD_PRELOAD', 'LD_AUDIT',
                 'DYLD_LIBRARY_PATH', 'DYLD_INSERT_LIBRARIES', 'DYLD_FALLBACK_LIBRARY_PATH')
        for name in names:
            with self.subTest(name=name), mock.patch.dict(os.environ, dict(original, **{name: 'inert-override'}), clear=True), \
                 self.assertRaisesRegex(machine.Rejected, 'compiler/native override'):
                machine._production_environment(artifacts, policy)

    def test_authenticated_cargo_config_is_read_then_rejects_effective_selection(self):
        for basename in ('config', 'config.toml'):
            with self.subTest(basename=basename), tempfile.TemporaryDirectory(prefix='inert-production-config-') as name:
                root = Path(name).resolve(); repository = root / 'checkout'; repository.mkdir()
                home = root / 'home'; home.mkdir(); (home / '.cargo').mkdir()
                (repository / '.cargo').mkdir()
                path = repository / '.cargo' / basename
                raw = b'[build]\nrustc="/inert/unobserved-compiler"\n'
                path.write_bytes(raw)
                policy = {'reviewed': True, 'absent': [], 'files': [{'root': 'repository',
                    'path': '.cargo/' + basename, 'bytes': len(raw), 'sha256': hashlib.sha256(raw).hexdigest()}]}
                source = types.SimpleNamespace(identity=lambda selected: machine._file(selected))
                with mock.patch.object(machine, '_file', wraps=machine._file) as readback, \
                     self.assertRaisesRegex(machine.NotReady, 'effective Cargo config'):
                    machine._production_configuration(source, artifacts, repository, {'HOME': str(home)}, policy)
                self.assertGreaterEqual(sum(call.args == (path,) for call in readback.call_args_list), 2)

    def test_rust_toolchain_config_keeps_independent_resolution_path(self):
        with tempfile.TemporaryDirectory(prefix='inert-production-toolchain-') as name:
            root = Path(name).resolve(); repository = root / 'checkout'; repository.mkdir()
            home = root / 'home'; home.mkdir(); (home / '.cargo').mkdir()
            path = repository / 'rust-toolchain.toml'
            raw = b'[toolchain]\nchannel="stable"\n'
            path.write_bytes(raw)
            policy = {'reviewed': True, 'absent': [], 'files': [{'root': 'repository', 'path': 'rust-toolchain.toml',
                'bytes': len(raw), 'sha256': hashlib.sha256(raw).hexdigest()}]}
            source = types.SimpleNamespace(identity=lambda selected: machine._file(selected))
            observed, cargo_home = machine._production_configuration(source, artifacts, repository, {'HOME': str(home)}, policy)
            self.assertEqual(cargo_home, home / '.cargo')
            self.assertEqual(next(row for row in observed['search_locations'] if row['path'] == str(path))['sha256'],
                             hashlib.sha256(raw).hexdigest())

    def test_native_helper_mutations_are_discarded_and_selection_verified(self):
        environment = {'PATH': '/inert/bin', 'CARGO_TERM_COLOR': 'always'}
        before = dict(environment)
        names = {'cc': 'cc', 'cxx': 'c++', 'ar': 'ar', 'linker': 'cc'}
        native = {'tools': {key: {'resolved': '/inert/bin/' + name} for key, name in names.items()},
                  'trees': ['inert native trees'], 'files': []}
        def helper(host, private):
            self.assertIsNot(private, environment)
            private.update(CC='/injected/cc', SDKROOT='/injected/sdk', PATH='/injected/path')
            return copy.deepcopy(native)
        def discover(name, actual):
            self.assertEqual(actual, before)
            return {'resolved': '/inert/bin/' + name}
        with mock.patch.object(machine, '_native_inputs', side_effect=helper), \
             mock.patch.object(machine, '_production_discover', side_effect=discover):
            self.assertEqual(machine._production_native({'name': 'Linux x86_64'}, environment), native)
        self.assertEqual(environment, before)

    def test_native_mismatch_cannot_silently_change_compiler(self):
        with mock.patch.object(machine, '_native_inputs', return_value={'tools': {'cc': {'resolved': '/different'}}}), \
             mock.patch.object(machine, '_production_discover', return_value={'resolved': '/actual'}), \
             self.assertRaisesRegex(machine.Rejected, 'unchanged production environment'):
            machine._production_native({'name': 'Linux x86_64'}, {'PATH': '/inert'})

    def test_apple_selection_remains_explicitly_source_held(self):
        with mock.patch.object(machine, '_native_inputs', return_value={'tools': {}}), \
             self.assertRaisesRegex(machine.NotReady, 'Apple'):
            machine._production_native({'name': 'macOS arm64'}, {'PATH': '/inert'})

    def test_full_structural_authority_keeps_bodies_and_link_routes(self):
        value = {'route': [{'path': '/inert/tool', 'device': 1, 'inode': 2, 'mode': 0o100755, 'link': 'real'}],
                 'body': {'path': '/inert/real', 'bytes': 7, 'sha256': 'a' * 64, 'stat': {'st_ino': 2}}}
        result = machine._production_authority(value)
        self.assertEqual(result, {'route': [{'path': '/inert/tool', 'mode': 0o100755, 'link': 'real'}],
                                  'body': {'path': '/inert/real', 'bytes': 7, 'sha256': 'a' * 64}})
        changed = copy.deepcopy(value); changed['body']['sha256'] = 'b' * 64
        self.assertNotEqual(result, machine._production_authority(changed))
        self.assertIn('stat', value['body'])

    def test_inert_complete_snapshot_preserves_all_source_and_input_readbacks(self):
        with inert_snapshot_inputs() as (source, context, document, projection, policies, root, rust_reader):
            result = machine._production_snapshot(source, artifacts, context, document, projection, policies, root)
            self.assertEqual(result['source'], document['source'])
            self.assertEqual(result['projection'], projection)
            self.assertEqual(result['environment']['CARGO_TERM_COLOR'], 'always')
            self.assertEqual(result['dependencies'], ['inert locked archive closure'])
            for name in ('native', 'rust', 'discovery', 'resolver_settings', 'configuration', 'workspace'):
                self.assertEqual(result[name], policies[5]['inputs'][name])
            rust_reader.assert_called_once_with(root, result['host'], None, parser_required=False)
            self.assertNotIn('maps', result)
            self.assertNotIsInstance(result, machine.ProductionToolAdmission)

    def test_each_structural_input_policy_drift_rejects_inert_snapshot(self):
        for name in ('discovery', 'resolver_settings', 'native', 'rust', 'workspace', 'configuration'):
            with self.subTest(name=name), inert_snapshot_inputs() as (source, context, document, projection, policies, root, _):
                policies[5]['inputs'][name] = {'authority': 'changed-inert-policy'}
                with self.assertRaisesRegex(machine.Rejected, 'input authority differs'):
                    machine._production_snapshot(source, artifacts, context, document, projection, policies, root)

    def test_original_source_or_projection_drift_precedes_any_tool_reads(self):
        for kind in ('source', 'projection'):
            with self.subTest(kind=kind), inert_snapshot_inputs() as (source, context, document, projection, policies, root, rust_reader):
                if kind == 'source':
                    source.verify_job = lambda context: {'authority': 'different-inert-source'}
                else:
                    source.projection = lambda context, name: {'authority': 'different-inert-projection'}
                with self.assertRaisesRegex(machine.Rejected, 'source/projection drift'):
                    machine._production_snapshot(source, artifacts, context, document, projection, policies, root)
                rust_reader.assert_not_called()

    def test_workspace_readback_records_existing_and_absent_ancestors(self):
        with tempfile.TemporaryDirectory(prefix='inert-production-workspace-') as name:
            repository = Path(name).resolve() / 'checkout'
            repository.mkdir()
            (repository / 'Cargo.toml').write_text('[package]\nname="inert"\nversion="0.0.0"\n')
            rows = machine._production_workspace(repository)
            self.assertEqual(rows[0]['path'], str(repository / 'Cargo.toml'))
            self.assertEqual(len(rows), len(repository.parents) + 1)
            self.assertTrue(all(row.get('absent') is True for row in rows[1:]))

    def test_ambient_ancestor_workspace_rejects(self):
        with tempfile.TemporaryDirectory(prefix='inert-production-workspace-') as name:
            parent = Path(name).resolve(); repository = parent / 'checkout'; repository.mkdir()
            (repository / 'Cargo.toml').write_text('[package]\nname="inert"\nversion="0.0.0"\n')
            (parent / 'Cargo.toml').write_text('[workspace]\nmembers=["checkout"]\n')
            with self.assertRaisesRegex(machine.Rejected, 'ancestor Cargo workspace'):
                machine._production_workspace(repository)

    def test_fixed_descriptor_is_detached_pure_data_and_cannot_issue_ownership(self):
        recipe = inert_policy()
        state = {'document': {'recipe': recipe}, 'snapshot': {'repository': '/inert/checkout',
                 'environment': {'PATH': '/inert/bin', 'CARGO_TERM_COLOR': 'always'}}, 'operations': {}}
        machine._production_register(state, 'build', recipe['argv'], 'ci-production-build', recipe['resource_policy'])
        data = machine.decode(state['operations']['build'])
        self.assertEqual(set(data), {'argv', 'cwd', 'environment', 'timeout_seconds', 'stream_limit_bytes', 'kind'})
        self.assertEqual(data['argv'], ['cargo', 'build', '--release', '--locked'])
        self.assertEqual(data['cwd'], '/inert/checkout')
        data['environment']['CC'] = '/inert/injected'
        self.assertNotIn('CC', machine.decode(state['operations']['build'])['environment'])
        with self.assertRaises(machine.Rejected):
            machine.production_operation_inputs(state, 'build')

    def test_fixed_descriptor_rejects_unknown_duplicate_or_changed_build(self):
        recipe = inert_policy()
        def state():
            return {'document': {'recipe': recipe}, 'snapshot': {'repository': '/inert', 'environment': {}}, 'operations': {}}
        for name, argv in (('arbitrary', ['sh']), ('build', ['cargo', 'build', '--offline']),
                           ('build', ['cargo', 'build', '--release', '--locked', '--target-dir', '/inert'])):
            with self.subTest(name=name, argv=argv), self.assertRaises(machine.Rejected):
                machine._production_register(state(), name, argv, 'ci-production-build')
        value = state()
        machine._production_register(value, 'build', recipe['argv'], 'ci-production-build')
        with self.assertRaisesRegex(machine.Rejected, 'duplicate'):
            machine._production_register(value, 'build', recipe['argv'], 'ci-production-build')

    def test_operation_roster_is_closed_and_runtime_roles_absent(self):
        self.assertEqual(machine._PRODUCTION_IDS, frozenset((
            'rustup-which-cargo', 'rustup-which-rustc', 'rustc-version', 'cargo-version', 'rustc-sysroot',
            'native-cc-version', 'native-cxx-version', 'native-ar-version', 'metadata-before', 'metadata-after', 'build')))
        self.assertFalse(any('parser' in name or 'runtime' in name for name in machine._PRODUCTION_IDS))

    def test_production_source_never_rewrites_original_tools_environment_or_source_map(self):
        appended = (HERE / 'machine.py').read_text().split(MARKER.decode(), 1)[1]
        self.assertIn("parser_required=False", appended)
        self.assertNotIn("'maps':", appended)
        self.assertNotIn('ToolAdmission.environment', appended)
        self.assertNotIn("'which', '--toolchain'", appended)
        self.assertIn("'cargo_network_policy': 'original-online'", appended)
        self.assertIn("'execution_qualified': False", appended)


if __name__ == '__main__':
    unittest.main()
