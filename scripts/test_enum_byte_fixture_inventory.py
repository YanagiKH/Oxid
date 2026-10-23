"""Current fixture registration after finite enum payloads extend the language.

The exact pre-extension registry/oracle controls remain mandatory on their
historical sources. Here only two test names change; executable new payload
semantics have separate Rust, LLVM and application checks.
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
UNIT_BEFORE = "byte_storage_previous_record_elements_and_enum_exclusions_are_unchanged"
UNIT_AFTER = "byte_storage_previous_record_elements_and_owned_enum_arrays"
PUBLIC_BEFORE = "byte_storage_public_unused_nested_enum_array_payload_stops_in_inner_parser"
PUBLIC_AFTER = "byte_storage_public_unused_nested_enum_array_payload_checks_without_effects"


def current_registry(registry):
    result = copy.deepcopy(registry)
    selected = [row for row in result["unit_tests"] if row["function"] == UNIT_BEFORE]
    gate.require(len(selected) == 1 and UNIT_AFTER not in {row["function"] for row in result["unit_tests"]},
                 "wrong original unit registry boundary")
    row = selected[0]
    gate.require(row["name"].endswith("::" + UNIT_BEFORE), "wrong original unit identity")
    row["function"] = UNIT_AFTER
    row["name"] = row["name"][:-len(UNIT_BEFORE)] + UNIT_AFTER
    gate.require(result["public_tests"].count(PUBLIC_BEFORE) == 1
                 and PUBLIC_AFTER not in result["public_tests"], "wrong original public registry boundary")
    result["public_tests"] = sorted(PUBLIC_AFTER if name == PUBLIC_BEFORE else name
                                    for name in result["public_tests"])
    return result


class CurrentByteFixtureInventory(unittest.TestCase):
    def test_exact_original_data_and_only_two_source_test_name_changes(self):
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
        self.assertEqual(len(changed), 1)
        self.assertEqual(changed[0][0]["function"], UNIT_BEFORE)
        self.assertEqual(changed[0][1]["function"], UNIT_AFTER)
        self.assertEqual(set(successor["public_tests"]) - set(original["public_tests"]), {PUBLIC_AFTER})
        self.assertEqual(set(original["public_tests"]) - set(successor["public_tests"]), {PUBLIC_BEFORE})

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
            gate.require(not current & previous and verify_repo.fixture_data_sources(root) == current | previous,
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
        for field in ("unit_tests", "public_tests"):
            changed = copy.deepcopy(original)
            if field == "unit_tests":
                changed[field] = [row for row in changed[field] if row["function"] != UNIT_BEFORE]
            else:
                changed[field].remove(PUBLIC_BEFORE)
            with self.subTest(field=field), self.assertRaisesRegex(ValueError, "registry boundary"):
                current_registry(changed)


if __name__ == "__main__":
    unittest.main()
