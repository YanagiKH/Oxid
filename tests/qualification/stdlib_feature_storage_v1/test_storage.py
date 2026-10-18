"""Inactive storage controls; synthetic live edits are not feature acceptance."""
import ast
import builtins
import copy
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import types
import unittest
from unittest.mock import patch

HERE = Path(__file__).absolute().parent
REPO = HERE.parents[2]
GENERATION = 'tests/qualification/package_json_current_v1/generation.py'
GENERATION_PIN = (13680, '66ca7aee20a7d17b332ab3c2f8bad72d5882e7a52f07d86ebbd6ccaa85cae9ea')

def load(path, raw=None, **scope):
    m = types.ModuleType('feature_storage_' + path.stem)
    m.__file__ = str(path)
    m.__dict__.update(scope)
    exec(compile(path.read_bytes() if raw is None else raw, str(path), 'exec'), m.__dict__)
    return m

d = load(HERE / 'derive.py')

def helpers():
    raw = (REPO / GENERATION).read_bytes()
    if (len(raw), hashlib.sha256(raw).hexdigest()) != GENERATION_PIN:
        raise ValueError('fixed generation bytes')
    g = load(REPO / GENERATION, raw)
    original = g.regular(REPO, d.PREVIOUS)
    derived = d.derive(original)
    if derived != (HERE / 'candidate.py').read_bytes():
        raise ValueError('candidate derivation changed')
    def private(raw, name):
        text = d.exact(raw.decode(), 'import generation as g\n', '')
        return load(HERE / name, text.encode(), g=g)
    return private(derived, 'candidate.py'), private(original, 'predecessor.py'), g

def copy_checkout(source, destination, tracked, package):
    """Copy tracked non-package files, then the package through one exclusive route."""
    package_name = package.relative_to(source).as_posix()
    destination.mkdir()
    for name in tracked:
        if not name or name == package_name or name.startswith(package_name + '/'):
            continue
        target = destination / name
        target.parent.mkdir(parents=True, exist_ok=True)
        shutil.copyfile(source / name, target)
    shutil.copytree(package, destination / package_name)


def historical_baseline(old, g, restored, descriptor):
    # These literal identities are independent of the candidate's mutable rows.
    fixed = (('docs/API.md', 'tests/qualification/stdlib_feature_storage_v1/archive/docs/API.md.data', 2358, 'a022ce558db0032ae34716a0dfa18d52b30fe1e99dc664b50ac030e5f72315d8'), ('docs/MODULES.md', 'tests/qualification/stdlib_feature_storage_v1/archive/docs/MODULES.md.data', 3249, '2787a21a75d110724bb53c32693c46e3efc72e08ee0d7fbf8b20044020392da7'), ('stdlib/numbers.ox', 'tests/qualification/stdlib_feature_storage_v1/archive/stdlib/numbers.ox.data', 315, 'a8c0d74e2e8b7e7964122ce8c9ce8806334dda6b8c55cb55c75a5a5695b14b3d'), ('tests/legacy_modules.rs', 'tests/qualification/stdlib_feature_storage_v1/archive/tests/legacy_modules.rs.data', 7782, '53cb238bba1626bd45868ea1e640cb7a3214c5474a744b5cec60a3cb1f9349f4'))
    with tempfile.TemporaryDirectory() as temporary:
        root = Path(temporary) / 'historical'
        names = subprocess.check_output(['git', '-C', str(REPO), 'ls-files', '-z']).decode().split('\0')
        copy_checkout(REPO, root, names, HERE)
        for name, archive, size, digest in fixed:
            body = g.regular(REPO, archive, size)
            if len(body) != size or hashlib.sha256(body).hexdigest() != digest:
                raise ValueError('fixed independent historical body: ' + name)
            (root / name).write_bytes(body)
        return old.capture(root, restored, descriptor)

