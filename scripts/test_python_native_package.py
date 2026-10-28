"""Pure packaging refusal controls; no compiler or native library is executed.

Minimal ELF-shaped bytes below are denial fixtures only. These unit controls
never treat them as a qualified native bundle or an end-to-end wheel result.
"""
import base64
from contextlib import contextmanager
import csv
import gzip
import hashlib
import importlib.util
import io
import json
import os
from pathlib import Path
import shutil
import struct
import tempfile
import unittest
from unittest.mock import patch
import zipfile


ROOT = Path(__file__).resolve().parents[1]
APP = ROOT / 'apps/python-pricing'


def load_module(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


backend = load_module('_oxid_python_package_contract', APP / 'build_backend/oxid_backend.py')
verifier = load_module('_oxid_python_package_verifier_contract', ROOT / 'scripts/verify_python_native_package.py')


@contextmanager
def cwd(path):
    previous = Path.cwd()
    os.chdir(path)
    try:
        yield
    finally:
        os.chdir(previous)


class PythonNativePackageContracts(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix='oxid-python-package-controls-')
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.project = {
            'name': 'control-pricing', 'version': '1.2.3',
            'description': 'Pure packaging controls', 'requires-python': '>=3.11',
        }
        self.tool = {'module': 'control_pricing', 'source': 'src/pricing.ox',
                     'sources': ['src/pricing.ox'], 'export': 'crate::line_total',
                     'profile': 'release', 'fuel': 1000000}
        for name in ('build_backend/oxid_backend.py', 'build_backend/shim.c.in'):
            destination = self.root / name
            destination.parent.mkdir(parents=True, exist_ok=True)
            destination.write_bytes((APP / name).read_bytes())
        (self.root / 'src').mkdir()
        (self.root / 'src/pricing.ox').write_text('pub fn line_total(a:i32,b:i32,c:i32)->i32{return a*b/c;}\n')
        self.write_config()

    def write_config(self, project=None, tool=None, extra=''):
        parts = ['[build-system]', 'requires = []', 'build-backend = "oxid_backend"',
                 'backend-path = ["build_backend"]', '', '[project]']
        for key, value in (self.project if project is None else project).items():
            parts.append(key + ' = ' + json.dumps(value))
        parts.extend(('', '[tool.oxid]'))
        for key, value in (self.tool if tool is None else tool).items():
            parts.append(key + ' = ' + json.dumps(value))
        (self.root / 'pyproject.toml').write_text('\n'.join(parts) + '\n' + extra)

    def deny_project(self):
        with patch.object(backend, 'tool_identity', side_effect=AssertionError('must not select tools')), \
                patch.object(backend.subprocess, 'Popen', side_effect=AssertionError('must not launch tools')):
            with self.assertRaises(backend.BuildError):
                backend.load_project(self.root)

    def denial_bundle(self):
        """Supply a tuple for one deliberately invalid field, never admission."""
        folder = self.root / 'denial-bundle'
        if folder.exists():
            shutil.rmtree(folder)
        folder.mkdir()
        machine = 62 if backend.platform.machine() == 'x86_64' else 183
        library = bytearray(64)
        library[:7] = b'\x7fELF\x02\x01\x01'
        struct.pack_into('<HHI', library, 16, 3, machine, 1)
        header = backend.native_header(3)
        config = backend.load_project(self.root)
        compiler = {'path': '/unexecuted/oxid', 'resolved': '/unexecuted/oxid', 'sha256': 'c' * 64}
        sources = {'pricing.ox': hashlib.sha256((self.root / 'src/pricing.ox').read_bytes()).hexdigest()}
        data = {'schema': 'oxid-scalar-native-library-v1', 'abi_version': 1,
                'export': self.tool['export'], 'symbol': 'oxid_library_call_v1',
                'parameters': ['i32'] * 3, 'result': 'i32',
                'statuses': {'success': 0, 'overflow': 1, 'division_by_zero': 2, 'fuel': 3, 'invalid_output': 4},
                'target': backend.host_target(), 'profile': 'release', 'execution_policy': 'bounded',
                'fuel': 1000000, 'library_sha256': hashlib.sha256(library).hexdigest(),
                'header_sha256': hashlib.sha256(header).hexdigest(), 'compiler_package_version': '0.9.0',
                'compiler_sha256': compiler['sha256'], 'ir_sha256': 'd' * 64,
                'sources': [{'path': name, 'sha256': digest} for name, digest in sources.items()]}
        (folder / 'liboxid_library.so').write_bytes(library)
        (folder / 'oxid_library.h').write_bytes(header)
        (folder / 'metadata.json').write_text(json.dumps(data))
        return folder, config, compiler, sources, data

    def test_static_project_keeps_an_explicit_source_inventory(self):
        config = backend.load_project(self.root)
        self.assertEqual(config['call_name'], 'line_total')
        self.assertEqual(config['sources'], ['src/pricing.ox'])
        self.assertEqual(set(config['payloads']), {*backend.BACKEND_FILES, 'src/pricing.ox'})
        self.assertEqual(config['payloads']['src/pricing.ox'], (self.root / 'src/pricing.ox').read_bytes())

    def test_unknown_and_malicious_project_metadata_is_refused_before_tools(self):
        for key, value in (
            ('name', '../escape'), ('name', 'Mixed_Case'), ('version', '1.0/escape'),
            ('version', '1.0+unsupported'), ('version', '01.2'),
            ('description', 'one\nRequires-Dist: injected'), ('description', 'x' * 513),
            ('requires-python', '>=3.10'), ('dynamic', ['version']),
        ):
            with self.subTest(key=key, value=value):
                self.write_config(project={**self.project, key: value})
                self.deny_project()
        self.write_config(extra='\n[unreviewed]\nvalue = "outside closed configuration"\n')
        self.deny_project()

    def test_tool_options_cannot_supply_paths_flags_tags_or_boolean_fuel(self):
        for key, value in (
            ('module', '../escape'), ('module', '_private'), ('module', 'a.b'),
            ('module', 'for'), ('module', 'class'),
            ('source', '/outside.ox'), ('source', 'src/../outside.ox'),
            ('source', 'src\\pricing.ox'), ('export', 'crate::x;whoami'),
            ('export', 'other::line_total'), ('profile', 'custom'),
            ('export', 'crate::class'), ('export', 'crate::None'),
            ('fuel', True), ('fuel', -1), ('fuel', 1000001),
            ('cflags', ['-Wl,unreviewed']), ('wheel-tag', 'cp310-abi3-manylinux_2_17_x86_64'),
        ):
            with self.subTest(key=key, value=value):
                self.write_config(tool={**self.tool, key: value})
                self.deny_project()

    def test_relative_paths_reject_control_characters_before_file_lookup(self):
        for name in ('src/a\n.ox', 'src/a\r.ox', 'src/a\t.ox', 'src/a\0.ox', 'src/a\x7f.ox'):
            with self.subTest(path=name), self.assertRaises(backend.BuildError):
                backend.relative(name)

    def test_manifest_refuses_omissions_case_aliases_and_undeclared_neighbors(self):
        for sources in ([], ['src/other.ox'], ['src/pricing.ox'] * 2,
                        ['src/pricing.ox', 'src/PRICING.ox'],
                        [f'src/f{i}.ox' for i in range(254)]):
            with self.subTest(sources=sources):
                self.write_config(tool={**self.tool, 'sources': sources})
                self.deny_project()
        self.write_config()
        (self.root / 'src/unlisted.ox').write_text('pub fn unlisted()->i32{return 0;}')
        self.deny_project()

    def test_source_links_and_hardlinks_are_refused(self):
        source = self.root / 'src/pricing.ox'
        body = source.read_bytes()
        outside = self.root / 'outside.ox'
        outside.write_bytes(body)
        source.unlink()
        source.symlink_to(outside)
        self.deny_project()
        source.unlink()
        os.link(outside, source)
        self.deny_project()

    def test_per_file_and_aggregate_source_limits_are_real_refusals(self):
        (self.root / 'src/pricing.ox').write_bytes(b'x' * (1024 * 1024 + 1))
        self.deny_project()
        sources = [f'src/f{i}.ox' for i in range(9)]
        (self.root / 'src/pricing.ox').unlink()
        for name in sources:
            (self.root / name).write_bytes(b'x' * (1024 * 1024))
        self.write_config(tool={**self.tool, 'source': sources[0], 'sources': sources})
        self.deny_project()

    def test_metadata_and_requirement_hooks_need_no_tools_and_preserve_old_outputs(self):
        output = self.root / 'metadata'
        output.mkdir()
        with cwd(self.root), \
                patch.object(backend, 'tool_identity', side_effect=AssertionError('metadata selected a tool')), \
                patch.object(backend, 'run_tool', side_effect=AssertionError('metadata launched a tool')), \
                patch.object(backend.subprocess, 'Popen', side_effect=AssertionError('metadata launched a process')):
            self.assertEqual(backend.get_requires_for_build_wheel(), [])
            self.assertEqual(backend.get_requires_for_build_sdist(), [])
            name = backend.prepare_metadata_for_build_wheel(output)
            prepared = output / name
            before = {path.name: path.read_bytes() for path in prepared.iterdir()}
            self.assertEqual(before, backend.metadata_files(backend.load_project()))
            self.assertIn(b'Requires-Python: >=3.11\n', before['METADATA'])
            self.assertNotIn(b'abi3', before['WHEEL'])
            self.assertNotIn(b'manylinux', before['WHEEL'])
            with self.assertRaises((OSError, backend.BuildError)):
                backend.prepare_metadata_for_build_wheel(output)
            self.assertEqual({path.name: path.read_bytes() for path in prepared.iterdir()}, before)

    def test_config_settings_and_existing_archives_fail_before_tool_discovery(self):
        output = self.root / 'archives'
        output.mkdir()
        config = backend.load_project(self.root)
        wheel = output / (backend.distribution(config) + '-' + backend.wheel_tag() + '.whl')
        sdist = output / (backend.distribution(config) + '.tar.gz')
        wheel.write_bytes(b'previous wheel')
        sdist.write_bytes(b'previous sdist')
        with cwd(self.root), patch.object(backend, 'tool_identity', side_effect=AssertionError('selected a tool')):
            for hook in (backend.get_requires_for_build_wheel, backend.get_requires_for_build_sdist):
                with self.assertRaises(backend.BuildError):
                    hook({'unsupported': 'setting'})
            for hook in (backend.build_wheel, backend.build_sdist):
                with self.assertRaises(backend.BuildError):
                    hook(output)
        self.assertEqual(wheel.read_bytes(), b'previous wheel')
        self.assertEqual(sdist.read_bytes(), b'previous sdist')

    def test_native_header_has_closed_zero_and_sixty_four_argument_boundaries(self):
        self.assertIn(b'oxid_library_call_v1(int32_t *out);', backend.native_header(0))
        header = backend.native_header(64)
        self.assertIn(b'int32_t arg0, ', header)
        self.assertIn(b'int32_t arg63, int32_t *out);', header)
        self.assertNotIn(b'arg64', header)
        for arity in (-1, 65, True, 1.0):
            with self.subTest(arity=arity), self.assertRaises(backend.BuildError):
                backend.native_header(arity)

    def test_build_environment_injection_is_removed_before_any_process_can_start(self):
        blocked = ('CC', 'CFLAGS', 'CPATH', 'COMPILER_PATH', 'GCC_EXEC_PREFIX',
                   'CCC_OVERRIDE_OPTIONS', 'CLANG_CONFIG_FILE', 'CLANG_CONFIG_FILE_USER_DIR',
                   'SDKROOT', 'LD_PRELOAD', 'LD_LIBRARY_PATH', 'LD_AUDIT', 'LD_RUN_PATH')
        observed = []
        class NoProcessStarted(Exception):
            pass
        def intercept(arguments, **options):
            observed.append(options['env'])
            raise NoProcessStarted()
        with patch.dict(os.environ, {name: 'untrusted-injection' for name in blocked}), \
                patch.object(backend.subprocess, 'Popen', side_effect=intercept):
            with self.assertRaises(NoProcessStarted):
                backend.run_tool(['/unexecuted/compiler'], self.root, 1, [])
        self.assertEqual(len(observed), 1)
        self.assertFalse(set(blocked) & set(observed[0]))

    def test_native_tuple_refuses_every_identity_abi_policy_and_source_mismatch(self):
        mutations = (
            ('schema', 'unrecognized'), ('abi_version', True), ('export', 'crate::other'),
            ('symbol', 'other_symbol'), ('parameters', ['i64']), ('parameters', ['i32'] * 65),
            ('parameters', None), ('unreviewed', True),
            ('result', 'i64'), ('statuses', {'success': 0}), ('target', 'wrong-target'),
            ('statuses', {'success': False, 'overflow': 1, 'division_by_zero': 2, 'fuel': 3, 'invalid_output': 4}),
            ('statuses', {'success': 0, 'overflow': True, 'division_by_zero': 2, 'fuel': 3, 'invalid_output': 4}),
            ('profile', 'dev'), ('execution_policy', 'hosted'), ('fuel', True),
            ('library_sha256', '0' * 64), ('header_sha256', '0' * 64),
            ('compiler_sha256', '0' * 64), ('compiler_package_version', 'bad-version'),
            ('ir_sha256', 'not-a-digest'), ('sources', []),
            ('sources', [{'path': '../outside.ox', 'sha256': 'd' * 64}]),
            ('sources', [{'path': 'other.ox', 'sha256': 'd' * 64}]),
        )
        for key, value in mutations:
            folder, config, compiler, sources, data = self.denial_bundle()
            data[key] = value
            (folder / 'metadata.json').write_text(json.dumps(data))
            with self.subTest(field=key, value=value), self.assertRaises(backend.BuildError):
                backend.validate_bundle(folder, config, compiler, sources)

    def test_native_tuple_refuses_extra_links_duplicate_keys_and_metadata_limits(self):
        for mutation in ('extra', 'link', 'duplicate', 'metadata-size', 'header', 'elf'):
            folder, config, compiler, sources, data = self.denial_bundle()
            if mutation == 'extra':
                (folder / 'unexpected').write_text('unlisted output')
            elif mutation == 'link':
                library = folder / 'liboxid_library.so'
                outside = self.root / 'linked-payload'
                library.rename(outside)
                library.symlink_to(outside)
            elif mutation == 'duplicate':
                (folder / 'metadata.json').write_text('{"schema":"duplicated",' + json.dumps(data)[1:])
            elif mutation == 'metadata-size':
                (folder / 'metadata.json').write_bytes(b' ' * (1024 * 1024 + 1))
            elif mutation == 'header':
                header = b'void arbitrary_native_abi(void);\n'
                (folder / 'oxid_library.h').write_bytes(header)
                data['header_sha256'] = hashlib.sha256(header).hexdigest()
                (folder / 'metadata.json').write_text(json.dumps(data))
            else:
                library = b'not an ELF shared library'
                (folder / 'liboxid_library.so').write_bytes(library)
                data['library_sha256'] = hashlib.sha256(library).hexdigest()
                (folder / 'metadata.json').write_text(json.dumps(data))
            with self.subTest(mutation=mutation), self.assertRaises(backend.BuildError):
                backend.validate_bundle(folder, config, compiler, sources)

    def test_record_contains_exact_member_census_sizes_and_independent_digests(self):
        files = {'package/__init__.py': b'package bytes', 'package/data': b'\x00\xff\n'}
        name = 'package-1.2.3.dist-info/RECORD'
        rows = list(csv.reader(io.StringIO(backend.record_bytes(files, name).decode())))
        self.assertEqual({row[0] for row in rows}, {*files, name})
        self.assertEqual(len(rows), 3)
        self.assertEqual(rows[-1], [name, '', ''])
        for path, digest, size in rows[:-1]:
            expected = base64.urlsafe_b64encode(hashlib.sha256(files[path]).digest()).rstrip(b'=').decode()
            self.assertEqual((digest, size), ('sha256=' + expected, str(len(files[path]))))

    def test_verifier_refuses_unsafe_duplicate_case_aliased_and_linked_wheels(self):
        cases = (('traversal', ('safe', '../escape'), False),
                 ('control', ('safe', 'bad\nname'), False),
                 ('duplicate', ('same', 'same'), False),
                 ('case', ('same', 'SAME'), False),
                 ('link', ('linked',), True),
                 ('census', tuple('entry' + str(i) for i in range(33)), False))
        for label, names, link in cases:
            path = self.root / (label + '-cp311-cp311-linux_x86_64.whl')
            with zipfile.ZipFile(path, 'w') as archive:
                for name in names:
                    info = zipfile.ZipInfo(name)
                    info.external_attr = ((0o120777 if link else 0o100644) << 16)
                    archive.writestr(info, b'x')
            with self.subTest(case=label), patch.object(
                    zipfile.ZipFile, 'read', side_effect=AssertionError('payload read before complete preflight')), \
                    self.assertRaises(ValueError):
                verifier.inspect_wheel(path, {'module': 'control', 'name': 'control', 'version': '1.2.3'},
                                       'cp311-cp311-linux_x86_64')

    def test_sdist_expansion_is_bounded_before_the_pax_parser_can_read_metadata(self):
        path = self.root / 'control-1.2.3.tar.gz'
        limit = 9 * 1024 * 1024
        with gzip.open(path, 'wb') as stream:
            stream.write(b'x' * (limit + 1))
        with patch.object(verifier.tarfile, 'open', side_effect=AssertionError('PAX parser ran before its input bound')):
            with self.assertRaisesRegex(ValueError, 'decompressed artifact bound'):
                verifier.inspect_sdist(path, {})
        path.write_bytes(b'x' * (limit + 1))
        with patch.object(verifier.gzip, 'GzipFile', side_effect=AssertionError('gzip ran before compressed input bound')):
            with self.assertRaisesRegex(ValueError, 'compressed artifact bound'):
                verifier.inspect_sdist(path, {})

    def test_zip_census_is_checked_before_the_zip_parser_allocates_entries(self):
        for count in (2, 33):
            path = self.root / f'raw-{count}-cp311-cp311-linux_x86_64.whl'
            with zipfile.ZipFile(path, 'w') as archive:
                for index in range(count):
                    archive.writestr(f'entry{index}', b'x')
            if count == 2:
                # Lie in EOCD: the real directory still has two records.
                body = bytearray(path.read_bytes())
                struct.pack_into('<HH', body, len(body) - 22 + 8, 1, 1)
                path.write_bytes(body)
            with self.subTest(count=count), patch.object(
                    verifier.zipfile, 'ZipFile', side_effect=AssertionError('ZIP parser allocated before its census')):
                with self.assertRaises(ValueError):
                    verifier.inspect_wheel(path, {'module': 'control', 'name': 'control', 'version': '1.2.3'},
                                           'cp311-cp311-linux_x86_64')


if __name__ == '__main__':
    unittest.main()
