#!/usr/bin/env python3
"""One deterministic source archive, bounded fresh materialization, no compiler."""
from common import *
import argparse,gzip,io,tarfile
MAX_FILES=445
MAX_FILE_BYTES=1_048_576
MAX_SOURCE_BYTES=16*1024*1024
MAX_TAR_BYTES=32*1024*1024
MAX_ARCHIVE_BYTES=32*1024*1024

def members_from_requests(requests,source_root):
    entries={};case_ids=set();count=0
    for line in pathlib.Path(requests).read_text().splitlines():
        request=json.loads(line);require(request['id']not in case_ids,'duplicate case ID');case_ids.add(request['id']);count+=1
        root=relative(request['source_root'])
        for item in request['source_files']:
            require(set(item)=={'path','bytes','sha256'},'unknown source member fields')
            path=(root/relative(item['path'])).as_posix();require(path not in entries,'aliased/repeated source member')
            require(type(item['bytes'])is int and 0<=item['bytes']<=MAX_FILE_BYTES,'source member size limit')
            full=pathlib.Path(source_root).resolve()/path;require(full.resolve()==full and not full.is_symlink(),'aliased source member');verify(full,item)
            entries[path]={'path':path,'bytes':item['bytes'],'sha256':item['sha256']}
    require(count==152 and len(entries)==445,'frozen source roster cardinality drift')
    require(sum(x['bytes']for x in entries.values())<=MAX_SOURCE_BYTES,'aggregate source ceiling')
    return [entries[p]for p in sorted(entries)]

def canonical_tar(members,payloads):
    stream=io.BytesIO()
    with tarfile.open(fileobj=stream,mode='w',format=tarfile.USTAR_FORMAT)as archive:
        for item in members:
            info=tarfile.TarInfo(item['path']);info.size=item['bytes'];info.mode=0o644;info.uid=info.gid=info.mtime=0;info.uname=info.gname=''
            archive.addfile(info,io.BytesIO(payloads[item['path']]))
    return stream.getvalue()

def pack(requests,source_root,output,oracle_manifest):
    requests=pathlib.Path(requests).resolve();source_root=pathlib.Path(source_root).resolve();out=fresh(output)
    oracle=read_json(oracle_manifest);require(oracle['schema']=='oxid-unit3-independent-pre-execution-v1','wrong oracle manifest')
    declared={x['path']:x for x in oracle['files']}
    verify(requests,declared[oracle['source_only_request_file']])
    members=members_from_requests(requests,source_root)
    require({x['path']for x in members}=={p for p in declared if p.startswith('sources/')},'oracle/source transport membership drift')
    for item in members:require(item==declared[item['path']],'request/oracle source identity mismatch')
    tar=canonical_tar(members,{x['path']:(source_root/x['path']).read_bytes()for x in members});require(len(tar)<=MAX_TAR_BYTES,'tar transport ceiling')
    (out/'sources.tar.gz').write_bytes(gzip.compress(tar,mtime=0));(out/'requests.jsonl').write_bytes(requests.read_bytes())
    manifest={'schema':SCHEMA+'-sources','assertion_mode':assertion_mode(),'members':members,'member_count':len(members),'source_bytes':sum(x['bytes']for x in members),'tar_bytes':len(tar),'tar_sha256':hashlib.sha256(tar).hexdigest(),'archive':{'path':'sources.tar.gz',**identity(out/'sources.tar.gz')},'requests':{'path':'requests.jsonl',**identity(out/'requests.jsonl')},'oracle_manifest':bound_file(oracle_manifest),'limits':{'members':MAX_FILES,'member_bytes':MAX_FILE_BYTES,'source_bytes':MAX_SOURCE_BYTES,'tar_bytes':MAX_TAR_BYTES,'archive_bytes':MAX_ARCHIVE_BYTES}}
    write_json(out/'source-transport.json',manifest);return manifest

