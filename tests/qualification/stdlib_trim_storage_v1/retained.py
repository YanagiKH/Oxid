"""Closed PR64 mechanical byte reader and IO-only private helper adaptation.

This is a named fixture, not a Python sandbox or a general filesystem overlay.
Only authenticated original code is adapted. Every data access re-reads and
hashes its fixed storage; producer transformations still hash those same bytes.
"""
import ast
import builtins
import hashlib
import json
import os
from pathlib import Path
import stat
import types
import unittest
from unittest.mock import patch

ROOT = 'tests/qualification/stdlib_trim_storage_v1/'
Q = 'tests/qualification/package_json_current_v1/'
U = 'tests/qualification/unit4_ci/'
S = 'tests/fixtures/typed_project_source_binding/'
BASE = '31f49568b1142dd58d80eef8f71faeeb99f32ba5'
MANIFEST = (495606, '9a45723d4cbc2cf3f8e3da26bca4bbd88e59d4e24184b57d365e872ac72e8d2f')
HELPERS = {'generation': Q+'generation.py', 'ci': Q+'ci.py',
           'derive_consumers': Q+'derive_consumers.py',
           'derive_scheduling': U+'derive_scheduling.py',
           'scheduling_derivation': U+'scheduling_derivation.py',
           'derive_hosted': Q+'derive_hosted.py'}
STDLIB = frozenset(('ast','copy','hashlib','json','os','pathlib','stat','types',
                   'sys','subprocess','unittest','contextlib','io','re','signal',
                   'selectors','time','tempfile','threading'))


def need(ok, message):
    if not ok:
        raise ValueError(message)


def sha(raw):
    return hashlib.sha256(raw).hexdigest()


def blob(kind, raw):
    return hashlib.sha1(kind.encode()+b' '+str(len(raw)).encode()+b'\0'+raw).hexdigest()


def regular(repo, name, size):
    need(type(name) is str and name and '\\' not in name and
         not name.startswith('/') and all(x not in ('', '.', '..') for x in name.split('/')),
         'noncanonical retained path')
    path = Path(repo) / name
    ancestors = []
    for parent in reversed(path.parents):
        info = parent.lstat()
        need(stat.S_ISDIR(info.st_mode) and not getattr(info,'st_file_attributes',0)&0x400,
             'unsafe retained ancestor')
        ancestors.append((parent,(info.st_dev,info.st_ino,info.st_mode)))
    before = path.lstat()
    need(stat.S_ISREG(before.st_mode) and before.st_nlink == 1 and
         not before.st_mode & 0o111 and before.st_size == size and
         not getattr(before,'st_file_attributes',0)&0x400, 'unsafe retained body')
    fields = ('st_dev','st_ino','st_mode','st_nlink','st_size','st_mtime_ns','st_ctime_ns')
    stamp = lambda x: tuple(getattr(x,f) for f in fields)
    fd = os.open(path,os.O_RDONLY|getattr(os,'O_NOFOLLOW',0)|getattr(os,'O_NONBLOCK',0)|getattr(os,'O_BINARY',0))
    try:
        opened = os.fstat(fd)
        left,right = stamp(before),stamp(opened)
        same = left == right
        if os.name == 'nt' and hasattr(before,'st_birthtime_ns') and hasattr(opened,'st_birthtime_ns'):
            same = left[:-1] == right[:-1] and before.st_birthtime_ns == opened.st_birthtime_ns
        need(same,'retained body replaced before read')
        with os.fdopen(fd,'rb',closefd=False) as stream:
            raw = stream.read(size+1)
        need(stamp(os.fstat(fd)) == right and len(raw) == size,'retained body changed during read')
    finally:
        os.close(fd)
    need(stamp(path.lstat()) == stamp(before),'retained name changed during read')
    for parent,expected in ancestors:
        info=parent.lstat()
        need((info.st_dev,info.st_ino,info.st_mode)==expected,'retained ancestor changed')
    return raw


def exact(text, before, after, count=1):
    need(text.count(before)==count,'missing/duplicate retained IO anchor')
    return text.replace(before,after)


def function(text, name):
    nodes=[n for n in ast.parse(text).body if isinstance(n,ast.FunctionDef) and n.name==name]
    need(len(nodes)==1,'exact retained function')
    return ast.get_source_segment(text,nodes[0])


