#!/usr/bin/env python3
"""Full canonical/live/import admission for the unchanged 64-case residual packet."""
import argparse
import json
from pathlib import Path
from qualify_hir_producers_v3 import Run, digest, require


class PacketRun(Run):
    def bindings(self, name, result, source, opa, wire):
        ast=b'AST3'+len(source).to_bytes(2,'little')+source+opa
        rows=[r for r in map(json.loads,result.stdout.splitlines()) if r.get('kind')=='hir-producer']
        require(len(rows)==2,name+': producer count')
        for row,role,inp,out in zip(rows,['parser','static'],[source,ast],[opa,wire]):
            require(row['role']==role and row['input_sha256']==digest(inp)
                    and row['executable_sha256']==digest((self.bundle/role).read_bytes())
                    and row['captured_stdout_sha256']==digest(out)
                    and row['captured_stdout_bytes']==len(out) and row['input_written']==len(inp)
                    and row['captured_stderr_bytes']==0 and row['leader_reaped'] and row['spawned']
                    and row['exit_status']==0 and row['stop']=='exited',name+': exact process binding')

    def packet(self):
        packet=Path(__file__).resolve().parents[1]/'tests/fixtures/v3_residual_capacity'
        expected=packet/'expected.tsv'
        require(digest(expected.read_bytes())=='0c1fba741d07a691dcff682349bbcfb2bc1c86392ff38ce56aff7008e5c24fdc','packet expectations changed')
        rows=[s.split('\t') for s in expected.read_text().splitlines() if s and not s.startswith('#')]
        require(len(rows)==64 and sum(r[6]=='PASS' for r in rows)==62,'packet membership')
        comparisons=[]
        for row in rows:
            name,group=row[:2];source=(packet/('cases' if group=='main' else 'probes')/(name+'.ox')).read_bytes()
            require(len(source)==int(row[2]),name+': exact source length')
            path=self.root/(name+'.ox');path.write_bytes(source)
            canonical=self.call(name+'-canonical-check',self.command('check',path),status=0 if row[6]=='PASS' else 1)
            parser=self.call(name+'-parser',[self.bundle/'parser'],source,status=None)
            require(digest(parser.stdout)==row[7],name+': parser difference requires explicit review')
            if row[6]=='REFUSE':
                refused=parser.returncode!=0 or parser.stdout[4]!=0
                if not refused:
                    static=self.call(name+'-static',[self.bundle/'static'],b'AST3'+len(source).to_bytes(2,'little')+source+parser.stdout,status=None)
                    refused=static.returncode==64 and not static.stdout
                require(refused,name+': expected refusal')
                live=self.call(name+'-live-check',self.command('check',path,self.bundle),status=1)
                require((live.stdout,live.stderr)==(canonical.stdout,canonical.stderr),name+': canonical refusal')
                output=self.root/'program.elf'
                canonical_compile=self.call(name+'-canonical-compile',self.command('compile',path,output=output),status=1)
                for route in ['live','import']:
                    output.write_bytes(b'preserve previous executable')
                    result=self.call(name+'-'+route+'-preserve',self.command('compile',path,self.bundle if route=='live' else None,self.root/(name+'-parser.stdout') if route=='import' else None,output),status=1)
                    require(output.read_bytes()==b'preserve previous executable',name+': refused compile changed output')
                    require((result.stdout,result.stderr)==(canonical_compile.stdout,canonical_compile.stderr),name+': refused compile canonical mismatch')
                comparisons.append(dict(case=name,expect='REFUSE',accepted=True));continue
            require(parser.returncode==0 and not parser.stderr and len(parser.stdout)==1304 and parser.stdout[:5]==b'OPA3\0',name+': parser success')
            opa=parser.stdout;ast=b'AST3'+len(source).to_bytes(2,'little')+source+opa
            static=self.call(name+'-static',[self.bundle/'static'],ast)
            wire=static.stdout;require(not static.stderr and len(wire)==2356 and wire[:1304]==opa and wire[1304:1309]==b'STF3\0',name+': static success')
            if row[8]!='-':require(digest(wire)==row[8],name+': strict/advisory byte difference requires explicit review')
            observation=self.root/(name+'-static.stdout')
            # A successful check invokes the real host's complete canonical AST,
            # resolution/fact/type, lowering and OIR comparison, including for
            # previously incomplete static observations. Framing alone cannot pass.
            for op in ['check','run']:
                ref=canonical if op=='check' else self.call(name+'-canonical-run',self.command(op,path),status=None)
                require(ref.returncode in [0,1],name+': canonical runtime status')
                for route in ['live','import']:
                    result=self.call(name+'-'+route+'-'+op,self.command(op,path,self.bundle if route=='live' else None,observation if route=='import' else None),status=ref.returncode)
                    actual=result.stdout
                    if route=='live':
                        self.bindings(name,result,source,opa,wire)
                        actual=b'\n'.join(line for line in actual.splitlines() if json.loads(line).get('kind')!='hir-producer')+b'\n'
                    require((actual,result.stderr)==(ref.stdout,ref.stderr),name+': '+route+' '+op+' canonical mismatch')
            elfs={route:self.root/(name+'.'+route+'.elf') for route in ['canonical','live','import']}
            for route,elf in elfs.items():
                result=self.call(name+'-'+route+'-compile',self.command('compile',path,self.bundle if route=='live' else None,observation if route=='import' else None,elf))
                if route=='live':self.bindings(name,result,source,opa,wire)
            require(elfs['live'].read_bytes()==elfs['import'].read_bytes(),name+': live/import ELF mismatch')
            native=self.call(name+'-canonical-native',[elfs['canonical']],status=None)
            for route in ['live','import']:
                result=self.call(name+'-'+route+'-native',[elfs[route]],status=native.returncode)
                require((result.stdout,result.stderr)==(native.stdout,native.stderr),name+': native mismatch')
            if name=='MIN_locals_k19_n11':
                # Raw-valid endpoint and active fact mutations must not become
                # success merely because their complete frame retains tag zero.
                span=bytearray(wire);at=14+2*129
                start=span[at]+256*span[at+129];end=span[14+4*129]+256*span[14+5*129]
                require(start<end,'mutation endpoint witness');start+=1;span[at]=start&255;span[at+129]=start>>8
                fact=bytearray(wire);fact[1324]^=1
                for kind,data in [('canonical-span',span),('canonical-fact',fact)]:
                    p=self.root/(name+'-'+kind+'.wire');p.write_bytes(data);self.rejected(name+'-'+kind,path,imported=p)
            comparisons.append(dict(case=name,expect='PASS',canonical_checked_import=True,static_sha256=digest(wire),reference_strength=row[9]))
        require(digest(self.compiler.read_bytes())==self.compiler_hash,'compiler changed')
        (self.root/'packet-summary.json').write_text(json.dumps(dict(status='finite-controls-passed',cases=64,valid=62,refused=2,controls=len(self.receipts),compiler_sha256=self.compiler_hash,harness_sha256=digest(Path(__file__).read_bytes()),expected_sha256=digest(expected.read_bytes()),manifest_sha256=digest((self.bundle/'manifest.txt').read_bytes()),receipts_sha256=digest((self.root/'experimental-receipts.json').read_bytes()),comparisons=comparisons),indent=2)+'\n')
        print(len(self.receipts),'packet production controls passed')


if __name__=='__main__':
    p=argparse.ArgumentParser(description=__doc__)
    for key in ['compiler','llvm-bin','bundle','old-bundle','output']:p.add_argument('--'+key,type=Path,required=True)
    a=p.parse_args();PacketRun(a.compiler,a.llvm_bin,a.bundle,a.output,a.old_bundle).packet()
