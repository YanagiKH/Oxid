"""Current fixture registration after finite enum payloads extend the language.

The exact pre-extension registry/oracle controls remain mandatory on their
historical sources. Explicit enum/field test renames are recorded here;
executable new semantics have separate Rust, LLVM and application checks.
"""
import copy
import hashlib
import json
from pathlib import Path
import subprocess
import sys
import textwrap
import unittest

import verify_bounded_byte_storage as gate

ROOT = Path(__file__).resolve().parents[1]
UNIT_RENAMES = {
    "byte_storage_previous_record_elements_and_enum_exclusions_are_unchanged":
        "byte_storage_previous_record_elements_and_owned_enum_arrays",
    "byte_storage_source_unused_nested_and_empty_record_fields_stay_closed":
        "byte_storage_source_unused_nested_and_empty_record_fields_are_admitted",
    "byte_storage_raw_record_byte_fields_are_rejected_even_unused_and_nested":
        "byte_storage_raw_record_byte_fields_are_checked_even_unused_and_nested",
    "byte_storage_raw_unused_and_nested_record_fields_reject_before_output_reserve":
        "byte_storage_raw_unused_and_nested_record_fields_keep_bounded_layout_and_output_reservations",
}
PUBLIC_RENAMES = {
    "byte_storage_public_unused_nested_enum_array_payload_stops_in_inner_parser":
        "byte_storage_public_unused_nested_enum_array_payload_checks_without_effects",
    "byte_storage_public_record_reference_projection_stops_at_declaration":
        "byte_storage_public_record_reference_projection_checks_without_effects",
}


def current_registry(registry):
    result = copy.deepcopy(registry)
    for before, after in UNIT_RENAMES.items():
        selected = [row for row in result["unit_tests"] if row["function"] == before]
        gate.require(len(selected) == 1 and after not in {row["function"] for row in result["unit_tests"]},
                     "wrong original unit registry boundary")
        row = selected[0]
        gate.require(row["name"].endswith("::" + before), "wrong original unit identity")
        row["function"] = after
        row["name"] = row["name"][:-len(before)] + after
    for before, after in PUBLIC_RENAMES.items():
        gate.require(result["public_tests"].count(before) == 1 and after not in result["public_tests"],
                     "wrong original public registry boundary")
    result["public_tests"] = sorted(PUBLIC_RENAMES.get(name, name) for name in result["public_tests"])
    return result


