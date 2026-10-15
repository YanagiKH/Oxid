"""Portable source-reader identity controls included by test_integration."""
import ast
import json
from pathlib import Path
import sys
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import patch
import common as q

REPO = Path(__file__).resolve().parents[3]


class PublishedViewIdentityControls(unittest.TestCase):
    path_reads_before_final = 3

    @classmethod
    def setUpClass(cls):
        base = REPO / 'tests/qualification/lexer_reservation_current'
        adapters = q.module('_view_identity_adapters', base / 'adapters.py')
        with patch.dict(sys.modules, {'adapters': adapters}):
            retained = q.module('_view_identity_transition', base / 'source_transition.py')
            with patch.dict(sys.modules, {'source_transition': retained}):
                cls.view = q.module('_view_identity_runtime', REPO /
                    'tests/qualification/published_formatter_current/published_view.py')
        cls.reject = adapters.Reject

    def info(self, **changes):
        fields = dict(st_dev=7, st_ino=11, st_mode=0o100644, st_nlink=1,
                      st_size=4, st_mtime_ns=200, st_ctime_ns=100,
                      st_birthtime_ns=100)
        fields.update(changes)
        return SimpleNamespace(**fields)

    def test_cross_api_preserves_identity(self):
        before, opened = self.info(), self.info(st_ctime_ns=300)
        with patch.object(self.view, 'os', SimpleNamespace(name='nt')):
            self.assertTrue(self.view._opened_identity_matches(before, opened))
            for field in ('st_dev', 'st_ino', 'st_mode', 'st_nlink', 'st_size',
                          'st_mtime_ns', 'st_birthtime_ns'):
                with self.subTest(field=field):
                    changed = self.info(st_ctime_ns=300)
                    setattr(changed, field, getattr(changed, field) + 1)
                    self.assertFalse(self.view._opened_identity_matches(before, changed))
        self.assertNotEqual(self.view._stat_identity(before), self.view._stat_identity(opened))

    def test_posix_cross_api_retains_ctime_even_with_birthtime(self):
        with patch.object(self.view, 'os', SimpleNamespace(name='posix')):
            self.assertFalse(self.view._opened_identity_matches(
                self.info(), self.info(st_ctime_ns=300)))

    def test_missing_birthtime_fails_closed(self):
        for missing in ('before', 'opened', 'both'):
            with self.subTest(missing=missing):
                before, opened = self.info(), self.info()
                for label, info in (('before', before), ('opened', opened)):
                    if missing in (label, 'both'):
                        del info.st_birthtime_ns
                with patch.object(self.view, 'os', SimpleNamespace(name='nt')):
                    self.assertTrue(self.view._opened_identity_matches(before, opened))
                    opened.st_ctime_ns += 1
                    self.assertFalse(self.view._opened_identity_matches(before, opened))

    def test_real_regular_file_and_replaced_open_target(self):
        view = self.view
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp).resolve()
            target = root / 'member'
            target.write_bytes(b'good')
            self.assertEqual(view._read_regular(root, 'member'), b'good')
            replacement = root / 'replacement'
            replacement.write_bytes(b'evil')
            original_open = view.os.open
            def swapped_open(path, flags):
                replacement.replace(target)
                return original_open(path, flags)
            with patch.object(view.os, 'open', side_effect=swapped_open):
                with self.assertRaisesRegex(self.reject, 'changed before read'):
                    view._read_regular(root, 'member')

    def test_real_hardlinked_file_is_rejected(self):
        view = self.view
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp).resolve()
            (root / 'member').write_bytes(b'good')
            view.os.link(root / 'member', root / 'alias')
            with self.assertRaisesRegex(self.reject, 'unsafe'):
                view._read_regular(root, 'member')

    def test_descriptor_ctime_race(self):
        self.check_windows_read_identity('descriptor')

    def test_path_ctime_race(self):
        self.check_windows_read_identity('path')

    def test_stable_cross_api_ctime(self):
        self.check_windows_read_identity(None)

    def check_windows_read_identity(self, change):
        view = self.view
        original_lstat, original_fstat = Path.lstat, view.os.fstat
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp).resolve()
            target = root / 'member'
            target.write_bytes(b'good')
            calls = {'path': 0, 'descriptor': 0}
            def path_stat(path):
                original = original_lstat(path)
                info = SimpleNamespace(**{name: getattr(original, name) for name in
                    ('st_dev', 'st_ino', 'st_mode', 'st_nlink', 'st_size', 'st_mtime_ns', 'st_ctime_ns')})
                info.st_birthtime_ns = 100
                info.st_ctime_ns = 100
                if path == target:
                    calls['path'] += 1
                    if change == 'path' and calls['path'] == self.path_reads_before_final:
                        info.st_ctime_ns += 1
                return info
            def fd_stat(fd):
                original = original_fstat(fd)
                info = SimpleNamespace(**{name: getattr(original, name) for name in
                    ('st_dev', 'st_ino', 'st_mode', 'st_nlink', 'st_size', 'st_mtime_ns', 'st_ctime_ns')})
                calls['descriptor'] += 1
                info.st_birthtime_ns = 100
                info.st_ctime_ns = 300 + (change == 'descriptor' and calls['descriptor'] == 2)
                return info
            fake_os = SimpleNamespace(**{name: getattr(view.os, name) for name in
                ('open', 'read', 'close', 'O_RDONLY')}, name='nt', fstat=fd_stat)
            with patch.object(view, 'os', fake_os), patch.object(Path, 'lstat', path_stat):
                if change is None:
                    self.assertEqual(view._read_regular(root, 'member'), b'good')
                else:
                    message = ('changed during read' if change == 'descriptor'
                               else 'name changed during read')
                    with self.assertRaisesRegex(self.reject, message):
                        view._read_regular(root, 'member')


