#!/usr/bin/env python3
"""Hand-authored source construction facts and finite trace arithmetic, not an Oxid parser/compiler.
No Oxid executable, candidate dump, mutable checkout or remote is read. Source inputs are named blobs.
"""
from pathlib import Path
import hashlib,json,subprocess
ROOT=None # Set by replay.py after zero-overwrite output admission.
REPO=None # Set from replay.py --repo; no network or checkout dependency.
COMMIT='bb2b5d7c39d5940cbbf959444646c2dacb8edcb4'
TREE='a5398061eecc0e35726b648bf7edcd56434e1337'
BASE='tests/fixtures/fixed_array_source_unit3/contracts-v2/fixtures/'
def sha(b): return hashlib.sha256(b).hexdigest()
def dump(path,obj):
 p=ROOT/path;p.parent.mkdir(parents=True,exist_ok=True);p.write_bytes((json.dumps(obj,indent=2,ensure_ascii=False)+'\n').encode('utf-8'))
def blob(path): return subprocess.check_output(['git','-C',REPO,'show',COMMIT+':'+path])
class Source:
 def __init__(self,name,text,original=None):
  self.name,self.text,self.original=name,text,original
  (ROOT/'fixtures'/name).mkdir(parents=True,exist_ok=True)
  (ROOT/'fixtures'/name/'main.ox').write_bytes(text.encode('utf-8'))
 def at(self,token,occurrence=0):
  # Exact byte anchoring of hand-selected tokens/whole expressions; no grammar inference.
  bs=self.text.encode(); token=token.encode(); p=-1
  for _ in range(occurrence+1):p=bs.index(token,p+1)
  return {'file':0,'start':p,'end':p+len(token)}
 def within(self,container,token,occurrence=0):
  start=self.at(container)['start']; bs=container.encode(); needle=token.encode(); p=-1
  for _ in range(occurrence+1):p=bs.index(needle,p+1)
  return {'file':0,'start':start+p,'end':start+p+len(needle)}
 def binding(self):return {'path':'main.ox','file':0,'bytes':len(self.text.encode()),'sha256':sha(self.text.encode()),'reused_path':self.original}
