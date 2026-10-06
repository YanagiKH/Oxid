"""Frozen resource successor controls, independent of current source admission."""
import copy
import importlib.util
import json
from pathlib import Path
import unittest
from unittest.mock import patch


REPO = Path(__file__).resolve().parents[1]
PACKAGE = REPO / "tests/fixtures/typed_project_source_binding"
spec = importlib.util.spec_from_file_location("stdin_source_binding", PACKAGE / "run.py")
binding = importlib.util.module_from_spec(spec)
spec.loader.exec_module(binding)


class StdinParserResourceTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.frozen = (REPO / binding.U2 / binding.RESOURCE).read_bytes()
        cls.enum_authority = json.loads((PACKAGE / "enum-authority.json").read_bytes())["resource_adapter"]
        cls.predecessor = cls.frozen.replace(binding.OLD_SEAM, binding.ENUM_RESOURCE_SEAM, 1)

    def test_closed_stdin_resource_preserves_exact_frozen_and_enum_inputs(self):
        self.assertEqual(binding.entry(binding.RESOURCE, self.predecessor), self.enum_authority["derived"])
        self.assertEqual(binding.STDIN_RESOURCE_ADAPTER["predecessor"], self.enum_authority["derived"])
        current = binding.adapt_stdin_parser_resource(self.predecessor)
        self.assertEqual(current.count(b"std_imports:StdImportPolicy::Closed,"), 1)
        self.assertEqual(current.replace(b"std_imports:StdImportPolicy::Closed,", b"", 1), self.predecessor)
        self.assertEqual(binding.adapt_stdin_parser_resource(current, reverse=True), self.predecessor)
        self.assertEqual(current.replace(binding.STDIN_RESOURCE_SEAM, binding.OLD_SEAM, 1), self.frozen)
        self.assertEqual(binding.entry(binding.RESOURCE, current), binding.STDIN_RESOURCE_ADAPTER["derived"])

    def test_resource_identity_rejects_drift_wrong_stage_and_double_application(self):
        current = binding.adapt_stdin_parser_resource(self.predecessor)
        for changed in (self.frozen, self.predecessor + b"\n", current,
                        self.predecessor.replace(b"EnumSyntaxPolicy::Closed", b"EnumSyntaxPolicy::Enabled")):
            with self.subTest(sha=binding.digest(changed)):
                with self.assertRaisesRegex(binding.BindingError, "wrong stdin parser resource input"):
                    binding.adapt_stdin_parser_resource(changed)
        for changed in (self.predecessor, current + b"\n",
                        current.replace(b"StdImportPolicy::Closed", b"StdImportPolicy::BuiltinCandidate")):
            with self.assertRaisesRegex(binding.BindingError, "wrong stdin parser resource input"):
                binding.adapt_stdin_parser_resource(changed, reverse=True)

    def test_resource_seam_and_output_changes_reject(self):
        with patch.object(binding, "STDIN_RESOURCE_SEAM", binding.STDIN_RESOURCE_SEAM + b" "):
            with self.assertRaisesRegex(binding.BindingError, "substitution identity differs"):
                binding.adapt_stdin_parser_resource(self.predecessor)
        changed = copy.deepcopy(binding.STDIN_RESOURCE_ADAPTER)
        changed["derived"]["sha256"] = "0" * 64
        with patch.object(binding, "STDIN_RESOURCE_ADAPTER", changed):
            with self.assertRaisesRegex(binding.BindingError, "wrong stdin parser resource output"):
                binding.adapt_stdin_parser_resource(self.predecessor)


if __name__ == "__main__":
    unittest.main()