class PublishedDispatcherIdentityControls(PublishedViewIdentityControls):
    @classmethod
    def setUpClass(cls):
        import os
        import stat
        path = REPO / 'tests/fixtures/typed_project_source_binding/run.py'
        tree = ast.parse(path.read_bytes())
        names = {'_read_regular', '_opened_identity_matches'}
        functions = [node for node in tree.body
                     if isinstance(node, ast.FunctionDef) and node.name in names]
        if {node.name for node in functions} != names or len(functions) != 2:
            raise AssertionError('missing exact dispatcher reader functions')
        namespace = {'_os': os, '_stat': stat}
        exec(compile(ast.Module(body=functions, type_ignores=[]), str(path), 'exec'), namespace)
        fields = ('st_dev', 'st_ino', 'st_mode', 'st_nlink', 'st_size',
                  'st_mtime_ns', 'st_ctime_ns')
        class Reader:
            def __init__(self):
                self.os = os
            def _read_regular(self, root, name):
                namespace['_os'] = self.os
                return namespace['_read_regular'](root / name)
            def _opened_identity_matches(self, before, opened):
                namespace['_os'] = self.os
                return namespace['_opened_identity_matches'](before, opened, fields)
            def _stat_identity(self, value):
                return tuple(getattr(value, field) for field in fields)
        cls.view = Reader()
        cls.reject = ValueError


