"""Size refusals must be native-admission diagnostics with a failed compile summary."""
import copy
import json
import subprocess
import unittest

import verify_hosted_capacity as verifier


class DiagnosticContract(unittest.TestCase):
    def result(self, rows, code=1, stderr=b''):
        return subprocess.CompletedProcess([], code, b'\n'.join(json.dumps(x).encode() for x in rows), stderr)

    def rows(self):
        return [{'kind': 'diagnostic', 'code': 'E0700', 'stage': 'native-admission',
                 'message': 'native preview function count limit exceeded (256)'},
                {'kind': 'compile-summary', 'success': False, 'errors': 1, 'output': None}]

    def test_real_compile_summary_contract_is_accepted(self):
        verifier.rejected(self.result(self.rows()), 'function count limit exceeded (256)')

    def test_an_unrelated_failure_or_success_cannot_qualify_a_size_refusal(self):
        mutations = [(0, 'code', 'E0100'), (0, 'stage', 'parse'),
                     (0, 'message', 'unrelated error'), (1, 'kind', 'build-summary'),
                     (1, 'success', True)]
        for index, key, value in mutations:
            rows = copy.deepcopy(self.rows())
            rows[index][key] = value
            with self.subTest(index=index, key=key):
                with self.assertRaises(ValueError):
                    verifier.rejected(self.result(rows), 'function count limit exceeded (256)')
        for code, stderr in [(0, b''), (101, b''), (1, b'unstructured error')]:
            with self.assertRaises(ValueError):
                verifier.rejected(self.result(self.rows(), code, stderr), 'function count limit exceeded (256)')

    def test_missing_or_duplicate_diagnostics_and_summaries_are_refused(self):
        rows = self.rows()
        for changed in [rows[:1], rows[1:], rows + rows[:1], rows + rows[1:]]:
            with self.assertRaises(ValueError):
                verifier.rejected(self.result(changed), 'function count limit exceeded (256)')

    def test_source_work_guard_must_keep_its_own_stage_and_resource_note(self):
        rows = [{'kind': 'diagnostic', 'code': 'E0400', 'stage': 'oir-owned-lower',
                 'notes': ['resource: ownership work']},
                {'kind': 'compile-summary', 'success': False}]
        verifier.verification_refused(self.result(rows))
        for key, value in [('code', 'E0700'), ('stage', 'native-admission'), ('notes', [])]:
            changed = copy.deepcopy(rows)
            changed[0][key] = value
            with self.assertRaises(ValueError):
                verifier.verification_refused(self.result(changed))


if __name__ == '__main__':
    unittest.main()
