"""Negative controls over observed receipts; never additional native executions."""
import copy,json,os,sys,unittest
from pathlib import Path
sys.path.insert(0,os.environ.get('NATIVE_MATCHER_SCRIPTS',str(Path(__file__).parent)))
import owned_source_native as n

class NativeMatcherControls(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        if 'NATIVE_OBSERVATIONS' not in os.environ: raise unittest.SkipTest('requires actual collected native receipts')
        cls.root=Path(os.environ['NATIVE_OBSERVATIONS'])
        fixture=Path(n.h.__file__).resolve().parents[1]/'tests/fixtures/owned_source/native-probe-sources.json'
        cls.items={item['id']:{**item,'source':source} for source,item in n.h.native_probe_expectations(fixture)}
        cls.directory=cls.root/'debug/guarded-overflow-after-write'
        cls.item=cls.items['guarded-overflow-after-write']
        cls.terminal=n.strict_json(cls.directory/'budget-67.json')

    def test_coherent_reference_native_wrong_success_is_rejected(self):
        row=copy.deepcopy(self.terminal);row['reference']={'result_type':'i32','result':42,'failure':None}
        row['native'].update(status=0,stdout='42\n',stderr='',diagnostic_stderr='',code=None,source_location=None)
        with self.assertRaises(ValueError):n.native_outcome(self.item,row,self.item['id']+'.ox')

    def test_each_native_terminal_field_is_independent(self):
        for key,value in [('status',True),('status',0),('stdout','42\n'),('diagnostic_stderr',''),('code','E0601'),('source_location','wrong:1:1')]:
            row=copy.deepcopy(self.terminal);row['native'][key]=value
            with self.subTest(key=key),self.assertRaises(ValueError):n.native_outcome(self.item,row,self.item['id']+'.ox')

    def test_probe_wire_payload_is_not_a_trusted_summary(self):
        row=n.strict_json(self.directory/'probe-67.json');row['native']['stores'][0]['bits']+=1
        with self.assertRaisesRegex(ValueError,'stderr/store correspondence'):n.native_outcome(self.item,row,self.item['id']+'.ox')

    def test_frozen_store_value_and_field_identity_are_checked(self):
        row=n.strict_json(self.directory/'probe-67.json');raw=n.strict_json(self.directory/'raw-view.json');catalog=n.strict_json(self.directory/'catalog.json');enriched=n.strict_json(self.directory/'payload-enrichment.json')
        actual=n.semantic_stores(self.item,raw,catalog,enriched,row['native']['stores']);wanted=n.h.expected_budget_row(self.item,67)['committed_stores'];n.same(wanted,actual,'baseline')
        site=next(s['site'] for s in catalog['stores'] if s['kind']=='WriteField' and any(e['site']==s['site'] for e in row['native']['stores']))
        changed=copy.deepcopy(row['native']['stores']);next(e for e in changed if e['site']==site)['bits']+=1
        with self.assertRaises(ValueError):n.same(wanted,n.semantic_stores(self.item,raw,catalog,enriched,changed),'changed stored value')
        altered=copy.deepcopy(enriched);next(e for e in altered['sites'] if e['site']==site)['field']['index']=99
        with self.assertRaises(ValueError):n.semantic_stores(self.item,raw,catalog,altered,row['native']['stores'])

    def test_exact_scalar_bits(self):
        self.assertEqual(n.bits_value(4294967295,'i32'),-1)
        self.assertEqual(n.bits_value(18446744073709551615,'i32'),-1)
        self.assertIs(n.bits_value(0,'bool'),False)
        for value,ty in [(True,'i32'),(-1,'i32'),(1<<64,'i32'),(2,'bool'),(1,'()')]:
            with self.assertRaises(ValueError):n.bits_value(value,ty)

if __name__=='__main__':unittest.main()
