#!/usr/bin/env python3
"""NEW reconstructed V3 receipt primitives; complete orchestration is not installed.

This module cannot issue a successful qualification receipt. Its finite helpers
are being reviewed before the complete recipe and upstream chain are assembled.
"""
import hashlib
import json
import math
import os
from pathlib import Path
import re
import stat
import subprocess


def need(condition,message):
    if not condition:raise ValueError(message)


def sha(raw):return hashlib.sha256(raw).hexdigest()


def strict_object(pairs):
    value={}
    for key,item in pairs:
        need(key not in value,'duplicate JSON key: '+key);value[key]=item
    return value


def finite_float(raw):
    value=float(raw);need(math.isfinite(value),'nonfinite JSON number');return value


def loads(raw):
    return json.loads(raw,object_pairs_hook=strict_object,parse_float=finite_float,
                      parse_constant=lambda value:(_ for _ in ()).throw(ValueError('nonfinite JSON value')))


def aliases(path):
    info=Path(path).lstat()
    return stat.S_ISLNK(info.st_mode) or bool(getattr(info,'st_file_attributes',0)&1024)


def regular_path(path):
    path=Path(path).absolute()
    need(not any(aliases(p) for p in (path,*path.parents)),'aliased/reparse input')
    need(stat.S_ISREG(path.lstat().st_mode),'nonregular input')
    return path


def windows_host():return os.name=='nt'


def stamp(info):
    if windows_host():
        need(hasattr(info,'st_birthtime_ns'),'missing native Windows birthtime')
        created=info.st_birthtime_ns
    else:created=info.st_ctime_ns
    return (info.st_dev,info.st_ino,info.st_mode,info.st_size,info.st_mtime_ns,created,
            getattr(info,'st_file_attributes',0))


def ancestor(path):
    need(not aliases(path),'aliased ancestor');info=path.lstat()
    birth=None
    if windows_host():
        need(hasattr(info,'st_birthtime_ns'),'missing native Windows ancestor birthtime');birth=info.st_birthtime_ns
    return (info.st_dev,info.st_ino,info.st_mode,birth,getattr(info,'st_file_attributes',0))


def read(path):
    path=regular_path(path);parents={p:ancestor(p) for p in path.parents};before=path.lstat()
    fd=os.open(path,os.O_RDONLY|getattr(os,'O_NOFOLLOW',0)|getattr(os,'O_NONBLOCK',0)|getattr(os,'O_BINARY',0))
    try:
        need(stamp(os.fstat(fd))==stamp(before),'input changed before open')
        chunks=[];remaining=before.st_size
        while remaining:
            chunk=os.read(fd,min(65536,remaining));need(chunk,'input shortened');chunks.append(chunk);remaining-=len(chunk)
        need(not os.read(fd,1) and stamp(os.fstat(fd))==stamp(before),'input changed during read')
    finally:os.close(fd)
    need(regular_path(path)==path and stamp(path.lstat())==stamp(before),'input changed after read')
    need({p:ancestor(p)for p in path.parents}==parents,'input ancestor changed')
    return b''.join(chunks)


def regular(path):return read(path)


def load(path):return loads(read(path))


def inventory(root):
    root=Path(root).absolute();need(root.is_dir() and not any(aliases(p)for p in(root,*root.parents)),'aliased evidence root')
    rows=[]
    for path in sorted(root.rglob('*')):
        need(not aliases(path),'aliased evidence member')
        mode=path.lstat().st_mode;name=path.relative_to(root).as_posix()
        if stat.S_ISDIR(mode):rows.append({'path':name,'kind':'directory','mode':stat.S_IMODE(mode)})
        else:
            raw=read(path);rows.append({'path':name,'kind':'file','mode':stat.S_IMODE(mode),'bytes':len(raw),'sha256':sha(raw)})
    return rows


def git(repo,*args):
    result=subprocess.run(['git','-C',str(repo),*args],capture_output=True,timeout=60)
    need(result.returncode==0 and not result.stderr,'Git identity command failed')
    return result.stdout


def tree_rows(raw):
    need(type(raw)is bytes and raw.endswith(b'\0'),'incomplete Git tree frame')
    rows=[]
    for item in raw[:-1].split(b'\0'):
        header,name=item.split(b'\t',1);mode,kind,blob=header.decode('ascii').split(' ');name=name.decode('utf-8')
        need(mode in ('100644','100755') and kind=='blob' and re.fullmatch('[0-9a-f]{40}',blob),'unsupported Git object')
        need(name and not name.startswith('/') and '\\' not in name and all(p not in ('','.','..')for p in name.split('/')),'noncanonical tree path')
        rows.append((name,mode,blob))
    need([r[0]for r in rows]==sorted({r[0]for r in rows}),'duplicate/reordered tree paths')
    return rows


