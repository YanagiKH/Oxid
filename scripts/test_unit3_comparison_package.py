"""Exercise the published oracle package through its real comparison entrance."""
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest


REPO = Path(__file__).resolve().parents[1]
PACKAGE = REPO / 'tests/fixtures/typed_project_unit3_independent'


class ComparisonPackageTests(unittest.TestCase):
    def test_admitted_package_materializes_and_prepares_without_builds(self):
        with tempfile.TemporaryDirectory() as temporary:
            output = Path(temporary)
            commands = [
                [str(PACKAGE / 'portable/verify_package.py'),
                 '--package', str(PACKAGE),
                 '--manifest', str(PACKAGE / 'package-manifest.json'),
                 '--receipt', str(output / 'package-verification.json')],
                [str(PACKAGE / 'portable/transport.py'), 'materialize',
                 '--archive', str(PACKAGE / 'source-transport/sources.tar.gz'),
                 '--manifest', str(PACKAGE / 'source-transport/source-transport.json'),
                 '--requests', str(PACKAGE / 'source-transport/requests.jsonl'),
                 '--output', str(output / 'inputs')],
                [str(PACKAGE / 'portable/comparison-v1/portable_compare.py'), 'prepare',
                 '--component-root', str(PACKAGE / 'components/oracles'),
                 '--component-manifest', str(PACKAGE / 'portable/comparison-v1/component-manifest.json'),
                 '--materialization', str(output / 'inputs/materialization.json'),
                 '--parser', str(PACKAGE / 'components/observer/parse_debug.py'),
                 '--output', str(output / 'comparison-view')],
            ]
            for arguments in commands:
                command = [sys.executable, '-B', *arguments]
                result = subprocess.run(command, cwd=REPO,
                                        env={**os.environ, 'PYTHONOPTIMIZE': '0'},
                                        capture_output=True, text=True, timeout=60)
                self.assertEqual(result.returncode, 0,
                                 f'{command!r}\n{result.stdout}\n{result.stderr}')
            prepared = json.loads((output / 'comparison-view/prepared-comparison.json').read_text())
            self.assertEqual(prepared['status'], 'prepared')


if __name__ == '__main__':
    unittest.main()
