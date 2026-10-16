"""Fixed trim storage controls. No compiler or qualification label changes."""
import ast
import copy
import importlib.util
import json
import os
from pathlib import Path
import sys
import tempfile
import unittest

sys.dont_write_bytecode = True
HERE = Path(__file__).absolute().parent
REPO = HERE.parents[2]
sys.path.insert(0, str(REPO / 'tests/qualification/package_json_current_v1'))
import generation as g


def load(name, path):
    spec = importlib.util.spec_from_file_location(name, path)
    value = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(value)
    return value


d = load('trim_storage_derivation', HERE / 'derive.py')
p = load('trim_storage_predecessor', HERE / 'predecessor.py')


def identities(bodies):
    return g.encoded([g.identity(n, raw) for n, raw in sorted(bodies.items())])


class StorageControls(unittest.TestCase):
    def test_exact_repeatable_derivation_and_retained_methods(self):
        original = g.regular(REPO, d.PREVIOUS)
        first, second = d.derive(original), d.derive(original)
        self.assertEqual(first, second)
        self.assertEqual(first, g.regular(REPO, d.ROOT + 'predecessor.py'))
        def methods(raw):
            text = raw.decode()
            return {n.name: ast.get_source_segment(text, n) for n in ast.parse(text).body
                    if isinstance(n, ast.FunctionDef)}
        old, new = methods(original), methods(first)
        self.assertEqual({n for n in old if old[n] != new[n]}, {'capture'})
        for wrong in [original + b'\n', original.replace(b'limit=row[', b'limit=1+row[')]:
            with self.assertRaises(ValueError):
                d.derive(wrong)

    def test_archive_modes_links_and_missing_rejected_without_fallback(self):
        body = g.regular(REPO, p._ARCHIVE)
        for mutation in ['missing', 'wrong', 'oversize', 'symlink', 'hardlink', 'executable', 'directory', 'parent-symlink']:
            with self.subTest(mutation=mutation), tempfile.TemporaryDirectory() as directory:
                root = Path(directory)
                target = root / p._ARCHIVE
                target.parent.mkdir(parents=True)
                real = root / 'real'; real.write_bytes(body)
                # A valid original at the live path must never rescue the archive.
                live = root / p._STRINGS; live.parent.mkdir(); live.write_bytes(body)
                if mutation == 'wrong': target.write_bytes(body.replace(b'return text;', b'return null;', 1))
                elif mutation == 'oversize': target.write_bytes(body + b'\n')
                elif mutation == 'symlink': target.symlink_to(real)
                elif mutation == 'hardlink': os.link(real, target)
                elif mutation == 'executable': target.write_bytes(body); target.chmod(0o755)
                elif mutation == 'directory': target.mkdir()
                elif mutation == 'parent-symlink':
                    target.parent.rmdir()
                    (root / 'elsewhere').mkdir()
                    (root / 'elsewhere' / 'strings.ox').write_bytes(body)
                    target.parent.symlink_to(root / 'elsewhere', target_is_directory=True)
                with self.assertRaises((ValueError, OSError)):
                    p._strings_archive(root, copy.deepcopy(p._STRINGS_ROW))

    def test_exact_row_and_coherent_archive_rehash_rejected(self):
        row = copy.deepcopy(p._STRINGS_ROW)
        for key, value in [('path', 'other.ox'), ('read_from', p._ARCHIVE),
                           ('bytes', 2345.0), ('bytes', True), ('mode', '100755'),
                           ('git_blob', '0' * 40), ('sha256', '0' * 64), ('extra', 0)]:
            changed = {**row, key: value}
            with self.subTest(field=key, value=value), self.assertRaises(ValueError):
                p._strings_archive(REPO, changed)
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            target = root / p._ARCHIVE; target.parent.mkdir(parents=True)
            raw = g.regular(REPO, p._ARCHIVE) + b'\n'; target.write_bytes(raw)
            changed = {**row, **g.identity(p._STRINGS, raw), 'git_blob': g.gitid('blob', raw)}
            with self.assertRaises(ValueError): p._strings_archive(root, changed)

    def test_descriptor_mapping_and_coherent_caller_seal_rejected(self):
        original = g.regular(REPO, p.VIEW_NAME)
        for mutation in ['whitespace', 'mapping', 'rehash']:
            with self.subTest(mutation=mutation), tempfile.TemporaryDirectory() as directory:
                root = Path(directory); target = root / p.VIEW_NAME
                target.parent.mkdir(parents=True)
                value = json.loads(original)
                row = next(r for r in value['files'] if r['path'] == p._STRINGS)
                if mutation == 'mapping': row['read_from'] = p._ARCHIVE
                if mutation == 'rehash': row['sha256'] = '0' * 64; row['git_blob'] = '0' * 40
                raw = original + b'\n' if mutation == 'whitespace' else g.encoded(value)
                target.write_bytes(raw)
                with self.assertRaisesRegex(ValueError, 'immutable accepted-view descriptor'):
                    p.capture(root, {}, g.identity(p.VIEW_NAME, raw))
        with self.assertRaisesRegex(ValueError, 'strings cannot be restored source'):
            p.capture(REPO, {p._STRINGS: g.regular(REPO, p._ARCHIVE)},
                      g.identity(p.VIEW_NAME, original))

    def test_complete_source_inverse_and_all_view_bodies(self):
        previous = g.regular(REPO, g.PACKAGE + g.PREDECESSOR_NAME)
        inputs = {r['path']: g.regular(REPO, r['path']) for r in json.loads(previous)['files']}
        outputs, restored = g.generate(previous, inputs,
            g.regular(REPO, g.ROOT + 'generated/' + g.PATCH_NAME),
            {n: g.regular(REPO, g.ROOT + 'source-proof/' + n + '.json') for n in g.READBACKS},
            g.regular(REPO, 'tests/fixtures/typed_project_source_binding_byte_storage_v1/run.py'))
        self.assertEqual((len(inputs), len(restored)), (376, 376))
        self.assertEqual(sum(inputs[n] == restored[n] for n in inputs), 374)
        self.assertNotIn(p._STRINGS, inputs)
        # Fixed independently captured accepted-baseline identities (all bodies).
        self.assertEqual(g.sha(identities(inputs)), 'ce9f01cd96b21a32f067e6f8aed5145b34d716fe3ccd85de3fca5bb08279d572')
        self.assertEqual(g.sha(identities(restored)), 'a82325e57eb4792e623ad3f5e4e80a612ed3370cfeff2c9345f9ae5436767438')
        for name, body in outputs.items():
            self.assertEqual(body, g.regular(REPO, g.ROOT + 'generated/' + name))
        descriptor = json.loads(g.regular(REPO, g.ROOT + 'source-seal-v1.json'))['predecessor_view']
        bodies = p.capture(REPO, restored, descriptor)
        self.assertEqual((len(bodies), sum(map(len, bodies.values()))), (2724, 78507317))
        self.assertEqual(g.sha(identities(bodies)), 'e70b8b790756e0fb9e0cd1341779c51646b964afcf9e2eade2bd9480716b1882')
        self.assertEqual(bodies[p._STRINGS], g.regular(REPO, p._ARCHIVE))
        self.assertNotEqual(bodies[p._STRINGS], g.regular(REPO, p._STRINGS))
        with p.view(REPO, restored, descriptor) as view:
            self.assertEqual(g.regular(view, p._STRINGS), bodies[p._STRINGS])
            runtime = p.load(view, g.PACKAGE + 'run.py', 'trim_historical_runtime')
            captured = runtime.preflight(view, view / g.PACKAGE.rstrip('/'))
            self.assertEqual(captured['inputs'], restored)
        self.assertFalse(view.exists())


if __name__ == '__main__':
    unittest.main()
