#!/usr/bin/env python3
"""Independently authored stable-filesystem fixtures; never invokes compiler code."""
if not __debug__:
    raise SystemExit('Qualification refuses optimized Python because it can remove verification guards')
import hashlib,json,os,socket
from pathlib import Path
ROOT=Path(__file__).resolve().parent
OUT=ROOT/'fixtures'
OUT.mkdir(exist_ok=True)
manifest=[]

def fixture(name, files, expect, entry='app.ox', special=()):
    d=OUT/name
    d.mkdir(exist_ok=True)
    for rel, data in files.items():
        p=d/rel;p.parent.mkdir(parents=True,exist_ok=True)
        p.write_bytes(data.encode() if isinstance(data,str) else data)
    unavailable=[]
    for kind,rel,target in special:
        p=d/rel;p.parent.mkdir(parents=True,exist_ok=True)
        if kind=='directory': p.mkdir(exist_ok=True)
        elif not p.exists() and not p.is_symlink():
            if kind=='symlink': p.symlink_to(target)
            elif kind=='hardlink': os.link(d/target,p)
            elif kind=='fifo': os.mkfifo(p)
            elif kind=='socket':
                # Creation was already attempted and denied. Do not retry.
                unavailable.append({'kind':kind,'reason':'Prior socket.socket(AF_UNIX) failed: [Errno 1] Operation not permitted; creation stopped'})
            else: raise ValueError(kind)
    source_manifest=[]
    # Only files authored above or declared hardlinks are read; never enumerate
    # and accidentally open a FIFO or follow an unrelated fixture symlink.
    for rel in list(files)+[rel for kind,rel,target in special if kind=='hardlink']:
        data=(d/rel).read_bytes()
        source_manifest.append({'path':rel,'bytes':len(data),'sha256':hashlib.sha256(data).hexdigest()})
    manifest.append({'id':name,'entry':entry,'sources':source_manifest,'special':special,'expected':expect,'unavailable':unavailable})

ok=lambda names: {'result':'ok','module_paths':names}
err=lambda code,stage,file,start,end: {'result':'error','code':code,'stage':stage,'primary':{'file':file,'start':start,'end':end}}
f='fn f()->(){return;}\n'
fixture('minimal',{'app.ox':'mod a;\n','a.ox':f},{**ok(['','a']),'usage':{'bytes':27,'tokens':18,'nodes':3,'eofs':2,'probes':1,'entries':2,'name_units':10}})
fixture('public_module_header',{'app.ox':'pub mod a;\n','a.ox':''},{**ok(['','a']),'usage':{'bytes':11,'tokens':7,'nodes':1,'eofs':2},'visibility_span':[0,3]})
fixture('no_textual_mod_discovery',{'app.ox':'// mod absent;\n/* pub mod gone; */\nfn modded()->(){return;}\n'},ok(['']))
fixture('nested_mod_rejected',{'app.ox':'fn f()->(){mod a;return;}\n'},{'result':'error','code':'E0101','stage':'parse','primary_file':0,'retained_files':1})
fixture('inline_mod_rejected',{'app.ox':'mod a {}\n'},{'result':'error','code':'E0101','stage':'parse','primary_file':0,'retained_files':1})
fixture('dfs_module_major',{
 'app.ox':'fn r0()->(){return;}\nstruct R0 {}\nmod a;\nfn r1()->(){return;}\nmod b;\nstruct R1 {}\n',
 'a.ox':'fn a0()->(){return;}\nstruct A0 {}\nmod c;\nfn a1()->(){return;}\nstruct A1 {}\n',
 'a/c.ox':'fn c0()->(){return;}\nstruct C0 {}\n',
 'b.ox':'fn b0()->(){return;}\nstruct B0 {}\n'},
 {**ok(['','a','a/c','b']),'function_handles':[[0,0,'r0'],[0,1,'r1'],[1,0,'a0'],[1,1,'a1'],[2,0,'c0'],[3,0,'b0']],
  'record_handles':[[0,0,'R0'],[0,1,'R1'],[1,0,'A0'],[1,1,'A1'],[2,0,'C0'],[3,0,'B0']]})
