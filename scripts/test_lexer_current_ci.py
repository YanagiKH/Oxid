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
CURRENT_SHA = 'aa021e6046786300d13b12c22e5cff3f2565b1ae8f739e0698bdad93fd33a633'
PREDECESSOR_SHA = '402db5018af489c30b2a57ed3ef558c055013af2b727a3ad0eb39ffc42125efa'
COMPILER_HEAD = 'c8e9a72afd9866f32b96f98ae24f61390039f421'
COMPILER_TREE = '9b35515f096b5619d4d5b3d4b0cb88ea2ccf2c37'
PRESERVATION_SHA = '38e329c1897f5c902dbfb73c6d3fbfecae2a97cb2f1a85e916b56c2a126698fc'
sys.path.insert(0, str(HELPER))
import ci_inventory


def sha(raw):
    return hashlib.sha256(raw).hexdigest()


def read(path):
    return json.loads(path.read_bytes())


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


if __name__ == '__main__':
    unittest.main()
