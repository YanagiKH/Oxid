#!/usr/bin/env python3
"""Materialize an identity-checked, minimal observation overlay. Never run a corpus."""
import argparse, difflib, hashlib, json, pathlib, re, shutil
HERE=pathlib.Path(__file__).resolve().parent
BASE=HERE.parent/'oxid-typed-project-execution'
MANIFEST=HERE.parent/'typed-project-unit3-evidence/source-inputs-core-v1.json'
MANIFEST_SHA='53ed114674e59266c8e9a809f6cba5ef35c9ea819318ab64a7ae6d9aebd20910'
def sha(data):return hashlib.sha256(data).hexdigest()
assert sha(MANIFEST.read_bytes())==MANIFEST_SHA
parser=argparse.ArgumentParser();parser.add_argument('--revision',required=True);args=parser.parse_args()
assert re.fullmatch(r'v[0-9]+',args.revision)
source=HERE/('source-'+args.revision)
assert not source.exists(),'preserve the prior source copy; do not overwrite'
manifest=json.loads(MANIFEST.read_text())
for item in manifest['files']:
    data=(BASE/item['path']).read_bytes()
    assert sha(data)==item['sha256'] and len(data)==item['bytes'],item['path']
for item in manifest['files']:
    p=source/item['path'];p.parent.mkdir(parents=True,exist_ok=True);shutil.copyfile(BASE/item['path'],p)
# Compile-time include_str inputs are source fixtures, never full target copies.
for directory in ('tests/fixtures','fixtures'):
    shutil.copytree(BASE/directory,source/directory,dirs_exist_ok=True)
original={}
def edit(path, transform):
    p=source/path;s=p.read_text();original.setdefault(path,s);p.write_text(transform(s))
def replace(s,old,new,n=1):
    assert s.count(old)==n,(old,s.count(old),n)
    return s.replace(old,new)
def insert_before(s,anchor,text,n=1):return replace(s,anchor,text+anchor,n)
def insert_after(s,anchor,text,n=1):return replace(s,anchor,anchor+text,n)
def hook(code):return '#[cfg(test)]\n'+code+'\n'
J='crate::frontend::unit3_observer'
macros='''
// Passive Unit3 field-address hooks. Original checks remain the only return value.
macro_rules! unit3_span {
    ($visitor:expr, $value:expr, $stored:expr) => {{
        let result = $visitor.span($value);
        #[cfg(test)]
        crate::frontend::unit3_observer::visit("span", $stored, Some($value), $visitor.unit3_validate(), result.is_ok());
        result
    }};
}
macro_rules! unit3_declaration {
    ($visitor:expr, $stored:expr) => {{
        let result = $visitor.declaration();
        #[cfg(test)]
        crate::frontend::unit3_observer::visit("declaration", $stored, None, $visitor.unit3_validate(), result.is_ok());
        result
    }};
}
'''
edit('src/frontend/mod.rs',lambda s:s.replace('mod ast;',macros+'\nmod ast;')+'\n#[cfg(test)]\nmod unit3_observer;\n')
for name,target in [('observer.rs','src/frontend/unit3_observer.rs'),('raw_scalar.rs','src/frontend/oir/unit3_raw_scalar.rs'),('raw_owned.rs','src/frontend/oir/owned/unit3_raw_owned.rs')]:
    shutil.copyfile(HERE/'overlay'/name,source/target)
edit('src/frontend/oir/mod.rs',lambda s:s+'\n#[cfg(test)]\nmod unit3_raw_scalar;\n')
edit('src/frontend/oir/owned/mod.rs',lambda s:s+'\n#[cfg(test)]\nmod unit3_raw_owned;\n')
def scalar_audit(s):
    s=insert_after(s,"impl<'s> Visitor<'s> {\n",hook('pub fn unit3_validate(&self)->bool {self.sources.is_some()}'))
    s=replace(s,'visitor.declaration()?;','unit3_declaration!(visitor, function)?;')
    s=re.sub(r'\b(self|visitor)\.span\(([^()]+)\)',lambda m:f'unit3_span!({m[1]}, {m[2]}, '+(m[2][1:] if m[2].startswith('*') else '&'+m[2])+')',s)
    s=insert_before(s,'    let mut count = Visitor::count();',hook('super::super::unit3_raw_scalar::register(raw);'))
    s=insert_before(s,'        Ok(usage)\n',hook(f'{J}::capture("audit-usage", &usage);'))
    return s