fixture('missing_unused',{'app.ox':'mod absent;\nfn main()->i32{return 0;}\n'},err('E0002','source',0,4,10))
fixture('no_mod_ox_fallback',{'app.ox':'mod a;\n','a/mod.ox':f},err('E0002','source',0,4,5))
fixture('unrelated_malformed',{'app.ox':'mod a;\n','a.ox':f,'other.ox':'\x00not code'},ok(['','a']))
fixture('unrelated_special',{'app.ox':'mod a;\n','a.ox':f},ok(['','a']),special=[('fifo','ignored.ox',None),('symlink','OTHER.ox','missing-target')])
fixture('root_parse_before_missing',{'app.ox':'mod a;\nfn broken( -> () {}\n'},{'result':'error','code':'E0100','stage':'parse','primary_file':0,'retained_files':1})
fixture('child_lex_before_sibling',{'app.ox':'mod a;\nmod absent;\n','a.ox':'/*'},err('E0100','lex',1,0,2))
fixture('child_parse_before_descendants',{'app.ox':'mod a;\nmod absent;\n','a.ox':'mod inner;\nfn broken( -> () {}\n'},{'result':'error','code':'E0100','stage':'parse','primary_file':1,'retained_files':2})
fixture('load_before_resolution',{'app.ox':'fn x()->(){return;}\nfn x()->(){return;}\nmod absent;\n'},err('E0002','source',0,44,50))
fixture('entry_symlink_directory',{'elsewhere/original.ox':'mod a;\n','view/a.ox':f,'elsewhere/a.ox':'/*'},ok(['','a']),entry='view/app.ox',special=[('symlink','view/app.ox','../elsewhere/original.ox')])
fixture('parent_directory_symlink',{'real/app.ox':'mod a;\n','real/a.ox':f},ok(['','a']),entry='alias/app.ox',special=[('symlink','alias','real')])
fixture('display_components',{'view/app.ox':'mod a;\n','view/a.ox':f},ok(['','a']),entry='./view/../view/app.ox')
fixture('child_final_symlink',{'app.ox':'mod a;\n','target.ox':f},err('E0005','source',0,4,5),special=[('symlink','a.ox','target.ox')])
fixture('child_dangling_symlink',{'app.ox':'mod a;\n'},err('E0005','source',0,4,5),special=[('symlink','a.ox','missing-target')])
fixture('child_directory_symlink',{'app.ox':'mod a;\n','a.ox':'mod b;\n','inner/b.ox':f},err('E0005','source',1,4,5),special=[('symlink','a','inner')])
fixture('child_final_directory',{'app.ox':'mod a;\n'},err('E0005','source',0,4,5),special=[('directory','a.ox',None)])
fixture('child_final_fifo',{'app.ox':'mod a;\n'},err('E0005','source',0,4,5),special=[('fifo','a.ox',None)])
fixture('child_final_socket',{'app.ox':'mod a;\n'},err('E0005','source',0,4,5),special=[('socket','a.ox',None)])
fixture('child_intermediate_file',{'app.ox':'mod a;\n','a.ox':'mod b;\n','a':'ordinary'},err('E0005','source',1,4,5))
fixture('repeated_entry_path',{'a.ox':'mod a;\n'}, {**err('E0005','source',0,4,5),'secondary':{'file':0,'start':7,'end':7}},special=[('symlink','app.ox','a.ox')])
fixture('distinct_hardlinks',{'app.ox':'mod a;\nmod b;\n','a.ox':'struct Item {}\n'}, {**ok(['','a','b']),'record_handles':[[1,0,'Item'],[2,0,'Item']]},special=[('hardlink','b.ox','a.ox')])
fixture('entry_hardlink_distinct',{'app.ox':'mod a;\n'},err('E0002','source',1,4,5),special=[('hardlink','a.ox','app.ox')])
fixture('fold_only',{'app.ox':'mod jobs;\n','Jobs.ox':f},err('E0005','source',0,4,8))
fixture('exact_and_fold',{'app.ox':'mod jobs;\n','jobs.ox':f,'Jobs.ox':'/*'},ok(['','jobs']))
fixture('neither_exact_nor_fold',{'app.ox':'mod jobs;\n','unrelated.ox':f},err('E0002','source',0,4,8))
fixture('sibling_fold_collision',{'app.ox':'mod Jobs;\nmod jobs;\n','Jobs.ox':f},{**err('E0005','source',0,14,18),'secondary':{'file':0,'start':4,'end':8}})
fixture('sibling_exact_collision',{'app.ox':'mod a;\nmod a;\n','a.ox':f},{**err('E0201','resolve',0,11,12),'secondary':{'file':0,'start':4,'end':5}})
fixture('first_missing_before_collision',{'app.ox':'mod Jobs;\nmod jobs;\n'},err('E0002','source',0,4,8))
fixture('intermediate_fold_only',{'app.ox':'mod a;\n','a.ox':'mod b;\n','A/b.ox':f},err('E0005','source',1,4,5))
fixture('separate_parent_names',{'app.ox':'mod a;\nmod b;\n','a.ox':'mod leaf;\n','b.ox':'mod leaf;\n','a/leaf.ox':f,'b/leaf.ox':f},ok(['','a','a/leaf','b','b/leaf']))
fixture('invalid_utf8',{'app.ox':'mod a;\n','a.ox':b'\xff'},err('E0003','source',0,4,5))
fixture('oxbc_before_utf8',{'app.ox':'mod a;\n','a.ox':b'OXBC\xff'},err('E0004','source',0,4,5))
fixture('empty_child',{'app.ox':'mod a;\n','a.ox':''},{**ok(['','a']),'usage':{'bytes':7,'tokens':5,'nodes':1,'eofs':2}})
fixture('file_local_expr_blocks',{'app.ox':'mod a;\nfn root()->i32{return 7;}\n','a.ox':'fn child()->i32{return 9;}\n'}, {**ok(['','a']),'local_expressions':[[0,0,'7'],[1,0,'9']],'local_blocks':[[0,0,0],[1,0,0]]})
fixture('utf8_crlf_origins',{'app.ox':'//é🦀\r\nmod a;\r\nfn root()->i32{return 7;}\r\n','a.ox':'//abc\r\nfn child()->i32{return 9;}\r\n'},ok(['','a']))
fixture('unrelated_nonutf8',{'app.ox':'mod a;\n','a.ox':f},{**ok(['','a']),'usage':{'probes':1,'entries':3,'name_units':20}})
nonutf8=os.fsencode(OUT/'unrelated_nonutf8')+b'/'+b'\xff'*10
fd=os.open(nonutf8,os.O_CREAT|os.O_WRONLY,0o600);os.close(fd)
# Validate literal location entries independently using the fixture bytes.
for row in manifest:
    exp=row['expected'];p=exp.get('primary')
    if p and p['file']==0:
        data=(OUT/row['id']/row['entry']).read_bytes()
        exp['primary']['literal_hex']=data[p['start']:p['end']].hex()
    for src in row['sources']:
        # The manifest inventory itself must not mutate on repeated generation.
        assert hashlib.sha256((OUT/row['id']/src['path']).read_bytes()).hexdigest()==src['sha256']
