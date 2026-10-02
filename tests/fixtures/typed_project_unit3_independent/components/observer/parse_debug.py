#!/usr/bin/env python3
"""Strict mechanical parser for derived Rust Debug; no source interpretation."""
import re

class DebugSyntaxError(ValueError): pass

class Parser:
    def __init__(self,text):self.s=text;self.i=0
    def skip(self):
        while self.i<len(self.s) and self.s[self.i].isspace():self.i+=1
    def peek(self):self.skip();return self.s[self.i] if self.i<len(self.s) else None
    def expect(self,char):
        if self.peek()!=char:raise DebugSyntaxError((self.i,char,self.s[self.i:self.i+60]))
        self.i+=1
    def string(self,quote='"'):
        self.expect(quote);out=[]
        while self.i<len(self.s):
            c=self.s[self.i];self.i+=1
            if c==quote:return ''.join(out)
            if c!='\\':out.append(c);continue
            if self.i>=len(self.s):break
            c=self.s[self.i];self.i+=1
            if c in {'n':'\n','r':'\r','t':'\t','0':'\0','\\':'\\','"':'"',"'":"'"}:
                out.append({'n':'\n','r':'\r','t':'\t','0':'\0','\\':'\\','"':'"',"'":"'"}[c])
            elif c=='x':
                h=self.s[self.i:self.i+2]
                if not re.fullmatch('[0-9a-fA-F]{2}',h):raise DebugSyntaxError(('hex escape',self.i))
                self.i+=2;out.append(chr(int(h,16)))
            elif c=='u':
                self.expect('{');m=re.match('[0-9a-fA-F]+',self.s[self.i:])
                if not m:raise DebugSyntaxError(('unicode escape',self.i))
                self.i+=len(m[0]);self.expect('}');out.append(chr(int(m[0],16)))
            else:raise DebugSyntaxError(('escape',c,self.i))
        raise DebugSyntaxError('unterminated Rust Debug string')
    def word(self):
        self.skip();m=re.match(r'[A-Za-z_][A-Za-z_0-9]*',self.s[self.i:])
        if not m:raise DebugSyntaxError(('word',self.i,self.s[self.i:self.i+60]))
        self.i+=len(m[0]);return m[0]
    def sequence(self,end):
        out=[]
        while self.peek()!=end:
            out.append(self.value())
            if self.peek()==',':self.i+=1
            elif self.peek()!=end:raise DebugSyntaxError(('sequence separator',self.i))
        self.expect(end);return out
    def value(self):
        p=self.peek()
        if p is None:raise DebugSyntaxError('unexpected EOF')
        if p in ('"',"'"):return self.string(p)
        if p=='[':self.i+=1;return self.sequence(']')
        if p=='(':self.i+=1;return self.sequence(')')
        if p=='{':
            self.i+=1;out=[]
            while self.peek()!='}':
                key=self.value();self.expect(':');out.append([key,self.value()])
                if self.peek()==',':self.i+=1
                elif self.peek()!='}':raise DebugSyntaxError(('map separator',self.i))
            self.i+=1;return {'tag':'$map','items':out}
        if p=='-' or p.isdigit():
            m=re.match('-?[0-9]+',self.s[self.i:]);self.i+=len(m[0]);return int(m[0])
        tag=self.word()
        if tag=='None':return None
        if tag=='true':return True
        if tag=='false':return False
        if self.peek()=='(':
            self.i+=1;return {'tag':tag,'items':self.sequence(')')}
        if self.peek()=='{':
            self.i+=1;out={'tag':tag}
            while self.peek()!='}':
                if self.s.startswith('..',self.i):
                    if tag!='SourceProvenance':raise DebugSyntaxError(('unexpected omitted struct fields',tag,self.i))
                    self.i+=2;out['debug_non_exhaustive']=True
                    if self.peek()==',':self.i+=1
                    if self.peek()!='}':raise DebugSyntaxError(('non-exhaustive suffix',self.i))
                    break
                key=self.word();self.expect(':')
                if key in out:raise DebugSyntaxError(('duplicate struct key',key))
                out[key]=self.value()
                if self.peek()==',':self.i+=1
                elif self.peek()!='}':raise DebugSyntaxError(('struct separator',self.i))
            self.i+=1;return out
        return {'tag':tag}

def parse(text):
    parser=Parser(text);out=parser.value()
    if parser.peek() is not None:raise DebugSyntaxError(('trailing input',parser.i))
    return out

