"""Independent current contract data controls; historical facts remain immutable."""
import hashlib
import json
from pathlib import Path
import subprocess
import sys
import unittest

REPO = Path(__file__).resolve().parents[1]
RESOURCE = REPO / 'tests/qualification/record_composition_resource_v1'


class CurrentCompositionContractTests(unittest.TestCase):
    def test_complete_independent_resource_evidence_seal(self):
        manifest = json.loads((RESOURCE / 'artifact-manifest.json').read_bytes())
        self.assertEqual(len(manifest['files']), 30)
        self.assertEqual({p.name for p in RESOURCE.iterdir()},
                         {r['path'] for r in manifest['files']} | {'artifact-manifest.json'})
        for row in manifest['files']:
            raw = (RESOURCE / row['path']).read_bytes()
            self.assertEqual(len(raw), row['bytes'], row['path'])
            self.assertEqual(hashlib.sha256(raw).hexdigest(), row['sha256'], row['path'])

    def test_independently_derived_resource_arithmetic(self):
        result = subprocess.run([sys.executable, '-B', str(RESOURCE / 'check-resource-arithmetic.py')],
                                capture_output=True, text=True, timeout=20)
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertIn('no candidate semantic output used', result.stdout)

    def test_array_record_case_amendment_preserves_unexecuted_historical_fact(self):
        amendment = json.loads((REPO / 'docs/architecture/record-composition-semantic-amendment-v1.json').read_bytes())
        historical = amendment['historical_manifest']
        raw = (REPO / historical['path']).read_bytes()
        self.assertEqual(len(raw), historical['bytes'])
        self.assertEqual(hashlib.sha256(raw).hexdigest(), historical['sha256'])
        case = amendment['case']
        original = next(r for r in json.loads(raw)['cases'] if r['id'] == 'excluded-array-record-field')
        self.assertEqual(case['historical_row'], original)
        self.assertEqual(original['status'], 'FROZEN_EXPECTATION_NOT_EXECUTED')
        self.assertEqual(original['expected']['diagnostics'][0]['code'], 'E0101')
        source = (REPO / case['source']['path']).read_bytes()
        self.assertEqual(hashlib.sha256(source).hexdigest(), case['source']['sha256'])
        self.assertEqual(source, b'struct R { a: [i32; 1] }\nfn main() -> i32 { return 0; }\n')
        self.assertEqual(case['current_expected'], {
            'status': 'accept', 'function_count': 1, 'result_type': 'i32', 'result': 0})
        self.assertIs(amendment['historical_execution_claim'], False)
        self.assertIs(amendment['current_execution_claim'], False)


if __name__ == '__main__':
    unittest.main()
