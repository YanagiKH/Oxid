"""Final targeted grammar/summary controls, authored without compiler observations."""
import json
import subprocess
import unittest
from test_reviewer_owned_source import m,h,expectation


class SummaryGrammarContractTests(unittest.TestCase):
    def completed(self,records):
        return subprocess.CompletedProcess([],0,b''.join((json.dumps(r)+'\n').encode() for r in records),b'')

    def test_repeated_summary_is_not_a_single_command_result(self):
        summary={'schema_version':1,'kind':'run-summary','success':True,'result':{'type':'i32','value':0}}
        with self.assertRaises(ValueError):h.parse_records(self.completed([summary,summary]),'run')

    def test_success_with_error_diagnostic_fails_closed(self):
        records=[{'schema_version':1,'kind':'diagnostic','stage':'oir-run','code':'E0604'},
                 {'schema_version':1,'kind':'run-summary','success':True,'result':{'type':'i32','value':0}}]
        with self.assertRaises(ValueError):h.parse_records(self.completed(records),'run')

    def test_schema_and_success_have_exact_scalar_types(self):
        for schema,success in ((True,True),(1,1),(1,1.0)):
            with self.subTest(schema=schema,success=success):
                with self.assertRaises(ValueError):h.parse_records(self.completed([
                    {'schema_version':schema,'kind':'run-summary','success':success,'result':{'type':'i32','value':0}}]),'run')

    def test_nonparameter_reference_types_fail_before_name_resolution(self):
        for position in ('field','result','annotation'):
            with self.subTest(position=position):
                b=m.Builder();records=(m.Record('S'),m.Record('S'))
                if position=='field':records+=(m.Record('T',(('p','&S'),)),)
                main=m.Function('main',(),'&S' if position=='result' else 'i32',
                    b.block(b.let('x',b.i(0),annotation='&S') if position=='annotation' else b.discard(b.i(0)),b.ret(b.i(0))))
                try:result=expectation(m.Program(records,(main,)))
                except ValueError as error:
                    self.assertIn('MODEL_DOMAIN',str(error));continue
                self.assertEqual((result['expected']['stage'],result['expected']['code']),('parse','E0101'))
                self.assertEqual(result['source'].encode()[slice(*result['expected']['span'])],b'&')


if __name__=='__main__':unittest.main(verbosity=2)
