"""Exact type and source-binding evidence for the bounded alias amendment."""
import copy,unittest
import test_alias_amendment as a

class AliasEvidenceEdges(unittest.TestCase):
    def test_secondary_offsets_do_not_coerce_floating_point_values(self):
        case=next(c for c in a.m.corpus() if c.identifier=='alias-4-0110-ssxx')
        expected=a.m.case_expectation(case)['expected'];pair=a.INVENTORY[case.identifier]['first_conflict_per_root'][0]
        for label in (0,1):
            for offset in ('start','end'):
                bad=a.diagnostic(pair);bad['secondary'][label]['span'][offset]=float(bad['secondary'][label]['span'][offset])
                with self.subTest(label=label,offset=offset):
                    with self.assertRaises(ValueError):a.h.match_diagnostic([bad],expected)

    def test_each_root_binding_is_the_named_local_at_the_frozen_declaration(self):
        for case in a.m.alias_cases():
            expected=a.m.case_expectation(case);bindings={x['identity']:x for x in expected['facts']['bindings']}
            roots={tuple(pair['declaration']):pair['root'] for pair in a.INVENTORY[case.identifier]['first_conflict_per_root']}
            for alternative in expected['expected'].get('diagnostic_alternatives',[]):
                binding=bindings[alternative['root_binding']]
                self.assertEqual(binding['class'],'local')
                self.assertEqual(binding['span'],alternative['declaration_span'])
                self.assertEqual(binding['name'],roots[tuple(alternative['declaration_span'])])

    def test_if_scope_reuse_has_independently_recomputed_schedule(self):
        case=next(c for c in a.m.corpus() if c.identifier=='lexical-reuse');value=a.m.case_expectation(case)
        self.assertEqual(value['expected']['result'],2)
        frame=value['schedule']['frames']['main']
        self.assertEqual(tuple(frame[k] for k in ('S','A','P','O','R','L','C','X')),(4,0,4,4,0,0,0,24))
        expected=[('Root',25),('Scalar',1),('Branch',1),
                  ('Scalar',1),('StorageLive',1),('Construct',2),('StorageLive',1),('MoveInitialize',2),('StorageEnd',2),
                  ('StorageEnd',2),('Goto',1),
                  ('Scalar',1),('StorageLive',1),('Construct',2),('StorageLive',1),('MoveInitialize',2),('StorageEnd',2),
                  ('ReadField',1),('StorageEnd',2),('ReturnScalar',5)]
        self.assertEqual([(i['operation'],i['cost']) for i in value['schedule']['items']],expected)
        self.assertEqual(value['schedule']['total_fuel'],56)

    def test_one_deterministic_ownership_failure_does_not_mean_reporting_both_alternatives(self):
        case=next(c for c in a.m.corpus() if c.identifier=='alias-4-0110-ssxx')
        expected=a.m.case_expectation(case)['expected'];pairs=a.INVENTORY[case.identifier]['first_conflict_per_root']
        with self.assertRaises(ValueError):a.h.match_diagnostic([a.diagnostic(pairs[0]),a.diagnostic(pairs[1])],expected)

if __name__=='__main__':unittest.main(verbosity=2)
