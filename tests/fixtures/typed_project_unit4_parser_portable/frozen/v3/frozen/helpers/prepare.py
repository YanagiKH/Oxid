#!/usr/bin/env python3
"""Derive an explicitly identified test-only source overlay; never edit the input."""
import argparse,difflib,hashlib,json,pathlib,re,shutil,subprocess
import sys
if sys.flags.optimize or not __debug__:
    raise SystemExit("OPTIMIZED_PYTHON_UNSUPPORTED: parser observer requires enabled admission checks")
HERE=pathlib.Path(__file__).resolve().parent
COMMIT='d9e6b9bf172abd5e15da7212c9e6224e29ccc768'
def sha(b): return hashlib.sha256(b).hexdigest()
def replace(s,old,new,n=1):
    assert s.count(old)==n,(old,s.count(old),n)
    return s.replace(old,new)
def hook(s): return '\n        #[cfg(test)] '+s+'\n'
J='crate::frontend::parser::unit4_observer'
def parser_overlay(s):
    s=replace(s,'    parser.skip();','    '+J+'::parser_enter(parser.project_recovery);\n    parser.skip();')
    s=replace(s,'    if diagnostics.is_empty() {','    '+J+'::parser_return(parser.nodes, parser.project_recovery);\n    if diagnostics.is_empty() {')
    s=replace(s,'        if let Err(error) = result {','        if let Err(error) = result {\n            '+J+'::recovery(true, parser.cursor, parser.peek());')
    s=replace(s,'        }\n    }\n    '+J+'::parser_return','            '+J+'::recovery(false, parser.cursor, parser.peek());\n        }\n    }\n    '+J+'::parser_return')
    s=replace(s,'    fn peek(&self) -> Token {\n        self.tokens[self.cursor]','    fn peek(&self) -> Token {\n        '+J+'::inspect(self.cursor, self.tokens[self.cursor]);\n        self.tokens[self.cursor]')
    s=replace(s,'        let token = self.peek();\n        if token.kind != Kind::Eof {','        let token = self.peek();\n        if token.kind != Kind::Eof {\n            '+J+'::event("consume", "token", self.cursor, token, "null".into());')
    s=replace(s,'        token\n    }\n    fn take','        '+J+'::position(self.cursor, self.tokens[self.cursor]);\n        token\n    }\n    fn take')
    triggers=['top_public','qualified_item_path','import_keyword','module_public','module_keyword','field_public','qualified_borrow','qualified_field']
    matches=list(re.finditer(r'(parser|self)\.project_recovery = true;',s));assert len(matches)==len(triggers)
    for m,trigger in reversed(list(zip(matches,triggers))):
        who=m[1];x=f'let unit4_before = {who}.project_recovery;\n                    '+m[0]+f'\n                    {J}::recognize(unit4_before, {who}.project_recovery, "{trigger}", {who}.cursor, {who}.peek());'
        s=s[:m.start()]+x+s[m.end():]
    s=replace(s,'&& self.next_kind() == Kind::Ident','&& self.unit4_field_next_kind() == Kind::Ident')
    s=replace(s,'&& self.peek().kind == Kind::Pub\n                && self.unit4_field_next_kind()', '&& self.unit4_field_current_kind() == Kind::Pub\n                && self.unit4_field_next_kind()')
    before='    fn double_colon(&self) -> bool {'
    method='''    fn unit4_field_current_kind(&self) -> Kind {
        let token = self.peek();
        OBS::field_current_read(self.cursor, token);
        token.kind
    }
    fn unit4_field_next_kind(&self) -> Kind {
        OBS::field_scan_begin(self.cursor, self.tokens[self.cursor]);
        let terminal = self.next_kind();
        OBS::field_scan_end(self.cursor, terminal);
        terminal
    }
'''.replace('OBS',J)
    next_start=s.index('    fn next_kind(&self) -> Kind {');next_end=s.index('    fn unit4_field_current_kind',next_start) if '    fn unit4_field_current_kind' in s else s.index(before,next_start)
    next_body=s[next_start:next_end]
    next_body=replace(next_body,'.iter()\n', '.iter()\n            .inspect(|token| '+J+'::field_token_inspect(self.tokens.as_ptr(), token))\n')
    s=s[:next_start]+next_body+s[next_end:]
    s=replace(s,before,method+before)
    # Identify the real gate's production at each original node call site.
    fs=list(re.finditer(r'    fn (\w+)\(',s))
    positions=[]
    for m in re.finditer(r'self\.node\(\)\?;',s):
        name=[f[1] for f in fs if f.start()<m.start()][-1]
        if name=='record' and 'while self.peek().kind != Kind::RBrace {' in s[[f.start() for f in fs if f.start()<m.start()][-1]:m.start()]: name='field'
        positions.append((m,name))
    for m,name in reversed(positions):s=s[:m.start()]+f'self.unit4_node("{name}")?;'+s[m.end():]
    s=replace(s,'    fn node(&mut self) -> Result<(), Box<Diagnostic>> {','    fn unit4_node(&mut self, production: &str) -> Result<(), Box<Diagnostic>> {\n        if '+J+'::reject_node(production) { self.node_limit = self.nodes; }\n        '+J+'::event("node_attempt", production, self.cursor, self.peek(), self.nodes.to_string());')
    # Differentiate ordinary and path segment node gates, retaining the original conditions.
    marker='        if self.nodes >= self.node_limit {\n'
    assert s.count(marker)==2
    p=s.index('    fn path_segment(');q=s.index('    fn absolute_path(',p)
    part=s[p:q]
    part=replace(part,'        let next = count','        '+J+'::event("path_count_checked", "path_segment", self.cursor, Token {kind: Kind::Ident, span: segment}, count.to_string());\n        let next = count')
    part=replace(part,'.ok_or_else(|| self.project_reserve_error(ReserveFailure::Overflow, segment))?;', '.ok_or_else(|| { '+J+'::event("path_count_overflow", "path_segment", self.cursor, Token {kind: Kind::Ident, span: segment}, count.to_string()); self.project_reserve_error(ReserveFailure::Overflow, segment) })?;',n=3)
    part=replace(part,'        if next > MAX_PATH_SEGMENTS {','        if next > MAX_PATH_SEGMENTS {\n            '+J+'::event("path_cap_reject", "path_segment", self.cursor, Token {kind: Kind::Ident, span: segment}, next.to_string());')
    part=replace(part,marker,'        '+J+'::event("node_attempt", "path_segment", self.cursor, Token {kind: Kind::Ident, span: segment}, self.nodes.to_string());\n'+marker+'            '+J+'::event("node_reject", "path_segment", self.cursor, Token {kind: Kind::Ident, span: segment}, self.nodes.to_string());\n')
    part=replace(part,'        self.allocator\n            .vector(&mut self.path_segments','        '+J+'::event("node_admit", "path_segment", self.cursor, Token {kind: Kind::Ident, span: segment}, self.nodes.to_string());\n        self.allocator\n            .vector(&mut self.path_segments')
    s=s[:p]+part+s[q:]
    p=s.index('    fn unit4_node(');q=s.index('    fn module(',p);part=s[p:q]
    part=replace(part,marker,marker+'            '+J+'::event("node_reject", production, self.cursor, self.peek(), self.nodes.to_string());\n')
    part=replace(part,'        self.nodes += 1;','        self.nodes += 1;\n        '+J+'::event("node_admit", production, self.cursor, self.peek(), self.nodes.to_string());')
    s=s[:p]+part+s[q:]
    # Log actual completed pushes, evaluating value expressions just once.
    out=[];cursor=0
    for m in list(re.finditer(r'(?m)^(\s*)([a-z_]+(?:\.[a-z_]+)*)\.push\(',s)):
        target=m[2]
        if target in ('diagnostics','inspected'):continue
        start=m.end();depth=1;i=start
        while depth:
            if s[i]=='(':depth+=1
            elif s[i]==')':depth-=1
            i+=1
        assert s[i]==';'
        who='parser' if m.start()<s.index('struct Parser') else 'self'
        value=s[start:i-1]
        addition=f'{m[1]}{{ let unit4_value = {value}; {target}.push(unit4_value); {J}::event("append", "{target}", {who}.cursor, {who}.peek(), {target}.len().to_string()); }}'
        out.extend([s[cursor:m.start()],addition]);cursor=i+1
    out.append(s[cursor:]);s=''.join(out)
    s='\n'.join(line.replace('parser.peek()', 'parser.tokens[parser.cursor]').replace('self.peek()', 'self.tokens[self.cursor]') if J in line else line for line in s.split('\n'))
    return s+'\n#[cfg(test)]\npub(super) mod unit4_observer;\n'
