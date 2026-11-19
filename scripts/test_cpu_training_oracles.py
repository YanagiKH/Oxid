import math
import struct
import unittest
import zlib

import cpu_training_oracles as oracle


def encoded(parameters=None, inputs=2, hidden=8, outputs=1, rng=None):
    initial, seed = oracle.initial_parameters()
    values = initial if parameters is None else parameters
    body = b'OXMD0001' + struct.pack('<IIIIQ', inputs, hidden, outputs,
                                    seed if rng is None else rng, 20)
    body += struct.pack('<' + 'd' * len(values), *values)
    return body + struct.pack('<I', zlib.crc32(body))


class NumericalOracleTests(unittest.TestCase):
    def test_independent_matrix_product(self):
        self.assertEqual(oracle.matrix([1., 2., 3., 4., 5., 6.],
                                       [7., 8., 9., 10., 11., 12.], 2, 3, 2),
                         [58., 64., 139., 154.])

    def test_central_differences_match_known_derivative(self):
        gradient = oracle.finite_gradient(oracle.square_model, [[1.5, -2.]], 0)
        for actual, expected in zip(gradient, [1.5, -2.]):
            self.assertAlmostEqual(actual, expected, places=8)

    def test_unused_nan_input_has_no_loss_contribution(self):
        case = next(row for row in oracle.cases if row['name'] == 'unreachable-nan')
        self.assertEqual(case['function']([1.5, -2.], [math.nan, math.nan]), -.25)
        gradient = oracle.finite_gradient(case['function'], [[1.5, -2.], [math.nan, math.nan]], 0)
        for value in gradient:
            self.assertAlmostEqual(value, .5, places=8)

    def test_seed_is_deterministic_and_full_model_is_finite(self):
        values, state = oracle.initial_parameters()
        self.assertEqual((values, state), oracle.initial_parameters())
        self.assertEqual(len(values), 33)
        self.assertEqual(state, 2452169066)
        self.assertTrue(all(math.isfinite(value) for value in values))

    def test_coherent_wrong_gradient_changes_actual_comparison_program(self):
        ordinary, original = oracle.generate(oracle.cases[0])
        mutated, changed = oracle.generate(oracle.cases[0], True)
        self.assertNotEqual(ordinary, mutated)
        self.assertEqual(changed['gradients']['a'][0], original['gradients']['a'][0] + 1.)


class CheckpointOracleTests(unittest.TestCase):
    def test_valid_format_preserves_parameters_and_counter(self):
        parsed = oracle.parse_checkpoint(encoded())
        self.assertEqual(parsed['updates'], 20)
        self.assertEqual(parsed['parameters'], oracle.initial_parameters()[0])

    def test_truncation_extent_and_checksum_are_rejected(self):
        body = encoded()
        for candidate in [body[:n] for n in (0, 1, 8, 31, 35, len(body) - 1)] + [body + b'x', body[:-1] + b'\0']:
            with self.subTest(length=len(candidate)), self.assertRaises(ValueError):
                oracle.parse_checkpoint(candidate)

    def test_checksummed_invalid_shape_and_rng_are_rejected(self):
        for options in [{'inputs': 0}, {'inputs': 0xffffffff}, {'hidden': 268435451}, {'rng': 0}]:
            with self.subTest(options=options), self.assertRaises(ValueError):
                oracle.parse_checkpoint(encoded(**options))

    def test_checksummed_nonfinite_parameters_are_rejected(self):
        for value in (math.nan, math.inf, -math.inf):
            values = oracle.initial_parameters()[0]
            values[0] = value
            with self.subTest(value=value), self.assertRaises(ValueError):
                oracle.parse_checkpoint(encoded(values))


if __name__ == '__main__':
    unittest.main()