class Function:
 def __init__(self,s,id,name):
  self.s=s;self.d={'id':id,'name':name,'result':'i32','span':s.at(name),'parameters':[],'bindings':[],'hir':[],'roots':[],'locals':[],'places':[],'owners':[],'references':[],'calls':[],'loans':[],'blocks':[{'id':0,'span':s.at(name),'merge':None,'statements':[],'terminator':None}]};self.block=0
 def expr(self,kind,span,ty='i32',**fields):
  id=len(self.d['hir']); e={'id':id,'kind':kind,'span':span,'type':ty,**fields}
  if isinstance(ty,str):
   local=len(self.d['locals']);self.d['locals'].append({'id':local,'ty':ty,'kind':'Temporary','span':span});e['local']=local
  self.d['hir'].append(e);return id
 def operand(self,e):return {'local':self.d['hir'][e]['local'],'span':self.d['hir'][e]['span']}
 def owner(self,kind,span,n):
  id=len(self.d['owners']);self.d['owners'].append({'id':id,'kind':kind,'span':span,'type':{'FixedArray':['i32',n]}});return id
 def row(self,kind,span,primary=None,cause=None,**fields):
  row={'kind':kind,'span':span,'origins':{'primary':primary or span,'cause':cause or span},**fields}
  self.d['blocks'][self.block]['statements'].append(row);return row
 def scalar(self,e,value,cause):return self.row('ScalarAssign',self.d['hir'][e]['span'],cause=cause,destination=self.d['hir'][e]['local'],value=value)
 def literal(self,e,n,cause):
  span=self.d['hir'][e]['span'];o=self.owner('Temporary',span,n);self.d['hir'][e]['owner']=o
  self.row('StorageLive',span,cause=cause,owner=o);self.row('ConstructArray',span,cause=cause,destination=o,elements=[self.operand(i) for i in self.d['hir'][e]['elements']]);return o
 def let_owner(self,e,name_span,stmt,n):
  src=self.d['hir'][e]['owner'];o=self.owner({'Local':{'mutable':True}},name_span,n)
  self.d['bindings'].append({'id':0,'span':name_span,'mutable':True,'scope':0,'parameter_position':None,'raw':{'Owner':o},'type':{'FixedArray':['i32',n]}})
  self.row('StorageLive',stmt,primary=name_span,owner=o);self.row('MoveInitialize',stmt,primary=name_span,destination=o,source=src);self.row('StorageEnd',stmt,primary=self.d['hir'][e]['span'],owner=src);return o
 def end(self,kind,span,**fields):self.d['blocks'][self.block]['terminator']={'kind':kind,'span':span,'origins':{'primary':span,'cause':span},**fields}
 def finish(self):
  d=self.d; rows=[r for b in d['blocks'] for r in b['statements']]; ends=[b['terminator'] for b in d['blocks']]
  c={key:len(d[key]) for key in ['locals','places','owners','references','parameters','calls','loans','blocks']}
  c.update(edges=sum(2 if e['kind']=='Branch' else int(e['kind'] in ['Goto','Invoke']) for e in ends),statements=len(rows),merges=sum(b['merge'] is not None for b in d['blocks']),descriptor_arguments=sum(len(x['arguments']) for x in d['calls']),preparations=sum(r['kind'].startswith('Prepare') for r in rows),constructed_fields=0,constructed_elements=sum(len(r['elements']) for r in rows if r['kind']=='ConstructArray'),diagnostic_origins=len(rows)+len(ends),max_constructor_fields=0,ownership_active=True)
  d['counts_all_four_passes']=c;d['completion_order']=list(range(len(d['hir'])))
  W=sum(max(1,o['type']['FixedArray'][1]) for o in d['owners']);V=c['locals']+c['places'];A=c['descriptor_arguments'];O=c['owners'];R=c['references'];L=c['loans'];C=c['calls'];X=V+A+W+4*O+8*R+12*L+2*C
  d['frame_model']={'V':V,'A':A,'W':W,'O':O,'R':R,'L':L,'C':C,'X':X,'root_activation_cost':1+X,'return_scalar_cost':1+W+L+C+R,'owner_widths':[max(1,o['type']['FixedArray'][1]) for o in d['owners']]}
  S=c['statements']+c['merges'];AA=A+c['preparations'];Q=c['constructed_elements'];n=1+2*c['blocks']+c['edges']+S+AA+Q+O+L+C+c['parameters']+R+V
  d['resource_model']={'local_expanded_S_plus_A_plus_F_plus_Q':S+AA+Q,'ownership_n':n,'ownership_work':(4*O+L+C+32)*n+4*c['diagnostic_origins'],'literal_operand_reservations_emit':[len(r['elements']) for r in rows if r['kind']=='ConstructArray'],'literal_operand_reservations_each_count_pass':[],'literal_cursor_child_visits_each_pass':Q,'literal_cursor_final_visits_each_pass':sum(r['kind']=='ConstructArray' for r in rows),'operand_copies_emit':Q,'measured_rust_widths':'pending, not measured by this model'}
  return d