class Storage(unittest.TestCase):
    def test_exact_storage_test_derivation(self):
        derivation = load(HERE / 'storage_test_derivation.py')
        raw = (HERE / 'archive/test_storage.py.data').read_bytes()
        self.assertEqual(derivation.derive(raw), (HERE / 'test_storage.py').read_bytes())
        with self.assertRaises(ValueError):
            derivation.derive(raw + b'\n')

    def test_checkout_copy_has_one_package_route_in_both_git_states(self):
        with tempfile.TemporaryDirectory() as temp:
            source = Path(temp) / 'source'
            source.mkdir()
            package = source / 'tests/qualification/package'
            package.mkdir(parents=True)
            (package / 'candidate.py').write_bytes(b'candidate\n')
            (package / 'nested').mkdir()
            (package / 'nested/archive.data').write_bytes(b'original\n')
            (source / 'base.txt').write_bytes(b'base\n')
            sibling = source / 'tests/qualification/package_extra/keep.txt'
            sibling.parent.mkdir()
            sibling.write_bytes(b'sibling\n')
            # This fixture owns its Git index even when the outer test invocation
            # uses a separate index to exercise the real tracked package state.
            env = dict(os.environ)
            env.pop('GIT_INDEX_FILE', None)
            def git(*args):
                return subprocess.check_output(['git', '-C', str(source), *args], env=env)
            git('init', '-q')
            git('add', 'base.txt', 'tests/qualification/package_extra/keep.txt')
            expected = {'base.txt': b'base\n',
                        'tests/qualification/package_extra/keep.txt': b'sibling\n',
                        'tests/qualification/package/candidate.py': b'candidate\n',
                        'tests/qualification/package/nested/archive.data': b'original\n'}
            for state in ('untracked', 'tracked'):
                if state == 'tracked':
                    git('add', 'tests/qualification/package')
                names = git('ls-files', '-z').decode().split('\0')
                self.assertEqual('tests/qualification/package/candidate.py' in names,
                                 state == 'tracked')
                destination = Path(temp) / state
                with self.subTest(state=state), patch.object(shutil, 'copyfile', wraps=shutil.copyfile) as copied:
                    copy_checkout(source, destination, names, package)
                    package_reads = [Path(call.args[0]) for call in copied.call_args_list
                                     if Path(call.args[0]).is_relative_to(package)]
                    self.assertCountEqual(package_reads, [package / 'candidate.py',
                                                         package / 'nested/archive.data'])
                actual = {p.relative_to(destination).as_posix(): p.read_bytes()
                          for p in destination.rglob('*') if p.is_file()}
                self.assertEqual(actual, expected)
                with self.assertRaises(FileExistsError):
                    copy_checkout(source, destination, names, package)

    def test_derivation_preserves_every_existing_function_except_capture(self):
        before = (REPO / d.PREVIOUS).read_bytes()
        after = d.derive(before)
        self.assertEqual(after, (HERE / 'candidate.py').read_bytes())
        def methods(raw):
            text = raw.decode()
            return {n.name: ast.get_source_segment(text, n) for n in ast.parse(text).body
                    if isinstance(n, ast.FunctionDef)}
        a, b = methods(before), methods(after)
        self.assertEqual({n for n in a if a[n] != b[n]}, {'capture'})
        self.assertEqual(set(b) - set(a), {'_feature_archive'})
        for raw in (before + b'\n', bytearray(before), before[:-1]):
            with self.assertRaises(ValueError): d.derive(raw)

    def test_baseline_git_and_frozen_row_identity(self):
        p, _, g = helpers()
        manifest = json.loads((HERE / 'originals.json').read_bytes())
        self.assertFalse(manifest['authority_issued'])
        self.assertEqual(manifest['baseline_commit'], 'cb546a2713c71c231242812475cadbdf2cef72aa')
        view = json.loads(g.regular(REPO, p.VIEW_NAME))
        source = json.loads(g.regular(REPO, g.PACKAGE + 'package-json-source-v1.json'))
        source_names = {r['path'] for r in source['files']}
        self.assertEqual(len(manifest['files']), 4)
        for row in manifest['files']:
            name = row['path']; fixed = dict(row); storage = fixed.pop('storage')
            self.assertEqual(fixed, p._FEATURE_ROWS[name])
            self.assertEqual(fixed, next(r for r in view['files'] if r['path'] == name))
            self.assertEqual(storage, p._FEATURE_ARCHIVE + name + '.data')
            raw = subprocess.check_output(['git', '-C', str(REPO), 'show', manifest['baseline_commit'] + ':' + name])
            self.assertEqual(p._feature_archive(REPO, fixed), raw)
            self.assertNotIn(name, source_names)
        self.assertNotIn('tests/stdlib_numbers.rs', source_names)
        self.assertFalse(any(n.startswith('tests/qualification/stdlib_feature_storage_v1/') for n in source_names))

    def test_every_archive_rejects_missing_mutated_alias_and_special_input(self):
        p, _, _ = helpers()
        for name, row in p._FEATURE_ROWS.items():
            body = p._feature_archive(REPO, row)
            for mutation in ('missing', 'changed', 'oversize', 'symlink', 'hardlink', 'executable', 'directory', 'fifo'):
                with self.subTest(name=name, mutation=mutation), tempfile.TemporaryDirectory() as t:
                    root = Path(t); dst = root / (p._FEATURE_ARCHIVE + name + '.data')
                    dst.parent.mkdir(parents=True)
                    live = root / name; live.parent.mkdir(parents=True, exist_ok=True); live.write_bytes(body)
                    real = root / 'real'; real.write_bytes(body)
                    if mutation == 'changed': dst.write_bytes(b'X' + body[1:])
                    elif mutation == 'oversize': dst.write_bytes(body + b'\n')
                    elif mutation == 'symlink': dst.symlink_to(real)
                    elif mutation == 'hardlink': os.link(real, dst)
                    elif mutation == 'executable': dst.write_bytes(body); dst.chmod(0o755)
                    elif mutation == 'directory': dst.mkdir()
                    elif mutation == 'fifo': os.mkfifo(dst)
                    with self.assertRaises((ValueError, OSError)): p._feature_archive(root, row)

    def test_rows_reject_relabel_types_and_coherent_rehash(self):
        p, _, g = helpers()
        for name, row in p._FEATURE_ROWS.items():
            for key, value in [('read_from', p._FEATURE_ARCHIVE + name + '.data'), ('bytes', True),
                               ('bytes', float(row['bytes'])), ('mode', '100755'), ('git_blob', '0'*40),
                               ('sha256', '0'*64), ('extra', 1), ('path', 'unlisted')]:
                with self.subTest(name=name, key=key), self.assertRaises(ValueError):
                    p._feature_archive(REPO, {**row, key: value})
            raw = p._feature_archive(REPO, row) + b'\n'
            with self.assertRaises(ValueError):
                p._feature_archive(REPO, {**row, **g.identity(name, raw), 'git_blob': g.gitid('blob', raw)})
            with self.assertRaisesRegex(ValueError, 'feature path cannot be restored source'):
                p.capture(REPO, {name: raw}, {})

    def test_real_complete_view_survives_four_live_edits_and_excludes_new_test(self):
        p, old, g = helpers()
        modules = dict(sys.modules)
        try:
            runtime = load(REPO / 'tests/fixtures/typed_project_source_binding/run.py')
            admitted = runtime._modules['package_current'].admit(REPO, runtime._SOURCE_SEAL)
            restored = admitted['predecessor_inputs']; descriptor = runtime._SOURCE_SEAL['predecessor_view']
            baseline = historical_baseline(old, g, restored, descriptor)
            self.assertEqual(len(baseline), 2724)
            self.assertEqual(sum(map(len, baseline.values())), 78507317)
            self.assertEqual(g.sha(g.encoded([g.identity(n, b) for n, b in sorted(baseline.items())])),
                             'e70b8b790756e0fb9e0cd1341779c51646b964afcf9e2eade2bd9480716b1882')
            with tempfile.TemporaryDirectory() as temp:
                root = Path(temp) / 'repo'
                names = subprocess.check_output(['git', '-C', str(REPO), 'ls-files', '-z']).decode().split('\0')
                copy_checkout(REPO, root, names, HERE)
                for name in p._FEATURE_ROWS:
                    (root / name).write_bytes(b'synthetic current feature edit\n')
                (root / 'tests/stdlib_numbers.rs').write_bytes(b'// synthetic new feature test\n')
                with self.assertRaisesRegex(ValueError, 'accepted view body:'):
                    old.capture(root, restored, descriptor)
                self.assertEqual(p.capture(root, restored, descriptor), baseline)
                # Execute the genuine package consumer through a private import
                # scope; no global predecessor alias or active pin is replaced.
                original_import = builtins.__import__
                def scoped_import(name, globals=None, locals=None, fromlist=(), level=0):
                    if name == 'predecessor' and level == 0:
                        return p
                    return original_import(name, globals, locals, fromlist, level)
                consumer_path = REPO / 'tests/qualification/package_json_current_v1/package_current.py'
                consumer_raw = consumer_path.read_bytes()
                self.assertEqual(hashlib.sha256(consumer_raw).hexdigest(),
                                 'f15b58cdb23562e572f4e558f068c5a4dab810ac442a486525db6f0764126355')
                consumer = load(consumer_path, consumer_raw,
                                __builtins__=dict(vars(builtins), __import__=scoped_import))
                if sys.flags.optimize:
                    with self.assertRaisesRegex(Exception, 'optimized Python is not supported'):
                        consumer.predecessor_capture(root, admitted, runtime._SOURCE_SEAL)
                else:
                    captured = consumer.predecessor_capture(root, admitted, runtime._SOURCE_SEAL)
                    self.assertEqual(captured['inputs'], restored)
                with p.view(root, restored, descriptor) as view:
                    for name in p._FEATURE_ROWS:
                        self.assertEqual((view / name).read_bytes(), baseline[name])
                    self.assertFalse((view / 'tests/stdlib_numbers.rs').exists())
                for name in p._FEATURE_ROWS:
                    self.assertEqual((root / name).read_bytes(), b'synthetic current feature edit\n')
                (root / '.dockerignore').write_bytes(b'changed unrelated historical input')
                with self.assertRaises(ValueError): p.capture(root, restored, descriptor)
        finally:
            for name in set(sys.modules) - set(modules):
                if getattr(sys.modules[name], '__file__', '').startswith(str(REPO)):
                    del sys.modules[name]
            for name in modules:
                if sys.modules.get(name) is not modules[name]: sys.modules[name] = modules[name]

if __name__ == '__main__': unittest.main()
