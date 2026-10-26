"""Portable identity and path controls for vector DATA registration, not execution."""
import ast
import hashlib
import json
import shutil
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

import verify_typed_vector_fixture_data as data


ROOT = Path(__file__).resolve().parents[1]


class VectorFixtureDataTests(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory()
        self.addCleanup(temporary.cleanup)
        self.root = Path(temporary.name)
        self.package = Path("tests/fixtures/typed_vectors")
        shutil.copytree(ROOT / self.package, self.root / self.package)
        self.manifest = self.root / data.MANIFEST
        self.document = json.loads(self.manifest.read_bytes())

    def repinned(self, document):
        # Reach structural controls independently of the immutable production
        # digest; never change the original cases.json or its checked-in pin.
        body = json.dumps(document).encode()
        self.manifest.write_bytes(body)
        return patch.object(data, "MANIFEST_SHA256", hashlib.sha256(body).hexdigest())

    def test_exact_source_roster_and_expectations_are_authenticated(self):
        sources = data.fixture_data_sources(self.root)
        self.assertEqual(sources, set((self.root / self.package).rglob("*.ox")))
        self.assertEqual(len(sources), 30)
        self.assertEqual(sum(row["status"] == "accept" for row in self.document["cases"]), 17)
        self.assertEqual(sum(row["status"] == "reject" for row in self.document["cases"]), 10)
        self.assertEqual(len(self.document["scenarios"]), 3)
        self.document["cases"][0]["expected_result"] = 41
        self.manifest.write_text(json.dumps(self.document))
        with self.assertRaisesRegex(RuntimeError, "manifest digest mismatch"):
            data.fixture_data_sources(self.root)

    def test_same_size_source_mutation_and_missing_source_are_rejected(self):
        source = self.root / self.document["cases"][0]["source"]
        body = source.read_bytes()
        source.write_bytes(bytes([body[0] ^ 1]) + body[1:])
        with self.assertRaisesRegex(RuntimeError, "body identity mismatch"):
            data.fixture_data_sources(self.root)
        source.unlink()
        with self.assertRaisesRegex(RuntimeError, "missing or non-file"):
            data.fixture_data_sources(self.root)

    def test_required_manifest_source_type_and_size_are_checked_before_read(self):
        body = self.manifest.read_bytes()
        self.manifest.unlink()
        with self.assertRaisesRegex(RuntimeError, "missing or non-file"):
            data.fixture_data_sources(self.root)
        self.manifest.mkdir()
        with self.assertRaisesRegex(RuntimeError, "missing or non-file"):
            data.fixture_data_sources(self.root)
        self.manifest.rmdir()
        self.manifest.write_bytes(body)
        source = self.root / self.document["cases"][0]["source"]
        source.write_bytes(b"x" * (data.MAX_INPUT_BYTES + 1))
        with self.assertRaisesRegex(RuntimeError, "exceeds size bound"):
            data.fixture_data_sources(self.root)

    def test_missing_extra_duplicate_and_retyped_rows_fail_after_repinning(self):
        original = json.dumps(self.document)
        for action in ("missing", "extra", "duplicate", "status", "schema", "scenario"):
            with self.subTest(action=action):
                document = json.loads(original)
                if action == "missing":
                    document["cases"].pop()
                elif action == "extra":
                    document["cases"].append(document["cases"][0].copy())
                elif action == "duplicate":
                    document["cases"][1] = document["cases"][0].copy()
                elif action == "status":
                    document["cases"][0]["status"] = "reject"
                elif action == "schema":
                    document["schema"] = True
                else:
                    document["scenarios"][0]["source"] = document["cases"][0]["source"]
                with self.repinned(document), self.assertRaises(RuntimeError):
                    data.fixture_data_sources(self.root)

    def test_exact_path_roster_rejects_escape_alias_and_neighbour_registration(self):
        original = self.document["cases"][0]["source"]
        for relative in ("/tmp/outside.ox", "../outside.ox", "C:/outside.ox", original.replace("/", "\\"),
                         original.replace("/accept/", "/./accept/"),
                         original.replace("/accept/", "//accept/"),
                         original.replace("/accept/", "/reject/../accept/"),
                         "tests/fixtures/typed_vectors/accept/neighbour.ox"):
            with self.subTest(relative=relative):
                self.document["cases"][0]["source"] = relative
                with self.repinned(self.document), self.assertRaisesRegex(RuntimeError, "membership differs"):
                    data.fixture_data_sources(self.root)

    def test_unlisted_neighbour_is_never_data(self):
        neighbour = self.root / self.package / "accept/unlisted.ox"
        neighbour.write_bytes(b"not a registered source\n")
        sources = data.fixture_data_sources(self.root)
        self.assertEqual(len(sources), 30)
        self.assertNotIn(neighbour, sources)

    def test_duplicate_json_keys_are_rejected_after_repinning(self):
        body = self.manifest.read_bytes().replace(b'"schema": 1,', b'"schema": 1, "schema": 1,', 1)
        self.manifest.write_bytes(body)
        with patch.object(data, "MANIFEST_SHA256", hashlib.sha256(body).hexdigest()), \
                self.assertRaisesRegex(RuntimeError, "duplicate vector fixture-data key"):
            data.fixture_data_sources(self.root)

    def test_symlinked_source_or_parent_is_rejected_with_matching_bytes(self):
        source = self.root / self.document["cases"][0]["source"]
        original = source.read_bytes()
        target = self.root / "same.ox"
        target.write_bytes(original)
        source.unlink()
        try:
            source.symlink_to(target)
        except (OSError, NotImplementedError) as error:
            self.skipTest(f"symlinks unavailable: {error}")
        with self.assertRaisesRegex(RuntimeError, "symlink"):
            data.fixture_data_sources(self.root)
        source.unlink()
        source.write_bytes(original)
        directory = source.parent
        moved = directory.with_name("original-accept")
        directory.rename(moved)
        directory.symlink_to(moved, target_is_directory=True)
        with self.assertRaisesRegex(RuntimeError, "symlink"):
            data.fixture_data_sources(self.root)

    def test_import_and_admission_need_no_resource_or_execution_controller(self):
        tree = ast.parse(Path(data.__file__).read_text())
        imports = {node.module if isinstance(node, ast.ImportFrom) else alias.name
                   for node in ast.walk(tree) if isinstance(node, (ast.Import, ast.ImportFrom))
                   for alias in node.names}
        self.assertEqual(imports, {"__future__", "hashlib", "json", "pathlib"})
        code = """import sys
from pathlib import Path
sys.path.insert(0, sys.argv[1])
for name in ('resource', 'verify_typed_vectors', 'verify_float_vector'):
    sys.modules[name] = None
import verify_typed_vector_fixture_data as data
sources = data.fixture_data_sources(Path(sys.argv[2]))
assert len(sources) == 30
print('portable vector DATA admission passed')
"""
        result = subprocess.run([sys.executable, "-B", "-S", "-c", code,
                                 str(ROOT / "scripts"), str(self.root)],
                                text=True, capture_output=True, check=False)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(result.stdout, "portable vector DATA admission passed\n")

    @unittest.skipUnless(shutil.which("git"), "Git is required for checkout conversion control")
    def test_autocrlf_preserves_only_the_exact_registered_data_bytes(self):
        shutil.copy2(ROOT / ".gitattributes", self.root / ".gitattributes")
        neighbour = self.root / self.package / "accept/unlisted.ox"
        neighbour.write_bytes(b"ordinary\nsource\n")
        checkout = self.root / "checkout"

        def git(*args):
            return subprocess.run(["git", "-c", "core.autocrlf=true", "-c", "core.eol=crlf",
                                   "-c", "core.safecrlf=false", *args], cwd=self.root,
                                  check=True, text=True, capture_output=True)

        git("init", "--quiet")
        git("add", "--", ".gitattributes", self.package.as_posix())
        git("checkout-index", "--all", "--force", "--prefix", checkout.as_posix() + "/")
        self.assertEqual((checkout / neighbour.relative_to(self.root)).read_bytes(), b"ordinary\r\nsource\r\n")
        sources = data.fixture_data_sources(checkout)
        self.assertEqual(len(sources), 30)
        for path in sources | {checkout / data.MANIFEST}:
            self.assertEqual(path.read_bytes(), (self.root / path.relative_to(checkout)).read_bytes())


if __name__ == "__main__":
    unittest.main()
