#!/usr/bin/env python3
"""Fail-closed public lexical route controls using real and controlled native producers."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess
import time

from build_streaming_lexer import BUNDLE_MAGIC
from streaming_lexer_protocol import request

ROOT=Path(__file__).resolve().parents[1]


def digest(data):return hashlib.sha256(data).hexdigest()

def require(ok,message):
    if not ok:raise RuntimeError(message)


def expected_receipts():
    rows=[("genuine-wire",0)]
    def failures(name,operations=("check","run","compile")):
        for operation in operations:
            for exists in ([True,False] if operation=="compile" else [False]):
                rows.append((name+"-"+operation+("-existing" if exists else "-absent"),1))
    names=("bad-magic","truncated","trailing","echo-base","echo-size","echo-source",
           "token-kind","partial-odd","token-padding","terminal-count",
           "stale-same-token-shape","false-diagnostic")
    for name in names:
        rows.append(("build-helper-"+name,0));failures(name)
    for name in ("status","stderr","stderr-limit","stdout-limit","timeout"):
        rows.append(("build-helper-"+name,0))
        failures(name,("check","compile") if name=="timeout" else ("check","run","compile"))
    for name in ("unterminated-string","unterminated-comment","unicode-domain","fuel-refusal",
                 "missing-bundle","identity-mismatch"):
        failures(name,("check","compile") if name=="fuel-refusal" else ("check","run","compile"))
    rows += [("process-run",7),("changed-module-before",0),("changed-module-after",0)]
    return rows


class Controls:
    def __init__(self,compiler,llvm,bundle,output,cc="cc"):
        self.compiler=compiler.resolve(strict=True);self.llvm=llvm.resolve(strict=True)
        self.bundle=bundle.resolve(strict=True);self.root=output.resolve();self.root.mkdir()
        self.cc=cc;self.env=dict(os.environ,OXID_LLVM_BIN=str(self.llvm));self.receipts=[]
        self.source=b"fn main()->i32{return 7;}";self.path=self.root/"main.ox";self.path.write_bytes(self.source)
        r=self.call("genuine-wire",[self.bundle/"lexer"],request(self.source))
        require(not r.stderr,"genuine producer stderr");self.wire=r.stdout

    def call(self,name,argv,data=None,status=0):
        command=[str(x)for x in argv];start=time.monotonic()
        r=subprocess.run(command,input=data,capture_output=True,env=self.env,cwd=ROOT,timeout=20)
        for stream in ("stdout","stderr"):(self.root/(name+"."+stream)).write_bytes(getattr(r,stream))
        self.receipts.append(dict(name=name,argv=command,status=r.returncode,elapsed_seconds=time.monotonic()-start,
                                 input_sha256=digest(data)if data is not None else None,
                                 stdout_sha256=digest(r.stdout),stderr_sha256=digest(r.stderr)))
        (self.root/"receipts.json").write_text(json.dumps(self.receipts,indent=2)+"\n")
        require(r.returncode==status,f"{name}: status{r.returncode}: {r.stdout!r} {r.stderr!r}")
        return r

    def bundle_for(self,name,data,mode="normal"):
        bundle=self.root/("bundle-"+name);bundle.mkdir()
        code='#include <unistd.h>\n#include <signal.h>\nstatic const unsigned char data[]={'+','.join(map(str,data))+'};\n'
        code+='int main(void){signal(SIGALRM,SIG_DFL);alarm(12);char input[4096];while(read(0,input,sizeof input)>0){};'
        if mode=="timeout":code+='for(;;)pause();'
        elif mode=="stderr":code+='write(2,"bad",3);'
        elif mode=="stderr-limit":code+='char err[5000]={0};write(2,err,sizeof err);'
        elif mode=="stdout-limit":code+='char out[8192]={0};write(1,out,sizeof out);'
        code+='write(1,data,sizeof data);return '+("17"if mode=="status"else"0")+';}\n'
        (bundle/"lexer.c").write_text(code)
        self.call("build-helper-"+name,[self.cc,"-O0",bundle/"lexer.c","-o",bundle/"lexer"])
        (bundle/"manifest.txt").write_text(BUNDLE_MAGIC+digest((bundle/"lexer").read_bytes())+"\n")
        return bundle

    def failure(self,name,bundle,source=None,diagnostic="E0703",parser=False,records=1,operations=("check","run","compile")):
        path=self.path
        if source is not None:
            path=self.root/(name+".ox");path.write_bytes(source)
        for operation in operations:
            for exists in ([True,False]if operation=="compile"else[False]):
                label=name+"-"+operation+("-existing"if exists else"-absent")
                target=self.root/(label+".elf")
                if exists:target.write_bytes(b"KEEP-LEXICAL-OUTPUT")
                argv=[self.compiler,operation,path,"--edition=typed-preview","--experimental-lexical-provider",bundle,"--message-format=json"]
                if operation=="compile":argv += ["--backend=llvm","--output",target]
                r=self.call(label,argv,status=1)
                rows=[json.loads(line)for line in r.stdout.splitlines()]
                receipts=[row for row in rows if row.get("kind")=="lexical-provider"]
                require(len(receipts)==records,label+": exact invocation count")
                for receipt in receipts:
                    require(receipt["fallback"] is False and receipt["producer_tokens_reached_parser"] is parser,label+": no fallback/parser transfer")
                require(any(row.get("code")==diagnostic for row in rows),label+": expected diagnostic")
                require(rows[-1].get("success") is False,label+": failed summary")
                if exists:require(target.read_bytes()==b"KEEP-LEXICAL-OUTPUT",label+": existing output preserved")
                else:require(not target.exists(),label+": absent output remains absent")

    def mutations(self):
        variants={"bad-magic":b"BAD!"+self.wire[4:],"truncated":self.wire[:-1],"trailing":self.wire+b"X"}
        for name,offset,value in (("echo-base",6,1),("echo-size",5,len(self.source)-1),
                                  ("echo-source",10,ord('x')),("token-kind",140,2),
                                  ("partial-odd",139,self.wire[139]-1),
                                  ("token-padding",140+self.wire[139],1)):
            raw=bytearray(self.wire);raw[offset]=value;variants[name]=bytes(raw)
        raw=bytearray(self.wire);raw[-8]^=1;variants["terminal-count"]=bytes(raw)
        for name,data in variants.items():self.failure(name,self.bundle_for(name,data))
        stale=self.bundle_for("stale-same-token-shape",self.wire)
        self.failure("stale-same-token-shape",stale,self.source.replace(b'7',b'8'))
        # A structurally complete diagnostic over a valid source must not replace canonical success.
        diagnostic=self.wire[:138]+bytes([68,1])+bytes(4)+len(self.source).to_bytes(4,'little')
        self.failure("false-diagnostic",self.bundle_for("false-diagnostic",diagnostic))
        for mode in ("status","stderr","stderr-limit","stdout-limit","timeout"):
            ops=("check","compile")if mode=="timeout"else("check","run","compile")
            self.failure(mode,self.bundle_for(mode,self.wire,mode),operations=ops)

    def genuine_failures(self):
        for name,source in (("unterminated-string",b'fn main()->i32 { "open'),
                            ("unterminated-comment",b'fn main()->i32 { /*open')):
            self.failure(name,self.bundle,source,diagnostic="E0100")
        self.failure("unicode-domain",self.bundle,"// 雪\nfn main()->i32{return 7;}".encode())
        self.failure("fuel-refusal",self.bundle,b'a'*65537,operations=("check","compile"))
        missing=self.root/"missing-bundle"
        self.failure("missing-bundle",missing,records=0)
        bad=self.root/"bad-manifest";bad.mkdir()
        (bad/"manifest.txt").write_text(BUNDLE_MAGIC+"0"*64+"\n")
        (bad/"lexer").write_bytes((self.bundle/"lexer").read_bytes());(bad/"lexer").chmod(0o755)
        self.failure("identity-mismatch",bad,records=0)

    def process_and_changed_modules(self):
        # Process stdout must remain exact bytes even while every module gets a receipt.
        source=b'fn main()->i32{return 7;}'
        path=self.root/"process.ox";path.write_bytes(source)
        result=self.call("process-run",[self.compiler,"run",path,"--edition=typed-preview","--entry-mode=process",
                                       "--experimental-lexical-provider",self.bundle],status=7)
        require(not result.stdout,"Process stdout was polluted")
        records=[json.loads(line)for line in result.stderr.splitlines()]
        require(len(records)==1 and records[0]["producer_tokens_reached_parser"] is True,"Process receipt channel")
        for revision,value in (("before",7),("after",18)):
            directory=self.root/revision;directory.mkdir()
            (directory/"main.ox").write_bytes(b'mod util; fn main()->i32{return crate::util::value();}')
            (directory/"util.ox").write_bytes((b' '*300 if revision=="after"else b'')+f'pub fn value()->i32{{return {value};}}'.encode())
            r=self.call("changed-module-"+revision,[self.compiler,"run",directory/"main.ox","--edition=typed-preview",
                                                  "--experimental-lexical-provider",self.bundle,"--message-format=json"])
            rows=[json.loads(line)for line in r.stdout.splitlines()]
            require(rows[-1]["result"]=={"type":"i32","value":value},"changed source result")
            records=[row for row in rows if row.get("kind")=="lexical-provider"]
            require(len(records)==2 and all(row["producer_tokens_reached_parser"] is True and row["fallback"] is False for row in records),"changed module provider usage")
            for row in records:
                data=Path(row["path"]).read_bytes();require(row["source_sha256"]==digest(data),"changed module source identity")

    def run(self):
        self.mutations();self.genuine_failures();self.process_and_changed_modules()
        summary=dict(schema_version=1,status="passed",compiler_sha256=digest(self.compiler.read_bytes()),
                     producer_sha256=digest((self.bundle/"lexer").read_bytes()),
                     harness_sha256=digest(Path(__file__).read_bytes()),receipts=len(self.receipts),
                     receipts_sha256=digest((self.root/"receipts.json").read_bytes()))
        (self.root/"summary.json").write_text(json.dumps(summary,indent=2)+"\n")
        print(json.dumps(summary,indent=2))
        return summary


if __name__=="__main__":
    parser=argparse.ArgumentParser(description=__doc__)
    for name in ("compiler","llvm-bin","bundle","output"):parser.add_argument("--"+name,required=True,type=Path)
    parser.add_argument("--cc",default="cc");a=parser.parse_args()
    Controls(a.compiler,a.llvm_bin,a.bundle,a.output,a.cc).run()
