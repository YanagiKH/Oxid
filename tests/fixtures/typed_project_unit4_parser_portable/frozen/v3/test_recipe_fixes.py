#!/usr/bin/env python3
"""Bounded regression checks for reviewer-requested nonsemantic V3 recipe fixes."""
import os
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest import mock

import portable as p
from synthetic_receipts import build_fixture
from test_passivity import append_passivity_fixture


class RecipeFixControls(unittest.TestCase):
    def test_initial_rustc_version_timeout_is_explicit_and_propagates(self):
        a = p.authority(); toolchain = Path('/synthetic/reviewed-toolchain')
        with mock.patch.object(p, 'authority', return_value=a), \
             mock.patch.object(p, 'session_at', return_value=({}, Path('/synthetic/session'))), \
             mock.patch.object(p, 'verify_toolchain'), \
             mock.patch.object(p.subprocess, 'check_output', side_effect=subprocess.TimeoutExpired('rustc', 15)) as probe:
            with self.assertRaises(subprocess.TimeoutExpired):
                p.build('/synthetic/session/session.json', 'debug', toolchain, '/synthetic/cache')
        self.assertEqual(probe.call_args.args[0], [str(toolchain / 'bin/rustc'), '--version', '--verbose'])
        self.assertEqual(probe.call_args.kwargs['timeout'], 15)

    def test_relative_session_passivity_and_symlink_rejection(self):
        with tempfile.TemporaryDirectory(prefix='oxid-relative-session-') as temp:
            temp = Path(temp); f = build_fixture(temp / 'session'); append_passivity_fixture(f)
            cwd = Path.cwd()
            try:
                os.chdir(temp)
                got = p.verify_passivity('session', 'session/session.json', 'debug', f['A'], set())
                self.assertEqual(got['pairs'], 6)
                session, root = p.session_at('session/session.json', f['A'])
                self.assertEqual(root, temp / 'session')
                self.assertEqual(session['root'], str(root))
                (temp / 'linked-session').symlink_to(temp / 'session', target_is_directory=True)
                with self.assertRaises(p.Rejected): p.session_at('linked-session/session.json', f['A'])
                with self.assertRaises(p.Rejected): p.verify_passivity('session', 'linked-session/session.json', 'debug', f['A'], set())
                (temp / 'linked.json').symlink_to(temp / 'session/session.json')
                with self.assertRaises(p.Rejected): p.session_at('linked.json', f['A'])
            finally:
                os.chdir(cwd)


if __name__ == '__main__':
    unittest.main(verbosity=2)
