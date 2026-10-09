"""Source-only RFC0031 controller controls; these do not claim hosted execution."""
import copy
from types import SimpleNamespace
import json
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import textwrap
import unittest
from unittest.mock import patch

import verify_bounded_byte_storage as gate
import verify_bounded_stdin_native as builds
import verify_bounded_u8_native as u8_gate
import verify_repo

REPO = Path(__file__).resolve().parents[1]


class PackageTests(unittest.TestCase):
    def test_portable_identities_equal_unchanged_native_predecessor(self):
        self.assertEqual(gate.BYTE_STORAGE_ARCHIVES, u8_gate.BYTE_STORAGE_ARCHIVES)
        self.assertEqual(gate.STAGER, u8_gate.STAGER)

    def test_fresh_portable_imports_admit_complete_fixtures_without_resource(self):
        script = textwrap.dedent('''
            import importlib
            import importlib.abc
            import json
            from pathlib import Path
            import sys

            class NoResource(importlib.abc.MetaPathFinder):
                def find_spec(self, fullname, path=None, target=None):
                    if fullname == 'resource':
                        raise ModuleNotFoundError('resource is unavailable on this host')

            sys.modules.pop('resource', None)
            sys.meta_path.insert(0, NoResource())
            repo = Path(sys.argv[1])
            sys.path.insert(0, str(repo / 'scripts'))
            importlib.import_module(sys.argv[2])
            import verify_bounded_byte_storage as gate
            import verify_repo
            manifest, registry, cases = gate.admit_package(repo)
            gate.admit_registry(repo, registry)
            current = gate.fixture_data_sources(repo)
            previous = verify_repo.predecessor_fixture_data_sources(repo)
            gate.require(len(cases) == 323 and len(current) == 325 and len(previous) == 188,
                         'portable fixture counts differ')
            gate.require(not current & previous and verify_repo.fixture_data_sources(repo) == current | previous,
                         'portable fixture registration differs')
            verify_repo.verify_test_fixture_registration(repo)
            gate.require(not {'resource', 'verify_bounded_stdin_native', 'verify_bounded_u8_native'} & sys.modules.keys(),
                         'portable registration loaded native execution helpers')
            print(json.dumps({'cases': len(cases), 'current': len(current), 'predecessor': len(previous)}))
        ''')
        for module in ('verify_bounded_byte_storage', 'verify_repo', 'verify_fixture_data',
                       'verify_bounded_typed_parser', 'verify_bounded_typed_static'):
            with self.subTest(module=module):
                result = subprocess.run([sys.executable, '-I', '-B', *(['-O'] if sys.flags.optimize else []),
                                         '-c', script, str(REPO), module], capture_output=True, text=True, timeout=60)
                self.assertEqual(result.returncode, 0, result.stderr)
                self.assertEqual(json.loads(result.stdout), {'cases': 323, 'current': 325, 'predecessor': 188})

    def copy_package(self):
        tmp = tempfile.TemporaryDirectory(); self.addCleanup(tmp.cleanup)
        root = Path(tmp.name)
        shutil.copytree(REPO / gate.PACKAGE, root / gate.PACKAGE)
        for path in (root / gate.PACKAGE).rglob("*"):
            if path.is_file(): path.chmod(0o644)
        return root

    def test_exact_immutable_complete_corpus_and_independent_transport_values(self):
        manifest, registry, cases = gate.admit_package(REPO)
        gate.admit_registry(REPO, registry)
        self.assertEqual(len(cases), 323)
        self.assertEqual(len(gate.fixture_data_sources(REPO)), 325)
        self.assertEqual(sum(c['expected']['kind'] == 'diagnostic' for c in cases), 49)
        self.assertEqual(sum(c['expected'].get('code', '').startswith('E06') for c in cases), 13)
        self.assertEqual(len(registry['unit_tests']), 79)
        self.assertEqual(sum(r['ignored'] for r in registry['unit_tests']), 4)
        self.assertEqual([len(r['artifacts']) for r in registry['native_tests']], [19, 20, 52, 2])
        self.assertEqual(len(registry['public_tests']), 10)
        self.assertEqual(len(registry['privacy_probes']), 11)
        self.assertEqual(gate.sha((REPO / gate.STAGER).read_bytes()), gate.STAGER_SHA256)

    def pre_portability_registry(self, manifest, registry):
        additions = {
            'frontend::oir::owned::source::byte_storage_tests::byte_storage_source_identical_module_text_keeps_runtime_file_identity',
            'frontend::oir::owned::source::byte_storage_tests::byte_storage_source_unused_nested_enum_array_payload_keeps_inner_origin',
        }
        self.assertEqual(len(registry['unit_tests']), 79)
        self.assertTrue(additions <= {r['name'] for r in registry['unit_tests']})
        predecessor = copy.deepcopy(registry)
        predecessor['unit_tests'] = [r for r in predecessor['unit_tests'] if r['name'] not in additions]
        self.assertEqual((len(predecessor['unit_tests']), len(predecessor['public_tests']), len(predecessor['privacy_probes'])), (77, 10, 11))
        previous_bytes = (json.dumps(predecessor, indent=2) + '\n').encode()
        self.assertEqual(gate.sha(previous_bytes), 'f199a3680c91526a40229499c64be6302ea17bd6855fc53571048c73aa9b1ad5')
        previous_manifest = copy.deepcopy(manifest)
        previous_manifest['files']['registry.json'] = {'bytes': len(previous_bytes), 'sha256': gate.sha(previous_bytes)}
        self.assertEqual(gate.sha((json.dumps(previous_manifest, indent=2, sort_keys=True) + '\n').encode()),
                         '85c8908a2bf94e69e352ddb2399e1a3c0eb80319ab0a82f0ccdb3ed8c617e979')
        return previous_manifest, predecessor

    def test_portability_preserves_exact_77_registry_and_data_manifest(self):
        manifest, registry, _ = gate.admit_package(REPO)
        self.pre_portability_registry(manifest, registry)

    def test_final_closure_preserves_exact_predecessor_registry_and_oracle(self):
        manifest, registry, _ = gate.admit_package(REPO)
        manifest, registry = self.pre_portability_registry(manifest, registry)
        additions = [
            'frontend::oir::owned::execute::byte_storage_codec_tests::byte_storage_maximum_owned_and_borrowed_arguments_use_full_scratch',
            'frontend::oir::owned::execute::byte_storage_codec_tests::byte_storage_reference_frame_ceiling_is_exact_for_byte_slice_reborrows',
            'frontend::oir::owned::native::byte_storage_tests::byte_storage_native_independent_templates_and_allocation_topology',
            'frontend::oir::owned::source::byte_storage_authority_tests::byte_storage_array_conversion_authority_rejects_nine_forged_origins',
            'frontend::oir::owned::source::byte_storage_authority_tests::byte_storage_checked_facade_native_diagnostics_keep_own_source_map',
            'frontend::oir::owned::source::byte_storage_authority_tests::byte_storage_trusted_lowering_keeps_distinct_read_write_origins',
            'frontend::oir::owned::source::hir_budget::byte_storage_resources::tests::byte_storage_hir_work_is_independently_counted_before_allocation',
            'frontend::oir::owned::source::hir_budget::byte_storage_resources::tests::byte_storage_raw_count_fill_demand_and_allocation_sites_are_independent',
            'frontend::oir::source::association::u8_tests::byte_storage_inherited_auth_walk_counters_refuse_overflow_and_mismatch',
            'frontend::parser::array_syntax_tests::byte_storage_borrow_argument_and_literal_reservation_topology_is_bounded',
        ]
        public = [
            'byte_storage_public_record_reference_projection_stops_at_declaration',
            'byte_storage_public_unused_nested_enum_array_payload_stops_in_inner_parser',
        ]
        probes = [
            'native-storage-plan-checked-access',
            'native-storage-plan-private-execution',
            'checked-source-private-sources',
            'checked-source-private-body',
            'checked-source-native-own-map-api',
            'checked-source-native-rejects-alternate-map',
        ]
        self.assertEqual(len(additions), 10)
        self.assertTrue(set(additions) <= {r['name'] for r in registry['unit_tests']})
        self.assertTrue(set(public) <= set(registry['public_tests']))
        self.assertEqual(registry['privacy_probes'][-6:], probes)
        predecessor = copy.deepcopy(registry)
        predecessor['unit_tests'] = [r for r in predecessor['unit_tests'] if r['name'] not in additions]
        predecessor['public_tests'] = [name for name in predecessor['public_tests'] if name not in public]
        predecessor['privacy_probes'] = predecessor['privacy_probes'][:-6]
        self.assertEqual((len(predecessor['unit_tests']), len(predecessor['public_tests']), len(predecessor['privacy_probes'])), (67, 8, 5))
        previous_bytes = (json.dumps(predecessor, indent=2) + '\n').encode()
        self.assertEqual(gate.sha(previous_bytes), 'daa33d2d0ac773a4850bad54e65ca446870a0cebae38938370eb9586f4a681fe')
        previous_manifest = copy.deepcopy(manifest)
        previous_manifest['files']['registry.json'] = {'bytes': len(previous_bytes), 'sha256': gate.sha(previous_bytes)}
        self.assertEqual(gate.sha((json.dumps(previous_manifest, indent=2, sort_keys=True) + '\n').encode()),
                         'e88475f4139cab893361afde022ae9637413ba35a0883f2c569e14b2825ef4e9')

    def test_missing_extra_changed_source_and_self_rehash_refuse(self):
        for mode in ('missing', 'extra', 'changed', 'rehash', 'seal', 'cache', 'empty-cache', 'empty-directory', 'archive-extra'):
            with self.subTest(mode=mode):
                root = self.copy_package(); package = root / gate.PACKAGE
                path = package / 'oracle-v3/corpus/transport-000/main.ox'
                if mode == 'missing': path.unlink()
                elif mode == 'extra': (path.parent / 'extra.ox').write_text('')
                elif mode == 'seal': (package / 'oracle-v3/original-seal.json').write_text('{}')
                elif mode == 'cache': (package / 'oracle-v3/__pycache__').mkdir(); (package / 'oracle-v3/__pycache__/x.pyc').write_bytes(b'x')
                elif mode == 'empty-cache': (package / 'oracle-v3/corpus/__pycache__').mkdir()
                elif mode == 'empty-directory': (package / 'predecessor/empty').mkdir()
                elif mode == 'archive-extra': (package / 'predecessor/extra.rs').write_text('')
                else:
                    path.write_text('fn main()->i32{return 99;}')
                    if mode == 'rehash':
                        p = package / 'source-data-manifest.json'; m = json.loads(p.read_text())
                        m['files']['oracle-v3/corpus/transport-000/main.ox'] = {'bytes': path.stat().st_size, 'sha256': gate.sha(path.read_bytes())}
                        p.write_text(json.dumps(m))
                with self.assertRaises((ValueError, RuntimeError)):
                    gate.admit_package(root)

    def test_symlink_and_unregistered_root_refuse(self):
        root = self.copy_package(); package = root / gate.PACKAGE
        path = package / 'oracle-v3/corpus/transport-000/main.ox'
        body = path.read_bytes(); path.unlink(); other = root / 'other'; other.write_bytes(body); path.symlink_to(other)
        with self.assertRaisesRegex((RuntimeError, ValueError), 'symlink'):
            gate.admit_package(root)
        path.unlink(); path.write_bytes(body); (package / 'extra.json').write_text('{}')
        with self.assertRaisesRegex(ValueError, 'closed membership'):
            gate.admit_package(root)

    def test_registry_is_source_checked_not_inferred_from_executable_listing(self):
        _, registry, _ = gate.admit_package(REPO)
        observed = gate.source_tests(REPO)
        for changed in (observed[:-1], observed + [observed[0]], [(a, b, not c) for a, b, c in observed]):
            with patch.object(gate, 'source_tests', return_value=changed), self.assertRaisesRegex(ValueError, 'source-derived'):
                gate.admit_registry(REPO, registry)
        for key, value in (('native_transport', registry['native_transport'][:-1]), ('public_tests', registry['public_tests'][:-1]),
                           ('privacy_probes', registry['privacy_probes'][:-1]), ('native_tests', registry['native_tests'][:-1])):
            with self.subTest(key=key), self.assertRaises(ValueError):
                gate.admit_registry(REPO, dict(registry, **{key: value}))

    def test_bytecode_contamination_rejected_even_when_python_uses_B(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp); (root / 'scripts').mkdir(); (root / 'tests').mkdir()
            for name in ('scripts/a.pyc', 'tests/a.pyo', 'scripts/__pycache__'):
                path = root / name
                path.mkdir() if path.name == '__pycache__' else path.write_bytes(b'cache')
                with self.assertRaisesRegex(ValueError, 'bytecode contamination'):
                    gate.reject_bytecode(root)
                path.rmdir() if path.is_dir() else path.unlink()
            gate.reject_bytecode(root)

    @unittest.skipUnless(shutil.which('git'), 'Git required for cross-host extraction control')
    def test_autocrlf_checkout_preserves_complete_immutable_package(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp) / 'index'; root.mkdir(); checkout = Path(tmp) / 'checkout'
            shutil.copy2(REPO / '.gitattributes', root / '.gitattributes')
            shutil.copytree(REPO / gate.PACKAGE, root / gate.PACKAGE)
            (root / 'ordinary.txt').write_bytes(b'ordinary\ntext\n')
            def git(*args):
                subprocess.run(['git', '-c', 'core.autocrlf=true', '-c', 'core.eol=crlf', '-c', 'core.safecrlf=false', *args],
                               cwd=root, capture_output=True, check=True)
            git('init', '--quiet')
            git('add', '--', '.gitattributes', gate.PACKAGE.as_posix(), 'ordinary.txt')
            git('checkout-index', '--all', '--force', '--prefix', checkout.as_posix() + '/')
            self.assertEqual((checkout / 'ordinary.txt').read_bytes(), b'ordinary\r\ntext\r\n')
            gate.admit_package(checkout)
            for path in (root / gate.PACKAGE).rglob('*'):
                if path.is_file():
                    self.assertEqual(path.read_bytes(), (checkout / path.relative_to(root)).read_bytes())

    def test_explicit_325_data_registrations_preserve_188_predecessor_equation(self):
        old = verify_repo.predecessor_fixture_data_sources(REPO)
        current = gate.fixture_data_sources(REPO)
        self.assertEqual(len(old), 188); self.assertEqual(len(current), 325)
        self.assertFalse(old & current)
        self.assertEqual(verify_repo.fixture_data_sources(REPO), old | current)
        verify_repo.verify_test_fixture_registration(REPO)


