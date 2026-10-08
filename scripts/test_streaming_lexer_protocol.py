"""Independent contextual stream controls; no producer executable is required."""
import copy
import json
from pathlib import Path
import struct
import tempfile
import unittest
from unittest.mock import patch

import build_streaming_lexer as builder
from streaming_lexer_protocol import Invalid, decode, output_bound, request


def frame(source, tokens, diagnostic=None):
    """Synthetic canonical packetizer for decoder tests, not producer evidence."""
    result=bytearray(b"LXS1")
    echo=bytearray(128)
    token_buffer=bytearray(100)
    at=0
    position=0
    for base in range(0,len(source)+1,128):
        used=min(128,len(source)-base)
        echo[:used]=source[base:base+used]
        result.extend(bytes([69,used])+struct.pack("<I",base)+echo)
        while position<len(tokens):
            kind,lo,hi=tokens[position]
            if hi>base+used or (kind==47 and used==128):
                break
            token_buffer[at:at+2]=bytes([kind,hi-base])
            at+=2
            position+=1
            if at==100:
                result.extend(token_buffer)
                at=0
        if at:
            result.extend(bytes([66,at])+token_buffer)
            at=0
        if used<128:
            break
    if diagnostic is None:
        result.extend(b"S"+struct.pack("<II",len(tokens),len(source)))
    else:
        tag,start,end=diagnostic
        result.extend(bytes([68,tag])+struct.pack("<II",start,end))
    return bytes(result)


class ProtocolTests(unittest.TestCase):
    def test_exact_source_and_full_width_spans(self):
        for n in (0,1,49,50,99,100,127,128,129,255,256,257,65536):
            source=b"x"*n
            tokens=([[2,0,n]] if n else [])+[[47,n,n]]
            self.assertEqual(decode(frame(source,tokens),source),{"tokens":tokens})

    def test_exact_raw_groups_and_carry_padding(self):
        for n in (49,50,51,99,100,128,129,256):
            source=b"+"*n
            tokens=[[41,i,i+1] for i in range(n)]+[[47,n,n]]
            self.assertEqual(decode(frame(source,tokens),source),{"tokens":tokens})
        source=b"x"*129
        wire=bytearray(frame(source,[[2,0,129],[47,129,129]]))
        # The second source echo keeps the preceding buffer's unused x tail.
        wire[4+134+6+1]^=1
        with self.assertRaisesRegex(Invalid,"carry padding"):
            decode(wire,source)
        wire=bytearray(frame(b"x",[[2,0,1],[47,1,1]]))
        wire[4+134+2+4]=1
        with self.assertRaisesRegex(Invalid,"token carry padding"):
            decode(wire,b"x")

    def test_truncation_suffix_and_same_shape_wrong_source(self):
        source=b"x"
        wire=frame(source,[[2,0,1],[47,1,1]])
        for end in range(len(wire)):
            with self.subTest(end=end),self.assertRaises(Invalid):
                decode(wire[:end],source)
        for suffix in (b"\0",b"S",wire,b"trailing"):
            with self.assertRaises(Invalid):
                decode(wire+suffix,source)
        with self.assertRaisesRegex(Invalid,"exact source"):
            decode(wire,b"y")

    def test_packet_and_source_mutations(self):
        source=b"x"
        original=frame(source,[[2,0,1],[47,1,1]])
        for offset,value in ((0,0),(4,0),(5,2),(6,1),(138,67),(139,0),(140,0),(141,2),(142,2),(143,0),(-1,1)):
            wire=bytearray(original);wire[offset]=value
            with self.subTest(offset=offset),self.assertRaises(Invalid):
                decode(wire,source)
        with self.assertRaisesRegex(Invalid,"budget"):
            decode(original,source,0)

    def test_diagnostic_fields_and_precedence_shapes(self):
        for tag,source in ((1,b'"open'),(2,b'/*open'),(3,b'x')):
            got=decode(frame(source,[],(tag,0,len(source))),source,0)
            self.assertEqual(got['diagnostic']['tag'],tag)
            self.assertEqual((got['diagnostic']['start'],got['diagnostic']['end']),(0,len(source)))
        for tag in (1,2):
            with self.assertRaisesRegex(Invalid,"unterminated diagnostic end"):
                decode(frame(b'abc',[],(tag,0,2)),b'abc',0)
        source=b'x'*65537
        self.assertEqual(decode(frame(source,[],(3,0,len(source))),source,0)['diagnostic']['end'],65537)
        self.assertEqual(decode(frame(b'',[[47,0,0]]),b'',0),{'tokens':[[47,0,0]]})

    def test_request_and_bound_domains(self):
        self.assertEqual(request(b'x',7),b'LXI1'+struct.pack('<II',1,7)+b'x')
        self.assertEqual(output_bound(1048576,100000),2133666)
        for source,limit in ((b'\xff',1),(b'x',-1),(b'x',100001),(b'x'*1048577,1)):
            with self.assertRaises(Invalid):request(source,limit)


class BuilderTests(unittest.TestCase):
    def test_exact_genuine_modular_source_closure(self):
        raw,sources=builder.checked_sources()
        manifest=json.loads(raw)
        self.assertEqual(set(sources),{'main','frame','scanner','keywords','data'})
        self.assertEqual(sum(map(len,sources.values())),10470)
        for name,data in sources.items():
            self.assertEqual(builder.digest(data),manifest['sources'][name]['sha256'])

    def test_changed_missing_extra_and_escaping_members_refused(self):
        with tempfile.TemporaryDirectory() as directory:
            root=Path(directory)
            _,sources=builder.checked_sources()
            fixture=root/'fixtures/typed-streaming-lexer';fixture.mkdir(parents=True)
            for name,data in sources.items():(fixture/(name+'.ox')).write_bytes(data)
            original=json.loads(builder.MANIFEST.read_bytes());manifest=fixture/'sources.json'
            variants=[]
            wrong=copy.deepcopy(original);wrong['sources']['main']['sha256']='0'*64;variants.append(wrong)
            wrong=copy.deepcopy(original);del wrong['sources']['frame'];variants.append(wrong)
            wrong=copy.deepcopy(original);wrong['sources']['unused']=wrong['sources']['main'];variants.append(wrong)
            wrong=copy.deepcopy(original);wrong['sources']['main']['path']='../main.ox';variants.append(wrong)
            for value in variants:
                manifest.write_text(json.dumps(value))
                with patch.object(builder,'ROOT',root),patch.object(builder,'MANIFEST',manifest):
                    with self.assertRaises((ValueError,KeyError)):builder.checked_sources()


if __name__=='__main__':unittest.main()
