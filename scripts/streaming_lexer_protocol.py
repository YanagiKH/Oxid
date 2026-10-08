"""Independent strict LXI1/LXS1 decoder; no production Rust dependencies."""
from __future__ import annotations
import struct
MAGIC=b'LXS1'
MAX_SOURCE=1048576
MAX_TOKENS=100000
MAX_TOKEN_BYTES=65536
DIAGNOSTICS={1:('E0100','unterminated string literal'),2:('E0100','unterminated block comment'),3:('E0400','token resource limit exceeded')}
class Invalid(ValueError):pass
def require(value, message):
 if not value: raise Invalid(message)
def request(source:bytes,limit:int=MAX_TOKENS)->bytes:
 require(source.isascii() and len(source)<=MAX_SOURCE,'source domain')
 require(0<=limit<=MAX_TOKENS,'token budget')
 return b'LXI1'+struct.pack('<II',len(source),limit)+source
def output_bound(n:int,limit:int=MAX_TOKENS)->int:
 require(0<=n<=MAX_SOURCE and 0<=limit<=MAX_TOKENS,'bound domain')
 chunks=n//128+1
 return 4+134*chunks+2*(min(n,limit)+1)+102*(chunks+1)+10
def decode(raw:bytes,source:bytes,limit:int=MAX_TOKENS)->dict:
 request(source,limit)
 require(raw[:4]==MAGIC,'magic')
 require(len(raw)<=output_bound(len(source),limit),'wire bound')
 offset=4;base=None;echoed=0;final_echo=False;last_echo=bytes(128);last_tokens=bytes(100);raw_group=bytearray();tokens=[];previous=0;eof=False;echo_count=0;partial_seen=False
 def take(n):
  nonlocal offset
  require(offset+n<=len(raw),'truncated record');part=raw[offset:offset+n];offset+=n;return part
 def token(kind,relative):
  nonlocal previous,eof
  require(base is not None and not eof,'token state')
  require(1<=kind<=47 and 0<=relative<=128,'token kind/offset')
  end=base+relative
  require(previous<=end<=echoed<=len(source),'token extent')
  if kind==47:
   require(final_echo and previous==end==len(source),'EOF span');eof=True
  else:
   require(previous<end and end-previous<=MAX_TOKEN_BYTES,'non-EOF width')
   require(len(tokens)<limit,'non-EOF budget')
  tokens.append([kind,previous,end]);previous=end
 while offset<len(raw):
  tag=raw[offset]
  if 1<=tag<=47:
   require(not partial_seen,'tokens after partial group')
   pair=take(2);raw_group.extend(pair);token(*pair)
   require(len(raw_group)<=100,'raw group width')
   if len(raw_group)==100:last_tokens=bytes(raw_group);raw_group.clear()
  elif tag==66:
   require(base is not None and not raw_group and not eof and not partial_seen,'partial group state')
   record=take(102);used=record[1];buf=record[2:]
   require(2<=used<=98 and used%2==0,'partial group size')
   require(buf[used:]==last_tokens[used:],'token carry padding')
   for i in range(0,used,2):token(buf[i],buf[i+1])
   last_tokens=buf;partial_seen=True
  elif tag==69:
   require(not raw_group and not eof and not final_echo,'echo state')
   record=take(134);used=record[1];new_base=int.from_bytes(record[2:6],'little');buf=record[6:]
   require(new_base==echoed,'echo base')
   expected=min(128,len(source)-echoed)
   require(used==expected,'echo logical size')
   require(buf[:used]==source[echoed:echoed+used],'exact source bytes')
   require(buf[used:]==last_echo[used:],'echo carry padding')
   base=new_base;echoed+=used;last_echo=buf;echo_count+=1;partial_seen=False
   final_echo=used<128
  elif tag in (68,83):
   require(not raw_group and final_echo and echoed==len(source),'terminal source/group state')
   require(echo_count==len(source)//128+1,'echo count')
   if tag==83:
    record=take(9);count,n=struct.unpack('<II',record[1:]);require(eof and count==len(tokens) and n==len(source),'success terminal')
    result={'tokens':tokens}
   else:
    record=take(10);kind=record[1];lo,hi=struct.unpack('<II',record[2:]);require(not eof and kind in DIAGNOSTICS,'diagnostic state')
    require(lo==previous and lo<hi<=len(source),'diagnostic extent')
    require(kind==3 or hi==len(source),'unterminated diagnostic end')
    code,message=DIAGNOSTICS[kind]
    result={'diagnostic':{'tag':kind,'code':code,'stage':'lex','message':message,'start':lo,'end':hi}}
   require(offset==len(raw),'trailing bytes')
   return result
  else:raise Invalid('unknown record')
 raise Invalid('missing terminal')
