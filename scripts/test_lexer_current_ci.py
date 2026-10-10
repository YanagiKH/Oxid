"""Source-only current CI activation controls; never execute Rust or a compiler."""
import ast
import copy
import hashlib
import importlib.util
import json
from pathlib import Path
import sys
import unittest

sys.dont_write_bytecode = True
REPO = Path(__file__).resolve().parents[1]
HELPER = REPO / 'tests/qualification/lexer_reservation_current'
PRESERVED = REPO / 'scripts/predecessors/byte_storage_ci_v1'
SOURCE = REPO / 'tests/fixtures/typed_project_source_binding'
LEXER_PRESERVED = REPO / 'scripts/predecessors/lexer_reservation_source_v1'
LEXER_PRESERVATION_SHA = '8d1ef7d92988d3f28de359231b291f34d069c00010dddedf58bf1116b765a8be'
CURRENT_SHA = '9432c61fc4f63b760e5f55599aedb24067a206e40e8b44b0911392c00cda7261'
PREDECESSOR_SHA = '402db5018af489c30b2a57ed3ef558c055013af2b727a3ad0eb39ffc42125efa'
COMPILER_HEAD = 'b3abc9f0dda99d6d8fe65d9c3a9ed31dedcbd489'
COMPILER_TREE = '7f5c9aa08c569c4d0b5a27391d8fc68337075d36'
PRESERVATION_SHA = '38e329c1897f5c902dbfb73c6d3fbfecae2a97cb2f1a85e916b56c2a126698fc'
sys.path.insert(0, str(HELPER))
import ci_inventory


def sha(raw):
    return hashlib.sha256(raw).hexdigest()


def read(path):
    return json.loads(path.read_bytes())



def verify_lexer_preservation_view(raw, bodies, directories, unsafe):
    """Authenticate inert predecessor data only; never import or execute it."""
    def need(condition, message):
        if not condition:
            raise ValueError(message)
    need(sha(raw) == LEXER_PRESERVATION_SHA, 'lexer preservation manifest pin')
    value = json.loads(raw)
    need(set(value) == {'schema', 'selection_head', 'files'} and
         value['schema'] == 'oxid-lexer-source-v1-facade-preservation-v1' and
         value['selection_head'] == '59f948e8919b31ee4dcfce63153eab7d03bc6f37',
         'lexer preservation closed schema/provenance')
    rows = value['files']
    names = [row['path'] for row in rows]
    need(len(rows) == 38 and names == sorted(set(names)), 'lexer preservation ordered38')
    fields = {'path', 'preserved_path', 'bytes', 'sha256', 'mode', 'git_blob',
              'origin_commit', 'origin_tree', 'origin_role', 'replacement_reason'}
    origins = {
        ('4f5e693688a6c5b307db36dc98bd742b60be7ae5', '01a4842b27d6742b9d2012ca2a0e36b3db5c96df', 'published-source-v1'): 27,
        ('241edce73d1d23c0c50d4c37ae46526455873a56', '28db0e7cca219082eb3a632bd84adff20d5704c4', 'accepted-receipt-only-successor'): 10,
        ('59f948e8919b31ee4dcfce63153eab7d03bc6f37', 'b0c48ee48dd60be8fdbb82d61a22b27ad4188857', 'accepted-post-receipt-maintenance'): 1}
    seen = {key: 0 for key in origins}
    expected_files, expected_directories = {'preservation.json'}, set()
    need(not unsafe, 'unsafe lexer preservation member')
    for row in rows:
        name = row['path']
        need(set(row) == fields and name and not name.startswith('/') and
             '\\' not in name and all(part not in ('', '.', '..') for part in name.split('/')),
             'lexer preservation exact member fields/path')
        need(row['preserved_path'] == 'scripts/predecessors/lexer_reservation_source_v1/' + name and
             row['mode'] == '100644' and bool(row['replacement_reason']), 'lexer preservation member provenance')
        origin = (row['origin_commit'], row['origin_tree'], row['origin_role'])
        need(origin in origins, 'lexer preservation exact origin'); seen[origin] += 1
        need(name in bodies, 'missing lexer preservation body')
        body, mode = bodies[name]
        need(mode == 0o644 and len(body) == row['bytes'] and sha(body) == row['sha256'] and
             hashlib.sha1(b'blob ' + str(len(body)).encode() + b'\0' + body).hexdigest() == row['git_blob'],
             'changed lexer preservation body/mode/blob')
        expected_files.add(name)
        expected_directories.update('/'.join(name.split('/')[:i]) for i in range(1, len(name.split('/'))))
    need(seen == origins, 'lexer preservation origin counts')
    need(set(bodies) == expected_files and set(directories) == expected_directories,
         'lexer preservation exact file/directory closure')
    need(bodies['preservation.json'] == (raw, 0o644), 'lexer preservation manifest body/mode')
    return value