# Canonical identity must retain native path bytes, including when lossy
# Unicode would collapse a permitted root-entry target and a distinct child.
d=OUT/'native_path_identity';d.mkdir(exist_ok=True);native=os.fsencode(d)
for leaf in [b'\xff',b'\xfe']:os.makedirs(native+b'/'+leaf,exist_ok=True)
raw_sources=[(b'\xff/a.ox',b'fn child()->(){return;}\n'),(b'\xfe/a.ox',b'mod a;\n')]
for rel,data in raw_sources:
    fd=os.open(native+b'/'+rel,os.O_CREAT|os.O_WRONLY|os.O_TRUNC,0o600);os.write(fd,data);os.close(fd)
for name,target in [(b'alias',b'\xff'),(b'\xff/app.ox',b'../\xfe/a.ox')]:
    if not os.path.lexists(native+b'/'+name):os.symlink(target,native+b'/'+name)
canonical_root=os.path.realpath(native+b'/alias/app.ox');canonical_child=os.path.realpath(native+b'/alias/a.ox')
assert canonical_root!=canonical_child
assert canonical_root.decode(errors='replace')==canonical_child.decode(errors='replace')
manifest.append({'id':'native_path_identity','entry':'alias/app.ox','sources':[
    {'native_relative_hex':rel.hex(),'bytes':len(data),'sha256':hashlib.sha256(data).hexdigest()} for rel,data in raw_sources],
    'expected':{**ok(['','a']),'canonical_root_file_hex':canonical_root.hex(),'canonical_child_hex':canonical_child.hex(),
                'note':'Root and child canonical bytes differ but lossy UTF-8 strings collide'},'unavailable':[]})
(ROOT/'fixture-manifest.json').write_text(json.dumps({'contract_sha256':'25faad72c1c05370e80c43c8ef890b99ebd608baa39f1431521387d5ca44e0aa','platform_fixture':'Linux native pathname bytes','fixtures':manifest},indent=2)+'\n')
print(f'Materialized {len(manifest)} fixture cases; no compiler invoked')
