#!/usr/bin/env python3
"""Check source-prescribed metadata against retained actuals and stream clones."""
import sys
if sys.flags.optimize:raise RuntimeError('Python assertions required')
from pathlib import Path
import copy,hashlib,json
from compare_mutations import HERE,ROOT,load
from metadata_correspondence import check
def main():
    case=load()['negative-exclusive_then_read'];rows=[]
    for profile in ('debug','release'):
        for field in ('primary','cause'):
            p=ROOT/f'typed-project-unit3-mutations/full-mutations-v1/{profile}/metadata-same-file-{field}/receipt.json'
            e=json.loads(p.read_text());baseline=json.loads(Path(e['baseline_observations']['path']).read_text());mutant=json.loads(Path(e['mutant_observations']['path']).read_text())
            check(case,baseline['raw_before_audit'])
            try:check(case,mutant['raw_before_audit'])
            except AssertionError as error:reason=repr(error)
            else:raise AssertionError(('wrong stored metadata escaped',field,profile))
            rows.append(dict(profile=profile,field=field,receipt_sha256=hashlib.sha256(p.read_bytes()).hexdigest(),baseline='MATCH',mutation='REJECTED',reason=reason,rendered_diagnostics_equal=baseline['diagnostics']==mutant['diagnostics']))
    assert all(r['rendered_diagnostics_equal'] for r in rows if r['field']=='cause')
    report=dict(schema='unit3-exact-source-metadata-controls-v1',actual_controls=rows,source_expectations='separate source-only AST supplement; no actual value used to prescribe metadata',compiler_invocations=0,actual_artifact_edits=0,original_expected_edits=0)
    (HERE/'metadata-correspondence-sensitivity.json').write_text(json.dumps(report,indent=2)+'\n');print('Four actual primary/cause controls rejected against source origins; baseline matches')
if __name__=='__main__':main()