def lexer_preservation_view():
    root = LEXER_PRESERVED
    bodies, directories, unsafe = {}, set(), []
    if not root.is_dir() or any(path.is_symlink() for path in (root, *root.parents)):
        raise ValueError('unsafe lexer preservation root')
    manifest = root / 'preservation.json'
    if manifest.is_symlink() or not manifest.is_file():
        raise ValueError('unsafe lexer preservation manifest')
    for path in root.rglob('*'):
        name = path.relative_to(root).as_posix()
        if path.is_symlink():
            unsafe.append(name)
        elif path.is_file():
            bodies[name] = (path.read_bytes(), path.stat().st_mode & 0o777)
        elif path.is_dir():
            directories.add(name)
        else:
            unsafe.append(name)
    if 'preservation.json' not in bodies:
        raise ValueError('unsafe lexer preservation manifest')
    return bodies['preservation.json'][0], bodies, directories, unsafe

class CurrentCIActivationControls(unittest.TestCase):
    def test_all_173_direct_entries_have_exact_individual_dispositions(self):
        original = ci_inventory.inventory(
            (REPO / 'docs/architecture/fallible-lexer-source-consumer-audit.md').read_bytes(),
            (PRESERVED / '.github/workflows/ci.yml').read_bytes())
        plan = read(HELPER / 'direct-consumer-dispositions.json')
        self.assertTrue(ci_inventory.validate_plan(original, plan['entries']))
        self.assertEqual(plan['entry_count'], 173)
        self.assertEqual(plan['current_source_manifest_sha256'], CURRENT_SHA)
        self.assertEqual((plan['compiler_head'], plan['compiler_tree']), (COMPILER_HEAD, COMPILER_TREE))
        self.assertEqual(sum(plan['disposition_counts'].values()), 173)
        for row, approved in zip(plan['entries'], original):
            self.assertEqual({key: row[key] for key in approved}, approved)
            self.assertEqual(row['active_command_sha256'], sha(row['active_command'].encode()))
            self.assertIn(row['execution_status'], ('not-run-source-only', 'blocked-private-interface'))

    def test_missing_duplicate_reordered_and_waived_dispositions_reject(self):
        original = ci_inventory.inventory(
            (REPO / 'docs/architecture/fallible-lexer-source-consumer-audit.md').read_bytes(),
            (PRESERVED / '.github/workflows/ci.yml').read_bytes())
        plan = read(HELPER / 'direct-consumer-dispositions.json')['entries']
        for mutation in ('missing', 'duplicate', 'reordered', 'command', 'waived', 'empty-rationale'):
            changed = copy.deepcopy(plan)
            if mutation == 'missing': changed.pop()
            elif mutation == 'duplicate': changed[-1] = changed[0]
            elif mutation == 'reordered': changed.reverse()
            elif mutation == 'command': changed[0]['command_sha256'] = '0' * 64
            elif mutation == 'waived': changed[0]['disposition'] = 'waived'
            else: changed[0]['rationale'] = ''
            with self.subTest(mutation=mutation), self.assertRaises(ValueError):
                ci_inventory.validate_plan(original, changed)

    def test_workflow_has_only_three_approved_active_substitutions(self):
        raw = (PRESERVED / '.github/workflows/ci.yml').read_bytes()
        self.assertEqual(sha(raw), ci_inventory.WORKFLOW_SHA)
        self.assertEqual(raw.count(PREDECESSOR_SHA.encode()), 2)
        old = b'tests/fixtures/typed_project_unit1_independent/run.py --repo . --profile both'
        new = b'tests/qualification/lexer_reservation_current/unit1_run_current.py --repo . --profile both'
        self.assertEqual(raw.count(old), 1)
        expected = raw.replace(PREDECESSOR_SHA.encode(), CURRENT_SHA.encode()).replace(old, new)
        self.assertEqual((REPO / '.github/workflows/ci.yml').read_bytes(), expected)

    def test_preserved_consumer_bodies_are_unchanged(self):
        self.assertEqual(sha((PRESERVED / 'preservation.json').read_bytes()), PRESERVATION_SHA)
        preservation = read(PRESERVED / 'preservation.json')
        self.assertEqual(len(preservation['files']), 19)
        names = [row['path'] for row in preservation['files']]
        self.assertEqual(names, sorted(set(names)))
        for row in preservation['files']:
            raw = (REPO / row['preserved_path']).read_bytes()
            self.assertEqual((len(raw), sha(raw)), (row['bytes'], row['sha256']), row['path'])

    def test_all_original_source_binding_case_bodies_remain_exact(self):
        name = 'scripts/test_typed_project_source_binding.py'
        old = (PRESERVED / name).read_text()
        new = (REPO / name).read_text()
        def methods(source):
            tree = ast.parse(source)
            cls = next(node for node in tree.body if isinstance(node, ast.ClassDef)
                       and node.name == 'SourceBindingTests')
            return {node.name: ast.get_source_segment(source, node) for node in cls.body
                    if isinstance(node, ast.FunctionDef)}
        before, after = methods(old), methods(new)
        self.assertEqual(list(before), list(after))
        for name in before:
            if name != 'setUpClass':
                self.assertEqual(after[name], before[name], name)
        self.assertIn('current["byte_storage_inputs"]', after['setUpClass'])
        self.assertIn('binding.preflight(restored, PACKAGE)', after['setUpClass'])

    def test_native_entrypoints_and_complete_source_counts_agree(self):
        import verify_bounded_byte_storage as byte
        import verify_bounded_enum_native as enum
        import verify_bounded_stdin_native as stdin
        import verify_bounded_stdout_native as stdout
        self.assertEqual((byte.SOURCE_SHA256, enum.REVIEWED_SOURCE_SHA256,
                          stdin.REVIEWED_SOURCE_SHA256, stdout.REVIEWED_SOURCE_SHA256), (CURRENT_SHA,) * 4)
        manifest = enum.read_reviewed_manifest(SOURCE / 'current-source.json')
        self.assertEqual(manifest, stdin.source_manifest(REPO, CURRENT_SHA))
        self.assertEqual(len(manifest['files']), stdin.REVIEWED_SOURCE_MEMBERS)
        compiler = [row for row in manifest['files'] if row['path'].startswith(('src/', 'native/'))
                    or row['path'] in ('Cargo.toml', 'Cargo.lock', 'build.rs')]
        self.assertEqual(len(compiler), stdin.REVIEWED_COMPILER_BODIES)
        self.assertEqual((manifest['reviewed_source_head'], manifest['source_only_tree']),
                         (COMPILER_HEAD, COMPILER_TREE))
        predecessor = (SOURCE / 'byte-storage-source-v1.json').read_bytes()
        self.assertEqual(sha(predecessor), PREDECESSOR_SHA)
        self.assertEqual(manifest['lexer_reservation_predecessor_sha256'], PREDECESSOR_SHA)
        with self.assertRaises(ValueError):
            enum.read_reviewed_manifest(SOURCE / 'byte-storage-source-v1.json')
        with self.assertRaises(ValueError):
            stdin.source_manifest(REPO, PREDECESSOR_SHA)

    def test_native_source_snapshot_contains_all_current_selected_inputs(self):
        import verify_owned_source_native as native
        import verify_bounded_enum_native as enum
        manifest = enum.read_reviewed_manifest(SOURCE / 'current-source.json')
        snapshot = native.source_inventory(REPO, manifest)
        for row in manifest['files']:
            self.assertEqual(snapshot[row['path']], row['sha256'], row['path'])
        self.assertEqual(snapshot['tests/fixtures/typed_project_source_binding/current-source.json'], CURRENT_SHA)

    def test_native_producer_closes_current_and_retained_adapter_roots(self):
        import verify_owned_array_native as producer
        roots = ('src', 'native', 'compiler', 'stdlib', 'rfcs', '.cargo',
                 'tests/fixtures/typed_project_source_binding',
                 'tests/fixtures/fixed_array_unit2d_independent',
                 'tests/qualification/lexer_reservation_current',
                 'tests/fixtures/typed_project_source_binding_byte_storage_v1',
                 'tests/qualification/unit2_u8_current',
                 'tests/fixtures/typed_project_unit2_independent',
                 'fixtures/typed-streaming-lexer')
        # Source-only membership fixture, not a committed native-build receipt.
        tracked = {path.relative_to(REPO).as_posix(): {} for name in roots
                   for path in (REPO / name).rglob('*') if path.is_file()}
        manifest, names = producer.source_members(REPO, tracked)
        self.assertEqual(sha((SOURCE / 'current-source.json').read_bytes()), CURRENT_SHA)
        self.assertTrue({row['path'] for row in manifest['files']} <= set(names))
        for root in roots[-5:]:
            self.assertTrue({p.relative_to(REPO).as_posix() for p in (REPO / root).rglob('*') if p.is_file()} <= set(names))


    def test_exact_38_lexer_facade_preservation_is_inert_and_complete(self):
        raw, bodies, directories, unsafe = lexer_preservation_view()
        value = verify_lexer_preservation_view(raw, bodies, directories, unsafe)
        self.assertEqual(sum(row['bytes'] for row in value['files']), 2010708)
        old_workflow = bodies['.github/workflows/ci.yml'][0]
        old_sha = b'aa021e6046786300d13b12c22e5cff3f2565b1ae8f739e0698bdad93fd33a633'
        self.assertEqual(old_workflow.count(old_sha), 2)
        self.assertEqual((REPO / '.github/workflows/ci.yml').read_bytes(),
                         old_workflow.replace(old_sha, CURRENT_SHA.encode()))

    def test_lexer_facade_manifest_body_mode_membership_and_symlink_reject(self):
        raw, bodies, directories, unsafe = lexer_preservation_view()
        first = json.loads(raw)['files'][0]['path']
        for mutation in ('manifest', 'missing', 'extra', 'body', 'mode', 'manifest-mode', 'empty-directory', 'symlink'):
            changed, dirs, bad = dict(bodies), set(directories), list(unsafe)
            candidate = raw
            if mutation == 'manifest': candidate += b'\n'
            elif mutation == 'missing': del changed[first]
            elif mutation == 'extra': changed['extra'] = (b'extra', 0o644)
            elif mutation == 'body': changed[first] = (changed[first][0] + b'\n', 0o644)
            elif mutation == 'mode': changed[first] = (changed[first][0], 0o755)
            elif mutation == 'manifest-mode': changed['preservation.json'] = (raw, 0o755)
            elif mutation == 'empty-directory': dirs.add('unexpected-empty-directory')
            else: bad.append(first)
            with self.subTest(mutation=mutation), self.assertRaises(ValueError):
                verify_lexer_preservation_view(candidate, changed, dirs, bad)


    def test_lexer_facade_reader_rejects_external_manifest_symlink_before_open(self):
        from unittest import mock
        for symlink in (True, False):
            root = mock.MagicMock(spec=Path)
            root.is_dir.return_value = True
            root.is_symlink.return_value = False
            root.parents = ()
            manifest = mock.MagicMock(spec=Path)
            manifest.is_symlink.return_value = symlink
            manifest.is_file.return_value = symlink
            root.__truediv__.return_value = manifest
            with self.subTest(symlink=symlink), mock.patch.dict(globals(), {'LEXER_PRESERVED': root}), mock.patch.object(Path, 'read_bytes') as reads:
                with self.assertRaisesRegex(ValueError, 'unsafe lexer preservation manifest'):
                    lexer_preservation_view()
                reads.assert_not_called()
                manifest.read_bytes.assert_not_called()
                root.rglob.assert_not_called()


if __name__ == '__main__':
    unittest.main()
