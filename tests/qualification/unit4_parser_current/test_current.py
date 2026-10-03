#!/usr/bin/env python3
"""Bounded current-source admission controls, never semantic qualification."""
import ast
import copy
import hashlib
import json
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch

import portable as p


class CurrentAuthorityControls(unittest.TestCase):
    def test_historical_and_current_authorities_remain_distinct(self):
        a = p.authority()
        historical = p.read(p.FROZEN / 'authority.json')
        self.assertEqual({k: v for k, v in a.items() if k not in ('current', 'current_source')}, historical)
        self.assertEqual([len(a[k]) for k in ('original_files', 'derived_files', 'control_derived_files')], [283, 286, 286])
        self.assertEqual([len(a['current'][k]) for k in ('current_base_files', 'current_derived_files', 'current_control_derived_files')], [284, 287, 287])
        self.assertEqual(len(p.compiler_map(a)), 114)
        self.assertNotEqual(a['candidate_source_manifest_sha256'], a['current']['current_candidate_source_manifest_sha256'])
        self.assertEqual([r['path'] for r in a['current']['source_delta']], [
            'src/frontend/oir/owned/mod.rs', 'src/frontend/oir/owned/plan.rs',
            'src/frontend/oir/owned_types.rs', 'src/frontend/oir/owned_types/array_tests.rs'])
        self.assertEqual(sum(r['before'] is None for r in a['current']['source_delta']), 1)

    def test_copied_algorithms_have_only_reviewed_change_boundaries(self):
        old_text = (p.FROZEN / 'portable.py').read_text()
        new_text = Path(p.__file__).read_text()
        functions = lambda text: {n.name: ast.get_source_segment(text, n) for n in ast.parse(text).body if isinstance(n, ast.FunctionDef)}
        old, new = functions(old_text), functions(new_text)
        allowed = {'authority', 'compiler_map', 'verify_checkout', 'prepare', 'verify_overlay',
                   'session_at', 'verify_cargo', 'comparator', 'effective_authority', 'main'}
        self.assertEqual(set(new) - set(old), {'current_candidate', 'current_overlay', 'verify_transition_records', 'verify_historical_overlay'})
        self.assertEqual(set(old) - set(new), set())
        for name in set(old) - allowed:
            with self.subTest(function=name): self.assertEqual(new[name], old[name])
        self.assertEqual(p.comparator()[1], p.read(p.FROZEN / 'derivation-proof.json'))
        self.assertEqual(p.sha((p.FROZEN / 'frozen/comparator/comparator.py').read_bytes()), p.COMPARATOR_SHA)

    def test_current_maps_cannot_be_replaced_with_historical_maps(self):
        a = p.authority()
        self.assertNotEqual(a['current']['current_base_files'], a['original_files'])
        for role in ('derived_files', 'control_derived_files'):
            self.assertNotEqual(a['current']['current_' + role], a[role])
            for path in p.CURRENT_PATHS:
                self.assertEqual(next(r for r in a['current']['current_' + role] if r['path'] == path),
                                 next(r['after'] for r in a['current']['source_delta'] if r['path'] == path))


class CheckoutControls(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix='oxid-current-parser-')
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name).resolve()
        self.a = p.authority()
        for row in [*self.a['current_source']['files'], self.a['current']['current_source_manifest']]:
            dest = self.root / row['path']; dest.parent.mkdir(parents=True, exist_ok=True)
            dest.write_bytes((p.REPOSITORY / row['path']).read_bytes())
        self.git('init', '-q')
        self.git('add', '.')
        self.git('commit', '-qm', 'Exact current admission fixture')

    def git(self, *args):
        return subprocess.check_output(['/usr/bin/git', '-c', 'user.name=Codex', '-c', 'user.email=codex@local.invalid',
                                        '-c', 'core.autocrlf=false', '-C', str(self.root), *args], stderr=subprocess.STDOUT)

    def rejects_before_git_or_child(self, message):
        with patch.object(p, 'git', side_effect=AssertionError('must reject before Git/tool use')):
            with self.assertRaisesRegex(p.Rejected, message): p.verify_checkout(self.root, self.a)

    def test_exact_current_bodies_and_git_are_admitted(self):
        bound = p.verify_checkout(self.root, self.a)
        self.assertEqual(len(bound['compiler_files']), 114)
        self.assertIs(bound['historical_source_equivalent'], False)
        self.assertIs(bound['current_source_bound'], True)
        self.assertEqual(bound['head'], self.git('rev-parse', 'HEAD').decode().strip())

    def test_changed_groundwork_rejects_before_git_or_child(self):
        path = self.root / p.CURRENT_PATHS[0]; path.write_bytes(path.read_bytes() + b'// changed\n')
        self.rejects_before_git_or_child('file bytes differ')

    def test_missing_new_member_rejects_before_git_or_child(self):
        (self.root / p.CURRENT_PATHS[-1]).unlink()
        self.rejects_before_git_or_child('missing regular file')

    def test_extra_compiler_member_rejects_before_git_or_child(self):
        (self.root / 'src/unapproved.rs').write_bytes(b'// not admitted\n')
        self.rejects_before_git_or_child('checkout complete current compiler roster')

    def test_coherently_rehashed_current_manifest_rejects_before_git_or_child(self):
        name = p.CURRENT_PATHS[-1]; path = self.root / name; path.write_bytes(path.read_bytes() + b'// coherent\n')
        manifest_path = self.root / self.a['current']['current_source_manifest']['path']
        value = p.read(manifest_path)
        row = next(r for r in value['files'] if r['path'] == name)
        row.update(bytes=path.stat().st_size, sha256=hashlib.sha256(path.read_bytes()).hexdigest())
        p.write(manifest_path, value)
        self.rejects_before_git_or_child('file bytes differ')

    def test_old_historical_body_cannot_substitute(self):
        name = p.CURRENT_PATHS[0]
        raw = p.git(p.REPOSITORY, 'show', self.a['base_commit'] + ':' + name)
        (self.root / name).write_bytes(raw)
        self.rejects_before_git_or_child('file bytes differ')

    def test_current_worktree_cannot_hide_wrong_committed_body(self):
        path = self.root / p.CURRENT_PATHS[0]; good = path.read_bytes()
        path.write_bytes(good + b'// committed wrong body\n')
        self.git('add', '.'); self.git('commit', '-qm', 'Wrong fixture Git body')
        path.write_bytes(good)
        with self.assertRaisesRegex(p.Rejected, 'checkout Git current input bytes'):
            p.verify_checkout(self.root, self.a)


if __name__ == '__main__':
    unittest.main(verbosity=2)
