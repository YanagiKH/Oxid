"""Bounded PR65 expected outputs; the immutable producer reads its real inputs.

No filesystem overlay, source-label rewrite, or historical proof regeneration.
"""
import builtins
import hashlib
import json
from pathlib import Path
import types
import sys

ROOT = 'tests/qualification/stdlib_text_case_v1/'
TRIM = 'tests/qualification/stdlib_trim_storage_v1/'
Q = 'tests/qualification/package_json_current_v1/'
U = 'tests/qualification/unit4_ci/'
HEAD = 'b416d460af657a53ca9419aefccbb2d29adafa97'
TREE = 'dea1fd6a8f2a7cdb071100d6e7e2edacf4120477'
MANIFEST = (76460, '57b691c477da258ac6319b770318347e38d1ea82c804cbd54e56a28342b6196e')
GENERATION = (13680, '66ca7aee20a7d17b332ab3c2f8bad72d5882e7a52f07d86ebbd6ccaa85cae9ea')

def need(ok, message):
    if not ok: raise ValueError(message)

def sha(raw): return hashlib.sha256(raw).hexdigest()
def blob(kind, raw): return hashlib.sha1(kind.encode()+b' '+str(len(raw)).encode()+b'\0'+raw).hexdigest()

def exact(text, before, after, count=1):
    need(text.count(before)==count, 'exact text adapter anchor')
    return text.replace(before, after)

def local(path, name):
    module=types.ModuleType(name);module.__file__=str(path)
    exec(compile(path.read_bytes(),str(path),'exec'),module.__dict__)
    return module

class Baseline:
    def __init__(self, repo):
        self.repo=Path(repo).absolute()
        # Bootstrap the unchanged, fixed source reader before any fixture code.
        raw=(self.repo/Q/'generation.py').read_bytes()
        need((len(raw),sha(raw))==GENERATION,'fixed generation reader')
        self.g=types.ModuleType('text_fixed_generation');self.g.__file__=str(self.repo/Q/'generation.py')
        exec(compile(raw,self.g.__file__,'exec'),self.g.__dict__)
        need(self.g.regular(self.repo,Q+'generation.py')==raw,'fixed generation physical body')
        raw=self.g.regular(self.repo,ROOT+'pr65-expected-v1.json')
        self.g.pin(raw,MANIFEST,'fixed PR65 expected manifest')
        value=json.loads(raw)
        need(value['schema']=='oxid-stdlib-text-pr65-expected-v1' and value['head']==HEAD and value['tree']==TREE,'PR65 provenance')
        commit=bytes.fromhex(value['commit_raw'])
        need(blob('commit',commit)==HEAD and commit.splitlines()[0]==('tree '+TREE).encode(),'PR65 raw commit')
        self.rows={r['path']:r for r in value['files']}
        need(len(self.rows)==len(value['files']) and sum(r['expected'] for r in self.rows.values())==27
             and sum(r['producer_input'] for r in self.rows.values())==12,'PR65 finite membership')
        trees={}
        for oid,hexraw in value['trees'].items():
            raw=bytes.fromhex(hexraw);need(blob('tree',raw)==oid,'PR65 raw tree');entries={};offset=0
            while offset<len(raw):
                end=raw.index(b'\0',offset);mode,name=raw[offset:end].decode().split(' ',1)
                child=raw[end+1:end+21].hex();need(name not in entries and len(child)==40,'PR65 tree entry')
                entries[name]=(mode,child);offset=end+21
            trees[oid]=entries
        used=set()
        for name,row in self.rows.items():
            oid=TREE;parts=name.split('/')
            for i,part in enumerate(parts):
                need(oid in trees,'missing PR65 tree');used.add(oid);mode,oid=trees[oid][part]
                need(mode==('100644' if i==len(parts)-1 else '40000'),'PR65 Git mode')
            need(oid==row['git_blob'],'PR65 path/blob association')
        need(used==set(trees),'unlisted PR65 tree')
        self.modules={}

    def read(self,name):
        need(name in self.rows,'unlisted PR65 read: '+name);row=self.rows[name]
        raw=self.g.regular(self.repo,row['storage'],limit=row['bytes'])
        need(len(raw)==row['bytes'] and sha(raw)==row['sha256'] and blob('blob',raw)==row['git_blob']
             and row['mode']=='100644','PR65 body: '+name)
        return raw

    def _import(self,name,globals=None,locals=None,fromlist=(),level=0):
        need(level==0,'relative PR65 helper import')
        if name=='generation':return self.g
        if name in ('activate','derive','retained'):return self.module(name)
        need(name.split('.')[0] in sys.stdlib_module_names,'unlisted PR65 helper import: '+name)
        return builtins.__import__(name,globals,locals,fromlist,level)

    def execute(self,name,path,raw,extra=None):
        module=types.ModuleType('text_pr65_'+name);module.__file__=str(self.repo/path)
        safe=dict(vars(builtins));safe['__import__']=self._import
        module.__dict__.update(__builtins__=safe)
        if extra:module.__dict__.update(extra)
        exec(compile(raw,module.__file__,'exec'),module.__dict__)
        return module

    def module(self,name):
        need(name in ('activate','derive','retained'),'unlisted PR65 helper')
        raw=self.read(TRIM+name+'.py')
        if name not in self.modules:
            text=raw.decode()
            if name=='activate':text=exact(text,'sys.path.insert(0,str(HERE))\n','')
            if name in ('activate','derive'):text=exact(text,'sys.dont_write_bytecode'+('=True' if name=='activate' else ' = True')+'\n','')
            self.modules[name]=self.execute(name,TRIM+name+'.py',text.encode())
        return self.modules[name]

    def outputs(self):
        for name,row in self.rows.items():
            if row['producer_input']:self.read(name)
        outputs=self.module('activate').generate(self.repo)
        need(set(outputs)=={n for n,r in self.rows.items() if r['expected']},'PR65 complete output set')
        for name,body in outputs.items():need(body==self.read(name),'PR65 reproduced output: '+name)
        return outputs

    def assert_outputs(self):self.outputs()
