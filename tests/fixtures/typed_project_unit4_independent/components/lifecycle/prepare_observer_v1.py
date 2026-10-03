#!/usr/bin/env python3
"""Make an additive, independently bound logical-lifecycle observer snapshot."""
import difflib, hashlib, json, shutil
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
OUT = Path(__file__).resolve().parent
BASE = ROOT / 'typed-project-unit4-dispatch-evidence/source-v1'
MANIFEST = ROOT / 'typed-project-unit4-dispatch-evidence/source-inputs-dispatch-v1.json'
EXPECTED = 'ceff06faf378814fa7952574538e16bc6c6d95c9615e7b656057861275c88cac'
DEST = OUT / 'observer-source-v1'
sha = lambda b: hashlib.sha256(b).hexdigest()
assert sha(MANIFEST.read_bytes()) == EXPECTED
manifest = json.loads(MANIFEST.read_text())
DEST.mkdir(exist_ok=False)
for f in manifest['files']:
    p = BASE/f['path']; b = p.read_bytes()
    assert len(b) == f['bytes'] and sha(b) == f['sha256'], str(p)
    q = DEST/f['path']; q.parent.mkdir(parents=True, exist_ok=True); q.write_bytes(b)

changes = []
def insert(path, old, new, count=1):
    p=DEST/path; text=p.read_text()
    assert text.count(old)==count, (path, old, text.count(old))
    p.write_text(text.replace(old,new))

hook = 'crate::frontend::lifecycle_observer::event'
insert('src/frontend/mod.rs', 'mod ast;', 'mod lifecycle_observer;\nmod ast;')
(DEST/'src/frontend/lifecycle_observer.rs').write_text('''//! Test-only observer in an isolated qualification snapshot, absent from production.
//! Events are logical operations, not OS syscall counts. No source data is read here.
use std::io::Write;
pub(super) fn event(kind: &str, subject: &str) {
    if let Some(path) = std::env::var_os("UNIT4_LIFECYCLE_LOG") {
        let mut file = std::fs::OpenOptions::new().create(true).append(true)
            .open(path).expect("lifecycle evidence open");
        writeln!(file, "{{\\"event\\":{},\\"subject\\":{}}}",
            super::diagnostic::json_string(kind), super::diagnostic::json_string(subject))
            .expect("lifecycle evidence write");
    }
}
''')
p='src/frontend/project.rs'
insert(p,'        let mut file = File::open(path).map_err(|e| io_error(e, display, origin))?;',
       f'        {hook}("source_open_attempt", display);\n        let mut file = File::open(path).map_err(|e| io_error(e, display, origin))?;\n        {hook}("source_open_complete", display);\n        {hook}("source_read_attempt", display);')
insert(p,'        self.project.usage.source_bytes =\n', f'        {hook}("source_read_complete", display);\n        self.project.usage.source_bytes =\n')
p='src/frontend/lexer.rs'
insert(p,'    let limit = limit.min(MAX_TOKENS);',f'    {hook}("lex_attempt", source.path());\n    let limit = limit.min(MAX_TOKENS);')
insert(p,'    Ok(tokens)\n',f'    {hook}("lex_complete", source.path());\n    Ok(tokens)\n')
p='src/frontend/parser.rs'
insert(p,'    let mut parser = Parser {',f'    {hook}("parse_attempt", source.path());\n    let mut parser = Parser {{')
insert(p,'    if diagnostics.is_empty() {\n        Ok((',f'    if diagnostics.is_empty() {{\n        {hook}("parse_complete", source.path());\n        Ok((')
p='src/frontend/oir/source/sealed.rs'
insert(p,'    let owner =\n',f'    {hook}("checker_attempts", "original");\n    let owner =\n')
insert(p,'    let (body, entry) = if ast.uses_owned_syntax(source) {',f'    {hook}("route_attempts", "original");\n    let (body, entry) = if ast.uses_owned_syntax(source) {{\n        {hook}("route_completions", "owned");')
insert(p,'        // Preserve the original scalar schedule and diagnostic adapters exactly.',f'        {hook}("route_completions", "scalar");\n        // Preserve the original scalar schedule and diagnostic adapters exactly.')
insert(p,'    Ok(CheckedSourceProgram {',f'    {hook}("checked_program_completions", "original");\n    Ok(CheckedSourceProgram {{')
insert(p,'    match check_project(project, limits, work, allocator, CheckDepth::Executable)? {',f'    {hook}("checker_attempts", "project");\n    match check_project(project, limits, work, allocator, CheckDepth::Executable)? {{')
insert(p,'    let route = if sources.owned(work).map_err(|e| vec![*e])? {',f'    {hook}("route_attempts", "project");\n    let route = if sources.owned(work).map_err(|e| vec![*e])? {{')
insert(p,'    #[cfg(test)]\n    work.observe(crate::frontend::declaration_index::Observation::Route {',f'    {hook}("route_completions", if route == ProjectRoute::Owned {{ "owned" }} else {{ "scalar" }});\n    #[cfg(test)]\n    work.observe(crate::frontend::declaration_index::Observation::Route {{')
insert(p,'    Ok(Checked::Executable(CheckedSourceProgram {',f'    {hook}("checked_program_completions", "project");\n    Ok(Checked::Executable(CheckedSourceProgram {{')
p='src/frontend/declaration_index/sealed.rs'
insert(p,'    work.restrict(limits.work);',f'    {hook}("index_attempts", "");\n    work.restrict(limits.work);')
insert(p,'        Ok(DeclarationIndex { tables })',f'        {hook}("index_completions", "");\n        Ok(DeclarationIndex {{ tables }})')
p='src/frontend/driver.rs'
insert(p,'        let verified = executable?;',f'        let verified = executable?;\n        {hook}("consumer_entry", "");')

patch=[]; bindings=[]
for f in manifest['files']:
    rel=f['path']; before=(BASE/rel).read_bytes(); after=(DEST/rel).read_bytes()
    if before!=after:
        changes.append(rel)
        patch.extend(difflib.unified_diff(before.decode().splitlines(True),after.decode().splitlines(True),fromfile='a/'+rel,tofile='b/'+rel))
    bindings.append({'path':rel,'bytes':len(after),'sha256':sha(after)})
rel='src/frontend/lifecycle_observer.rs'; after=(DEST/rel).read_bytes()
bindings.append({'path':rel,'bytes':len(after),'sha256':sha(after)})
patch.extend(difflib.unified_diff([],after.decode().splitlines(True),fromfile='/dev/null',tofile='b/'+rel))
(OUT/'observer-additive-v1.patch').write_text(''.join(patch))
result={'base_manifest_sha256':EXPECTED,'observer_script_sha256':sha(Path(__file__).read_bytes()),
 'additive_patch_sha256':sha((OUT/'observer-additive-v1.patch').read_bytes()),'changed_paths':changes,
 'additional_paths':[rel],'observations':'logical source/phase operations, not syscall tracing',
 'production_control_flow_changed':False,'files':bindings}
(OUT/'observer-source-manifest-v1.json').write_text(json.dumps(result,indent=2)+'\n')
print(json.dumps({k:v for k,v in result.items() if k!='files'},indent=2))
