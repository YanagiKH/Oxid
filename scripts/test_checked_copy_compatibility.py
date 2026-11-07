"""Mutation controls for current Copy source admission, not language qualification."""
import ast
import copy
import json
from pathlib import Path
import os
import subprocess
import tempfile
import types
import unittest
from unittest.mock import patch

import verify_checked_copy_compatibility as gate

ROOT = Path(__file__).resolve().parents[1]
DISPATCH = 'tests/typed_project_dispatch.rs'


def git_body(commit, name):
    return subprocess.check_output(['git', 'show', commit + ':' + name], cwd=ROOT)


def names():
    body = (ROOT / gate.BINDING / 'current-source.json').read_bytes()
    if gate.sha(body) != gate.BASELINE_SOURCE_SHA:
        raise AssertionError('historical source manifest changed')
    return {row['path'] for row in json.loads(body)['files']}


class CheckedCopyAdmission(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory(prefix='oxid-copy-policy-')
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        self.baseline = {name: b'// fixed historical input\n' for name in names()}
        self.parent = {**self.baseline,
                       **{name: b'// fixed predecessor addition\n'
                          for name in gate.PREDECESSOR_ADDED}}
        self.parent[DISPATCH] = (ROOT / DISPATCH).read_bytes()
        self.current = dict(self.parent)
        self.current.update({name: b'// reviewed Copy input\n'
                             for name in gate.CHECKED_COPY_CHANGED | gate.CHECKED_COPY_ADDED})
        self.changed = sorted(gate.CHECKED_COPY_CHANGED)[0]
        self.current[self.changed] = b'#[test]\nfn first(){}\n#[test]\nfn second(){}\n'
        for name, body in self.current.items():
            self.put(name, body)
        pins = {name: (len(self.current[name]), gate.sha(self.current[name]))
                for name in gate.CHECKED_COPY_CHANGED | gate.CHECKED_COPY_ADDED}
        directives = {name: gate.source_directives(name, self.current[name]) for name in pins}
        for name, value in [('CHECKED_COPY_INPUTS', pins), ('REVIEWED_DIRECTIVES', directives)]:
            mocked = patch.object(gate, name, value)
            mocked.start()
            self.addCleanup(mocked.stop)

    def put(self, name, body):
        path = self.root / name
        path.parent.mkdir(parents=True, exist_ok=True)
        path.write_bytes(body)

    def capture(self):
        return gate.capture_sources(self.root, self.baseline, self.parent)

    def test_exact_495_parent_and_four_additions_are_admitted(self):
        result = self.capture()
        self.assertEqual(len(self.parent), 495)
        self.assertEqual(len(result), 499)
        self.assertEqual(set(result), set(self.parent) | gate.CHECKED_COPY_ADDED)
        for name in set(self.parent) - gate.CHECKED_COPY_CHANGED:
            self.assertEqual(result[name], self.parent[name])

    def test_manifest_keeps_every_separate_semantic_claim_false(self):
        result = gate.current_manifest(self.capture(), self.baseline,
                                       {'source_kind': 'worktree'}, self.parent)
        self.assertEqual(result['source_policy_input_members'], 49)
        self.assertEqual(result['predecessor_compiler_input_members'], 495)
        self.assertEqual({row['path'] for row in result['files']}, set(self.current))
        self.assertEqual(result['checked_copy_pins_provisional'], gate.PINS_PROVISIONAL)
        self.assertIs(result['checked_copy_records_semantics_qualified_by_this_gate'], False)
        for key, value in result.items():
            if key.endswith('qualified_by_this_gate'):
                self.assertIs(value, False, key)

    def test_changed_hash_must_match_the_reviewed_source(self):
        self.put(self.changed, self.current[self.changed] + b'// changed\n')
        with self.assertRaisesRegex(ValueError, 'reviewed input'):
            self.capture()

    def test_changed_length_must_match_the_reviewed_source(self):
        length, digest = gate.CHECKED_COPY_INPUTS[self.changed]
        gate.CHECKED_COPY_INPUTS[self.changed] = (length + 1, digest)
        with self.assertRaisesRegex(ValueError, 'reviewed input'):
            self.capture()

    def test_directive_membership_is_exact(self):
        del gate.REVIEWED_DIRECTIVES[self.changed]
        with self.assertRaisesRegex(ValueError, 'directive membership'):
            self.capture()

    def test_directive_order_and_repeat_count_cannot_be_rewritten(self):
        actual = gate.REVIEWED_DIRECTIVES[self.changed]
        inherited = gate.library.source_directives(self.changed, self.current[self.changed])
        expanded = tuple(value for value, count in actual['attributes'] for _ in range(count))
        self.assertEqual(expanded, inherited['attributes'])
        self.assertEqual(actual['attributes'], ((b'#[test]', 2),))
        for values in [((b'#[test]', 1),), ((b'#[test]', 3),),
                       ((b'#[ignore]', 1), (b'#[test]', 1)),
                       ((b'#[test]', 1), (b'#[ignore]', 1))]:
            gate.REVIEWED_DIRECTIVES[self.changed] = {**actual, 'attributes': values}
            with self.assertRaisesRegex(ValueError, 'directive changed'):
                self.capture()
        gate.REVIEWED_DIRECTIVES[self.changed] = actual

    def test_unchanged_input_remains_closed(self):
        name = sorted(set(self.parent) - gate.CHECKED_COPY_CHANGED - {DISPATCH})[0]
        self.put(name, self.parent[name] + b'// changed\n')
        with self.assertRaisesRegex(ValueError, 'outside checked-Copy boundary'):
            self.capture()

    def test_protected_dispatch_bytes_cannot_enter_the_reviewed_delta(self):
        self.put(DISPATCH, self.parent[DISPATCH] + b'// changed\n')
        with self.assertRaisesRegex(ValueError, 'protected retained input'):
            self.capture()
        with patch.object(gate, 'CHECKED_COPY_CHANGED', gate.CHECKED_COPY_CHANGED | {DISPATCH}):
            with self.assertRaisesRegex(ValueError, 'protected input cannot enter'):
                self.capture()

    def test_extra_source_member_is_refused(self):
        self.put('src/unreviewed.rs', b'// not reviewed\n')
        with self.assertRaisesRegex(ValueError, 'membership'):
            self.capture()

    def test_missing_source_member_is_refused(self):
        (self.root / sorted(gate.CHECKED_COPY_ADDED)[0]).unlink()
        with self.assertRaisesRegex(ValueError, 'membership'):
            self.capture()

    def test_empty_extra_source_directory_is_refused(self):
        (self.root / 'src/unreviewed').mkdir()
        with self.assertRaisesRegex(ValueError, 'directory'):
            self.capture()

    def test_symlink_source_is_refused(self):
        path = self.root / self.changed
        path.unlink()
        path.symlink_to(self.root / DISPATCH)
        with self.assertRaisesRegex(ValueError, 'symlink'):
            self.capture()

    def test_hardlinked_source_is_refused(self):
        path = self.root / self.changed
        alias = self.root / 'linked-source'
        os.link(path, alias)
        with self.assertRaisesRegex(ValueError, 'linked'):
            self.capture()

    def test_executable_source_is_refused(self):
        (self.root / self.changed).chmod(0o755)
        with self.assertRaisesRegex(ValueError, 'executable'):
            self.capture()

    def test_parent_membership_cannot_shrink(self):
        del self.parent[sorted(gate.PREDECESSOR_ADDED)[0]]
        with self.assertRaisesRegex(ValueError, 'predecessor compiler input membership'):
            self.capture()

    def test_reviewed_source_roster_cannot_grow_or_shrink(self):
        saved = gate.CHECKED_COPY_INPUTS.pop(self.changed)
        with self.assertRaisesRegex(ValueError, 'reviewed checked-Copy input membership'):
            self.capture()
        gate.CHECKED_COPY_INPUTS[self.changed] = saved
        gate.CHECKED_COPY_INPUTS['src/unlisted.rs'] = saved
        with self.assertRaisesRegex(ValueError, 'reviewed checked-Copy input membership'):
            self.capture()


class CheckedCopyFrozenReplay(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.adapter, cls.adapter_body = gate.authenticate_parser_resource_adapter(ROOT)
        retained_body = git_body(gate.BASELINE, gate.RETAINED)
        if gate.sha(retained_body) != gate.RETAINED_RUNNER_SHA:
            raise AssertionError('retained replay predicates changed')
        retained = types.ModuleType('_checked_copy_retained_control')
        retained.__file__ = str(ROOT / gate.RETAINED)
        exec(compile(retained_body, retained.__file__, 'exec'), retained.__dict__)
        raw = git_body(gate.BASELINE, retained.U2 + '/' + retained.RESOURCE)
        stdin = retained.adapt_stdin_parser_resource(raw.replace(retained.OLD_SEAM, retained.ENUM_RESOURCE_SEAM, 1))
        cls.prior = gate.library.adapt_parser_resource(gate.library.library.adapt_parser_resource(stdin))

    def test_all_49_predecessor_policy_bytes_are_immutable(self):
        self.assertEqual(len(gate.PREDECESSOR_POLICIES), 49)
        for name, digest in gate.PREDECESSOR_POLICIES.items():
            before = git_body(gate.PREDECESSOR, name)
            self.assertEqual(gate.sha(before), digest, name)
            self.assertEqual(gate.regular(ROOT, name), before, name)

    def test_only_one_parser_initializer_changes_and_roundtrips(self):
        derived = self.adapter.adapt(self.prior)
        self.assertEqual(derived.count(self.adapter.AFTER), 1)
        self.assertEqual(self.adapter.reverse(derived), self.prior)
        self.assertEqual(derived.replace(self.adapter.AFTER, self.adapter.BEFORE, 1), self.prior)
        descriptor = self.adapter.descriptor()
        self.assertIs(descriptor['checked_copy_semantics_qualified'], False)
        self.assertEqual(descriptor['preserved'], ['all assertions', 'all resource formulas', 'all test names'])

    def test_parser_prefix_suffix_repetition_and_reverse_tampering_are_refused(self):
        derived = self.adapter.adapt(self.prior)
        for bad in [b'x' + self.prior, self.prior + b'x',
                    self.prior.replace(self.adapter.BEFORE, self.adapter.BEFORE * 2, 1), derived]:
            with self.assertRaises(ValueError):
                self.adapter.adapt(bad)
        for bad in [b'x' + derived, derived + b'x',
                    derived.replace(self.adapter.AFTER, self.adapter.AFTER + b' ', 1)]:
            with self.assertRaises(ValueError):
                self.adapter.reverse(bad)

    def test_entire_qualified_replay_block_matches_the_predecessor_ast(self):
        def replay(path):
            tree = ast.parse(path.read_bytes())
            run = next(node for node in tree.body if isinstance(node, ast.FunctionDef) and node.name == 'run')
            return next(node for node in ast.walk(run)
                        if isinstance(node, ast.If) and ast.unparse(node.test) == 'not args.prepare_only')
        current = replay(ROOT / 'scripts/verify_checked_copy_compatibility.py')
        predecessor = replay(ROOT / gate.PREDECESSOR_POLICY)
        self.assertEqual(ast.dump(current, include_attributes=False), ast.dump(predecessor, include_attributes=False))
        self.assertGreater(len(current.body), 10)


if __name__ == '__main__':
    unittest.main()
