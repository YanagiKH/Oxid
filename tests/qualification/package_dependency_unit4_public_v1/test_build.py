"""Pure structural/mutation tests. Synthetic claims never become build evidence."""
import ast
import copy
import hashlib
import os
import tempfile
import types
import unittest
from pathlib import Path

path = Path(__file__).absolute().with_name('build.py')
build = types.ModuleType('public_build_under_test')
build.__file__ = str(path)
exec(compile(path.read_bytes(), str(path), 'exec'), build.__dict__)


def role_files():
    path = Path(__file__).absolute().parent.parent / 'package_dependency_unit4_source_v1/public-source-successor-authority.json'
    raw = path.read_bytes()
    build.need(hashlib.sha256(raw).hexdigest() == '392071a60bccbf00882a9419ca1ab7ec5197455d803189a9638abcf792650ac3',
               'fixed installed source authority')
    return build.binding.decode(raw)['maps']['new_ordinary376']


def metadata_claim(root, target, files):
    package_id = 'path+' + root.as_uri() + '#oxid@0.9.0'
    targets = [
        {'kind': ['bin'], 'crate_types': ['bin'], 'name': 'oxid', 'src_path': str(root / 'src/cli.rs'),
         'edition': '2021', 'doc': True, 'doctest': False, 'test': True},
        {'kind': ['custom-build'], 'crate_types': ['bin'], 'name': 'build-script-build',
         'src_path': str(root / 'build.rs'), 'edition': '2021', 'doc': False, 'doctest': False, 'test': False},
    ]
    for row in files:
        p = Path(row['path'])
        if len(p.parts) == 2 and p.parts[0] == 'tests' and p.suffix == '.rs':
            targets.append({'kind': ['test'], 'crate_types': ['bin'], 'name': p.stem,
                            'src_path': str(root / p), 'edition': '2021',
                            'doc': False, 'doctest': False, 'test': True})
    return {'version': 1, 'resolve': None,
            'packages': [{'id': package_id, 'name': 'oxid', 'version': '0.9.0', 'edition': '2021',
                          'manifest_path': str(root / 'Cargo.toml'), 'targets': targets}],
            'workspace_members': [package_id], 'workspace_default_members': [package_id],
            'workspace_root': str(root), 'target_directory': str(target)}