def adapt(name, raw):
    text=raw.decode()
    if name=='derive_scheduling':
        text=exact(text,"sys.path.insert(0, str(REPO / PACKAGE))\n",'')
        before=function(text,'base')
        text=exact(text,before,"def base(path):\n    return _reader.git_read(BASE, path)")
        before=function(text,'ensure_base')
        text=exact(text,before,"def ensure_base(repo=REPO):\n    raise ValueError('retained fixture cannot fetch')")
        replacements={
            "(REPO/HERE/'integration-roster-v1.json').read_bytes()":1,
            "(REPO/'tests/qualification/published_formatter_current/published-direct-consumer-dispositions-v1.json').read_bytes()":1,
            "(REPO/PACKAGE/'predecessor-v1/.github/workflows/ci.yml').read_bytes()":1,
            "(REPO/'tests/fixtures/typed_project_source_binding/current-source.json').read_bytes()":1,
            '(REPO/name).read_bytes()':2}
        for old,count in replacements.items():
            text=exact(text,old,'_reader.read_path('+old[:-len('.read_bytes()')]+')',count)
    elif name=='scheduling_derivation':
        for old in ["(HERE / 'integration-roster-v1.json').read_bytes()", "(HERE/'integration-roster-v1.json').read_bytes()"]:
            text=exact(text,old,"_reader.read(U+'integration-roster-v1.json')")
    elif name=='ci':
        text=exact(text,function(text,'scheduling'),
                   "def scheduling():\n    return _reader.fresh_scheduling()")
    return text.encode()