class CurrentByteFixtureInventory(unittest.TestCase):
    def test_exact_original_data_and_only_explicit_source_test_name_changes(self):
        self.assertEqual(hashlib.sha256(Path(gate.__file__).read_bytes()).hexdigest(),
                         "3fc7ed6d9264e5446485b52a74106216fe236235b517190835a6b70e5bbb1f5d")
        _, original, cases = gate.admit_package(ROOT)
        before = json.dumps(original, sort_keys=True)
        successor = current_registry(original)
        gate.admit_registry(ROOT, successor)
        self.assertEqual(json.dumps(original, sort_keys=True), before)
        self.assertEqual(len(cases), 323)
        self.assertEqual(len(gate.fixture_data_sources(ROOT)), 325)
        self.assertEqual(len(successor["unit_tests"]), 79)
        self.assertEqual(sum(row["ignored"] for row in successor["unit_tests"]), 4)
        for key in original.keys() - {"unit_tests", "public_tests"}:
            self.assertEqual(successor[key], original[key])
        changed = [(old, new) for old, new in zip(original["unit_tests"], successor["unit_tests"], strict=True)
                   if old != new]
        self.assertEqual(len(changed), 4)
        self.assertEqual({old["function"] for old, _ in changed}, set(UNIT_RENAMES))
        self.assertEqual({new["function"] for _, new in changed}, set(UNIT_RENAMES.values()))
        self.assertEqual(set(successor["public_tests"]) - set(original["public_tests"]), set(PUBLIC_RENAMES.values()))
        self.assertEqual(set(original["public_tests"]) - set(successor["public_tests"]), set(PUBLIC_RENAMES))

    def test_new_imports_keep_portable_fixture_admission_without_native_helpers(self):
        script = textwrap.dedent('''
            import importlib
            import importlib.abc
            import json
            from pathlib import Path
            import sys
            class NoResource(importlib.abc.MetaPathFinder):
                def find_spec(self, fullname, path=None, target=None):
                    if fullname == 'resource':
                        raise ModuleNotFoundError('resource is unavailable on this host')
            sys.modules.pop('resource', None)
            sys.meta_path.insert(0, NoResource())
            root = Path(sys.argv[1])
            sys.path.insert(0, str(root / 'scripts'))
            importlib.import_module(sys.argv[2])
            import verify_bounded_byte_storage as gate
            import verify_repo
            from test_enum_byte_fixture_inventory import current_registry
            _, registry, cases = gate.admit_package(root)
            gate.admit_registry(root, current_registry(registry))
            current = gate.fixture_data_sources(root)
            previous = verify_repo.predecessor_fixture_data_sources(root)
            gate.require(len(cases) == 323 and len(current) == 325 and len(previous) == 188,
                         'portable fixture counts differ')
            vectors = verify_repo.typed_vector_fixture_data_sources(root)
            gate.require(len(vectors) == 30 and not vectors & (current | previous),
                         'portable vector fixture registration differs')
            copies = verify_repo.copy_record_fixture_data_sources(root)
            gate.require(copies == {root / 'tests/fixtures/copy_records/all_scalars.ox',
                                    root / 'tests/fixtures/copy_records/wide_unicode.ox'}
                         and not copies & (current | previous | vectors),
                         'portable Copy fixture registration differs')
            owned_names = (
                'aliases.ox', 'bytebuf.ox', 'conflicting_borrows.ox', 'copy_operation.ox',
                'double_move.ox', 'empty_new.ox', 'enum16.ox', 'failed_reserve.ox',
                'fault_pop.ox', 'fault_push_file.ox', 'files.ox', 'generic_record.ox',
                'index_read.ox', 'nested_vector.ox', 'package_main.ox', 'package_model.ox',
                'unused_pop.ox', 'unused_push.ox', 'unused_template.ox', 'zero_key.ox',
            )
            owned_vectors = verify_repo.owned_record_vector_fixture_data_sources(root)
            gate.require(len(owned_vectors) == 20
                         and owned_vectors == {root / 'scripts/owned_record_vector_acceptance' / name
                                               for name in owned_names}
                         and not owned_vectors & (current | previous | vectors | copies),
                         'portable owned-record vector fixture registration differs')
            registered = verify_repo.fixture_data_sources(root)
            gate.require(registered == current | previous | vectors | copies | owned_vectors,
                         'portable complete fixture registration differs')
            gate.require(not current & previous and registered - copies - owned_vectors == current | previous | vectors,
                         'portable fixture registration differs')
            verify_repo.verify_test_fixture_registration(root)
            gate.require(not {'resource', 'verify_bounded_stdin_native', 'verify_bounded_u8_native'} & sys.modules.keys(),
                         'portable registration loaded native execution helpers')
            print(json.dumps({'cases': len(cases), 'current': len(current), 'predecessor': len(previous)}))
        ''')
        for module in ("verify_bounded_byte_storage", "verify_repo", "verify_fixture_data",
                       "verify_bounded_typed_parser", "verify_bounded_typed_static"):
            with self.subTest(module=module):
                result = subprocess.run([sys.executable, "-I", "-B", "-c", script, str(ROOT), module],
                                        capture_output=True, text=True, timeout=60)
                self.assertEqual(result.returncode, 0, result.stderr)
                self.assertEqual(json.loads(result.stdout), {"cases": 323, "current": 325, "predecessor": 188})

    def test_missing_original_name_cannot_silently_drop_a_current_registry_check(self):
        _, original, _ = gate.admit_package(ROOT)
        for field, names in (("unit_tests", UNIT_RENAMES), ("public_tests", PUBLIC_RENAMES)):
            for name in names:
                changed = copy.deepcopy(original)
                if field == "unit_tests":
                    changed[field] = [row for row in changed[field] if row["function"] != name]
                else:
                    changed[field].remove(name)
                with self.subTest(field=field, name=name), self.assertRaisesRegex(ValueError, "registry boundary"):
                    current_registry(changed)


if __name__ == "__main__":
    unittest.main()