class BuildControls(unittest.TestCase):
    def test_exact_recipe_and_limits(self):
        debug = ['cargo', 'build', '--bin', 'oxid', '--locked', '--offline']
        self.assertEqual(build.recipe('debug'), debug)
        self.assertEqual(build.recipe('release'), debug + ['--release'])
        self.assertEqual(build.BUILD_TIMEOUT_SECONDS, 1800)
        self.assertEqual(build.MAX_STREAM_BYTES, 8 * 1024 * 1024)
        for wrong in ('test', 'dev', 'release-small', None):
            with self.assertRaises(build.Rejected):
                build.recipe(wrong)

    def test_supervisor_build_operation_is_explicit(self):
        tree = ast.parse(path.read_bytes())
        calls = [node for node in ast.walk(tree) if isinstance(node, ast.Call)
                 and isinstance(node.func, ast.Attribute) and node.func.attr == 'owned_probe']
        self.assertEqual(len(calls), 2)
        kinds = [next((keyword.value.value for keyword in node.keywords
                       if keyword.arg == 'operation_kind' and isinstance(keyword.value, ast.Constant)), 'probe')
                 for node in calls]
        self.assertCountEqual(kinds, ['probe', 'public-build'])

    def test_metadata_claim_mutations(self):
        root, target = Path('/compiler-source'), Path('/fresh-target')
        files = role_files()
        claim = metadata_claim(root, target, files)
        self.assertTrue(build.inspect_cargo_metadata(claim, root, target, files))
        mutants = []
        for field, value in (('id', 'path+file:///old#oxid@0.9.0'), ('manifest_path', '/old/Cargo.toml'),
                             ('version', '0.8.0'), ('name', 'other')):
            changed = copy.deepcopy(claim); changed['packages'][0][field] = value; mutants.append(changed)
        changed = copy.deepcopy(claim); changed['packages'][0]['targets'][0]['src_path'] = '/old/src/cli.rs'; mutants.append(changed)
        changed = copy.deepcopy(claim); changed['packages'][0]['targets'].pop(); mutants.append(changed)
        changed = copy.deepcopy(claim); changed['packages'][0]['targets'].append(copy.deepcopy(changed['packages'][0]['targets'][0])); mutants.append(changed)
        for field, value in (('target_directory', '/stale'), ('workspace_root', '/old'),
                             ('workspace_members', []), ('workspace_default_members', []), ('resolve', {})):
            changed = copy.deepcopy(claim); changed[field] = value; mutants.append(changed)
        for changed in mutants:
            with self.subTest(changed=changed), self.assertRaises(build.Rejected):
                build.inspect_cargo_metadata(changed, root, target, files)
        with self.assertRaises(build.Rejected):
            build.inspect_cargo_metadata(claim, root, target, files[:-1])

    def test_process_claim_requires_full_fresh_success_and_cleanup(self):
        argv, cwd, env = build.recipe('debug'), '/source', {'CARGO_INCREMENTAL': '0'}
        record = {'argv': argv, 'cwd': cwd, 'environment': env, 'pid': 7, 'status': 0,
                  'timed_out': False, 'stream_limit_exceeded': False, 'stream_limit_bytes': build.MAX_STREAM_BYTES,
                  'timeout_seconds': 1800, 'started_ns': 100, 'completed_ns': 200, 'cleanup_complete': True,
                  'stdout': '', 'stderr': '', 'stdout_sha256': hashlib.sha256(b'').hexdigest(),
                  'stderr_sha256': hashlib.sha256(b'').hexdigest()}
        def inspect(value):
            return build.inspect_process(value, argv, cwd, env, 90, 210, 1800)
        self.assertTrue(inspect(record))
        for field, value in (('argv', argv + ['--release']), ('cwd', '/old'), ('environment', {}),
                             ('pid', 0), ('status', 1), ('status', False), ('timed_out', True),
                             ('stream_limit_exceeded', True), ('stream_limit_bytes', 12),
                             ('timeout_seconds', 1), ('started_ns', 80), ('completed_ns', 220),
                             ('cleanup_complete', False), ('stdout', 'forged'), ('stderr_sha256', '0' * 64)):
            changed = copy.deepcopy(record); changed[field] = value
            with self.subTest(field=field, value=value), self.assertRaises(build.Rejected):
                inspect(changed)

    def test_rust_host_and_release_claims(self):
        for host, triple in build.HOSTS.items():
            text = 'rustc 1.99.0 (fixture)\nhost: ' + triple + '\nrelease: 1.99.0\n'
            self.assertTrue(build.inspect_rust_identity(text, host))
            for changed in (text.replace('1.99.0', '1.98.0'), text + 'host: ' + triple + '\n',
                            text.replace(triple, 'wrong-host')):
                with self.assertRaises(build.Rejected):
                    build.inspect_rust_identity(changed, host)

    def test_role_paths_are_dedicated_and_finite(self):
        root = Path('/fresh-run')
        for role in ('ordinary', 'lifecycle'):
            self.assertEqual(build.public_paths(root, role),
                             (root / 'targets' / ('public-' + role), root / 'public-build' / role))
        for role in ('parser_observed', '../operations', ''):
            with self.assertRaises(build.Rejected):
                build.public_paths(root, role)
        with self.assertRaises(build.Rejected):
            build.public_paths('relative', 'ordinary')

    def test_output_paths_fresh_disjoint_and_no_symlinks(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp).resolve()
            self.assertEqual(build._canonical_future(root / 'fresh'), root / 'fresh')
            with self.assertRaises(build.Rejected):
                build._canonical_future(root)
            self.assertTrue(build._overlaps(root / 'x', root / 'x/y'))
            self.assertFalse(build._overlaps(root / 'x', root / 'y'))
            if os.name != 'nt':
                (root / 'alias').symlink_to(root, target_is_directory=True)
                with self.assertRaises(build.Rejected):
                    build._canonical_future(root / 'alias/fresh')

    def test_synthetic_artifact_claim_is_checked_but_never_admitted(self):
        with tempfile.TemporaryDirectory() as temp:
            path = Path(temp).resolve() / 'oxid'
            raw = bytearray(64); raw[:7] = b'\x7fELF\x02\x01\x01'; raw[16:18] = b'\x02\x00'
            raw[18:20] = b'\x3e\x00'; raw[20:24] = b'\x01\x00\x00\x00'; raw[52:54] = b'\x40\x00'
            path.write_bytes(raw)
            measured = path.stat()
            record = {'path': str(path), 'bytes': len(raw), 'sha256': hashlib.sha256(raw).hexdigest(),
                      'stat': {k: getattr(measured, k) for k in ('st_dev', 'st_ino', 'st_mode', 'st_size', 'st_mtime_ns', 'st_ctime_ns')}}
            start, end = measured.st_mtime_ns - 1, measured.st_mtime_ns + 1
            self.assertTrue(build.inspect_artifact(record, path, start, end, 'Linux x86_64'))
            with self.assertRaises(build.Rejected):
                build.inspect_artifact(record, path, end + 1, end + 2, 'Linux x86_64')
            changed = copy.deepcopy(record); changed['path'] = str(path.parent / 'relocated')
            with self.assertRaises(build.Rejected):
                build.inspect_artifact(changed, path, start, end, 'Linux x86_64')
            path.write_bytes(b'changed')
            with self.assertRaises(build.Rejected):
                build.inspect_artifact(record, path, start, end, 'Linux x86_64')
            with self.assertRaises(build.Rejected):
                build.read_build_binding(record)

    def test_coherently_rehashed_nonexecutable_and_wrong_architecture_reject(self):
        with tempfile.TemporaryDirectory() as temp:
            path = Path(temp).resolve() / 'claimed-oxid'
            for host, raw in (
                ('Linux x86_64', b'\x7fELF\x02\x01\x01' + b'\x00' * 57),
                ('macOS x86_64', b'\xcf\xfa\xed\xfe\x07\x00\x00\x01' + b'\x00' * 24),
                ('Windows x86_64', b'MZ' + b'\x00' * 62),
            ):
                path.write_bytes(raw)
                measured = path.stat()
                record = {'path': str(path), 'bytes': len(raw), 'sha256': hashlib.sha256(raw).hexdigest(),
                          'stat': {k: getattr(measured, k) for k in ('st_dev', 'st_ino', 'st_mode', 'st_size', 'st_mtime_ns', 'st_ctime_ns')}}
                with self.subTest(host=host), self.assertRaises(build.Rejected):
                    build.inspect_artifact(record, path, measured.st_mtime_ns - 1,
                                           measured.st_mtime_ns + 1, host)

    def test_bounded_read_rejects_size_symlink_and_extra_evidence(self):
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp).resolve()
            path = root / 'body'
            path.write_bytes(b'bounded')
            with self.assertRaises(build.Rejected):
                build._read_file(path, maximum=2)
            self.assertEqual(build._read_file(path, maximum=7)[1], b'bounded')
            build._closed_names(root, {'body'}, None)
            (root / 'extra').write_bytes(b'extra')
            with self.assertRaises(build.Rejected):
                build._closed_names(root, {'body'}, None)
            if os.name != 'nt':
                (root / 'alias').symlink_to(path)
                with self.assertRaises(build.Rejected):
                    build._read_file(root / 'alias')

    def test_no_boolean_or_claim_unlocks_producer_or_runtime(self):
        with self.assertRaises(build.Rejected):
            build.build({'status': 'pass', 'execution_qualified': True}, tool_admitted=True, execution_ready=True)
        with self.assertRaises(build.Rejected):
            build.read_build_binding({'status': 'pass', 'execution_qualified': True})
        with self.assertRaises(build.NotReady):
            build.collect({'status': 'pass', 'execution_qualified': True})
        with self.assertRaises(TypeError):
            build.BuildResult({'status': 'built'})
        with self.assertRaises(build.Rejected):
            build.read_build_binding(object.__new__(build.BuildResult))


if __name__ == '__main__':
    unittest.main()
