#!/usr/bin/env python3
"""Extract declaration shapes from a pinned git tree, without implementation code."""
import argparse,hashlib,json,re,subprocess
from pathlib import Path
ROOT=Path(__file__).resolve().parent
REPO=ROOT.parent/'oxid-record-composition'
parser=argparse.ArgumentParser(); parser.add_argument('--tree',default='f6b7dee8bac4ebcc27ad020db9940344c5e4ae41'); parser.add_argument('--stem',default='declaration-layout'); args=parser.parse_args(); TREE=args.tree; STEM=args.stem
SPECS={
 'src/frontend/source.rs':['SourceFileId','Span'],
 'src/frontend/hir.rs':['Ty'],
 'src/frontend/oir/owned_types.rs':['RecordId','FixedArrayTy','AggregateTy','BorrowedTy','FieldId','OwnerPlaceId','ValueTy','BorrowKind','ParameterTy','RawRecordDecl','RawFieldDecl','Layout','FieldDecl','RecordDecl','DeclarationUsage','Declarations','ContainmentSummary','ScalarLeaf','ScalarLeaves'],
 'src/frontend/oir/mod.rs':['LocalId','Operand'],
 'src/frontend/oir/owned/mod.rs':['FieldInitializer'],
 'src/frontend/oir/owned/budget.rs':['FunctionCounts'],
 'src/frontend/oir/owned/source/hir.rs':['BindingId','Record','Field','AccessBase','Projection'],
}
rows=[];parts=[]
for path,names in SPECS.items():
 raw=subprocess.check_output(['git','-C',str(REPO),'show',f'{TREE}:{path}']);text=raw.decode()
 row={'path':path,'sha256':hashlib.sha256(raw).hexdigest(),'declarations':[]}
 for name in names:
  m=re.search(r'\b(?:struct|enum) '+re.escape(name)+r'\b',text)
  if m is None:
   assert name in ['ContainmentSummary','ScalarLeaf','ScalarLeaves','FieldInitializer'],name
   continue
  begin=m.start();brace=text.find('{',begin);semi=text.find(';',begin)
  if semi>=0 and semi<brace:end=semi+1
  else:
   depth=1;end=brace+1
   while depth:
    depth+=(text[end]=='{')-(text[end]=='}');end+=1
  exact=text[begin:end]
  clean=re.sub(r'\bpub(?:\([^)]*\))?\s*','',exact).replace('hir::Ty','Ty')
  # No repr attributes exist on these selected declarations; explicitly refuse future ones.
  pre=text[max(0,begin-180):begin];assert not re.search(r'#\[repr[^\]]*\]\s*(?:#\[[^\]]*\]\s*)*(?:pub(?:\([^)]*\))?\s*)?$',pre),name
  parts.append(clean)
  row['declarations'].append({'name':name,'line':text[:begin].count('\n')+1,'exact_sha256':hashlib.sha256(exact.encode()).hexdigest()})
 rows.append(row)
names=[d['name'] for row in rows for d in row['declarations']]+['Option<Projection>','(FieldId, Operand)','Option<(ValueTy, usize, usize)>','[Option<(ValueTy, usize, usize)>;65]','(usize,usize)']
if 'FieldInitializer' in names:names+=['(FieldId, FieldInitializer)']
program='#![allow(dead_code)]\nconst MAX_CONTAINMENT_DEPTH: usize = 64;\n'+'\n'.join(parts)+'\nfn main(){\n'
for name in names:
 ty=name+"<'_>" if name=='ScalarLeaves' else name
 program+=f'println!("{{}} {{}} {{}}", "{name}", std::mem::size_of::<{ty}>(), std::mem::align_of::<{ty}>());\n'
program+='}\n';(ROOT/(STEM+'-probe.rs')).write_text(program)
(ROOT/(STEM+'-inputs.json')).write_text(json.dumps({'tree':TREE,'method':'Exact type declarations only; visibility removed and hir::Ty path shortened. No implementation methods or candidate outputs used. Selected types have no repr attributes. Derive attributes intentionally absent because they do not affect representation. The depth64 constant is RFC0020 contract.','files':rows,'probe_sha256':hashlib.sha256(program.encode()).hexdigest()},indent=2)+'\n')