def snapshot(name,index_return=0,rhs_bad=False,index_bad=False):
 old=BASE+('rhs-snapshot-final-bounds-failure' if index_return else 'rhs-snapshot-success')+'/main.ox';text=blob(old).decode()
 if rhs_bad:text=text.replace('= a[0];','= a[1];')
 if index_bad:text=text.replace('return 0; }','return a[1]; }',1)
 s=Source(name,text,None if rhs_bad or index_bad else old);a={'FixedArray':['i32',1]}
 touch=Function(s,0,'touch');main=Function(s,1,'main')
 ts=s.at('a[0] = 99;');tr=s.at('return a[1];' if index_bad else f'return {index_return};');tp=s.within('touch(a:','a')
 touch.d['parameters']=[{'Reference':0}];touch.d['references']=[{'id':0,'position':0,'kind':'Exclusive','span':tp,'type':a}];touch.d['bindings']=[{'id':0,'span':tp,'mutable':False,'scope':0,'parameter_position':0,'raw':{'Reference':0},'type':{'Reference':{'kind':'Exclusive','aggregate':a}}}]
 touch.expr('I32',s.at('99'),value=99);touch.expr('I32',s.within('a[0] = 99;','0'),value=0)
 if index_bad:
  touch.expr('I32',s.within('return a[1];','1'),value=1);touch.expr('IndexRead',s.within('return a[1];','a[1]'),base=0,base_span=s.within('return a[1];','a'),index=2)
 else:touch.expr('I32',s.within(f'return {index_return};',str(index_return)),value=index_return)
 touch.d['roots']=[{'statement':'IndexAssign','span':ts,'value':0,'index':1,'base':0,'base_span':s.within('a[0] = 99;','a'),'target_span':s.within('a[0] = 99;','a[0]'),'operator_span':s.within('a[0] = 99;','=')},{'statement':'Return','span':tr,'value':3 if index_bad else 2}]
 touch.scalar(0,{'I32':99},ts);touch.scalar(1,{'I32':0},ts);touch.row('WriteIndex',ts,primary=s.within('a[0] = 99;','a[0]'),base={'Parameter':0},index=touch.operand(1),value=touch.operand(0));touch.scalar(2,{'I32':1 if index_bad else index_return},tr)
 if index_bad:touch.row('ReadIndex',touch.d['hir'][3]['span'],cause=tr,destination=3,base={'Parameter':0},index=touch.operand(2))
 touch.end('ReturnScalar',tr,value=touch.operand(3 if index_bad else 2))
 let=s.at('let mut a = [5];');store=s.at('a[touch(&mut a)] = a[1];' if rhs_bad else 'a[touch(&mut a)] = a[0];');ret=s.at('return a[0];');rhs='a[1]' if rhs_bad else 'a[0]'
 main.expr('I32',s.within('[5]','5'),value=5);main.expr('ArrayLiteral',s.at('[5]'),a,elements=[0]);main.expr('I32',s.within('= '+rhs+';',str(int(rhs_bad))),value=int(rhs_bad));main.expr('IndexRead',s.within('= '+rhs+';',rhs),base=0,base_span=s.within('= '+rhs+';','a'),index=2)
 call=s.at('touch(&mut a)');borrow=s.at('&mut a');main.expr('Call',call,target=0,args=[{'Borrow':{'kind':'Exclusive','place':{'Owner':0},'span':borrow,'name_span':s.within('&mut a','a'),'star_span':None}}]);main.expr('I32',s.within('return a[0];','0'),value=0);main.expr('IndexRead',s.within('return a[0];','a[0]'),base=0,base_span=s.within('return a[0];','a'),index=5)
 main.d['roots']=[{'statement':'Let','span':let,'binding':0,'init':1},{'statement':'IndexAssign','span':store,'base':0,'value':3,'index':4,'target_span':s.at('a[touch(&mut a)]'),'base_span':s.within('a[touch(&mut a)]','a'),'operator_span':s.within('a[touch(&mut a)] =','=')},{'statement':'Return','span':ret,'value':6}]
 main.scalar(0,{'I32':5},let);main.literal(1,1,let);owner=main.let_owner(1,s.within('let mut a','a'),let,1)
 main.scalar(2,{'I32':int(rhs_bad)},store);main.row('ReadIndex',main.d['hir'][3]['span'],cause=store,destination=2,base={'Owner':owner},index=main.operand(2))
 main.d['calls']=[{'id':0,'target':0,'target_identity':{'file':0,'name':'touch','declaration':s.at('touch')},'arguments':[{'Borrow':0}],'result':{'Scalar':3},'parent':None,'span':call}];main.d['loans']=[{'id':0,'call':0,'argument':0,'authority':{'Owner':owner},'kind':'Exclusive','type':a,'span':borrow}]
 main.row('OpenCall',call,cause=store,call=0);main.row('PrepareBorrow',borrow,call=0,argument=0,loan=0);main.end('Invoke',call,call=0,continuation=1);main.d['blocks'][0]['terminator']['origins']['cause']=store
 main.d['blocks'].append({'id':1,'span':call,'merge':None,'statements':[],'terminator':None});main.block=1
 main.row('WriteIndex',store,primary=s.at('a[touch(&mut a)]'),base={'Owner':owner},index=main.operand(4),value=main.operand(3));main.scalar(5,{'I32':0},ret);main.row('ReadIndex',main.d['hir'][6]['span'],cause=ret,destination=5,base={'Owner':owner},index=main.operand(5));main.row('StorageEnd',ret,owner=owner);main.end('ReturnScalar',ret,value=main.operand(6))
 functions=[touch.finish(),main.finish()]
 effects={
 (1,0,0):['define main.local0 = 5'],(1,0,1):['main.owner0 Dead/g0 -> Uninitialized/g1'],(1,0,2):['main.owner0 payload [5], Available/g2'],(1,0,3):['main.owner1 Dead/g0 -> Uninitialized/g1'],(1,0,4):['transfer [5] owner0/g2 -> owner1/g1','main.owner0 Moved/g3; owner1 Available/g2'],(1,0,5):['main.owner0 Dead/g4'],(1,0,6):[f'define main.local1 = {int(rhs_bad)}'],(1,0,7):['read owner1[0] = 5; define main.local2 = 5'],(1,0,8):['main.call0 Preparing; zero owned stages'],(1,0,9):['acquire main.loan0/instance1 Exclusive on root(frame0,activation1,owner1,g2)','owner1 exclusive_children = 1'],(1,0,'end'):['main.call0 InFlight','enter touch frame1 activation2; reference0 forwards main.loan0; no owner parameter or result owner'],(0,0,0):['define touch.local0 = 99'],(0,0,1):['define touch.local1 = 0'],(0,0,2):['write root owner1[0] = 99; generation stays2'],(0,0,3):[f'define touch.local2 = {1 if index_bad else index_return}'],(0,0,'end'):[f'define main.local3 = {index_return}','release main.loan0; owner1 exclusive_children = 0','main.call0 Closed; drop touch frame; branch main.block1'],(1,1,0):['write root owner1[0] = saved main.local2 = 5; generation stays2'],(1,1,1):['define main.local4 = 0'],(1,1,2):['read owner1[0] = 5; define main.local5 = 5'],(1,1,3):['main.owner1 Dead/g3; payload remains [5]'],(1,1,'end'):['return5; drop main frame']}
 order=[(1,0,i) for i in range(10)]+[(1,0,'end')]+[(0,0,i) for i in range(len(functions[0]['blocks'][0]['statements']))]+[(0,0,'end')]+[(1,1,i) for i in range(4)]+[(1,1,'end')]
 failing=(1,0,7) if rhs_bad else (0,0,4) if index_bad else (1,1,0) if index_return else None
 result={'schema':'oxid-independent-array-lowering-v1','case':name,'source':s.binding(),'entry':1,'functions':functions,'static_ownership':'Accept under agreed new lowering templates; authoritative verifier has not run','frame_identities':{'main':[0,1],'touch':[1,2]},'absent_identities':['no staged owned argument','no owned parameter','no owned result','no target HIR wrapper or target-read local'],'runtime':trace(functions,1,order,effects,failing)}
 return result