class ExecutionAdmissionTests(unittest.TestCase):
    def test_non_linux_native_entries_refuse_before_importing_native_helpers(self):
        script = textwrap.dedent('''
            import importlib.abc
            from pathlib import Path
            import sys
            from unittest.mock import patch

            class NoNativeHelpers(importlib.abc.MetaPathFinder):
                def find_spec(self, fullname, path=None, target=None):
                    if fullname in ('resource', 'verify_bounded_stdin_native', 'verify_bounded_u8_native'):
                        raise ModuleNotFoundError('native helper imported before host admission: ' + fullname)

            sys.modules.pop('resource', None)
            sys.meta_path.insert(0, NoNativeHelpers())
            sys.path.insert(0, str(Path(sys.argv[1]) / 'scripts'))
            import verify_bounded_byte_storage as gate
            refused = 0
            for system, machine in (('Windows', 'AMD64'), ('Darwin', 'arm64'), ('Linux', 'aarch64')):
                with patch.object(gate.platform, 'system', return_value=system), \\
                        patch.object(gate.platform, 'machine', return_value=machine):
                    for operation in (gate.verify, gate.prepare_public_build):
                        try:
                            operation(None, None)
                        except ValueError as error:
                            gate.require(str(error) == 'Linux x86_64 required', 'unexpected host refusal')
                            refused += 1
                        else:
                            raise ValueError('native entry admitted an unsupported host')
            gate.require(refused == 6, 'native host refusal count differs')
            print(refused)
        ''')
        result = subprocess.run([sys.executable, '-I', '-B', *(['-O'] if sys.flags.optimize else []),
                                 '-c', script, str(REPO)], capture_output=True, text=True, timeout=60)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(result.stdout.strip(), '6')

    def test_missing_duplicate_or_extra_test_discovery_and_skips_refuse(self):
        expected = ['alpha', 'beta']; good = b'alpha: test\nbeta: test\n\n2 tests, 0 benchmarks\n'
        u8_gate.admit_listing(good, expected)
        for bad in (good.replace(b'alpha', b'beta'), good.replace(b'alpha: test\n', b''), good.replace(b'2 tests', b'3 tests')):
            with self.assertRaises(ValueError): u8_gate.admit_listing(bad, expected)
        good = b'test result: ok. 75 passed; 0 failed; 0 ignored; 0 measured; 123 filtered out; finished in 0.12s\n'
        u8_gate.admit_execution(good, 75)
        for bad in (good.replace(b'75 passed', b'0 passed'), good.replace(b'0 ignored', b'4 ignored'), good + good):
            with self.assertRaises(ValueError): u8_gate.admit_execution(bad, 75)

    def test_full_diagnostic_contract_and_runtime_check_run_separation(self):
        cases = gate.admit_package(REPO)[2]
        case = next(c for c in cases if c['expected'].get('code') == 'E0606'); expected = case['expected']
        diagnostic = {k: expected[k] for k in ('code', 'stage', 'message', 'primary', 'secondary')}
        diagnostic.update(kind='diagnostic', notes=[])
        good = json.dumps(diagnostic).encode()
        gate.admit_observation(good, b'', expected, 'run', True)
        gate.admit_observation(b'{"kind":"check-summary","success":true}', b'', expected, 'check', False)
        for key, value in (('code', 'E0601'), ('message', 'different'), ('primary', {}), ('secondary', [{}]), ('notes', ['extra'])):
            with self.assertRaises(ValueError):
                gate.admit_observation(json.dumps(dict(diagnostic, **{key: value})).encode(), b'', expected, 'run', True)
        with self.assertRaises(ValueError): gate.admit_observation(good, b'unexpected', expected, 'run', True)

    def test_value_success_requires_real_result_and_summary(self):
        expected = {'value': 255}
        gate.admit_observation(b'{"result":{"type":"i32","value":255}}', b'', expected, 'run', False)
        for data in (b'', b'{"result":{"type":"i32","value":254}}', b'{"result":{"type":"u8","value":255}}'):
            with self.assertRaises(ValueError): gate.admit_observation(data, b'', expected, 'run', False)
        with self.assertRaises(ValueError): gate.admit_observation(b'{"success":true}', b'', expected, 'compile', False)

    def test_compile_success_rejects_extra_diagnostics_duplicate_fields_and_bad_JSON(self):
        good = b'{"kind":"compile-summary","success":true}'
        gate.admit_observation(good, b'', {}, 'compile', False)
        for data in (good + b'\n{"kind":"diagnostic","code":"E0700"}', good + b'\n{}',
                     b'{"kind":"compile-summary","success":false,"success":true}', b'{malformed'):
            with self.assertRaises((ValueError, RuntimeError)):
                gate.admit_observation(data, b'', {}, 'compile', False)

    def test_toolchain_consumer_rejects_PATH_override_and_stale_actual_version(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            for name in ('cargo', 'rustc'):
                (root / name).write_bytes(('tool ' + name).encode())
                (root / ('tool-' + name + '.stdout')).write_bytes(b'release: 1.99.0\n')
            tools = {name: gate.identity(root / name) for name in ('cargo', 'rustc')}
            def which(name, **kwargs): return str(root / name)
            with patch.object(gate.shutil, 'which', side_effect=which), patch.object(gate, 'invoke', return_value=(b'release: 1.99.0\n', b'')) as run:
                result = gate.admit_toolchain(root, root, {}, tools, root)
                self.assertEqual(result['cargo'], str(root / 'cargo'))
                self.assertEqual(run.call_count, 2)
                for override in ('RUSTC', 'RUSTUP_TOOLCHAIN', 'CARGO_BUILD_RUSTC', 'RUSTC_WRAPPER'):
                    with self.assertRaisesRegex(ValueError, 'override'):
                        gate.admit_toolchain(root, root, {override: 'other'}, tools, root)
            # A rustup/other dispatch shim can have the same basename as the
            # tool without being the actual binary pinned by enum build receipts.
            shims = root / 'shims'; shims.mkdir()
            (shims / 'cargo').write_bytes(b'dispatch shim')
            with patch.object(gate.shutil, 'which', return_value=str(shims / 'cargo')), patch.object(gate, 'invoke') as execute, self.assertRaisesRegex(ValueError, 'resolved toolchain'):
                gate.admit_toolchain(root, root, {}, tools, root)
            execute.assert_not_called()
            with patch.object(gate.shutil, 'which', return_value=None), self.assertRaisesRegex(ValueError, 'resolved toolchain'):
                gate.admit_toolchain(root, root, {}, tools, root)
            with patch.object(gate.shutil, 'which', side_effect=which), patch.object(gate, 'invoke', return_value=(b'release: 1.98.0\n', b'')), self.assertRaisesRegex(ValueError, 'current tool version'):
                gate.admit_toolchain(root, root, {}, tools, root)

    def test_controller_and_builder_cannot_skip_head_source_or_toolchain_admission(self):
        for operation in (gate.verify, gate.prepare_public_build):
            for mode in ('head', 'source', 'toolchain'):
                with self.subTest(operation=operation.__name__, mode=mode), tempfile.TemporaryDirectory() as tmp:
                    root = Path(tmp); repo = root / 'repo'; repo.mkdir(); evidence = root / 'build'; evidence.mkdir()
                    (evidence / 'tool-rustc.stdout').write_text('release: 1.99.0\n')
                    output = root / 'output'; output.mkdir()
                    args = SimpleNamespace(repo=repo, llvm_bin=root, build_evidence=evidence, expected_head='expected',
                                           cargo='cargo', event_sha='event', public_build_evidence=evidence)
                    receipt = dict(head='expected', tree='tree', reviewed_source_manifest={'sha256': gate.SOURCE_SHA256})
                    if mode == 'source': receipt['reviewed_source_manifest']['sha256'] = 'stale'
                    def git(directory, label, *args, **kwargs):
                        return (((('stale' if mode == 'head' else 'expected') + '\ntree\n').encode() if label == 'git-head' else b''), b'')
                    def read(path): return {} if path.name == 'tools.json' else receipt
                    with patch.dict(gate.os.environ, {'CARGO_TARGET_DIR': str(repo / 'target')}, clear=True), \
                            patch.object(gate.enum_gate, 'git_command', side_effect=git), \
                            patch.object(builds, 'source_manifest', return_value={}), \
                            patch.object(builds, 'source_identity', return_value=[]), \
                            patch.object(gate, 'read_json', side_effect=read), \
                            patch.object(gate, 'admit_package', return_value=({}, {}, [])), \
                            patch.object(gate, 'admit_registry'), \
                            patch.object(gate, 'controller_inputs', return_value=[]), \
                            patch.object(gate, 'identity', return_value={'sha256': gate.STAGER_SHA256}), \
                            patch.object(gate, 'admit_toolchain', side_effect=ValueError('toolchain sentinel')) as toolchain, \
                            patch.object(gate, 'invoke') as execute:
                        with self.assertRaisesRegex(ValueError, 'toolchain sentinel' if mode == 'toolchain' else 'head' if mode == 'head' else 'source'):
                            operation(args, output)
                        self.assertEqual(toolchain.call_count, 1 if mode == 'toolchain' else 0)
                        execute.assert_not_called()

    def test_native_evidence_requires_every_source_IR_ELF_stream_and_status(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = Path(tmp)
            files = {'.elf': b'\x7fELFtest', '.ll': b'; module', '.ox': b'original', '.stdout': b'', '.stderr': b'',
                     '.status': b'1\n', '.source-sha256': gate.sha(b'original').encode() + b'\n', '.run.txt': b'source-free'}
            for suffix, data in files.items(): (root / ('case' + suffix)).write_bytes(data)
            self.assertEqual(len(gate.native_artifacts(root, ['case'])), 8)
            for suffix in files:
                path = root / ('case' + suffix); data = path.read_bytes(); path.unlink()
                with self.assertRaises(ValueError): gate.native_artifacts(root, ['case'])
                path.write_bytes(data)
            (root / 'extra.elf').write_bytes(b'\x7fELF')
            with self.assertRaises(ValueError): gate.native_artifacts(root, ['case'])
            (root / 'extra.elf').unlink(); (root / 'case.ox').write_bytes(b'changed')
            with self.assertRaisesRegex(ValueError, 'source identity'): gate.native_artifacts(root, ['case'])

    def test_provider_exact_216_roster_and_source_tool_boundaries(self):
        parities, refusals, captures = gate.provider.expected_results()
        summary = dict(schema='rfc0031-byte-storage-provider-boundary-v1', status='passed', compiler_sha256='a', source_head='h', source_tree='t',
            recipe=gate.provider.recipe(), unchanged_invalid_parities=parities, closed_boundary_refusals=refusals,
            pre_provider_parse_refusals=[r for r in refusals if '/byte-enum-payload/' in r],
            protocol_or_import_refusals=[r for r in refusals if '/byte-enum-payload/' not in r],
            captures=[dict(version=v, name=n, source_sha256=s, kind=k) for v, n, s, k in captures],
            frozen_protocols=['OPA1/AST1/STF1', 'OPA2/AST2/STF2'], caps=[128, 255], frame_bytes=[1559, 2607, 1575], invalid_source_llvm_invocations=0)
        gate.validate_provider(summary, {'sha256': 'a'}, 'h', 't')
        for key, value in (('compiler_sha256', 'wrong'), ('source_head', 'old'), ('source_tree', 'old'),
                           ('closed_boundary_refusals', refusals[:-1]), ('unchanged_invalid_parities', []), ('captures', []),
                           ('caps', [256, 256]), ('invalid_source_llvm_invocations', 1)):
            with self.subTest(key=key), self.assertRaises(ValueError):
                gate.validate_provider(dict(summary, **{key: value}), {'sha256': 'a'}, 'h', 't')

    def test_failed_and_timeout_commands_preserve_original_outputs(self):
        failures = [subprocess.CompletedProcess(['tool'], 7, b'out', b'err'),
                    subprocess.TimeoutExpired(['tool'], 1, output=b'partial', stderr=b'error')]
        for result in failures:
            with tempfile.TemporaryDirectory() as tmp:
                root = Path(tmp); options = {'side_effect': result} if isinstance(result, Exception) else {'return_value': result}
                with patch.object(gate.common.subprocess, 'run', **options), self.assertRaises(ValueError):
                    gate.invoke(root, 'failed', ['tool'], cwd=root, env={})
                receipt = gate.read_json(root / 'failed.json')
                self.assertNotEqual(receipt['status'], 0)
                self.assertEqual(receipt['stdout_sha256'], gate.sha((root / 'failed.stdout').read_bytes()))
                self.assertEqual(receipt['stderr_sha256'], gate.sha((root / 'failed.stderr').read_bytes()))


class PublicBuildTests(unittest.TestCase):
    def fixture(self):
        tmp = tempfile.TemporaryDirectory(); self.addCleanup(tmp.cleanup)
        root = Path(tmp.name); repo = root / 'repo'; target = repo / 'target'; source = repo / gate.PUBLIC_SOURCE
        source.parent.mkdir(parents=True); source.write_text('test source')
        deps = target / 'debug/deps'; deps.mkdir(parents=True)
        binary = deps / 'typed_byte_storage_native-0123456789abcdef'; binary.write_bytes(b'\x7fELFtest')
        cli = target / 'debug/oxid'; cli.write_bytes(b'\x7fELFcli')
        evidence = root / 'build'; directory = evidence / 'debug'; directory.mkdir(parents=True)
        destination = root / 'retained'; destination.mkdir()
        receipt = {'head': 'h', 'tree': 't'}; gate.save(evidence / 'source-identity.json', receipt)
        profile = dict(test=True, opt_level='0', debug_assertions=True, overflow_checks=True, debuginfo=2)
        rows = [dict(reason='compiler-artifact', target=dict(name='typed_byte_storage_native', kind=['test'], src_path=str(source)), profile=profile, executable=str(binary)),
                dict(reason='compiler-artifact', target=dict(name='oxid', kind=['bin'], src_path=str(repo / 'src/cli.rs')), profile=dict(profile, test=False), executable=str(cli)),
                dict(reason='build-finished', success=True)]
        data = b'\n'.join(json.dumps(r).encode() for r in rows) + b'\n'
        (directory / 'public-build.stdout').write_bytes(data); (directory / 'public-build.stderr').write_bytes(b'')
        gate.save(directory / 'public-build.json', dict(argv=['cargo', 'test', '--locked', '--test', 'typed_byte_storage_native', '--no-run', '--message-format=json'],
            status=0, cwd=str(repo), stdout_sha256=gate.sha(data), stderr_sha256=gate.sha(b'')))
        gate.save(directory / 'public-binary.json', dict(public=gate.identity(binary), cli=gate.identity(cli), source=gate.identity(source), profile='debug', head='h', tree='t'))
        return evidence, destination, repo, target, receipt

    def test_exact_public_Cargo_receipt_admits_once(self):
        evidence, destination, repo, target, receipt = self.fixture()
        binary = gate.admit_public_build(evidence, destination, repo, target, 'debug', receipt)
        self.assertEqual(binary.name, 'typed_byte_storage_native-0123456789abcdef')
        self.assertTrue((destination / 'public-binary.json').is_file())

    def test_stale_cli_test_source_binary_profile_and_head_refuse(self):
        for mode in ('cli', 'source', 'binary', 'head', 'flags', 'command', 'stream'):
            with self.subTest(mode=mode):
                evidence, destination, repo, target, receipt = self.fixture(); directory = evidence / 'debug'
                if mode in ('cli', 'source', 'binary'):
                    path = target / 'debug/oxid' if mode == 'cli' else repo / gate.PUBLIC_SOURCE if mode == 'source' else target / 'debug/deps/typed_byte_storage_native-0123456789abcdef'
                    path.write_bytes(path.read_bytes() + b'changed')
                elif mode == 'head': receipt = dict(receipt, head='other')
                elif mode == 'stream': (directory / 'public-build.stderr').write_bytes(b'changed')
                elif mode == 'command':
                    p = directory / 'public-build.json'; obj = gate.read_json(p); obj['argv'].append('--release'); gate.save(p, obj)
                else:
                    p = directory / 'public-build.stdout'; rows = [json.loads(line) for line in p.read_bytes().splitlines()]; rows[0]['profile']['opt_level'] = '1'
                    p.write_bytes(b'\n'.join(json.dumps(r).encode() for r in rows))
                with self.assertRaises(ValueError): gate.admit_public_build(evidence, destination, repo, target, 'debug', receipt)


if __name__ == '__main__':
    unittest.main()
