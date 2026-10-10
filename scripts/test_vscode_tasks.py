"""Run dependency-free template checks with the actual ECMAScript regex engine."""
import shutil
import subprocess
import unittest
from pathlib import Path


class VscodeTaskTemplateTests(unittest.TestCase):
    def test_actual_template_and_ecmascript_patterns(self):
        node = shutil.which("node")
        if node is None:
            self.skipTest("Node unavailable; ECMAScript matcher validation was not run")
        root = Path(__file__).resolve().parents[1]
        result = subprocess.run(
            [node, str(root / "editors/vscode/test_tasks.cjs")],
            cwd=root, capture_output=True, text=True, timeout=30, check=False,
        )
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertIn("PASS:", result.stdout)


if __name__ == "__main__":
    unittest.main()
