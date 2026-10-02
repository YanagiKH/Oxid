#!/usr/bin/env python3
"""Apply the exact reviewed overlay to explicit hash-verified compiler inputs."""
from common import *
import argparse,re,shutil

def apply_patch(root,patch_text):
    lines=patch_text.splitlines(keepends=True);at=0;touched=[]
    while at<len(lines):
        require(lines[at].startswith('--- '),'invalid exact patch header');old=lines[at][4:].rstrip('\n');at+=1
        require(at<len(lines)and lines[at].startswith('+++ '),'missing exact patch target');new=lines[at][4:].rstrip('\n');at+=1
        require(new.startswith('b/'),'patch destination prefix');rel=relative(new[2:]);require(rel.as_posix()not in touched,'duplicate patch target');touched.append(rel.as_posix());path=root/rel
        if old=='/dev/null':require(not path.exists(),'new patch file already exists');before=[]
        else:
            require(old=='a/'+rel.as_posix(),'patch path rename not allowed');require(path.is_file(),'patch source absent');before=path.read_text().splitlines(keepends=True)
        result=[];cursor=0;hunks=0
        while at<len(lines)and lines[at].startswith('@@ '):
            match=re.fullmatch(r'@@ -(\d+)(?:,(\d+))? \+(\d+)(?:,(\d+))? @@.*\n?',lines[at]);require(match is not None,'invalid exact hunk header');at+=1;hunks+=1
            old_line=int(match[1]);old_count=int(match[2]or'1');new_line=int(match[3]);new_count=int(match[4]or'1')
            offset=old_line-1 if old_line else 0;require(cursor<=offset<=len(before),'hunk source offset');result.extend(before[cursor:offset]);cursor=offset
            require(len(result)==(new_line-1 if new_line else 0),'hunk target offset');removed=added=0
            while at<len(lines)and not lines[at].startswith(('@@ ','--- ')):
                line=lines[at];at+=1;require(line and line[0]in ' +-','unknown patch record');kind=line[0];text=line[1:]
                if kind in ' -':require(cursor<len(before)and before[cursor]==text,'exact patch context differs');cursor+=1;removed+=1
                if kind in ' +':result.append(text);added+=1
            require(removed==old_count and added==new_count,'hunk cardinality mismatch')
        require(hunks>0,'patch has no hunk');result.extend(before[cursor:]);path.parent.mkdir(parents=True,exist_ok=True);path.write_text(''.join(result))
    require(bool(touched),'empty adapter patch');return touched

def assemble(repo,core_manifest,observer_root,observer_freeze,output):
    repo=pathlib.Path(repo).resolve();observer_root=pathlib.Path(observer_root).resolve()
    core=read_json(core_manifest);freeze=read_json(observer_freeze)
    require(identity(core_manifest)['sha256']==freeze['core_manifest_sha256'],'core/frozen adapter identity mismatch')
    check_manifest(repo,core['files']);check_manifest(observer_root,freeze['files'])
    frozen_files={x['path']:x for x in freeze['files']}
    for name in ('overlay-v5.patch','overlay-manifest-v5.json','run.py','parse_debug.py'):require(name in frozen_files,'reviewed frozen component absent')
    overlay=read_json(observer_root/'overlay-manifest-v5.json')
    require(overlay['core_manifest_sha256']==identity(core_manifest)['sha256'],'overlay/core mismatch')
    verify(observer_root/'overlay-v5.patch',frozen_files['overlay-v5.patch']);require(identity(observer_root/'overlay-v5.patch')['sha256']==overlay['patch_sha256'],'overlay patch mismatch')
    out=fresh(output);source=out/'source';source.mkdir();adapter=out/'frozen-observer';adapter.mkdir()
    for item in core['files']:
        relative_path=relative(item['path']);destination=source/relative_path;destination.parent.mkdir(parents=True,exist_ok=True);shutil.copyfile(repo/relative_path,destination)
    touched=apply_patch(source,(observer_root/'overlay-v5.patch').read_text())
    expected={x['path']:x for x in overlay['files']};actual=files_manifest(source)
    for item in actual:require(item['path']in expected and item==expected[item['path']],'prepared source differs from reviewed overlay')
    for name in ('overlay-v5.patch','overlay-manifest-v5.json','run.py','parse_debug.py'):
        shutil.copyfile(observer_root/name,adapter/name)
    shutil.copyfile(core_manifest,out/'core-source-manifest.json');shutil.copyfile(observer_freeze,out/'frozen-observer-manifest.json')
    manifest={'schema':SCHEMA+'-prepared-source','assertion_mode':assertion_mode(),'core_manifest':{'path':'core-source-manifest.json',**identity(out/'core-source-manifest.json')},'frozen_observer_manifest':{'path':'frozen-observer-manifest.json',**identity(out/'frozen-observer-manifest.json')},'adapter_files':files_manifest(adapter),'patch_sha256':identity(adapter/'overlay-v5.patch')['sha256'],'files':actual,'patched_files':touched,'source_directory':'source','adapter_directory':'frozen-observer','uncompiled_auxiliary_fixture_files_omitted':[x['path']for x in overlay['files']if x['path']not in {a['path']for a in actual}],'status':'prepared'}
    write_json(out/'prepared-source.json',manifest);return manifest

def main():
    p=argparse.ArgumentParser();p.add_argument('--repo',required=True);p.add_argument('--core-manifest',required=True);p.add_argument('--observer-root',required=True);p.add_argument('--observer-freeze',required=True);p.add_argument('--output',required=True);a=p.parse_args()
    result=assemble(a.repo,a.core_manifest,a.observer_root,a.observer_freeze,a.output);print(json.dumps({'status':result['status'],'source_files':len(result['files']),'output':str(pathlib.Path(a.output).resolve())}))
if __name__=='__main__':main()