def checkout_inputs(repo,expected_head,event_sha,initial_tree=None):
    need(type(expected_head)is str and re.fullmatch('[0-9a-f]{40}',expected_head) and
         type(event_sha)is str and re.fullmatch('[0-9a-f]{40}',event_sha),'missing exact external checkout context')
    head=git(repo,'rev-parse','HEAD').decode().strip();need(head==expected_head,'unexpected checkout head')
    tree=git(repo,'rev-parse',expected_head+'^{tree}').decode().strip()
    need(not git(repo,'status','--porcelain=v1','--untracked-files=all','--ignored=matching'),'dirty/untracked/ignored checkout input')
    raw=git(repo,'ls-tree','-r','-z',expected_head)
    need(initial_tree is None or raw==initial_tree,'checkout tree roster changed')
    rows=tree_rows(raw)
    index=git(repo,'ls-files','--stage','-z');indexed={}
    for item in index.rstrip(b'\0').split(b'\0'):
        header,name=item.split(b'\t',1);mode,blob,stage=header.decode().split();name=name.decode()
        need(stage=='0' and name not in indexed,'duplicate/unmerged index');indexed[name]=(mode,blob)
    need(set(indexed)=={r[0]for r in rows},'index/tree roster mismatch')
    identities=[]
    for name,mode,blob in rows:
        need(indexed[name]==(mode,blob),'index/tree object mismatch')
        path=Path(repo)/name;body=read(path)
        need(hashlib.sha1(b'blob '+str(len(body)).encode()+b'\0'+body).hexdigest()==blob,'checkout body mismatch: '+name)
        if not windows_host():need(('100755'if path.lstat().st_mode&0o111 else'100644')==mode,'checkout mode mismatch: '+name)
        identities.append({'path':name,'mode':mode,'blob':blob,'bytes':len(body),'sha256':sha(body)})
    need(git(repo,'rev-parse','HEAD').decode().strip()==head and
         git(repo,'rev-parse',expected_head+'^{tree}').decode().strip()==tree and
         not git(repo,'status','--porcelain=v1','--untracked-files=all','--ignored=matching') and
         git(repo,'ls-tree','-r','-z',expected_head)==raw
         and git(repo,'ls-files','--stage','-z')==index,'checkout changed during admission')
    return {'head':head,'tree':tree,'event_sha':event_sha,'inputs':identities},raw


def exact_execution(stdout,source,parser_output,static_output,executables):
    """Bind both actual producer executions; caller separately checks canonical bytes."""
    need(type(stdout)is bytes and stdout.endswith(b'\n'),'incomplete compiler JSON stream')
    rows=[loads(line)for line in stdout.splitlines()]
    need(all(type(row)is dict for row in rows),'nonobject compiler record')
    observed=[row for row in rows if row.get('kind')=='hir-producer']
    need([row.get('role')for row in observed]==['parser','static'],'missing/duplicate/reordered producer roles')
    inputs=[source,b'AST3'+len(source).to_bytes(2,'little')+source+parser_output]
    outputs=[parser_output,static_output]
    keys={'schema_version','edition','kind','role','executable_sha256','input_sha256','input_written',
          'captured_stdout_bytes','captured_stdout_sha256','captured_stderr_bytes','captured_stderr_sha256',
          'spawned','leader_reaped','stop','exit_status','signal'}
    for row,role,inp,out in zip(observed,('parser','static'),inputs,outputs):
        need(set(row)==keys,'unknown/missing producer binding')
        need(type(row['schema_version'])is int and row['schema_version']==1 and row['edition']=='typed-preview','producer schema/edition')
        need(row['executable_sha256']==executables[role] and row['input_sha256']==sha(inp) and
             row['captured_stdout_sha256']==sha(out) and row['captured_stderr_sha256']==sha(b''),'producer digest binding')
        for key,expected in [('input_written',len(inp)),('captured_stdout_bytes',len(out)),('captured_stderr_bytes',0),('exit_status',0)]:
            need(type(row[key])is int and row[key]==expected,'producer integer binding: '+key)
        need(row['spawned']is True and row['leader_reaped']is True and row['stop']=='exited' and row['signal']is None,
             'producer lifecycle binding')
    return b'\n'.join(line for line in stdout.splitlines()if loads(line).get('kind')!='hir-producer')+b'\n'


if __name__=='__main__':
    raise SystemExit('Incomplete reconstruction: this module cannot qualify a checkout or issue a complete receipt.')
