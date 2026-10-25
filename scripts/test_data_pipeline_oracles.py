import math
import struct
import unittest

import data_pipeline_oracles as data
import optimizer_oracles as optimizer


class DataPipelineOracleTests(unittest.TestCase):
    def test_known_xorshift_words_and_shuffle(self):
        word = 1
        actual = []
        for _ in range(5):
            word = data.random_word(word)
            actual.append(word)
        self.assertEqual(actual, [270369, 67634689, 2647435461, 307599695, 2398689233])
        self.assertEqual(data.sampler(7, 3, True, 1)['order'], [4, 3, 1, 2, 5, 6, 0])

    def test_every_row_once_per_epoch_with_partial_and_oversized_batches(self):
        for case in data.sampling_cases():
            state = data.sampler(case['rows'], case['size'], case['shuffle'], case['seed'])
            count = (case['rows']-1)//case['size']+1
            for epoch in range(4):
                rows = []
                for _ in range(count):
                    rows.extend(data.indices(state))
                    state = data.advance(state)
                self.assertEqual(sorted(rows), list(range(case['rows'])), case['name'])
                self.assertEqual((state['epoch'], state['cursor']), (epoch+1, 0))

    def test_restore_at_each_batch_preserves_future_epochs(self):
        for case in data.sampling_cases():
            first = data.sampler(case['rows'], case['size'], case['shuffle'], case['seed'])
            second = first
            for _ in range(case['batches']):
                second = data.parse_sampler(data.encode_sampler(second))
                self.assertEqual(data.indices(first), data.indices(second))
                first, second = data.advance(first), data.advance(second)
                self.assertEqual(first, second)

    def test_dataset_bits_and_finite_validation(self):
        encoded = data.dataset([[0., -0.], [1., -2.]], [[.25], [.5]])
        decoded = data.parse_dataset(encoded)
        self.assertEqual(data.dataset(decoded['inputs'], decoded['targets']), encoded)
        for offset, form, value in [(8, '<I', 0), (12, '<I', 0xffffffff), (20, '<I', 1),
                                     (24, '<Q', (1 << 64)-1), (32, '<d', math.nan), (40, '<d', math.inf)]:
            changed = bytearray(encoded[:-4])
            struct.pack_into(form, changed, offset, value)
            with self.assertRaises(ValueError):
                data.parse_dataset(data.crc(changed))

    def test_coherent_sampler_mutations_cannot_hide_invalid_progress_or_order(self):
        encoded = data.encode_sampler(data.sampler(7, 3, True, 1))
        for offset, form, value in [(8, '<I', 0), (12, '<I', 0), (16, '<I', 2), (20, '<I', 0),
                                     (24, '<I', 1), (28, '<I', 1), (32, '<Q', 1),
                                     (40, '<Q', 1), (48, '<Q', 0xffffffffffffffff),
                                     (56, '<I', 7), (56, '<I', 3), (16, '<I', 0)]:
            changed = bytearray(encoded[:-4])
            struct.pack_into(form, changed, offset, value)
            with self.assertRaises(ValueError):
                data.parse_sampler(data.crc(changed))

    def test_counter_overflow_does_not_change_state(self):
        state = data.sampler(7, 3, True, 1)
        state.update(batches=data.MAXIMUM, epoch=data.MAXIMUM//3, cursor=(data.MAXIMUM % 3)*3)
        before = data.encode_sampler(state)
        self.assertEqual(data.parse_sampler(before), state)
        with self.assertRaises(OverflowError):
            data.advance(state)
        self.assertEqual(data.encode_sampler(state), before)

    def test_generic_minibatch_gradient_matches_finite_difference(self):
        parameters = [.1, -.2, .3, -.1, .4, .2, -.3, .1, .2, .1, -.1, .3,
                      1.5, 1.3, 1.2, 1.4, .2, -.1, .1, .3, -.2, .4, .3, -.2, .1, -.1]
        inputs = [[.2, -.4, .1], [.5, .1, -.2], [-.1, .3, .2]]
        targets = [[.1, .3], [-.2, .4], [.3, -.1]]
        for selected in [[0], [1, 2], [2, 0, 1]]:
            x, y = [inputs[i] for i in selected], [targets[i] for i in selected]
            _, gradient = data.loss_gradient(parameters, x, y, 3, 4, 2)
            for index, derivative in enumerate(gradient):
                plus, minus = parameters[:], parameters[:]
                plus[index] += 1e-6
                minus[index] -= 1e-6
                high = data.loss_gradient(plus, x, y, 3, 4, 2)[0]
                low = data.loss_gradient(minus, x, y, 3, 4, 2)[0]
                self.assertAlmostEqual(derivative, (high-low)/2e-6, delta=1e-8)

    def test_session_binds_dimensions_dataset_and_sampling_state(self):
        parameters, rng = optimizer.model_oracle.initial_parameters()
        model = data.crc(b'OXMD0001'+struct.pack('<IIIIQ', 2, 8, 1, rng, 0)+struct.pack('<33d', *parameters))
        state = optimizer.encode_state(optimizer.initial_state(optimizer.config('adam'), 33))
        training = optimizer.session(model, state)
        source = data.dataset([[0., 0.], [0., 1.], [1., 0.], [1., 1.]], [[0.], [1.], [1.], [0.]])
        sampling = data.encode_sampler(data.sampler(4, 3, True, 42))
        encoded = data.session(training, source, sampling)
        self.assertEqual(data.parse_session(encoded)['sampler']['cursor'], 0)
        with self.assertRaises(ValueError):
            data.session(training, source, data.encode_sampler(data.sampler(3, 2, True, 42)))
        changed = bytearray(encoded[:-4])
        struct.pack_into('<Q', changed, 24, data.MAXIMUM)
        with self.assertRaises(ValueError):
            data.parse_session(data.crc(changed))


if __name__ == '__main__':
    unittest.main()
