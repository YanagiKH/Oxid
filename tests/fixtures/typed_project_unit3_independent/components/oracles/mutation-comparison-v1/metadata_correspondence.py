"""Exact stored diagnostic origins, independently prescribed by source AST."""
import sys
if sys.flags.optimize:raise RuntimeError('Python assertions required')
import json
from pathlib import Path
from expected_projection import verify_manifest
from compare_static import eq
SUPPLEMENT=Path(__file__).resolve().parent.parent/'supplements/diagnostic-origin-correspondence-v1'
SHA='755fcf1e92d695ab5202481e3d88edd349d4faf382be8eb4a5db4e3df759f11d'
def check(case,raw):
    verify_manifest(SUPPLEMENT/'supplement-manifest.json',SHA)
    expected=json.loads((SUPPLEMENT/'expectations.json').read_text())
    eq(case.id,expected['case'],'source-defined metadata control');eq(case.case['route'],expected['route'],'metadata route')
    for wanted in expected['obligations']:
        function=raw['functions'][wanted['function']];eq(wanted['function'],function['id'],'metadata original function')
        found=[s for b in function['blocks'] for s in b['statements'] if s['kind']['tag']==wanted['operation'] and s['span']==wanted['operation_span']]
        eq(1,len(found),'unique actual raw instruction at independent source expression')
        eq({'tag':'DiagnosticOrigins','primary':wanted['primary'],'cause':wanted['cause']},found[0]['diagnostic_origins'],'exact stored primary and enclosing-statement cause')
    return dict(obligations=len(expected['obligations']),authority='independent source AST and established lowering-origin rule')
