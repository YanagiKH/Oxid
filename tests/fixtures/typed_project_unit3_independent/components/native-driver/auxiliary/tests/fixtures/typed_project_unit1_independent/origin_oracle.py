#!/usr/bin/env python3
"""Independent UTF-8/CRLF byte oracle. No production code or output imports."""
if not __debug__:
    raise SystemExit('Qualification refuses optimized Python because it can remove verification guards')
import hashlib,json
from pathlib import Path
ROOT=Path(__file__).resolve().parent
files=[('root\t.ox','aé🦀\r\nZ\t'),('dir\nb.ox','abcdefg\r\nQ'),('c\x1b.ox','é\r\nzz')]

def valid(file,start,end):
    if not 0<=file<len(files) or start<0 or start>end:return False
    b=files[file][1].encode()
    if end>len(b):return False
    try: b[:start].decode();b[:end].decode()
    except UnicodeDecodeError:return False
    return True

def loc(b,offset):
    # Byte LF delimiting and independent Python Unicode decoding; no Rust data.
    prefix=b[:offset]
    prefix.decode()
    return [1+prefix.count(b'\n'),1+len(prefix.rsplit(b'\n',1)[-1].decode())]

def origin(file,start,end):
    assert valid(file,start,end)
    path,text=files[file];b=text.encode()
    return {'file':file,'path':path,'start':start,'end':end,'text':b[start:end].decode(),'begin':loc(b,start),'finish':loc(b,end)}

queries=[(0,1,3),(1,1,3),(0,9,10),(1,1,3),(2,4,6),(0,7,7),(0,8,8),(0,9,9),(2,6,6)]
invalid=[(0,1,2),(0,2,3),(0,4,7),(0,10,9),(0,0,12),(3,0,0),(1,11,11)]
assert origin(0,1,3)['text']=='é'
assert origin(1,1,3)['text']=='bc'
assert loc(files[0][1].encode(),7)==[1,4]
assert loc(files[0][1].encode(),8)==[1,5]
assert loc(files[0][1].encode(),9)==[2,1]
assert all(not valid(*p) for p in invalid)
result={'files':[{'id':i,'path':p,'text':t,'bytes':len(t.encode()),'sha256':hashlib.sha256(t.encode()).hexdigest()} for i,(p,t) in enumerate(files)],
        'valid_queries':[origin(*p) for p in queries],'invalid_queries':[list(p) for p in invalid],
        'diagnostic_labels':[origin(0,9,10),origin(1,1,3),origin(2,4,6)]}
(ROOT/'origin-oracle.json').write_text(json.dumps(result,ensure_ascii=False,indent=2)+'\n')
print(f'Independent origin oracle: {len(queries)} valid origins, {len(invalid)} malformed spans, three independent labels')
