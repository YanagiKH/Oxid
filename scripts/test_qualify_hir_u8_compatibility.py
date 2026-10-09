"""Fast controls for the new current-source gate; not producer observations."""
import subprocess
import unittest

import qualify_hir_u8_compatibility as gate


class U8CompatibilityControls(unittest.TestCase):
    def test_rosters_stay_disjoint_ascii_and_within_frozen_v1_cap(self):
        rosters = gate.recipe()
        names = sum(rosters.values(), [])
        self.assertEqual(len(names), len(set(names)))
        for _, source in gate.VALID + tuple((n, s) for n, s, _ in gate.INVALID) + gate.CHANGED_TYPES + tuple((n, s) for n, s, _ in gate.CHANGED_SYNTAX):
            self.assertTrue(source.isascii())
            self.assertLessEqual(len(source), 128)
        self.assertEqual((gate.OPA_BYTES, gate.SUCCESS_BYTES, gate.DIAGNOSTIC_BYTES), (1559, 2607, 1575))

    def test_complete_successor_recipe_is_distinct_from_frozen_recipe(self):
        parities, refusals, captures = gate.expected_results()
        self.assertEqual((len(parities), len(refusals), len(captures)), (36, 144, 12))
        self.assertEqual(len(parities), len(set(parities)))
        self.assertEqual(len(refusals), len(set(refusals)))
        self.assertTrue(all('i64' in item or 'multiple-type-errors' in item for item in parities))
        self.assertTrue(any('u8-forged-i32-artifact' in item for item in refusals))
        self.assertEqual({item[0] for item in captures}, {1, 2})

    def test_success_requires_real_summary_and_no_diagnostics(self):
        good = b'{"kind":"check-summary","success":true}\n'
        self.assertEqual(len(gate.successful_check(subprocess.CompletedProcess([], 0, good, b''))), 1)
        for status, stdout, stderr in [(1, good, b''), (0, good, b'bad'), (0, b'', b''),
                                      (0, good.replace(b'true', b'false'), b''),
                                      (0, b'{"kind":"diagnostic"}\n' + good, b'')]:
            with self.assertRaises(RuntimeError):
                gate.successful_check(subprocess.CompletedProcess([], status, stdout, stderr))

    def test_forged_success_is_only_a_closed_protocol_negative_control(self):
        for version in (1, 2):
            digit = str(version).encode()
            opa = bytearray(gate.OPA_BYTES)
            opa[:5] = b'OPA' + digit + b'\0'
            opa[8] = 7
            diagnostic = bytes(opa) + b'STF' + digit + b'\1' + bytes(11)
            integer = bytearray(gate.SUCCESS_BYTES)
            integer[:gate.OPA_BYTES] = opa
            integer[gate.OPA_BYTES:gate.OPA_BYTES + 5] = b'STF' + digit + b'\0'
            forged = gate.forged_i32_success(bytes(opa), diagnostic, bytes(integer), version)
            self.assertEqual(forged[:gate.OPA_BYTES], opa)
            self.assertEqual(len(forged), 2607)
            self.assertNotEqual(forged, diagnostic)
            for bad in [bytes(integer[:-1]), bytes(integer[:8]) + bytes([8]) + bytes(integer[9:])]:
                with self.assertRaises(RuntimeError):
                    gate.forged_i32_success(bytes(opa), diagnostic, bad, version)
            with self.assertRaises(RuntimeError):
                gate.forged_i32_success(bytes(opa), diagnostic, bytes(integer), 3 - version)


if __name__ == '__main__':
    unittest.main()