NUMERIC_IDS={'DefId','RecordId','LocalId','PlaceId','BlockId','OwnerPlaceId','ReferenceParamId','CallSiteId','LoanId','SourceFileId','ModuleId'}
def canonical(value,raw=False):
    """Mechanical primitives; raw-only enum renaming never touches Event payloads."""
    if isinstance(value,list):return [canonical(x,raw)for x in value]
    if not isinstance(value,dict):return value
    tag=value['tag']
    if tag=='Span':return [canonical(value['file'],raw),value['start'],value['end']]
    if tag in NUMERIC_IDS:
        assert set(value)=={'tag','items'} and len(value['items'])==1 and type(value['items'][0])is int,tag
        return value['items'][0]
    if tag=='FieldId':return [canonical(value['record'],raw),value['index']]
    if tag=='Some':
        assert set(value)=={'tag','items'} and len(value['items'])==1
        return canonical(value['items'][0],raw)
    out={k:canonical(v,raw)for k,v in value.items()if k!='tag'};out={'tag':tag,**out}
    if raw and tag=='Assign' and 'items'in out:
        assert len(out['items'])==1 and out['items'][0]['tag']=='Assign'
        return out['items'][0]
    field={'Load':'place','Copy':'operand','Return':'value','ReturnScalar':'value','Scalar':'scalar','Goto':'target','StorageLive':'owner','StorageEnd':'owner','Discard':'owner','OpenCall':'call','ReturnOwned':'owner','I32':'value','Bool':'value'}.get(tag)
    if raw and field and 'items'in out:
        assert len(out['items'])==1,('unexpected raw tuple arity',tag,out)
        out[field]=out.pop('items')[0]
    if raw and tag=='Construct' and 'fields'in out:
        assert all(isinstance(x,list) and len(x)==2 for x in out['fields'])
        out['fields']=[{'field':x[0],'value':x[1]}for x in out['fields']]
    return out

def checked_raw(parsed):
    assert parsed['tag']=='CheckedSourceProgram'
    body=parsed['body'];assert set(body)=={'tag','items'}and len(body['items'])==1
    route=body['tag'].lower();body=body['items'][0]
    if route=='scalar':
        assert body['tag']=='VerifiedProgram';raw=body['program']
    elif route=='owned':
        assert body['tag']=='SourceProgram'and body['witness']['tag']=='VerifiedOwnedProgram'
        raw=body['witness']['program']
    else:raise DebugSyntaxError(('unknown checked body',route))
    raw=canonical(raw,raw=True)
    validate_raw(raw,route)
    return route,canonical(parsed['entry']),raw

def validate_raw(raw,route):
    """Exhaustive enum grammar validation. This computes no expected visits."""
    def shape(value,fields):
        assert set(value)=={'tag',*fields},('unknown/missing raw payload field',value['tag'],sorted(value),sorted(fields))
    def scalar(s):
        assert s['tag']in {'Assign','Initialize','Store'},('unknown Statement',s['tag'])
        shape(s,{'Assign':{'destination','value','span'},'Initialize':{'place','value','span'},'Store':{'place','value','operator_span','span'}}[s['tag']])
        if s['tag']=='Assign':
            value=s['value'];assert value['tag']in {'Load','NotBool','Bool','I32','Unit','Copy','CompareScalar','CheckedI32'},('unknown Rvalue',value['tag'])
            shape(value,{'Load':{'place'},'NotBool':{'operand','operator_span'},'Bool':{'value'},'I32':{'value'},'Unit':set(),'Copy':{'operand'},'CompareScalar':{'op','left','right','operator_span'},'CheckedI32':{'op','left','right','operator_span'}}[value['tag']])
    owned_payload={'Scalar':{'scalar'},'StorageLive':{'owner'},'StorageEnd':{'owner'},'Construct':{'destination','fields'},'MoveInitialize':{'destination','source'},'Replace':{'destination','source'},'Discard':{'owner'},'ReadField':{'destination','base','field'},'WriteField':{'base','field','value'},'OpenCall':{'call'},'PrepareScalar':{'call','argument','value'},'PrepareOwned':{'call','argument','source'},'PrepareBorrow':{'call','argument','loan'}}
    term_payload={'Branch':{'condition','then_block','else_block'},'Goto':{'target'},'Call':{'target','args','destination','continuation'},'Return':{'value'},'Invoke':{'call','continuation'},'ReturnScalar':{'value'},'ReturnOwned':{'owner'}}
    for f in raw['functions']:
        for b in f['blocks']:
            for s in b['statements']:
                if route=='scalar':scalar(s)
                else:
                    k=s['kind'];assert k['tag']in {'Scalar','StorageLive','StorageEnd','Construct','MoveInitialize','Replace','Discard','ReadField','WriteField','OpenCall','PrepareScalar','PrepareOwned','PrepareBorrow'},('unknown OwnedInstruction',k['tag'])
                    shape(k,owned_payload[k['tag']])
                    if k['tag']=='Scalar':scalar(k['scalar'])
            if b['terminator']is not None:
                k=b['terminator']['kind'];allowed={'Branch','Goto','Call','Return'}if route=='scalar'else{'Branch','Goto','Invoke','ReturnScalar','ReturnOwned'}
                assert k['tag']in allowed,('unknown TerminatorKind',k['tag'])
                shape(k,term_payload[k['tag']])

def trace(parsed):
    rows=canonical(parsed)
    for row in rows:
        assert row['tag']=='Row'
        row['payload']=canonical(parse(row['payload']))
        if row['context'] is not None:row['context']['operation']=canonical(parse(row['context']['operation']),raw=True)
    return rows

if __name__=='__main__':
    import json,pathlib,sys
    print(json.dumps(parse(pathlib.Path(sys.argv[1]).read_text()),ensure_ascii=False,indent=2))
