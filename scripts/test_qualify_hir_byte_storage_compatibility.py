"""Source-only RFC0031 recipe controls, never producer execution evidence."""
import unittest
import qualify_hir_byte_storage_compatibility as gate


class ByteStorageProviderControls(unittest.TestCase):
    def test_disjoint_bounded_domains(self):
        names = sum(gate.recipe().values(), [])
        self.assertEqual(len(names), len(set(names)))
        sources = [*gate.VALID, *((n, s) for n, s, _ in gate.INVALID),
                   *gate.ACCEPTED, *((n, s) for n, s, _ in gate.FENCES)]
        for _, source in sources:
            self.assertTrue(source.isascii())
            self.assertLessEqual(len(source), 128)
        self.assertEqual(len(gate.ACCEPTED), 8)
        self.assertEqual([c for _, _, c in gate.FENCES], ['E0202', 'E0202', 'E0100'])

    def test_exact_two_version_recipe(self):
        parities, refusals, captures = gate.expected_results()
        self.assertEqual((len(parities), len(refusals), len(captures)), (12, 216, 2))
        self.assertEqual(len(parities), len(set(parities)))
        self.assertEqual(len(refusals), len(set(refusals)))
        self.assertEqual(sum('/byte-enum-payload/' in row for row in refusals), 12)
        self.assertEqual(sum('/byte-enum-payload/' not in row for row in refusals), 204)
        self.assertEqual({row[0] for row in captures}, {1, 2})
        self.assertTrue(all('old-i64-error' in name for name in parities))
        for name in gate.NEGATIVES:
            self.assertEqual(sum('/' + name + '/' in label for label in refusals), 12)

    def test_reuses_unchanged_builders_and_caps(self):
        self.assertIs(gate.Run.build, gate.predecessor.Run.build)
        self.assertIs(gate.Run.capture, gate.predecessor.Run.capture)
        self.assertIs(gate.Run.refuse, gate.predecessor.Run.refuse)
        self.assertIs(gate.Run.positive, gate.scalar.Run.positive)
        self.assertEqual((gate.scalar.OPA_BYTES, gate.scalar.SUCCESS_BYTES,
                          gate.scalar.DIAGNOSTIC_BYTES), (1559, 2607, 1575))
        self.assertEqual(tuple(map(len, gate.scalar.expected_results())), (36, 144, 12))


if __name__ == '__main__':
    unittest.main()
