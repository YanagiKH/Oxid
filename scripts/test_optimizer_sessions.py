import struct
import unittest

import optimizer_oracles as oracle
import verify_optimizer_training as training


def model_bytes(updates=0):
    parameters, rng = oracle.model_oracle.initial_parameters()
    return oracle.crc(b'OXMD0001'+struct.pack('<IIIIQ', 2, 8, 1, rng, updates)
                      + struct.pack('<33d', *parameters))


class SessionContract(unittest.TestCase):
    def test_legacy_models_restore_fixed_rate_sgd_and_the_update_count(self):
        decoded = oracle.parse_session(model_bytes(7))
        self.assertTrue(decoded['legacy'])
        self.assertEqual(decoded['optimizer'], oracle.initial_state(oracle.config(), 33, steps=7))

    def test_full_sessions_keep_independent_model_and_optimizer_counters(self):
        state = oracle.initial_state(oracle.config('adam'), 33)
        parameters = [0.] * 33
        for _ in range(3):
            parameters, state = oracle.update(parameters, [0.]*33, state)
        body = oracle.session(model_bytes(7), oracle.encode_state(state))
        decoded = oracle.parse_session(body)
        self.assertFalse(decoded['legacy'])
        self.assertEqual(decoded['model']['updates'], 7)
        self.assertEqual(decoded['optimizer']['steps'], 3)

    def test_outer_checksum_cannot_hide_a_bad_component_or_shape(self):
        model = model_bytes()
        state = oracle.encode_state(oracle.initial_state(oracle.config('adam'), 33))
        body = oracle.session(model, state)
        for offset in [32, 24+len(model)+40]:
            changed = bytearray(body[:-4]); changed[offset] ^= 1
            with self.assertRaises(ValueError):
                oracle.parse_session(oracle.crc(changed))
        wrong_shape = oracle.encode_state(oracle.initial_state(oracle.config(), 3))
        with self.assertRaises(ValueError):
            oracle.parse_session(oracle.session(model, wrong_shape))

    def test_recovery_mutations_include_coherent_inner_checksums(self):
        for kind in ['sgd', 'adam', 'adamw']:
            state = oracle.initial_state(oracle.config(kind), 33)
            body = model_bytes() if kind == 'sgd' else oracle.session(model_bytes(), oracle.encode_state(state))
            controls = training.corruptions(body, kind != 'sgd')
            self.assertEqual(len(controls), 3 if kind == 'sgd' else 7)
            for name, altered in controls:
                with self.subTest(kind=kind, name=name):
                    with self.assertRaises(ValueError):
                        oracle.parse_session(altered)

    def test_all_optimizer_negative_controls_are_invalid_with_their_recorded_bytes(self):
        cases = oracle.invalid_states()
        self.assertGreater(len(cases), 30)
        self.assertEqual(len({name for name, _ in cases}), len(cases))
        for name, body in cases:
            with self.subTest(name=name):
                with self.assertRaises(ValueError):
                    oracle.parse_state(body)


if __name__ == '__main__':
    unittest.main()