edit('src/frontend/oir/source/association.rs',scalar_audit)
def owned_audit(s):
    s=replace(s,'visitor.declaration()?;', 'unit3_declaration!(visitor, record)?;',n=3) if False else s
    s=s.replace('    visitor.declaration()?;\n    visitor.span(record.span)?;','    unit3_declaration!(visitor, record)?;\n    visitor.span(record.span)?;')
    s=s.replace('        visitor.declaration()?;\n        visitor.span(field.span)?;','        unit3_declaration!(visitor, field)?;\n        visitor.span(field.span)?;')
    s=s.replace('    visitor.declaration()?;\n    visitor.span(function.span)?;','    unit3_declaration!(visitor, function)?;\n    visitor.span(function.span)?;')
    assert 'visitor.declaration()' not in s
    # Keep the existing copied Option input/behavior in all configurations; add
    # only a test-build stored-field reference for passive address attribution.
    s=replace(s,'    visitor: &mut Visitor<\'_>,\n) -> Result<(), Box<Diagnostic>> {','    visitor: &mut Visitor<\'_>,\n    #[cfg(test)] stored: &Option<DiagnosticOrigins>,\n) -> Result<(), Box<Diagnostic>> {')
    s=s.replace('origins(statement.diagnostic_origins, visitor)?;','origins(statement.diagnostic_origins, visitor, #[cfg(test)] &statement.diagnostic_origins)?;')
    s=s.replace('origins(terminator.diagnostic_origins, visitor)?;','origins(terminator.diagnostic_origins, visitor, #[cfg(test)] &terminator.diagnostic_origins)?;')
    def address(m):
        value=m[1]
        ptr='&stored.as_ref().expect("same actual origin option").'+value.split('.')[1] if value.startswith('origins.') else '&'+value
        return f'unit3_span!(visitor, {value}, {ptr})'
    s=re.sub(r'visitor\.span\(([^()]+)\)',address,s)
    s=insert_before(s,'    let mut count = Visitor::count();',hook('super::super::unit3_raw_owned::register(raw);'))
    return s
edit('src/frontend/oir/owned/source/association.rs',owned_audit)
edit('src/frontend/oir/source/sealed.rs',lambda s:insert_after(s,'    let frozen = facts.finish(work, allocator)?;\n',hook(f'{J}::capture("index", &frozen);')))
def scalar_runtime(s):
    s=insert_after(s,'fn charge(fuel: &mut usize, cost: usize, origin: Span) -> Result<(), RunFailure> {\n',hook(f'{J}::charge(cost,*fuel,origin);'))
    s=replace(s,'&mut |_| {},',f'&mut |event| {J}::event("existing_scalar_event", &event),')
    s=replace(s,'    let limits = Limits {\n        fuel: limits.fuel.min(MAX_FUEL),', '    let limits = Limits {\n        #[cfg(test)]\n        fuel: '+J+'::fuel(limits.fuel).min(MAX_FUEL),\n        #[cfg(not(test))]\n        fuel: limits.fuel.min(MAX_FUEL),')
    s=insert_before(s,'    let mut live_slots = preflight(',hook(f'{J}::context("scalar", entry.id.0,None,None,"entry",&entry.id);'))
    s=insert_after(s,'    observe(Event::Enter(entry.id));\n',hook(f'{J}::event("operation_commit",&());'))
    s=insert_after(s,'            if let Some(merge) = &block.merge {\n',hook(f'{J}::context("scalar",current.id.0,None,Some(active.block.0),"merge",merge);'))
    s=insert_after(s,'                write(active, current, merge.destination, value, merge.span)?;\n',hook(f'{J}::event("operation_commit",&());'))
    s=insert_after(s,'        if let Some(statement) = block.statements.get(active.next) {\n',hook(f'{J}::context("scalar",current.id.0,None,Some(active.block.0),format!("statement[{{}}]",active.next),statement);'))
    s=insert_before(s,'                    active.next = add(active.next, 1, statement.span())?;',hook(f'{J}::event("operation_commit",&());'))
    s=insert_before(s,'            active.next = add(active.next, 1, assign.span)?;',hook(f'{J}::event("operation_commit",&());'))
    s=insert_before(s,'        match &end.kind {',hook(f'{J}::context("scalar",current.id.0,None,Some(active.block.0),"terminator",end);'))
    s=insert_after(s,'                live_slots = next_slots;\n',hook(f'{J}::event("operation_commit",&());'))
    s=replace(s,'                active.next = 0;\n            }','                active.next = 0;\n'+hook(f'{J}::event("operation_commit",&());')+'            }',n=2)
    s=replace(s,'                    (None, None) if live_slots == 0 => return Ok(value),','                    (None, None) if live_slots == 0 => {\n'+hook(f'{J}::event("operation_commit",&());')+'return Ok(value);\n},')
    s=insert_after(s,'                        caller.next = 0;\n',hook(f'{J}::event("operation_commit",&());'))
    # Two scalar storage primitives, with explicit actual destination identity.
    start=s.index('fn store(');end=s.index('fn write(',start)
    part=s[start:end]
    part=insert_before(part,'    *slot = Some(value);\n',hook('let unit3_before = *slot;'))
    part=insert_after(part,'    *slot = Some(value);\n',hook(f'{J}::event("scalar_place_write",&(frame.function,place.id,unit3_before,*slot,place.span,initialize));'))
    s=s[:start]+part+s[end:]
    return s
