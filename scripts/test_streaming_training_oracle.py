import struct
import unittest

import streaming_training_oracle as oracle


class StreamingTrainingOracleTests(unittest.TestCase):
    def test_literal_wire_bytes_and_rows(self):
        self.assertEqual(oracle.grid(1), bytes.fromhex(
            '4f5844533030303101000000020000000100000000000000'
            '0000000000000000000000000000e03f0000000000000080'))
        body = oracle.encode(oracle.SAMPLE)
        self.assertEqual(struct.unpack_from('<IIII', body, 8), (8, 2, 1, 0))
        self.assertEqual(len(body), 216)

    def test_corpus_has_unique_boundary_and_failure_cases(self):
        cases = oracle.reader_cases()
        self.assertEqual(len({row[0] for row in cases}), len(cases))
        self.assertGreater(len(dict((row[0], row[1]) for row in cases)['larger-than-heap']), 16384)
        self.assertEqual({row[2] for row in cases}, {0, 1, 2, 3, 4, 8, 9, 10, 1002})

    def test_analytic_training_known_losses_and_partial_batches(self):
        initial, final = oracle.train(oracle.SAMPLE, 30, 4)
        self.assertAlmostEqual(initial, 3.68183708824125, places=13)
        self.assertAlmostEqual(final, .01213086307405941, places=13)
        for epochs, size in [(3, 3), (1, 128)]:
            before, after = oracle.train(oracle.SAMPLE, epochs, size)
            self.assertLess(after, before)

    def test_summary_rejects_wrong_progress_or_losses(self):
        correct = b'Rows: 8\nEpochs: 30\nBatches: 60\nInitial MSE: 3.68184e0\nFinal MSE: 1.21309e-2\n'
        oracle.check_summary(correct, oracle.SAMPLE, 30, 4)
        for changed in [correct.replace(b'Batches: 60', b'Batches: 59'),
                        correct.replace(b'1.21309e-2', b'1.21309e-1'),
                        correct.replace(b'1.21309e-2', b'nan'), correct+b'extra\n']:
            with self.assertRaises(ValueError):
                oracle.check_summary(changed, oracle.SAMPLE, 30, 4)


if __name__ == '__main__':
    unittest.main()
