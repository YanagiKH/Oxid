import argparse
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

import verify_float_roots as gate


class FloatRootsControllerTests(unittest.TestCase):
    def test_json_retains_the_existing_u32_number_and_u64_string_contracts(self):
        for width, ty, representation in [(32, 'u32', 0), (64, 'u64', '0')]:
            value = {'schema_version': 1, 'edition': 'typed-preview', 'kind': 'run-summary',
                     'success': True, 'errors': 0, 'result': {'type': ty, 'value': representation}}
            gate.verify_json(subprocess.CompletedProcess([], 0, json.dumps(value).encode(), b''), width, 0, ty)

    def test_json_rejects_lossy_wrong_type_and_boolean_numeric_substitutes(self):
        for width, ty, bad_values in [(32, 'u32', ['0', False, 0.0]), (64, 'u64', [0, False, '1'])]:
            for bad in bad_values:
                value = {'schema_version': 1, 'edition': 'typed-preview', 'kind': 'run-summary',
                         'success': True, 'errors': 0, 'result': {'type': ty, 'value': bad}}
                with self.assertRaisesRegex(ValueError, 'JSON square-root'):
                    gate.verify_json(subprocess.CompletedProcess([], 0, json.dumps(value).encode(), b''), width, 0, ty)

    def test_only_the_selected_numeric_operation_changes_in_the_argument_parser(self):
        for width in [32, 64]:
            original, result, _ = gate.previous.source('f' + str(width), 'identity')
            source, actual = gate.source(width)
            self.assertEqual(actual, result)
            self.assertEqual(source.count('a.sqrt()'), 1)
            self.assertEqual(source.replace('a.sqrt()', 'a', 1), original)
        with patch.object(gate.previous, 'source', return_value=('fn changed(){}', 'u64', None)):
            with self.assertRaisesRegex(ValueError, 'source boundary'):
                gate.source(64)

    def test_both_signs_and_all_ieee_classes_are_exercised_deterministically(self):
        for width, fraction, exponent in [(32, 23, 255), (64, 52, 2047)]:
            values = gate.patterns(width)
            sign = 1 << (width - 1)
            infinity = exponent << fraction
            self.assertEqual(values, sorted(set(gate.patterns(width))))
            self.assertGreaterEqual(len(values), 100)
            for pattern in [0, sign, 1, sign | 1, infinity, sign | infinity, infinity | 1]:
                self.assertIn(pattern, values)

    def test_wrong_host_cannot_create_outputs_or_launch_commands(self):
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / 'result'
            with patch.object(gate.target, 'required_host', side_effect=ValueError('unsupported host')):
                with patch.object(gate.subprocess, 'run') as run:
                    with self.assertRaisesRegex(ValueError, 'unsupported host'):
                        gate.verify(argparse.Namespace(output=output))
                    run.assert_not_called()
            self.assertFalse(output.exists())

    def test_native_verification_cannot_silently_omit_the_toolchain(self):
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / 'result'
            with patch.object(gate.target, 'required_host', return_value=gate.target.X86_64):
                with self.assertRaisesRegex(ValueError, '--llvm-bin'):
                    gate.verify(argparse.Namespace(output=output, reference_only=False, llvm_bin=None))
            self.assertFalse(output.exists())

    def test_a_successful_process_with_the_wrong_numeric_result_cannot_pass(self):
        with tempfile.TemporaryDirectory() as directory:
            output = Path(directory) / 'result'
            args = argparse.Namespace(output=output, compilers=[Path(sys.executable)], reference_only=True, llvm_bin=None)
            fake = subprocess.CompletedProcess([], 0, b'1\n', b'')
            with patch.object(gate.target, 'required_host', return_value=gate.target.X86_64):
                with patch.object(gate, 'patterns', return_value=[1]):
                    with patch.object(gate.subprocess, 'run', return_value=fake):
                        with self.assertRaisesRegex(ValueError, 'exact integer oracle'):
                            gate.verify(args)
            report = json.loads((output / 'result.json').read_text())
            self.assertEqual(report['status'], 'failed')
            self.assertEqual(report['native_cases'], 0)
            self.assertEqual(report['deployments'], [])


if __name__ == '__main__':
    unittest.main()
