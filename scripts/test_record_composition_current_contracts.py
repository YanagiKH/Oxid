"""Independent current contract data controls; historical facts remain immutable."""
import hashlib
import importlib.util
import json
from pathlib import Path
import subprocess
import sys
import unittest
import tempfile

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

    def test_extractor_uses_explicit_clean_checkout_and_fresh_output(self):
        spec = importlib.util.spec_from_file_location(
            'resource_source_binding', REPO / 'tests/fixtures/typed_project_source_binding/run.py')
        binding = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(binding)
        captured = binding.preflight(REPO)
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory); checkout = root / 'checkout'; output = root / 'probe'
            subprocess.run(['git', 'clone', '--quiet', '--shared', str(REPO), str(checkout)], check=True)
            self.assertEqual(subprocess.check_output(['git', '-C', str(checkout), 'status', '--porcelain']), b'')
            argv = [sys.executable, '-B', str(RESOURCE / 'derive-layout-probe.py'),
                    '--repo', str(checkout), '--tree', 'HEAD', '--output', str(output)]
            result = subprocess.run(argv, capture_output=True, text=True, timeout=30)
            self.assertEqual(result.returncode, 0, result.stderr)
            self.assertEqual((output / 'declaration-layout-probe.rs').read_bytes(),
                             (RESOURCE / 'declaration-layout-probe.rs').read_bytes())
            inputs = json.loads((output / 'declaration-layout-inputs.json').read_bytes())
            original = json.loads((RESOURCE / 'declaration-layout-inputs.json').read_bytes())
            # Whole-file provenance changes with checked unary negation; the
            # selected declaration shapes and independently derived probe do
            # not. Bind BOTH source views without retargeting historical facts.
            expected = json.loads(json.dumps(original['files']))
            for row in expected:
                path = row['path']
                self.assertEqual(hashlib.sha256(captured['composition_inputs'][path]).hexdigest(),
                                 row['sha256'], path)
                row['sha256'] = hashlib.sha256(captured['inputs'][path]).hexdigest()
                if path == 'src/frontend/oir/owned/source/hir.rs':
                    for declaration in row['declarations']:
                        if declaration['name'] in ('AccessBase', 'Projection'):
                            declaration['line'] += 4
            self.assertEqual(inputs['files'], expected)
            self.assertEqual([new['path'] for old, new in zip(original['files'], expected)
                              if old['sha256'] != new['sha256']],
                             ['src/frontend/hir.rs', 'src/frontend/oir/mod.rs',
                              'src/frontend/oir/owned/mod.rs', 'src/frontend/oir/owned/source/hir.rs'])
            self.assertEqual(inputs['requested_ref'], 'HEAD')
            self.assertEqual(inputs['tree'], subprocess.check_output(
                ['git', '-C', str(checkout), 'rev-parse', 'HEAD^{tree}'], text=True).strip())
            self.assertEqual(inputs['repository'], str(checkout))
            before = {p.name: p.read_bytes() for p in output.iterdir()}
            result = subprocess.run(argv, capture_output=True, text=True, timeout=30)
            self.assertNotEqual(result.returncode, 0)
            self.assertIn('output already exists', result.stderr)
            self.assertEqual(before, {p.name: p.read_bytes() for p in output.iterdir()})
        binding.assert_unchanged(REPO, captured)

    def test_extractor_missing_repository_or_source_object_fails_clearly(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            for repo, tree, message in ((root / 'missing', 'HEAD', 'repository directory does not exist'),
                                         (REPO, '0' * 40, 'Git source object is unavailable')):
                result = subprocess.run([sys.executable, '-B', str(RESOURCE / 'derive-layout-probe.py'),
                                         '--repo', str(repo), '--tree', tree, '--output', str(root / 'output')],
                                        capture_output=True, text=True, timeout=30)
                self.assertNotEqual(result.returncode, 0)
                self.assertIn(message, result.stderr)
                self.assertFalse((root / 'output').exists())

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