def materialize(archive_path,manifest_path,requests_path,output):
    manifest=read_json(manifest_path);require(manifest['schema']==SCHEMA+'-sources','wrong source transport schema')
    require(pathlib.Path(archive_path).stat().st_size<=MAX_ARCHIVE_BYTES,'compressed size ceiling')
    verify(archive_path,manifest['archive']);verify(requests_path,manifest['requests'])
    with gzip.open(archive_path,'rb')as source:tar=source.read(MAX_TAR_BYTES+1)
    require(len(tar)<=MAX_TAR_BYTES and len(tar)==manifest['tar_bytes']and hashlib.sha256(tar).hexdigest()==manifest['tar_sha256'],'decompressed size/hash mismatch')
    expected={}
    for item in manifest['members']:
        relative(item['path']);require(item['path']not in expected,'duplicate manifest member');expected[item['path']]=item
        require(type(item['bytes'])is int and 0<=item['bytes']<=MAX_FILE_BYTES,'member size ceiling')
    require(len(expected)==manifest['member_count']==445 and sum(x['bytes']for x in expected.values())==manifest['source_bytes']<=MAX_SOURCE_BYTES,'aggregate roster/size mismatch')
    payloads={};actual_order=[]
    with tarfile.open(fileobj=io.BytesIO(tar),mode='r:')as archive:
        for member in archive:
            relative(member.name);require(member.isreg()and not member.pax_headers,'non-regular/extended archive member')
            require(member.name in expected and member.name not in payloads,'extra/duplicate archive member')
            require(member.size==expected[member.name]['bytes']<=MAX_FILE_BYTES,'archive member size mismatch')
            require(member.mode==0o644 and member.uid==member.gid==member.mtime==0 and member.uname==member.gname=='','noncanonical archive metadata')
            handle=archive.extractfile(member);require(handle is not None,'missing archive payload');data=handle.read(MAX_FILE_BYTES+1)
            require(len(data)==member.size and hashlib.sha256(data).hexdigest()==expected[member.name]['sha256'],'archive member hash mismatch')
            payloads[member.name]=data;actual_order.append(member.name)
    require(set(payloads)==set(expected)and actual_order==sorted(expected),'missing/unsorted member roster')
    require(tar==canonical_tar([expected[p]for p in actual_order],payloads),'noncanonical/trailing archive content')
    # Create only after all membership/content checks; never use extractall.
    out=fresh(output)
    for name in actual_order:
        destination=out/relative(name);destination.parent.mkdir(parents=True,exist_ok=True);destination.write_bytes(payloads[name])
    (out/'requests.jsonl').write_bytes(pathlib.Path(requests_path).read_bytes())
    check_manifest(out,list(expected.values()))
    receipt={'schema':SCHEMA+'-materialized','assertion_mode':assertion_mode(),'source_transport':bound_file(manifest_path),'archive':bound_file(archive_path),'requests':bound_file(out/'requests.jsonl'),'source_root':str(out),'members':list(expected.values()),'status':'materialized'}
    write_json(out/'materialization.json',receipt);return receipt

def main():
    p=argparse.ArgumentParser();commands=p.add_subparsers(dest='operation',required=True)
    s=commands.add_parser('pack');s.add_argument('--requests',required=True);s.add_argument('--source-root',required=True);s.add_argument('--oracle-manifest',required=True);s.add_argument('--output',required=True)
    s=commands.add_parser('materialize');s.add_argument('--archive',required=True);s.add_argument('--manifest',required=True);s.add_argument('--requests',required=True);s.add_argument('--output',required=True)
    a=p.parse_args();result=pack(a.requests,a.source_root,a.output,a.oracle_manifest)if a.operation=='pack'else materialize(a.archive,a.manifest,a.requests,a.output)
    print(json.dumps({'schema':result['schema'],'output':str(pathlib.Path(a.output).resolve())}))
if __name__=='__main__':main()
