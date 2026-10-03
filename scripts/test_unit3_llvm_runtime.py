"""Independent small Debian-layout controls for the Unit3 LLVM stage boundary."""
import copy
import contextlib
import hashlib
import importlib.util
import json
import io
import os
from pathlib import Path
import sys
import tarfile
import tempfile
import unittest
from unittest import mock

NATIVE = (Path(__file__).resolve().parents[1] / 'tests/fixtures/'
          'typed_project_unit3_independent/portable/native-v1')


def load(name):
    if str(NATIVE) not in sys.path:
        sys.path.insert(0, str(NATIVE))
    spec = importlib.util.spec_from_file_location(name, NATIVE / (name + '.py'))
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


stage = load('stage_llvm_runtime')
inventories = [load(name).library_inventory
               for name in ('native_portable', 'native_portable_grouped')]


class LLVMRuntimeStageTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.source = self.root / 'usr/lib/x86_64-linux-gnu'
        self.source.mkdir(parents=True)
        self.output = self.root / 'stage'
        # Small byte identities are a test-only seam; the CLI cannot change pins.
        self.files = copy.deepcopy(stage.RUNTIME_FILES)
        for name, row in self.files.items():
            data = b'fixture ELF for ' + name.encode()
            (self.source / name).write_bytes(data)
            row.update(bytes=len(data), sha256=hashlib.sha256(data).hexdigest())
        (self.source / stage.ALIAS).symlink_to(stage.ALIAS_TARGET)
        (self.source / 'perl/5.40.1').mkdir(parents=True)
        (self.source / 'perl/5.40').symlink_to('5.40.1')
        (self.source / 'unrelated.so').write_bytes(b'ordinary host library')
        for name, value in [('SOURCE_DIRECTORY', self.source), ('RUNTIME_FILES', self.files)]:
            patch = mock.patch.object(stage, name, value)
            patch.start()
            self.addCleanup(patch.stop)
        patch = mock.patch.object(stage, 'command', side_effect=self.command)
        self.query = patch.start()
        self.addCleanup(patch.stop)

    def command(self, argv):
        if argv[0] == 'readelf':
            return f' 0x0 (SONAME) Library soname: [{Path(argv[-1]).name}]\n'
        selector = argv[-1]
        if argv[1] == '--show':
            return f'{selector}\t1:19.1.7-3+b1\tamd64\tinstalled\n'
        if argv[1] == '--listfiles':
            package = selector.split(':')[0]
            names = [name for name, row in self.files.items() if row['package'] == package]
            if package == 'libllvm19':
                names.append(stage.ALIAS)
            return '\n'.join(str(self.source / name) for name in names) + '\n'
        if argv[1] == '--search':
            package = 'libllvm19' if Path(selector).name == stage.ALIAS else self.files[Path(selector).name]['package']
            owner = 'libllvm19:amd64' if package == 'libllvm19' else 'libclang-cpp19'
            return f'{owner}: {selector}\n'
        self.fail(f'unexpected command {argv}')

    def assert_refused(self):
        with self.assertRaises((ValueError, OSError)):
            stage.stage(self.output)
        self.assertFalse((self.output / 'llvm-runtime-stage.json').exists())

    def test_debian_broad_directory_fails_but_exact_stage_passes_both_inventories(self):
        for inventory in inventories:
            with self.assertRaisesRegex(ValueError, 'symlink must resolve to a regular file'):
                inventory(self.source)
        receipt = stage.stage(self.output)
        libraries = Path(receipt['library_directory'])
        self.assertEqual(receipt['status'], 'verified')
        self.assertEqual(set(path.name for path in libraries.iterdir()), {*self.files, stage.ALIAS})
        self.assertEqual((libraries / stage.ALIAS).readlink(), Path(stage.ALIAS_TARGET))
        for inventory in inventories:
            result = inventory(libraries)
            self.assertEqual(len(result['entries']), 3)
            self.assertEqual(result['unique_target_bytes'], sum(row['bytes'] for row in self.files.values()))
        self.assertEqual(json.loads((self.output / 'llvm-runtime-stage.json').read_text()), receipt)

    def test_content_identity_ignores_stage_location(self):
        first = stage.stage(self.output)
        second = stage.stage(self.root / 'second-stage')
        self.assertEqual(first['content_sha256'], second['content_sha256'])
        self.assertNotEqual(first['library_directory'], second['library_directory'])

    def test_missing_required_file(self):
        (self.source / 'libclang-cpp.so.19.1').unlink()
        self.assert_refused()

    def test_wrong_bytes_with_same_size(self):
        path = self.source / 'libclang-cpp.so.19.1'
        path.write_bytes(b'x' * path.stat().st_size)
        self.assert_refused()

    def test_nonregular_required_directory(self):
        path = self.source / 'libclang-cpp.so.19.1'
        path.unlink()
        path.mkdir()
        self.assert_refused()

    def test_required_fifo_is_rejected_without_blocking(self):
        path = self.source / 'libclang-cpp.so.19.1'
        path.unlink()
        os.mkfifo(path)
        self.assert_refused()

    def test_required_directory_symlink(self):
        path = self.source / 'libclang-cpp.so.19.1'
        path.unlink()
        path.symlink_to('perl/5.40.1')
        self.assert_refused()

    def test_required_file_symlink_substitution(self):
        path = self.source / 'libclang-cpp.so.19.1'
        replacement = self.root / 'external.so'
        path.rename(replacement)
        path.symlink_to(replacement)
        self.assert_refused()

    def test_alias_targets_dangling_escaping_absolute_and_wrong(self):
        alias = self.source / stage.ALIAS
        for target in ('missing.so', '../../../../external.so',
                       str(self.source / stage.ALIAS_TARGET), 'libclang-cpp.so.19.1'):
            with self.subTest(target=target):
                alias.unlink()
                alias.symlink_to(target)
                self.assert_refused()

    def test_reused_output(self):
        self.output.mkdir()
        self.assert_refused()

    def test_partial_copy_failure_never_writes_success_receipt(self):
        checked = stage.checked_file

        def fail_second_copy(path, expected, destination=None):
            if destination is not None and path.name == 'libclang-cpp.so.19.1':
                raise OSError('simulated copy failure')
            return checked(path, expected, destination)

        with mock.patch.object(stage, 'checked_file', side_effect=fail_second_copy):
            self.assert_refused()
        self.assertTrue((self.output / 'libraries/libLLVM.so.19.1').exists())
        self.assert_refused()

    def test_aliased_output_or_parent(self):
        destination = self.root / 'elsewhere'
        destination.mkdir()
        self.output.symlink_to(destination, target_is_directory=True)
        self.assert_refused()
        self.output = self.output / 'child'
        self.assert_refused()

    def test_wrong_package_version_architecture_status_and_name(self):
        for old, new in [('19.1.7', '19.1.8'), ('amd64', 'arm64'),
                         ('installed', 'unpacked'), ('libllvm19', 'libllvm190')]:
            with self.subTest(new=new):
                self.query.side_effect = lambda argv: self.command(argv).replace(old, new)
                self.assert_refused()

    def test_missing_package_path_or_multiple_wrong_owners(self):
        for query, suffix in [('--listfiles', 'missing'), ('--search', 'extra-owner: path\n')]:
            with self.subTest(query=query):
                self.query.side_effect = lambda argv: (suffix if argv[1] == query else self.command(argv))
                self.assert_refused()

    def test_wrong_soname(self):
        self.query.side_effect = lambda argv: ' (SONAME) [libLLVM.so.20.1]\n' if argv[0] == 'readelf' else self.command(argv)
        self.assert_refused()

    def test_mutation_remains_visible_to_both_qualification_inventories(self):
        receipt = stage.stage(self.output)
        libraries = Path(receipt['library_directory'])
        original = [inventory(libraries) for inventory in inventories]
        (libraries / 'libclang-cpp.so.19.1').write_bytes(b'modified after qualification')
        for inventory, expected in zip(inventories, original):
            with self.assertRaisesRegex(ValueError, 'qualified LLVM library content/targets changed'):
                stage.require(inventory(libraries) == expected, 'qualified LLVM library content/targets changed')

    def test_alias_mutation_remains_visible_to_both_inventories(self):
        receipt = stage.stage(self.output)
        libraries = Path(receipt['library_directory'])
        original = [inventory(libraries) for inventory in inventories]
        (libraries / stage.ALIAS).unlink()
        (libraries / stage.ALIAS).symlink_to('libclang-cpp.so.19.1')
        for inventory, expected in zip(inventories, original):
            self.assertNotEqual(inventory(libraries), expected)

    def test_compact_archive_retains_small_binding_receipts_and_no_libraries(self):
        import collect_unit3_ci_diagnostics as diagnostics
        stage.stage(self.output)
        root = self.root / 'qualification'
        (root / 'llvm-runtime').mkdir(parents=True)
        receipt = self.output / 'llvm-runtime-stage.json'
        (root / 'llvm-runtime/llvm-runtime-stage.json').write_bytes(receipt.read_bytes())
        qualified = root / 'native/native-component/qualified-build-tools.json'
        qualified.parent.mkdir(parents=True)
        qualified.write_text('{"status":"fixture"}\n')
        libraries = root / 'llvm-runtime/libraries'
        libraries.mkdir()
        (libraries / 'libLLVM.so.19.1').write_bytes(b'not compact evidence')
        destination = self.root / 'compact.tar.gz'
        with contextlib.redirect_stdout(io.StringIO()):
            summary = diagnostics.compact(root, destination, 'fixture')
        with tarfile.open(destination) as archive:
            self.assertEqual(set(archive.getnames()), {
                'llvm-runtime/llvm-runtime-stage.json',
                'native/native-component/qualified-build-tools.json', 'compact-index.json'})
        self.assertLess(summary['included_payload_bytes'], 15 * 1024 * 1024)
        self.assertEqual(summary['uncompressed_archive_limit'], 16 * 1024 * 1024)


if __name__ == '__main__':
    unittest.main()
