#!/usr/bin/env python3
"""Check hand-derived current resource constants against independent layout probes."""
import json
from pathlib import Path
root=Path(__file__).resolve().parent
j=json.loads((root/'current-resource-appendix-v1.json').read_text())
a=j['independent_layouts']['composition'];b=j['independent_layouts']['predecessor'];k=j['derived_constants']
R,F=4096,65536
assert (a['RecordDecl']['bytes'],a['FieldDecl']['bytes'])==(72,64)
assert (b['RecordDecl']['bytes'],b['FieldDecl']['bytes'])==(64,56)
assert k['maximum_table_after']==R*a['RecordDecl']['bytes']+F*a['FieldDecl']['bytes']==4489216
assert k['maximum_table_before']==R*b['RecordDecl']['bytes']+F*b['FieldDecl']['bytes']==3932160
assert k['table_delta']==8*(R+F)==557056
assert k['maximum_table_after']<k['table_cap']==8*1024*1024
assert k['maximum_graph_element_payload']==R*(a['ContainmentSummary']['bytes']+1)+64*a['(usize,usize)']['bytes']==136192
assert k['maximum_table_plus_summary_element_payload']==k['maximum_table_after']+R*a['ContainmentSummary']['bytes']==4620288
assert k['scalar_only_maximum_sum_layout']==4*F+R-((F+1023)//1024)==266176
sizes=[1]
for _ in range(19):sizes.append(2*sizes[-1])
assert len(sizes)==20 and 1+2*(len(sizes)-1)==39
assert sum(sizes)==k['composition_dag_sum_before_empty']==1048575
assert sum(sizes)+1==k['sum_layout_cap']<sum(sizes)+2
assert a['ScalarLeaves']['bytes']==8+65*a['Option<(ValueTy, usize, usize)>']['bytes']+8==2096
assert a['(FieldId, FieldInitializer)']['bytes']==56 and a['FieldId']['bytes']==16
assert k['observer_fixed_auxiliary_after']-k['observer_fixed_auxiliary_before']==32
assert k['projection_path_elements_cap']==(64*1024*1024)//a['FieldId']['bytes']==4194304
print('Independent resource arithmetic: pass; no candidate semantic output used')