class PublishedLifecycleIdentityControls(PublishedViewIdentityControls):
    path_reads_before_final = 2

    @classmethod
    def setUpClass(cls):
        import os
        import stat
        path = REPO / 'tests/qualification/unit4_public_v3/published_lifecycle.py'
        tree = ast.parse(path.read_bytes())
        names = {'_read_regular', '_opened_identity_matches', '_stamp',
                 '_safe_ancestors', '_valid_package_schema'}
        functions = [node for node in tree.body
                     if isinstance(node, ast.FunctionDef) and node.name in names]
        if {node.name for node in functions} != names or len(functions) != len(names):
            raise AssertionError('missing exact lifecycle reader functions')
        def need(condition, message):
            if not condition:
                raise ValueError(message)
        namespace = {'os': os, 'stat': stat, 'Path': Path,
                     '_need': need, 'MAX_BODY': 2 * 1024 * 1024}
        exec(compile(ast.Module(body=functions, type_ignores=[]), str(path), 'exec'), namespace)
        class Reader:
            def __init__(self):
                self.os = os
            def _read_regular(self, root, name):
                namespace['os'] = self.os
                return namespace['_read_regular'](root / name)[0]
            def _opened_identity_matches(self, before, opened):
                namespace['os'] = self.os
                return namespace['_opened_identity_matches'](before, opened)
            def _stat_identity(self, value):
                return namespace['_stamp'](value)
        cls.view, cls.reject = Reader(), ValueError
        cls.schema_check = staticmethod(namespace['_valid_package_schema'])

    def test_package_schema_matches_genuine_integer_version(self):
        manifest = json.loads((REPO /
            'tests/fixtures/typed_project_source_binding/package-manifest.json').read_bytes())
        self.assertTrue(self.schema_check(manifest))
        for value in (True, False, '1', 1.0, 0, 2, None):
            with self.subTest(schema=value):
                self.assertFalse(self.schema_check({**manifest, 'schema': value}))
        self.assertFalse(self.schema_check({'files': manifest['files']}))
        self.assertFalse(self.schema_check({**manifest, 'extra': 1}))

    def test_posix_without_linux_atime_flag(self):
        view = self.view
        portable = SimpleNamespace(**{name: getattr(view.os, name) for name in
            ('open', 'read', 'close', 'fstat', 'O_RDONLY')}, name='posix')
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp).resolve()
            (root / 'member').write_bytes(b'good')
            with patch.object(view, 'os', portable):
                self.assertEqual(view._read_regular(root, 'member'), b'good')

    def test_windows_reparse_member_and_ancestor_are_rejected(self):
        original = Path.lstat
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp).resolve()
            member = root / 'member'
            member.write_bytes(b'good')
            for unsafe in (root, member):
                def info(path):
                    result = original(path)
                    if path != unsafe:
                        return result
                    return SimpleNamespace(**{name: getattr(result, name) for name in
                        ('st_dev', 'st_ino', 'st_mode', 'st_nlink', 'st_size')},
                        st_file_attributes=0x400)
                with self.subTest(path=unsafe), patch.object(Path, 'lstat', info):
                    with patch.object(self.view.os, 'open', side_effect=AssertionError('opened unsafe path')):
                        with self.assertRaisesRegex(ValueError, 'aliased'):
                            self.view._read_regular(root, 'member')

    def test_final_reparse_member_and_ancestor_are_rejected(self):
        original = Path.lstat
        fields = ('st_dev', 'st_ino', 'st_mode', 'st_nlink', 'st_size',
                  'st_mtime_ns', 'st_ctime_ns')
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp).resolve()
            member = root / 'member'
            member.write_bytes(b'good')
            for unsafe in (root, member):
                calls = 0
                def info(path):
                    nonlocal calls
                    result = original(path)
                    if path == unsafe:
                        calls += 1
                        if calls == 2:
                            return SimpleNamespace(**{name: getattr(result, name)
                                for name in fields}, st_file_attributes=0x400)
                    return result
                with self.subTest(path=unsafe), patch.object(Path, 'lstat', info):
                    with self.assertRaisesRegex(ValueError,
                            'input (name|ancestor) changed during read'):
                        self.view._read_regular(root, 'member')

    def test_ancestor_identity_change_is_rejected(self):
        original = Path.lstat
        with tempfile.TemporaryDirectory() as temp:
            root = Path(temp).resolve()
            (root / 'member').write_bytes(b'good')
            calls = 0
            def info(path):
                nonlocal calls
                result = original(path)
                if path == root:
                    calls += 1
                    if calls == 2:
                        return SimpleNamespace(st_dev=result.st_dev,
                            st_ino=result.st_ino + 1, st_mode=result.st_mode)
                return result
            with patch.object(Path, 'lstat', info):
                with self.assertRaisesRegex(ValueError, 'ancestor changed'):
                    self.view._read_regular(root, 'member')


if __name__ == '__main__':
    unittest.main(verbosity=2)
