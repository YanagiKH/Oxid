import copy
import math
import struct
import unittest

import optimizer_oracles as oracle


class MathematicalContract(unittest.TestCase):
    def test_first_adam_step_and_decoupled_weight_decay(self):
        for name, expected in [('adam', .99), ('adamw', .989)]:
            state = oracle.initial_state(oracle.config(name), 1)
            parameters, updated = oracle.update([1.], [1.], state)
            self.assertAlmostEqual(parameters[0], expected, places=9)
            self.assertEqual(updated['steps'], 1)
            self.assertAlmostEqual(updated['first'][0], .1)
            self.assertAlmostEqual(updated['second'][0], .001)

    def test_decimal_clipping_handles_overflow_and_subnormals(self):
        self.assertEqual(oracle.clipped([3., 4.], 5.), [3., 4.])
        self.assertEqual(oracle.clipped([3., 4.], 2.5), [1.5, 2.])
        tiny = oracle.value(1)
        self.assertEqual([oracle.bits(x) for x in oracle.clipped([3., 4., 0.], tiny)], [1, 1, 0])
        values = oracle.clipped([1e308, -1e308], 1.)
        self.assertTrue(all(math.isfinite(x) for x in values))
        self.assertAlmostEqual(math.hypot(*values), 1.)

    def test_schedule_counts_successful_updates_and_errors_preserve_inputs(self):
        state = oracle.initial_state(oracle.config(rate=1., factor=.5, every=2), 1)
        parameters = [0.]
        for _ in range(4):
            parameters, state = oracle.update(parameters, [1.], state)
        self.assertEqual(parameters, [-3.])
        before = copy.deepcopy(state)
        with self.assertRaises(ValueError):
            oracle.update(parameters, [math.nan], state)
        self.assertEqual(state, before)

    def test_analytic_model_gradient_agrees_with_independent_finite_differences(self):
        parameters = list(oracle.model_oracle.initial_parameters()[0])
        # Central differences require a differentiable point; initial zero
        # biases put the all-zero sample directly on ReLU's kink.
        parameters[16:24] = [.125] * 8
        actual = oracle.model_gradient(parameters)
        for index, expected in enumerate(actual):
            plus, minus = list(parameters), list(parameters)
            plus[index] += 1e-6
            minus[index] -= 1e-6
            derivative = (oracle.model_oracle.forward_loss(plus) - oracle.model_oracle.forward_loss(minus)) / 2e-6
            self.assertAlmostEqual(derivative, expected, delta=2e-8)


class BinaryContract(unittest.TestCase):
    def test_optimizer_wire_offsets_and_crc_round_trip(self):
        for name, length in [('sgd', 132), ('adam', 180), ('adamw', 180)]:
            settings = oracle.config(name, clip=2., factor=.5, every=7)
            state = oracle.initial_state(settings, 3)
            body = oracle.encode_state(state)
            self.assertEqual(len(body), length)
            self.assertEqual(struct.unpack_from('<QQ', body, 112), (length-132, 7))
            self.assertEqual(oracle.parse_state(body), state)
            changed = bytearray(body)
            changed[-1] ^= 1
            with self.assertRaises(ValueError):
                oracle.parse_state(changed)

    def test_coherent_crc_cannot_hide_bad_metadata_or_moments(self):
        state = oracle.initial_state(oracle.config('adam'), 3)
        original = oracle.encode_state(state)
        for offset, fmt, value in [(8, '<I', 3), (20, '<I', 1), (40, '<d', 1.),
                                   (56, '<d', 0.), (80, '<d', .9),
                                   (128, '<d', 1.), (152, '<d', -1.), (104, '<d', .5)]:
            changed = bytearray(original[:-4])
            struct.pack_into(fmt, changed, offset, value)
            with self.subTest(offset=offset):
                with self.assertRaises(ValueError):
                    oracle.parse_state(oracle.crc(changed))

    def test_ulp_comparison_does_not_mask_subnormal_errors(self):
        self.assertEqual(oracle.ulp_distance(oracle.value(1), oracle.value(2)), 1)
        self.assertGreater(oracle.ulp_distance(0., 1e-300), 1000)
        with self.assertRaises(ValueError):
            oracle.ulp_distance(math.nan, 0.)


if __name__ == '__main__':
    unittest.main()