edit('src/frontend/oir/execute.rs',scalar_runtime)
def owned_runtime(s):
    s=insert_after(s,'    fn charge(&mut self, cost: usize, span: Span) -> Result<()> {\n',hook(f'{J}::charge(cost,self.fuel,span);'))
    # Preserve every original Event push and its argument. The appended observer
    # reads the event already present, without adding/replacing Event variants.
    matches=list(re.finditer(r'self\.events\s*\.push\(',s))
    for m in reversed(matches):
        i=m.end();depth=1
        while depth:
            if s[i]=='(':depth+=1
            elif s[i]==')':depth-=1
            i+=1
        assert s[i]==';'
        s=s[:i+1]+'\n'+hook(f'{J}::event("existing_owned_event",self.events.last().expect("just appended event"));')+s[i+1:]
    s=insert_after(s,'        o.state = state;\n',hook(f'{J}::event("owner_transition",&(frame,self.frames[frame].function,self.raw_key(frame,owner),state,span));'))
    s=insert_before(s,'                self.frames[frame].slots[index] = Some(value);',hook('let unit3_before=self.frames[frame].slots[index];'))
    s=insert_after(s,'                self.frames[frame].slots[index] = Some(value);\n',hook(f'{J}::event("owned_scalar_place_write",&(frame,self.frames[frame].activation,f.id,place.id,unit3_before,self.frames[frame].slots[index],*span,matches!(statement,Statement::Initialize{{..}})));'))
    s=insert_after(s,'    let limits = limits.bounded();\n',hook('let limits = Limits { fuel: '+J+'::fuel(limits.fuel), ..limits };'))
    s=insert_before(s,'        machine.activation_preflight(\n',hook(f'{J}::context("owned",entry.0,Some(machine.next_activation),None,"entry",&entry);'))
    s=insert_after(s,'        machine.install(root);\n',hook(f'{J}::event("operation_commit",&());'))
    s=insert_after(s,'                if let Some(merge) = &b.merge {\n',hook(f'{J}::context("owned",f.id.0,Some(self.frames[frame].activation),Some(self.frames[frame].block.0),"merge",merge);'))
    s=insert_after(s,'                    self.write(frame, merge.destination, value, merge.span)?;\n',hook(f'{J}::event("operation_commit",&());'))
    s=insert_after(s,'            if let Some(statement) = b.statements.get(self.frames[frame].next) {\n',hook(f'{J}::context("owned",f.id.0,Some(self.frames[frame].activation),Some(self.frames[frame].block.0),format!("statement[{{}}]",self.frames[frame].next),statement);'))
    s=insert_after(s,'                self.statement(frame, &statement.kind, span)?;\n',hook(f'{J}::event("operation_commit",&());'))
    s=insert_before(s,'            let cost = self.plan.terminator_cost(f.id, &end.kind);',hook(f'{J}::context("owned",f.id.0,Some(self.frames[frame].activation),Some(self.frames[frame].block.0),"terminator",end);'))
    s=insert_after(s,'                self.dispatch(frame, call, continuation, end.span)?;\n',hook(f'{J}::event("operation_commit",&());'))
    s=insert_after(s,'                    self.branch(frame, if value { then_block } else { else_block });\n',hook(f'{J}::event("operation_commit",&());'))
    s=replace(s,'                OwnedTerminatorKind::Goto(target) => self.branch(frame, target),','                OwnedTerminatorKind::Goto(target) => {self.branch(frame, target);\n'+hook(f'{J}::event("operation_commit",&());')+'},')
    s=replace(s,'                    if let Some(value) = self.return_value(frame, &end.kind, end.span)? {','                    let unit3_return = self.return_value(frame, &end.kind, end.span)?;\n'+hook(f'{J}::event("operation_commit",&());')+'                    if let Some(value) = unit3_return {')
    # Observe bytes only after the original offset/type checks. No interpreter
    # helper is called to compute a semantic pre-value in uninitialized storage.
    start=s.index('    fn store_field(');end=s.index('    fn transfer_payload(',start)
    part=s[start:end]
    part=insert_after(part,'        let bytes = &mut self.frames[key.frame as usize].payload;\n',hook('let unit3_width=match value {Scalar::I32(_)=>4,Scalar::Bool(_)|Scalar::Unit=>1};')+hook('let unit3_before=bytes.get(offset..offset+unit3_width).map(|v|v.to_vec());'))
    part=insert_before(part,'        Ok(())\n',hook(f'{J}::event("payload_write",&(key,field,value,span,unit3_before,bytes.get(offset..offset+unit3_width)));'))
    s=s[:start]+part+s[end:]
    return s