def main():
    ap=argparse.ArgumentParser();ap.add_argument('--repo',type=pathlib.Path,required=True);ap.add_argument('--output',type=pathlib.Path,required=True);ap.add_argument('--control',action='store_true');a=ap.parse_args()
    repo=a.repo.resolve();dest=a.output.resolve();assert not dest.exists(),dest
    head=subprocess.check_output(['git','rev-parse','HEAD'],cwd=repo,text=True).strip();assert head==COMMIT,head
    tracked=subprocess.check_output(['git','ls-files','-z'],cwd=repo).decode().split('\0')[:-1]
    inputs=[];changes=[];dest.mkdir(parents=True)
    for path in tracked:
        if path.startswith(('tests/fixtures/','docs/','spec/','examples/','apps/','packages/','.github/')):continue
        p=repo/path
        if not p.is_file():continue
        raw=p.read_bytes();assert raw==subprocess.check_output(['git','show',head+':'+path],cwd=repo),('dirty input',path);inputs.append({'path':path,'bytes':len(raw),'sha256':sha(raw)})
        target=dest/path;target.parent.mkdir(parents=True,exist_ok=True);target.write_bytes(raw)
    manifest={'schema':'oxid-unit4-candidate-source-manifest-v1','commit':head,'files':inputs}
    (dest/'candidate-source-manifest.json').write_text(json.dumps(manifest,sort_keys=True,indent=2)+'\n')
    def edit(path,fn):
        p=dest/path;old=p.read_text();new=fn(old);p.write_text(new)
        changes.append({'path':path,'before_sha256':sha(old.encode()),'after_sha256':sha(new.encode())})
        return ''.join(difflib.unified_diff(old.splitlines(True),new.splitlines(True),fromfile='a/'+path,tofile='b/'+path))
    patch=''
    patch+=edit('src/frontend/ast.rs',lambda s:replace(s,'impl Program {','impl Program {\n    #[cfg(test)]\n    pub(super) fn unit4_source_handle(&self) -> (u64,usize,usize) { crate::frontend::parser::unit4_observer::provenance_handle(&self.source) }'))
    if not a.control:
        patch+=edit('src/frontend/parser.rs',parser_overlay)
        patch+=edit('src/frontend/source.rs',lambda s:replace(replace(s,'    pub(super) fn try_text(&self, span: Span) -> Option<&str> {\n        if span.file', '    pub(super) fn try_text(&self, span: Span) -> Option<&str> {\n        '+J+'::source_read(span.end.saturating_sub(span.start));\n        if span.file'),'    pub fn text(&self) -> &str {\n        &self.text','    pub fn text(&self) -> &str {\n        '+J+'::source_read(self.text.len());\n        &self.text'))
        patch+=edit('src/frontend/declaration_index/resource.rs',lambda s:replace(replace(s,'    pub(super) fn preflight(&self, origin: Span) -> Result<(), Box<Diagnostic>> {', '    pub(super) fn preflight(&self, origin: Span) -> Result<(), Box<Diagnostic>> {\n        '+J+'::namespace_debit(1);'),"        operation: &'static str,\n    ) -> Result<(), Box<Diagnostic>> {","        operation: &'static str,\n    ) -> Result<(), Box<Diagnostic>> {\n        "+J+'::namespace_debit(units);'))
        patch+=edit('src/frontend/lexer.rs',lambda s:replace(replace(s,'        tokens.push(Token { kind, span });','        tokens.push(Token { kind, span });\n        '+J+'::lex_token(*tokens.last().unwrap());'),'    Ok(tokens)\n','    '+J+'::lex_token(*tokens.last().unwrap());\n    Ok(tokens)\n'))
        patch+=edit('src/frontend/project/budget.rs',lambda s:replace(s,'        #[cfg(test)]\n        self.trace.push(ReserveEvent {','        #[cfg(test)]\n        '+J+'::reserve(kind, length, element_bytes, success);\n        #[cfg(test)]\n        self.trace.push(ReserveEvent {'))
    else:patch+=edit('src/frontend/parser.rs',lambda s:s+'\n#[cfg(test)]\npub(super) mod unit4_observer;\n')
    target=dest/'src/frontend/parser/unit4_observer.rs';target.parent.mkdir(exist_ok=True)
    target.write_bytes((HERE/'observer.rs').read_bytes())
    (dest/'instrumentation.patch').write_text(patch)
    overlay=[]
    for p in sorted(dest.rglob('*')):
        if p.is_file():overlay.append({'path':str(p.relative_to(dest)),'bytes':p.stat().st_size,'sha256':sha(p.read_bytes())})
    observer_files=[{'path':p.name,'bytes':p.stat().st_size,'sha256':sha(p.read_bytes())} for p in sorted(HERE.iterdir()) if p.is_file() and p.name not in ('comparator.py',)]
    observer_identity=json.dumps(observer_files,sort_keys=True,separators=(',',':')).encode()
    (dest/'observer-source-manifest.json').write_bytes(observer_identity)
    receipt={'observer_files':observer_files,'schema':'oxid-unit4-observer-overlay-v1','control':a.control,'source':str(dest),'base_commit':head,'candidate_source_manifest_sha256':sha((dest/'candidate-source-manifest.json').read_bytes()),'observer_source_sha256':sha(observer_identity),'instrumentation':changes,'files':overlay}
    (dest/'overlay-manifest.json').write_text(json.dumps(receipt,sort_keys=True,indent=2)+'\n')
    print(json.dumps({'overlay_manifest':str(dest/'overlay-manifest.json'),'files':len(inputs),'control':a.control}))
if __name__=='__main__':main()
