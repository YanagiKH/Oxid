#!/usr/bin/env python3
"""Retained reviewer counterexamples, adapted only to the v2 execution API."""
import contextlib
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest

import portable as p


@contextlib.contextmanager
def poison(values):
    old = os.environ.copy()
    try:
        os.environ.update(values)
        yield
    finally:
        os.environ.clear(); os.environ.update(old)


class ReviewRegressions(unittest.TestCase):
    def test_git_overrides_both_directions_and_false_index_objects_config(self):
        a = p.authority()
        historical = Path(os.environ['UNIT4_PORTABLE_HISTORICAL_REPO'])
        current = Path(os.environ['UNIT4_PORTABLE_CHECKOUT'])
        with tempfile.TemporaryDirectory(prefix='oxid-git-poison-') as temp:
            root = Path(temp)
            shadow = root / 'git'; shadow.write_text('#!/bin/sh\nprintf FORGED_GIT_HEAD\\n\n'); shadow.chmod(0o755)
            for target, other in ((current, historical), (historical, current)):
                baseline = p.verify_checkout(target, a)
                values = {'GIT_DIR': p.git(other, 'rev-parse', '--absolute-git-dir').decode().strip(),
                          'GIT_WORK_TREE': str(other), 'GIT_COMMON_DIR': '/nonexistent/foreign-common-dir',
                          'GIT_INDEX_FILE': '/nonexistent/foreign-index', 'GIT_OBJECT_DIRECTORY': '/nonexistent/foreign-objects',
                          'GIT_ALTERNATE_OBJECT_DIRECTORIES': '/nonexistent/foreign-alternates', 'GIT_CONFIG': '/nonexistent/foreign-config',
                          'GIT_CONFIG_SYSTEM': '/nonexistent/system-config', 'GIT_CONFIG_GLOBAL': '/nonexistent/global-config',
                          'GIT_CONFIG_COUNT': '1', 'GIT_CONFIG_KEY_0': 'core.worktree', 'GIT_CONFIG_VALUE_0': str(other),
                          'GIT_CONFIG_PARAMETERS': 'malformed', 'GIT_REPLACE_REF_BASE': 'refs/foreign', 'PATH': str(root)}
                with self.subTest(target=target), poison(values):
                    self.assertEqual(p.verify_checkout(target, a), baseline)
            with self.assertRaises(p.Rejected): p.git(current / 'src', 'rev-parse', 'HEAD')

    def test_unselected_which_and_assembler_cannot_enter_lookup(self):
        a = p.authority(); original = Path(os.environ['UNIT4_PORTABLE_TOOLCHAIN'])
        # Clone selected files using hard links on the compiler filesystem. Never
        # alter a selected file; only newly named unselected scripts are written.
        with tempfile.TemporaryDirectory(prefix='oxid-review-toolchain-', dir=original.parent) as temp:
            root = Path(temp); clone = root / 'toolchain'; clone.mkdir()
            for row in a['compiler_files']:
                path = clone / row['path']; path.parent.mkdir(parents=True, exist_ok=True)
                os.link(original / row['path'], path)
            for name in ('which', 'as'):
                script = clone / 'bin' / name
                script.write_text('#!/bin/sh\nprintf "UNPINNED_' + name.upper() + '_EXECUTED\\n"\n'); script.chmod(0o755)
            p.verify_toolchain(clone, a)
            with self.assertRaises(p.Rejected): p.minimal_env(root, root / 'cache', clone)
            view = p.create_executable_view(root / 'rust-bin', clone, a)
            env = p.minimal_env(root, root / 'cache', clone, view)
            found = subprocess.check_output(['which', 'rustc'], env=env, text=True, timeout=5).strip()
            self.assertEqual(found, str(view / 'rustc'))
            assembled = subprocess.check_output(['as', '--version'], env=env, text=True, timeout=5)
            self.assertNotIn('UNPINNED_', assembled)
            p.process_tools(env)
            self.assertEqual(p.verify_sysroot(view, clone, env), str(clone))
            extra = view / 'which'; extra.symlink_to(clone / 'bin/which')
            with self.assertRaises(p.Rejected): p.minimal_env(root, root / 'cache', clone, view)
            extra.unlink()
            (view / 'cargo').unlink(); (view / 'cargo').symlink_to(clone / 'bin/which')
            with self.assertRaises(p.Rejected): p.minimal_env(root, root / 'cache', clone, view)


if __name__ == '__main__':
    unittest.main(verbosity=2)
