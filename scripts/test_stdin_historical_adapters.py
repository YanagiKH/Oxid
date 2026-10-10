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
        historical_path = PACKAGE.parent / "typed_project_source_binding_byte_storage_v1" / "run.py"
        self.assertEqual(binding.digest(historical_path.read_bytes()),
                         "584e8ee937cb2d3ef62d59d3eb69a79bfb6afce23df86de3df305f81afae7c76")
        spec = importlib.util.spec_from_file_location("stdin_historical_source_binding", historical_path)
        historical = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(historical)
        historical_owner = historical.adapt_stdin_parser_resource.__globals__
        active_owner = binding.adapt_stdin_parser_resource.__globals__
        self.assertIs(historical_owner, vars(historical))
        self.assertIs(active_owner, vars(binding._runtime))
        self.assertIsNot(active_owner, vars(binding))
        self.assertIsNot(active_owner, historical_owner)
        self.assertEqual(active_owner["__name__"], "lexer_reservation_retained_execution")
        self.assertEqual(binding.adapt_stdin_parser_resource.__code__,
                         historical.adapt_stdin_parser_resource.__code__)
        self.assertEqual(binding.STDIN_RESOURCE_ADAPTER, historical.STDIN_RESOURCE_ADAPTER)
        self.assertEqual(binding.STDIN_RESOURCE_SEAM, historical.STDIN_RESOURCE_SEAM)
        self.assertIsNot(binding.BindingError, historical.BindingError)

        for label, api in (("immutable-historical", historical), ("active-facade", binding)):
            with self.subTest(owner=label):
                adapter = api.adapt_stdin_parser_resource
                owner = adapter.__globals__
                self.assertIs(owner["adapt_stdin_parser_resource"], adapter)
                self.assertIs(owner["BindingError"], api.BindingError)
                self.assertIs(owner["STDIN_RESOURCE_ADAPTER"], api.STDIN_RESOURCE_ADAPTER)
                original_seam = owner["STDIN_RESOURCE_SEAM"]
                original_authority = owner["STDIN_RESOURCE_ADAPTER"]
                self.assertEqual(api.entry(api.RESOURCE, self.predecessor),
                                 original_authority["predecessor"])
                baseline = api.adapt_stdin_parser_resource(self.predecessor)
                self.assertEqual(api.entry(api.RESOURCE, baseline), original_authority["derived"])
                self.assertEqual(api.adapt_stdin_parser_resource(baseline, reverse=True), self.predecessor)

                changed_seam = original_seam + b" "
                with patch.dict(owner, {"STDIN_RESOURCE_SEAM": changed_seam}):
                    self.assertIs(adapter.__globals__["STDIN_RESOURCE_SEAM"], changed_seam)
                    self.assertIs(api.adapt_stdin_parser_resource, adapter)
                    self.assertNotEqual(api.digest(owner["STDIN_RESOURCE_SEAM"]),
                                        original_authority["substitution"]["new_sha256"])
                    with self.assertRaisesRegex(api.BindingError, "substitution identity differs"):
                        api.adapt_stdin_parser_resource(self.predecessor)
                self.assertIs(owner["STDIN_RESOURCE_SEAM"], original_seam)

                changed = copy.deepcopy(original_authority)
                changed["derived"]["sha256"] = "0" * 64
                with patch.dict(owner, {"STDIN_RESOURCE_ADAPTER": changed}):
                    self.assertIs(adapter.__globals__["STDIN_RESOURCE_ADAPTER"], changed)
                    self.assertIs(api.adapt_stdin_parser_resource, adapter)
                    self.assertEqual(changed["predecessor"], original_authority["predecessor"])
                    self.assertEqual(changed["substitution"], original_authority["substitution"])
                    self.assertNotEqual(api.entry(api.RESOURCE, baseline), changed["derived"])
                    with self.assertRaisesRegex(api.BindingError, "wrong stdin parser resource output"):
                        api.adapt_stdin_parser_resource(self.predecessor)
                self.assertIs(owner["STDIN_RESOURCE_ADAPTER"], original_authority)
                self.assertEqual(api.adapt_stdin_parser_resource(self.predecessor), baseline)


if __name__ == "__main__":
    unittest.main()