edit('src/frontend/oir/owned/execute.rs',owned_runtime)
edit('src/frontend/oir/native.rs',lambda s:replace(s,'        self.native_module_fuel(entry, sources, execute::MAX_FUEL)','        let fuel = execute::MAX_FUEL;\n'+hook(f'let fuel = {J}::fuel(fuel);')+'        self.native_module_fuel(entry, sources, fuel)'))
edit('src/frontend/oir/owned/native.rs',lambda s:replace(s,'    native_module_limits(witness, entry, sources, plan::MAX_FUEL, Limits::DEFAULT)','    let fuel = plan::MAX_FUEL;\n'+hook(f'let fuel = {J}::fuel(fuel);')+'    native_module_limits(witness, entry, sources, fuel, Limits::DEFAULT)'))
patch=[]
for path,old in original.items():
    new=(source/path).read_text()
    patch.extend(difflib.unified_diff(old.splitlines(True),new.splitlines(True),fromfile='a/'+path,tofile='b/'+path))
for path in ('src/frontend/unit3_observer.rs','src/frontend/oir/unit3_raw_scalar.rs','src/frontend/oir/owned/unit3_raw_owned.rs'):
    patch.extend(difflib.unified_diff([], (source/path).read_text().splitlines(True),fromfile='/dev/null',tofile='b/'+path))
patch_path=HERE/('overlay-'+args.revision+'.patch');patch_path.write_text(''.join(patch))
receipt={'schema':1,'revision':args.revision,'source':str(source),'core_manifest_sha256':MANIFEST_SHA,'verified_files':len(manifest['files']),'patch_sha256':sha(patch_path.read_bytes()),'compiler_executed':False,'files':[{'path':str(p.relative_to(source)),'sha256':sha(p.read_bytes()),'bytes':p.stat().st_size}for p in sorted(source.rglob('*'))if p.is_file()]}
(HERE/('overlay-manifest-'+args.revision+'.json')).write_text(json.dumps(receipt,indent=2)+'\n')
print(json.dumps({k:v for k,v in receipt.items() if k!='files'}))