def cost(f,r,functions):
 fm=f['frame_model'];k=r['kind'];ws=fm['owner_widths']
 if k in ['StorageEnd','Discard']:return 1+ws[r['owner']]
 if k=='ConstructArray':return 1+ws[r['destination']]
 if k in ['MoveInitialize','PrepareOwned']:return 1+ws[r['source']]
 if k=='Replace':return 1+2*ws[r['source']]
 if k=='OpenCall':return 1+sum('Owned' in a for a in f['calls'][r['call']]['arguments'])
 if k=='Invoke':
  c=f['calls'][r['call']];b=sum('Borrow' in a for a in c['arguments']);return 1+len(c['arguments'])+functions[c['target']]['frame_model']['X']+sum(ws[a['Owned']] for a in c['arguments'] if 'Owned' in a)+b*(b-1)//2
 if k=='ReturnScalar':return fm['return_scalar_cost']
 if k=='ReturnOwned':return fm['return_scalar_cost']+ws[r['owner']]
 return 1

def trace(functions,entry,order,effects,failing=None):
 f=functions[entry];steps=[{'identity':'root-activation','cost':f['frame_model']['root_activation_cost'],'span':f['span'],'effects':[f'enter {f["name"]} frame0 activation1; all slots allocated, owners Dead/g0']}]
 for ident in order:
  fi,bi,si=ident;fun=functions[fi];block=fun['blocks'][bi];r=block['terminator'] if si=='end' else block['statements'][si]
  span=r['origins']['primary'] if r['kind'] in ['ReadIndex','WriteIndex','ArrayLength'] else r['span']
  steps.append({'identity':list(ident),'kind':r['kind'],'cost':cost(fun,r,functions),'span':span,'effects':effects.get(ident,[]),'paid_failure':'E0606' if ident==failing else None})
  if ident==failing:steps[-1]['effects']=[];break
 cumulative=0
 for step in steps:step['fuel_before']=cumulative;cumulative+=step['cost'];step['fuel_after']=cumulative
 budgets=[]
 for fuel in range(cumulative+1):
  paid=[];remaining=fuel;terminal=None
  for j,step in enumerate(steps):
   if remaining<step['cost']:terminal={'code':'E0601','span':step['span'],'at_step':j};break
   remaining-=step['cost'];paid.append(j)
   if step.get('paid_failure'):terminal={'code':step['paid_failure'],'span':step['span'],'at_step':j};break
  budgets.append({'fuel':fuel,'paid_steps':paid,'remaining':remaining,'terminal':terminal or {'result':5 if len(functions)>1 else 2}})
 return {'claim':'source-derived reference contract, not observed execution or native machine evidence','steps':steps,'terminal_budget':cumulative,'every_budget_0_through_terminal':budgets,'failure_rule':'a failing charge has no Charge event, effects or fuel subtraction; a paid bounds failure has a Charge event but no WriteIndex/ReadIndex event','no_unwind_claim':'Failure snapshots may retain live frames/loans; normal-return release applies only after a paid successful return'}