class Fixture:
    def __init__(self, repo):
        self.repo=Path(repo).absolute()
        raw=regular(self.repo,ROOT+'mechanical-inputs-v1.json',MANIFEST[0])
        need(sha(raw)==MANIFEST[1],'fixed mechanical manifest')
        value=json.loads(raw)
        need(value['baseline']=='8debaa96c0238e3376567fa2c76517fda9937037' and
             value['git_base']==BASE,'retained commit association')
        self.rows={r['path']:r for r in value['rows']}
        need(len(self.rows)==len(value['rows'])==1063,'closed mechanical member count')
        self.git_rows={r['path']:r for r in value['git_rows']}
        need(len(self.git_rows)==15,'closed historical Git inputs')
        self.modules={}
        self.read_log=[]
        self._git_proof(value)

    def _git_proof(self, value):
        commit=bytes.fromhex(value['base_commit_raw'])
        need(blob('commit',commit)==BASE,'fixed raw Git commit')
        root=commit.splitlines()[0].decode().split(' ')[1]
        need(root==value['git_base_tree']=='c31f377ccca7ea34b8f741e8ce725975108e481d','fixed Git tree')
        trees={}
        for oid,hexraw in value['base_trees'].items():
            raw=bytes.fromhex(hexraw);need(blob('tree',raw)==oid,'raw Git tree identity')
            entries={};offset=0
            while offset<len(raw):
                end=raw.index(b'\0',offset);mode,name=raw[offset:end].decode().split(' ',1)
                child=raw[end+1:end+21].hex();need(name not in entries and len(child)==40,'Git entry')
                entries[name]=(mode,child);offset=end+21
            trees[oid]=entries
        used=set()
        for name,row in self.git_rows.items():
            need(row['commit']==BASE,'wrong per-blob commit association')
            oid=root;parts=name.split('/')
            for i,part in enumerate(parts):
                need(oid in trees,'missing Git tree proof');used.add(oid)
                mode,oid=trees[oid][part]
                need(mode==('100644' if i==len(parts)-1 else '40000'),'Git input mode')
            need(oid==row['git_blob'],'Git path/blob association')
        need(used==set(trees),'unlisted Git proof tree')

    def _body(self, row):
        raw=regular(self.repo,row['storage'],row['bytes'])
        need(sha(raw)==row['sha256'] and blob('blob',raw)==row['git_blob'] and
             row['mode']=='100644','retained body identity: '+row['path'])
        self.read_log.append(row['path'])
        return raw

    def read(self, name):
        need(name in self.rows,'unlisted retained read: '+str(name))
        return self._body(self.rows[name])

    def read_path(self, path):
        path=Path(path).absolute()
        need(path.is_relative_to(self.repo),'retained read outside logical root')
        return self.read(path.relative_to(self.repo).as_posix())

    def json_path(self, path):
        return json.loads(self.read_path(path))

    def git_read(self, commit, name):
        need(commit==BASE and name in self.git_rows,'unlisted Git commit/path')
        return self._body(self.git_rows[name])

    def authenticate_all(self):
        for name in self.rows:self.read(name)
        for name in self.git_rows:self.git_read(BASE,name)

    def _import(self, name, globals=None, locals=None, fromlist=(), level=0):
        need(level==0,'relative retained import forbidden')
        if name in HELPERS:return self.module(name)
        need(name.split('.')[0] in STDLIB,'unlisted retained import: '+name)
        return builtins.__import__(name,globals,locals,fromlist,level)

    def _exec(self, name, path, raw):
        m=types.ModuleType(name);m.__file__=str(self.repo/path)
        safe=dict(vars(builtins));safe['__import__']=self._import
        def no_open(*args,**kwargs):raise ValueError('unlisted retained open')
        safe['open']=no_open
        m.__dict__.update(__builtins__=safe,_reader=self,BASE=BASE,U=U)
        exec(compile(raw,m.__file__,'exec'),m.__dict__)
        return m

    def module(self, name):
        need(name in HELPERS,'unlisted retained helper')
        path=HELPERS[name]
        # Reauthenticate even on reuse: a late import cannot hide changed storage.
        raw=self.read(path)
        need('code' in self.rows[path]['roles'],'unlisted executable body')
        if name not in self.modules:self.modules[name]=self._exec(name,path,adapt(name,raw))
        return self.modules[name]

    def fresh_scheduling(self):
        path=HELPERS['scheduling_derivation'];raw=self.read(path)
        return self._exec('scheduling_derivation',path,adapt('scheduling_derivation',raw))

    def scheduling_outputs(self):
        return self.module('derive_scheduling').generate()

    def assert_scheduling_outputs(self):
        for name,body in self.scheduling_outputs().items():
            need(body==self.read(name),'retained expected output: '+name)

    def hosted_suite(self):
        path=Q+'test_hosted.py';raw=self.read(path);need('test-code' in self.rows[path]['roles'],'unlisted test code')
        text=raw.decode()
        text=exact(text,'sys.path.insert(0,str(HERE))\n','')
        # All original g.regular calls are the seven explicit fixture reads.
        text=exact(text,'g.regular(REPO,', '_reader.read(',7)
        module=self._exec('retained_hosted_tests',path,text.encode())
        return unittest.defaultTestLoader.loadTestsFromTestCase(module.HostedBridgeControls)

    def scheduling_suite(self, verify_current):
        path=U+'test_scheduling.py';raw=self.read(path)
        need('test-code' in self.rows[path]['roles'],'unlisted test code')
        text=raw.decode();tree=ast.parse(text)
        cls=next(n for n in tree.body if isinstance(n,ast.ClassDef) and n.name=='SchedulingControls')
        names={'test_named_derivation_outputs_and_immutable_history','test_original_consumer_generator_emits_named_workflow'}
        methods=[n for n in cls.body if isinstance(n,ast.FunctionDef) and n.name in names]
        need(len(methods)==2,'exact retained mechanical methods')
        chunks=[]
        for node in methods:
            body=ast.get_source_segment(text,node)
            # Replace only original IO calls, identified structurally, not strings.
            replacements=[]
            for n in ast.walk(ast.parse(body)):
                if isinstance(n,ast.Call) and isinstance(n.func,ast.Attribute) and n.func.attr=='read_bytes':
                    old=ast.get_source_segment(body,n);target=ast.get_source_segment(body,n.func.value)
                    replacements.append((old,'_reader.read_path('+target+')'))
            need(len(replacements) == (3 if node.name == "test_named_derivation_outputs_and_immutable_history" else 8), "closed test IO roster")
            for old,new in set(replacements):
                count = 2 if old == "(activation.REPO/name).read_bytes()" else 1
                body=exact(body,old,new,count)
            if node.name=='test_named_derivation_outputs_and_immutable_history':
                body=exact(body,"q.verify_package(activation.REPO, q.read(q.HERE/'inputs.json'))",'_verify_current()')
                body=exact(body,'q.read(activation.REPO/name)','_reader.json_path(activation.REPO/name)')
            chunks.append('\n'.join('    '+line for line in body.splitlines()))
        code='class RetainedScheduling(unittest.TestCase):\n'+'\n\n'.join(chunks)+'\n'
        a=self.module('derive_scheduling')
        scope={'unittest':unittest,'activation':a,'d':a.d,'copy':__import__('copy'),
               'json':json,'patch':patch,'_reader':self,'_verify_current':verify_current}
        exec(compile(code,str(self.repo/path),'exec'),scope)
        return unittest.defaultTestLoader.loadTestsFromTestCase(scope['RetainedScheduling'])
