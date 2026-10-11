"""Pure byte/AST/mock controls. Never import or execute retained/derived controllers.

Run with Python -B. The installed repository is the default fixture root. An
explicit OXID_PACKAGE_OWNED_RETAINED_ROOT override selects a retained fixture;
no alternate checkout is silently discovered and no missing fixture is skipped.
"""
from __future__ import annotations

import ast
from collections import Counter
import hashlib
import importlib.util
import json
import os
from pathlib import Path
import stat
import sys
import types
import unittest
from unittest.mock import patch


PACKAGE = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location("owned_inert_test_subject", PACKAGE / "owned.py")
owned = importlib.util.module_from_spec(spec)
spec.loader.exec_module(owned)


class Hostile:
    def __getattribute__(self, name):
        raise AssertionError("caller object was inspected")

    def __iter__(self):
        raise AssertionError("caller object was iterated")

    def __bool__(self):
        raise AssertionError("caller object was coerced")


class OwnedInertTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.recipe = owned._recipes()
        override = os.environ.get("OXID_PACKAGE_OWNED_RETAINED_ROOT")
        cls.root = Path(override).resolve(strict=True) if override else PACKAGE.parents[2]
        cls.bodies = {name: (cls.root / name).read_bytes() for name in cls.recipe["retained_inputs"]}
        cls.result = owned.fixed_derivation(cls.bodies)
        cls.derived = cls.result["members"]["producer-both"]["body"]
        cls.original = cls.bodies["scripts/verify_owned_array_native.py"]
        cls.tree = ast.parse(cls.derived)
        cls.functions = {node.name: node for node in cls.tree.body if isinstance(node, ast.FunctionDef)}

    def mutation_rejected(self, name, body):
        inputs = dict(self.bodies)
        inputs[name] = body
        with self.assertRaises(owned.DerivationError):
            owned.fixed_derivation(inputs)

    def edit_rejected(self, identifier, replacement):
        edit = next(row for row in self.recipe["edits"] if row["id"] == identifier)
        changed = self.derived[:edit["derived_start"]] + replacement + self.derived[edit["derived_end"]:]
        with self.assertRaises(owned.DerivationError):
            owned.inverse("producer-both", changed)

    def test_closed_four_role_output(self):
        self.assertEqual(set(self.result["members"]), {"producer-both", "independent-profile", "package-both", "compact-audit"})
        self.assertEqual({v["body"] for v in self.result["members"].values()}, {self.derived})
        self.assertEqual(self.result["status"], "inert-unregistered")
        self.assertIs(self.result["semantic_pass"], False)
        self.assertIs(self.result["execution_performed"], False)

    def test_exact_inverse_all_roles(self):
        for member in self.result["members"]:
            with self.subTest(member=member):
                self.assertEqual(owned.inverse(member, self.derived), self.original)

    def test_every_retained_body_authenticates_before_ast(self):
        with patch.object(owned.ast, "parse", side_effect=AssertionError("AST before authentication")):
            for name, body in self.bodies.items():
                with self.subTest(name=name):
                    self.mutation_rejected(name, body + b"\n")

    def test_every_retained_member_required(self):
        for name in self.bodies:
            with self.subTest(name=name):
                inputs = dict(self.bodies)
                del inputs[name]
                with self.assertRaises(owned.DerivationError):
                    owned.fixed_derivation(inputs)

    def test_extra_retained_member_rejected(self):
        inputs = {**self.bodies, "unreviewed.py": b"raise Exception()"}
        with self.assertRaises(owned.DerivationError):
            owned.fixed_derivation(inputs)

    def test_retained_closure_rejects_non_dict_without_inspection(self):
        with self.assertRaises(owned.DerivationError):
            owned.fixed_derivation(Hostile())

    def test_retained_closure_rejects_mapping_subclass(self):
        class Forged(dict):
            def items(self):
                raise AssertionError("mapping subclass inspected")
        with self.assertRaises(owned.DerivationError):
            owned.fixed_derivation(Forged(self.bodies))

    def test_retained_closure_rejects_bytes_subclass(self):
        class Forged(bytes):
            pass
        name = next(iter(self.bodies))
        self.mutation_rejected(name, Forged(self.bodies[name]))

    def test_retained_closure_rejects_non_bytes(self):
        name = next(iter(self.bodies))
        self.mutation_rejected(name, Hostile())

    def test_retained_closure_rejects_string_subclass(self):
        class Forged(str):
            pass
        inputs = {Forged(name): body for name, body in self.bodies.items()}
        with self.assertRaises(owned.DerivationError):
            owned.fixed_derivation(inputs)

    def test_caller_mutation_after_authentication_cannot_change_derivation(self):
        inputs = dict(self.bodies)
        original_authenticate = owned._authenticate
        def mutate_after_authentication(passed, recipe):
            snapshot = original_authenticate(passed, recipe)
            passed.clear()
            passed["scripts/verify_owned_array_native.py"] = b"malicious substituted controller"
            return snapshot
        with patch.object(owned, "_authenticate", side_effect=mutate_after_authentication):
            result = owned.fixed_derivation(inputs)
        self.assertEqual(result["members"]["producer-both"]["body"], self.derived)

    def test_authenticated_snapshot_is_immutable(self):
        snapshot = owned._authenticate(self.bodies, self.recipe)
        with self.assertRaises(TypeError):
            snapshot["src/main.rs"] = b"substitution"

    def test_inverse_rejects_unknown_member_before_body_inspection(self):
        for member in ("debug", "release", "all", "producer", Hostile()):
            with self.subTest(member=type(member).__name__):
                with self.assertRaises(owned.DerivationError):
                    owned.inverse(member, Hostile())

    def test_inverse_rejects_non_bytes(self):
        with self.assertRaises(owned.DerivationError):
            owned.inverse("producer-both", Hostile())

    def test_inverse_rejects_relabelled_original(self):
        with self.assertRaises(owned.DerivationError):
            owned.inverse("producer-both", self.original)

    def test_inverse_rejects_all_derivative_mutations(self):
        for offset in (0, len(self.derived) // 2, len(self.derived) - 1):
            changed = bytearray(self.derived)
            changed[offset] ^= 1
            with self.subTest(offset=offset), self.assertRaises(owned.DerivationError):
                owned.inverse("producer-both", bytes(changed))

    def test_first_executable_statement_is_unconditional_barrier(self):
        statements = [node for node in self.tree.body if not (isinstance(node, ast.Expr) and isinstance(node.value, ast.Constant) and isinstance(node.value.value, str))
                      and not (isinstance(node, ast.ImportFrom) and node.module == "__future__")]
        self.assertIsInstance(statements[0], ast.Raise)
        self.assertEqual(statements[0].exc.func.id, "RuntimeError")
        self.assertIn("inert owned-array", statements[0].exc.args[0].value)
        self.assertLess(statements[0].lineno, min(node.lineno for node in self.tree.body if isinstance(node, ast.Import)))

    def test_removing_barrier_rejected(self):
        marker = b'raise RuntimeError("inert owned-array current-source controller; execution has not been authorized")\n'
        with self.assertRaises(owned.DerivationError):
            owned.inverse("producer-both", self.derived.replace(marker, b"", 1))

    def test_policy_is_held_before_caller_inspection(self):
        with self.assertRaises(owned.AuthorityHeld):
            owned.current_policy(Hostile())

    def test_plan_is_held_before_caller_inspection(self):
        with self.assertRaises(owned.AuthorityHeld):
            owned.plan(Hostile())

    def test_run_is_held_before_caller_inspection(self):
        with self.assertRaises(owned.AuthorityHeld):
            owned.run(Hostile(), profile=Hostile(), current_envelope=Hostile())

    def test_matching_path_ambient_owner_does_not_authorize(self):
        forged = types.ModuleType("package_dependency_ci_v1.source")
        forged.owns_adapter = lambda *args: True
        forged.load_adapter = lambda *args: object()
        with patch.dict(sys.modules, {"package_dependency_ci_v1.source": forged, "source": forged}):
            for api in (owned.current_policy, owned.plan, owned.run):
                with self.subTest(api=api.__name__), self.assertRaises(owned.AuthorityHeld):
                    api({"registry": "owned-array", "original_job_context": forged})

    def test_live_apis_start_with_raise(self):
        module = ast.parse((PACKAGE / "owned.py").read_bytes())
        functions = {node.name: node for node in module.body if isinstance(node, ast.FunctionDef)}
        for name in ("current_policy", "plan", "run"):
            self.assertIsInstance(functions[name].body[0], ast.Raise)
        self.assertIsInstance(self.functions["_package_owned_owner"].body[0], ast.Raise)

    def test_self_consistent_forged_compact_envelope_cannot_authorize(self):
        authority = self.recipe["source_relationship"]
        envelope = {"fixed_authority": authority, "profile": "debug", "producer_binding": {"head": "a" * 40},
                    "original_process_completion": "success", "owner": "original", "sealed": True}
        envelope["sha256"] = hashlib.sha256(json.dumps(envelope, sort_keys=True).encode()).hexdigest()
        for api in (owned.current_policy, owned.plan, owned.run):
            with self.subTest(api=api.__name__), self.assertRaises(owned.AuthorityHeld):
                api(envelope)

    def test_wrong_head_profile_reused_artifact_and_missing_completion_cannot_authorize(self):
        for envelope in ({"head": "b" * 40}, {"profile": "other"}, {"artifact": "previous-run"}, {"completion": None}):
            with self.subTest(envelope=envelope), self.assertRaises(owned.AuthorityHeld):
                owned.run(envelope)

    def test_current_manifest_and_main_pins(self):
        relation = self.result["source_relationship"]
        self.assertEqual(relation["current_manifest"]["sha256"], "f67d373bf5e2f8e620e17fa3f8fc8234211338738d46d65bbdf5658e0e34eac5")
        self.assertEqual(relation["current_main"]["sha256"], "73b79fa86e076a48bf3a0ca7548a1864b9a2928670f9e13512c280a63fec93cb")
        self.assertEqual(relation["current_checkpoint"]["head"], "ce522e393646ce6478848cee31f307cb4b6ad5e8")
        self.assertEqual(relation["current_checkpoint"]["tree"], "8c3f4837e1fc89a6272ebee3552ce617ce584131")
        self.assertEqual(len(self.recipe["current_all376_members"]), 376)

    def test_historical_manifest_substitution_rejected(self):
        relation = self.recipe["source_relationship"]
        self.mutation_rejected(relation["current_manifest"]["path"], self.bodies[relation["historical_manifest"]["path"]])

    def test_wrong_main_even_with_forged_matching_manifest_rejected(self):
        relation = self.recipe["source_relationship"]
        bodies = dict(self.bodies)
        bodies["src/main.rs"] += b"\n// different current main\n"
        manifest = json.loads(bodies[relation["current_manifest"]["path"]])
        for row in manifest["files"]:
            if row["path"] == "src/main.rs":
                row.update(bytes=len(bodies["src/main.rs"]), sha256=hashlib.sha256(bodies["src/main.rs"]).hexdigest())
        bodies[relation["current_manifest"]["path"]] = json.dumps(manifest).encode()
        with self.assertRaises(owned.DerivationError):
            owned.fixed_derivation(bodies)

    def test_complete_unchanged_function_class_inventory(self):
        proof = self.recipe["preservation"]
        self.assertEqual(len(proof["original_controller_functions"]), 76)
        self.assertEqual(proof["unchanged_function_count"], 61)
        self.assertEqual(len(proof["changed_functions"]), 15)
        original = {row["name"]: row for row in proof["original_controller_functions"]}
        derived = {row["name"]: row for row in proof["derived_controller_functions"]}
        for name in proof["unchanged_functions"]:
            for key in ("bytes", "sha256", "ast_sha256"):
                self.assertEqual(original[name][key], derived[name][key], (name, key))

    def test_each_non_island_byte_is_preserved(self):
        for span in self.recipe["preservation"]["unchanged_source_spans"]:
            self.assertEqual(self.original[span["start"]:span["end"]], self.derived[span["derived_start"]:span["derived_end"]])
        self.assertEqual(sum(row["end"] - row["start"] for row in self.recipe["preservation"]["unchanged_source_spans"])
                         + sum(row["end"] - row["start"] for row in self.recipe["edits"]), len(self.original))

    def test_only_two_explicit_historical_preflight_command_exceptions(self):
        proof = self.recipe["preservation"]
        before = Counter(row["ast_sha256"] for row in proof["original_commands"])
        after = Counter(row["ast_sha256"] for row in proof["derived_commands"])
        removed = Counter(row["ast_sha256"] for row in proof["command_exception_nodes"])
        self.assertEqual(before - removed, after)
        self.assertEqual(sum(removed.values()), 2)
        self.assertEqual({row["name"] for row in proof["command_exception_nodes"]}, {"run_child", "command_admission"})

    def test_historical_uppercase_and_carrier_constants_exact(self):
        self.assertEqual(owned._uppercase(self.original), owned._uppercase(self.derived))
        for name in ("scripts/replay_fixed_array_unit2d.py", "scripts/replay_fixed_array_unit2d_current.py"):
            self.assertIn(name, self.recipe["preservation"]["retained_python_functions"])
            self.mutation_rejected(name, self.bodies[name].replace(b"def ", b"#def ", 1))

    def test_changed_producer_roster_rejected(self):
        self.mutation_rejected("scripts/verify_owned_array_native.py", self.original.replace(b"*ROSTER", b"*SLICE_ROSTER", 1))

    def test_dropped_compact_current_manifest_rejected(self):
        self.edit_rejected("compact-current-explicit-roster", b'    if relative in fixed:\n')

    def test_dropped_compact_current_main_rejected(self):
        edit = next(row for row in self.recipe["edits"] if row["id"] == "compact-current-explicit-roster")
        self.edit_rejected(edit["id"], edit["after"].replace('"source/inputs/src/main.rs", ', '').encode())

    def test_dropped_current_replay_envelope_rejected(self):
        self.edit_rejected("replay-current-envelope-original-owner-seal", b'        write_json(capture / "closure-manifest.json", observed.pop("closure"))')

    def test_unexpected_prepared_source_overlay_rejected(self):
        edit = self.recipe["edits"][0]
        self.edit_rejected(edit["id"], edit["after"].replace('if name != native:', 'if name not in (native, "src/main.rs"):').encode())

    def test_no_historical_field_relabel(self):
        self.assertIn(b'"current_source_sha256": rows[BINDING + "/current-source.json"]["sha256"]', self.derived)
        self.assertIn(b'"package_current_source": _package_capture_association', self.derived)
        self.assertIn(b'current_source_path = "producer/source/inputs/" + BINDING + "/current-source.json"', self.derived)
        self.assertIn("remain historical", self.recipe["historical_fields"])

    def test_current_envelope_is_outside_frozen_state_and_seals(self):
        self.assertEqual(self.recipe["current_envelope_namespace"], "independent/<profile>-invocation/current-source.json")
        self.assertIn(b'write_json(capture / "current-source.json", envelope)', self.derived)
        self.assertNotIn(b'write_json(evidence / "current-source.json"', self.derived)
        self.assertNotIn(b'state["package_current_source"]', self.derived)
        self.assertIn("validate_independent_seal", self.recipe["preservation"]["unchanged_functions"])
        self.assertIn("validate_independent_full_closure", self.recipe["preservation"]["unchanged_functions"])

    def test_all_source_sensitive_functions_have_exact_named_islands(self):
        expected = {"source_members", "capture_sources", "verify_sources", "execute_run", "verify_producer", "compact_member",
                    "package_evidence", "independent_schema", "independent_body", "independent_command", "execute_independent",
                    "verify_independent_invocation", "verify_independent_snapshot", "audit_compact", "main"}
        self.assertEqual(set(self.recipe["preservation"]["changed_functions"]), expected)
        self.assertEqual({row["function"] for row in self.recipe["edits"]} - {"<module>"}, expected)
        self.assertEqual(len({row["id"] for row in self.recipe["edits"]}), len(self.recipe["edits"]))

    def test_required_current_owner_seams_are_explicit(self):
        names = {node.func.attr for node in ast.walk(self.tree) if isinstance(node, ast.Call) and isinstance(node.func, ast.Attribute)}
        expected = {"readmit_current_all376", "authenticated_controller_members", "capture_source_association",
                    "execute_current_preflight", "verify_current_preflight", "verify_fresh_producer_test_artifact",
                    "verify_producer_terminal", "authenticate_frozen_replay_closure", "verify_replay_source_image",
                    "verify_replay_terminal_source_binary_join", "seal_current_replay_envelope", "verify_current_replay_envelope",
                    "verify_current_replay_snapshot", "verify_current_package_snapshot", "verify_compact_producer_association",
                    "verify_compact_replay_association", "verify_downloaded_tar_association"}
        self.assertTrue(expected <= names, expected - names)

    def test_current_carrier_authenticated_before_definition_execution(self):
        function = self.functions["independent_schema"]
        lines = self.derived.splitlines()
        body = b"\n".join(lines[function.lineno - 1:function.end_lineno])
        self.assertLess(body.index(b"44bebc91"), body.index(b"exec(compile"))
        self.assertLess(body.index(b"authenticate_frozen_replay_closure"), body.index(b"exec(compile"))

    def test_original_build_native_and_replay_argv_preserved(self):
        for expression in (
            b'[tools["cargo"]["path"], "test", "--locked", "--offline", "--bin", "oxid", "--no-run", "--message-format=json", "--jobs", "2"]',
            b'[str(copied), *ROSTER, "--exact", "--ignored", "--nocapture", "--test-threads=1", "--format", "pretty", "--color", "never"]',
            b'"--profile", profile, "--cargo-home", tools["effective_environment"]["CARGO_HOME"]]'):
            self.assertIn(expression, self.original)
            self.assertEqual(self.original.count(expression), self.derived.count(expression))

    def test_full_producer_discovery_and_array_contract(self):
        proof = self.recipe["preservation"]
        producer = proof["semantic_obligations"]["owned_producer"]
        self.assertEqual(len(producer["ROSTER"]), 16)
        self.assertEqual(sum(len(producer[key]) for key in ("ROSTER", "SLICE_ROSTER", "COMPOSITION_ROSTER", "PROJECTED_SLICE_ROSTER", "ENUM_ROSTER")), 33)
        self.assertEqual([(row["distinct_inputs"], row["compiled_elfs"], row["elf_processes"], row["additional_guarded_modules"], row["reference_comparisons_derived"]) for row in producer["FAMILIES"]],
                         [(195,390,5037,195,4842),(39,78,309,39,270),(9,18,1083,9,1074),(8,16,288,8,280),(4,8,229,4,225),(4,14,112,4,102)])
        self.assertEqual((proof["array_files_per_profile"], proof["array_elf_executions_per_profile"], proof["array_reference_comparisons_per_profile"]), (1307,7058,6793))

    def test_independent_17_names_and_six_phases(self):
        independent = self.recipe["preservation"]["semantic_obligations"]["independent"]
        self.assertEqual(len(independent["ordinary_fixed_names"]), 7)
        self.assertEqual(len(independent["native_fixed_names"]), 8)
        self.assertEqual(independent["total_per_profile"], 17)
        self.assertEqual(independent["phases"], ["prepare","build","ordinary","native","physical","verify"])
        self.assertEqual(independent["profiles"], ["debug","release"])

    def test_all_43_frozen_replay_inputs_are_authenticated(self):
        prefix = "tests/fixtures/fixed_array_unit2d_independent/"
        manifest = json.loads(self.bodies[prefix + "replay-inputs-v1.json"])
        self.assertEqual(len(manifest["files"]), 43)
        for row in manifest["files"]:
            self.assertEqual(owned._record(self.bodies[prefix + row["path"]]), {key: row[key] for key in ("bytes", "sha256")})

    def test_independent_full_semantic_resource_physical_inventory(self):
        independent = self.recipe["preservation"]["semantic_obligations"]["independent"]
        supplement = independent["semantic_supplement"]
        self.assertEqual(len(supplement["raw_denials"]), 7)
        self.assertEqual(supplement["positive_shared_alias"]["expected_i32"], 77)
        self.assertEqual([row["expected_i32"] for row in supplement["extreme_cases"]], [-2147483648,2147483647,-2147483648,2147483647])
        self.assertEqual((supplement["work_assertion"]["diagnostic_bytes"], supplement["work_assertion"]["count_expansions_greater_than"]), (164,292))
        structure = independent["structure_sidecars"]
        self.assertEqual((len(structure["structure_sidecars"]), structure["bounds_sites"], structure["pointer_phis"]), (275,379,28))
        coverage = independent["coverage_contract"]
        self.assertEqual((coverage["physical_positive"],coverage["physical_mutants"],coverage["baseline_export_inputs"],coverage["baseline_unique_modules"]), (18,11,90,59))

    def test_compact_limits_and_full_only_semantics_unchanged(self):
        self.assertEqual(self.recipe["preservation"]["compact_limits"], {"compressed_bytes":33554432,"decoded_bytes":268435456})
        for fragment in (b'total <= 256 * 1024 * 1024', b'compact_record["bytes"] <= COMPACT_LIMIT', b'full_only_members', b'without pruning'):
            self.assertEqual(self.original.count(fragment), self.derived.count(fragment))
        self.assertIn("independent_compact_member", self.recipe["preservation"]["unchanged_functions"])

    def test_original_route_identity_preserved_as_obligation(self):
        self.assertEqual(self.recipe["roles"]["producer-both"]["original_routes"], ["ci.yml:L563"])
        self.assertEqual(self.recipe["roles"]["independent-profile"]["original_routes"], ["ci.yml:L577","ci.yml:L589"])
        self.assertEqual(self.recipe["roles"]["package-both"]["original_routes"], ["ci.yml:L918"])
        self.assertEqual(self.recipe["preservation"]["retained_control_obligations"], {"owned_controller":84,"independent_replay":48})

    def test_ast_normalization_is_versioned_and_empty_type_params_stable(self):
        self.assertIn("owned-ast-v1", self.recipe["preservation"]["ast_normalization"])
        node = ast.parse("def f(x):\n    return x + 1\n").body[0]
        first = owned._ast_identity(node)
        node.type_params = []
        self.assertEqual(first, owned._ast_identity(node))
        node.body[0].value.right.value = 2
        self.assertNotEqual(first, owned._ast_identity(node))

    def test_mutated_recipe_rejected_before_json_parse(self):
        body = (PACKAGE / "recipes.json").read_bytes() + b" "
        read = owned._read_regular
        parse = owned._json
        def tamper(path, limit):
            return body if path.name == "recipes.json" else read(path, limit)
        def guarded_parse(data):
            if data == body:
                raise AssertionError("parsed unauthenticated recipe")
            return parse(data)
        with patch.object(owned, "_read_regular", side_effect=tamper), patch.object(owned, "_json", side_effect=guarded_parse):
            with self.assertRaises(owned.DerivationError):
                owned.fixed_derivation(Hostile())

    def test_unlisted_package_member_rejected(self):
        real = list(PACKAGE.iterdir())
        with patch.object(owned.Path, "iterdir", return_value=iter([*real, PACKAGE / "extra.py"])):
            with self.assertRaises(owned.DerivationError):
                owned._recipes()

    def test_missing_package_member_rejected(self):
        real = [path for path in PACKAGE.iterdir() if path.name != "test_owned.py"]
        with patch.object(owned.Path, "iterdir", return_value=iter(real)):
            with self.assertRaises(owned.DerivationError):
                owned._recipes()

    def test_changed_sibling_body_rejected(self):
        read = owned._read_regular
        def tamper(path, limit):
            data = read(path, limit)
            return data + b" " if path.name == "test_owned.py" else data
        with patch.object(owned, "_read_regular", side_effect=tamper):
            with self.assertRaises(owned.DerivationError):
                owned._recipes()

    def test_package_symlink_rejected_before_open(self):
        real = Path.lstat
        def symlink(path):
            if path == (PACKAGE / "recipes.json"):
                return types.SimpleNamespace(st_mode=stat.S_IFLNK, st_size=1)
            return real(path)
        with patch.object(owned.Path, "lstat", symlink), patch.object(owned.os, "open", side_effect=AssertionError("opened symlink")):
            with self.assertRaises(owned.DerivationError):
                owned._read_regular(PACKAGE / "recipes.json", 524288)

    def test_package_oversize_rejected_before_open(self):
        real = Path.lstat
        def oversized(path):
            if path == (PACKAGE / "recipes.json"):
                return types.SimpleNamespace(st_mode=stat.S_IFREG, st_size=524289)
            return real(path)
        with patch.object(owned.Path, "lstat", oversized), patch.object(owned.os, "open", side_effect=AssertionError("opened oversized member")):
            with self.assertRaises(owned.DerivationError):
                owned._read_regular(PACKAGE / "recipes.json", 524288)

    def test_package_reads_have_fixed_finite_bounds(self):
        self.assertEqual(owned._PACKAGE_LIMITS, {"owned.py":65536,"recipes.json":524288,"test_owned.py":65536,"package-manifest.json":8192})
        module = ast.parse((PACKAGE / "owned.py").read_bytes())
        read = next(node for node in module.body if isinstance(node, ast.FunctionDef) and node.name == "_read_regular")
        calls = [node for node in ast.walk(read) if isinstance(node, ast.Call) and isinstance(node.func, ast.Attribute) and node.func.attr == "read"]
        self.assertEqual(len(calls), 1)
        self.assertEqual(ast.dump(calls[0].args[0]), "BinOp(left=Name(id='limit', ctx=Load()), op=Add(), right=Constant(value=1))")

    def test_original_owner_archive_limits_remain_explicitly_held(self):
        self.assertIn("finite archive/member byte budgets and original process deadline coverage", self.recipe["held_authorities"])

    def test_duplicate_json_keys_rejected(self):
        with self.assertRaises(owned.DerivationError):
            owned._json(b'{"name":1,"name":2}')

    def test_repository_inventories_have_no_workspace_paths(self):
        for name in self.recipe["retained_inputs"]:
            self.assertFalse(name.startswith(("/", "KH/")))
            self.assertNotIn("..", Path(name).parts)
        for name in self.recipe["repository_file_roster"]:
            self.assertTrue(name.startswith("tests/qualification/package_dependency_owned_array_current_v1/"))
        self.assertNotIn(b"KH/", (PACKAGE / "owned.py").read_bytes())

    def test_four_file_acyclic_package_manifest(self):
        manifest = json.loads((PACKAGE / "package-manifest.json").read_bytes())
        self.assertEqual(manifest["repository_file_roster"], self.recipe["repository_file_roster"])
        self.assertEqual(set(manifest["files"]), {"owned.py","recipes.json","test_owned.py"})
        for name, expected in manifest["files"].items():
            self.assertEqual(owned._record((PACKAGE / name).read_bytes()), expected)
        self.assertIs(manifest["execution_authorized"], False)
        self.assertEqual(manifest["status"], "inert-unregistered")


if __name__ == "__main__":
    unittest.main()
