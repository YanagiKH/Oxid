#!/usr/bin/env python3
"""Strict mechanical parser for derived Rust Debug; no source interpretation."""
import re

HEX_RUN = re.compile(r"[0-9a-fA-F]+")
WORD = re.compile(r"[A-Za-z_][A-Za-z_0-9]*")
INTEGER = re.compile(r"-?[0-9]+")

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
                self.expect('{');m=HEX_RUN.match(self.s,self.i)
                if not m:raise DebugSyntaxError(('unicode escape',self.i))
                self.i+=len(m[0]);self.expect('}');out.append(chr(int(m[0],16)))
            else:raise DebugSyntaxError(('escape',c,self.i))
        raise DebugSyntaxError('unterminated Rust Debug string')
    def word(self):
        self.skip();m=WORD.match(self.s,self.i)
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
            m=INTEGER.match(self.s,self.i);self.i+=len(m[0]);return int(m[0])
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