def grouped():
 name='grouped-complete-access-and-index';original=BASE+name+'/main.ox';s=Source(name,blob(original).decode(),original);f=Function(s,0,'main');arr={'FixedArray':['i32',2]};let=s.at('let mut a: [i32; 2] = [5, 7];');store=s.at('a[(0)] = (a[1]);');ret=s.at('return (a.len());')
 facts=[('I32',s.within('[5, 7]','5'),'i32',{'value':5}),('I32',s.within('[5, 7]','7'),'i32',{'value':7}),('ArrayLiteral',s.at('[5, 7]'),arr,{'elements':[0,1]}),('I32',s.within('a[1]','1'),'i32',{'value':1}),('IndexRead',s.at('a[1]'),'i32',{'base':0,'base_span':s.within('a[1]','a'),'index':3}),('Group',s.at('(a[1])'),'i32',{'inner':4}),('I32',s.within('(0)','0'),'i32',{'value':0}),('Group',s.at('(0)'),'i32',{'inner':6}),('ArrayLength',s.at('a.len()'),'i32',{'base':0,'base_span':s.within('a.len()','a')}),('Group',s.at('(a.len())'),'i32',{'inner':8})]
 for kind,span,ty,more in facts:f.expr(kind,span,ty,**more)
 f.d['roots']=[{'statement':'Let','span':let,'binding':0,'init':2},{'statement':'IndexAssign','span':store,'base':0,'base_span':s.within('a[(0)]','a'),'target_span':s.at('a[(0)]'),'operator_span':s.within('a[(0)] =','='),'value':5,'index':7},{'statement':'Return','span':ret,'value':9}]
 f.scalar(0,{'I32':5},let);f.scalar(1,{'I32':7},let);f.literal(2,2,let);o=f.let_owner(2,s.within('let mut a:','a'),let,2);f.scalar(3,{'I32':1},store);f.row('ReadIndex',s.at('a[1]'),cause=store,destination=3,base={'Owner':o},index=f.operand(3));f.scalar(5,{'Copy':f.operand(4)},store);f.scalar(6,{'I32':0},store);f.scalar(7,{'Copy':f.operand(6)},store);f.row('WriteIndex',store,primary=s.at('a[(0)]'),base={'Owner':o},index=f.operand(7),value=f.operand(5));f.row('ArrayLength',s.at('a.len()'),cause=ret,destination=7,base={'Owner':o});f.scalar(9,{'Copy':f.operand(8)},ret);f.row('StorageEnd',ret,owner=o);f.end('ReturnScalar',ret,value=f.operand(9));d=f.finish()
 effects={(0,0,0):['local0=5'],(0,0,1):['local1=7'],(0,0,2):['owner0 Uninitialized/g1'],(0,0,3):['owner0 payload[5,7] Available/g2'],(0,0,4):['owner1 Uninitialized/g1'],(0,0,5):['owner0 Moved/g3; owner1 payload[5,7] Available/g2'],(0,0,6):['owner0 Dead/g4'],(0,0,7):['local2=1'],(0,0,8):['local3=7 from owner1[1]'],(0,0,9):['local4=copy local3=7'],(0,0,10):['local5=0'],(0,0,11):['local6=copy local5=0'],(0,0,12):['owner1[0]=saved local4=7; payload[7,7]'],(0,0,13):['local7=2 after verified whole-base read'],(0,0,14):['local8=copy local7=2'],(0,0,15):['owner1 Dead/g3'],(0,0,'end'):['return2; drop frame']}
 return {'schema':'oxid-independent-array-lowering-v1','case':name,'source':s.binding(),'entry':0,'functions':[d],'static_ownership':'Accept under agreed templates; verifier not run','absent_identities':['no target HIR wrapper','no target ReadIndex','no target scalar slot'],'runtime':trace([d],0,[(0,0,i) for i in range(16)]+[(0,0,'end')],effects)}

def main():
 if subprocess.check_output(['git','-C',REPO,'rev-parse',COMMIT+'^{tree}']).decode('utf-8').strip()!=TREE:raise ValueError('named baseline tree mismatch')
 models=[grouped(),snapshot('rhs-snapshot-success'),snapshot('rhs-snapshot-final-bounds-failure',1),snapshot('rhs-bounds-before-index',rhs_bad=True),snapshot('index-read-bounds-after-effect',index_bad=True)]
 for model in models:dump(Path('models')/(model['case']+'.json'),model)
 dump('summary.json',{'schema':'oxid-independent-lowering-checkpoint-01','commit':COMMIT,'tree':TREE,'no_oxid_run':True,'cases':[{'name':m['case'],'source':m['source'],'terminal_fuel':m['runtime']['terminal_budget'],'functions':[{'name':f['name'],'counts':f['counts_all_four_passes'],'frame':f['frame_model']} for f in m['functions']]} for m in models]})
 print(json.dumps([{'case':m['case'],'terminal_fuel':m['runtime']['terminal_budget']} for m in models]))
if __name__=='__main__':
 raise SystemExit('Use replay.py --repo PATH --out NEW_DIRECTORY')
