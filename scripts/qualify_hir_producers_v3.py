#!/usr/bin/env python3
"""Isolated V3 production-path qualification with real source-built ELFs.

Not a release proof or CI gate. Keep every command and stream for review.
"""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import time


def digest(data):
    return hashlib.sha256(data).hexdigest()


def require(value, message):
    if not value:
        raise AssertionError(message)


class Run:
    def __init__(self, compiler, llvm_bin, bundle, output, old_bundle):
        self.compiler = compiler.resolve(strict=True)
        self.compiler_hash = digest(self.compiler.read_bytes())
        self.bundle = bundle.resolve(strict=True)
        self.old_bundle = old_bundle.resolve(strict=True)
        self.root = output.resolve()
        self.root.mkdir()
        self.env = dict(os.environ, OXID_LLVM_BIN=str(llvm_bin.resolve(strict=True)))
        self.receipts = []

    def call(self, name, argv, data=None, status=0):
        start = time.monotonic()
        command = list(map(str, argv))
        result = subprocess.run(command, input=data, capture_output=True, env=self.env, timeout=30)
        for stream in ['stdout', 'stderr']:
            (self.root / (name+'.'+stream)).write_bytes(getattr(result, stream))
        self.receipts.append(dict(name=name, argv=command, status=result.returncode,
                                  elapsed_seconds=time.monotonic()-start,
                                  stdin_sha256=digest(data) if data is not None else None,
                                  stdout_sha256=digest(result.stdout), stderr_sha256=digest(result.stderr)))
        (self.root/'experimental-receipts.json').write_text(json.dumps(self.receipts, indent=2)+'\n')
        require(status is None or result.returncode == status, f'{name}: expected {status}, got {result.returncode}: {result.stdout!r} {result.stderr!r}')
        return result

    def command(self, op, source, bundle=None, imported=None, output=None):
        argv = [self.compiler, op, source, '--edition=typed-preview', '--message-format=json']
        if bundle is not None:
            argv += ['--experimental-hir-producers', bundle]
        if imported is not None:
            argv += ['--experimental-hir-import', imported]
        if op == 'compile':
            argv += ['--backend=llvm', '--output', output or self.root/'program.elf']
        return argv

    def rejected(self, name, source, bundle=None, imported=None):
        output = self.root/'program.elf'
        output.write_bytes(b'preserve previous executable')
        result = self.call(name, self.command('compile', source, bundle, imported), status=1)
        require(output.read_bytes() == b'preserve previous executable', name+': failure changed output')
        require(b'"success":false' in result.stdout and any(code in result.stdout for code in [b'"code":"E0702"', b'"code":"E0703"']),
                name+': missing producer/import rejection (output-exists is not evidence)')
        return result

    def helper(self, name, parser_bytes, static_bytes, mode='normal'):
        bundle = self.root/name
        bundle.mkdir()
        for role, data in [('parser', parser_bytes), ('static', static_bytes)]:
            source = bundle/(role+'.c')
            source.write_text('#include <unistd.h>\nint main(void){unsigned char b; while(read(0,&b,1)>0){}\n'+
                              ('sleep(8);' if mode == 'timeout' and role == 'parser' else '')+
                              ('write(2,"!",1);' if mode == 'stderr' and role == 'parser' else '')+
                              'unsigned char out[]={'+','.join(map(str,data))+'};\n'+
                              ('for(unsigned i=0;i<sizeof out;i++)write(1,out+i,1);' if mode == 'fragmented' else
                               'write(1,out,sizeof out);')+'return 0;}\n')
            self.call(name+'-'+role+'-build', ['cc', '-O0', source, '-o', bundle/role])
        self.manifest(bundle)
        return bundle

    def manifest(self, bundle, version=3):
        (bundle/'manifest.txt').write_text(f'OXID-HIR-PRODUCERS-{version}\n'+''.join(
            f'{role} {digest((bundle/role).read_bytes())}\n' for role in ['parser','static']))

    def program(self, name, source):
        path = self.root/(name+'.ox')
        path.write_bytes(source)
        opa = self.call(name+'-parser', [self.bundle/'parser'], source).stdout
        require(len(opa)==1304 and opa[:4]==b'OPA3' and int.from_bytes(opa[12:14],'little')==len(source), name+': OPA3 framing')
        ast = b'AST3'+len(source).to_bytes(2,'little')+source+opa
        wire = self.call(name+'-static', [self.bundle/'static'], ast).stdout
        require(len(wire)==2356 and wire[:1304]==opa and wire[1304:1309]==b'STF3\0', name+': STF3 framing')
        observation = self.root/(name+'-static.stdout')
        for op in ['check', 'run', 'compile']:
            result = self.call(name+'-'+op, self.command(op, path, self.bundle, output=self.root/(name+'.live.elf')))
            records = [row for row in map(json.loads,result.stdout.splitlines()) if row.get('kind')=='hir-producer']
            require(len(records)==2 and not result.stderr, name+': process evidence')
            for row, role, inp, out in zip(records, ['parser','static'], [source,ast], [opa,wire]):
                require(row['role']==role and row['input_sha256']==digest(inp)
                        and row['executable_sha256']==digest((self.bundle/role).read_bytes())
                        and row['captured_stdout_sha256']==digest(out)
                        and row['captured_stdout_bytes']==len(out) and row['input_written']==len(inp)
                        and row['captured_stderr_bytes']==0 and row['leader_reaped'] and row['spawned']
                        and row['exit_status']==0 and row['stop']=='exited', name+': exact execution binding')
        native = self.call(name+'-native', [self.root/(name+'.live.elf')])
        require(native.stdout==b'0\n', name+': native result')
        native_bytes=(self.root/(name+'.live.elf')).read_bytes()
        self.call(name+'-import', self.command('compile', path, imported=observation, output=self.root/(name+'.import.elf')))
        require((self.root/(name+'.import.elf')).read_bytes()==native_bytes, name+': import identity')
        return path, opa, ast, wire

    def retained_capacity(self):
        corpus=Path(__file__).resolve().parents[1]/'tests/fixtures/v3_retained_capacity'
        for name, binding in json.loads((corpus/'manifest.json').read_text()).items():
            source=(corpus/(name+'.ox')).read_bytes()
            require(digest(source)==binding['source_sha256'], name+': corpus binding')
            name='retained-'+name
            path=self.root/(name+'.ox');path.write_bytes(source)
            opa=self.call(name+'-parser',[self.bundle/'parser'],source).stdout
            require(len(opa)==1304 and opa[:4]==b'OPA3', name+': parser frame')
            ast=b'AST3'+len(source).to_bytes(2,'little')+source+opa
            wire=self.call(name+'-static',[self.bundle/'static'],ast).stdout
            require(len(wire)==2356 and wire[:1304]==opa and wire[1304:1309]==b'STF3\0', name+': static frame')
            for op in ['check','run']:
                canonical=self.call(name+'-canonical-'+op,self.command(op,path),status=None if op=='run' else 0)
                require(canonical.returncode in [0,1], name+': unexpected canonical status')
                live=self.call(name+'-live-'+op,self.command(op,path,self.bundle),status=canonical.returncode)
                filtered=b'\n'.join(line for line in live.stdout.splitlines() if json.loads(line).get('kind')!='hir-producer')+b'\n'
                require(filtered==canonical.stdout and not live.stderr, name+': canonical '+op)
            canonical_elf=self.root/(name+'.canonical.elf')
            live_elf=self.root/(name+'.live.elf')
            import_elf=self.root/(name+'.import.elf')
            self.call(name+'-canonical-compile',self.command('compile',path,output=canonical_elf))
            self.call(name+'-live-compile',self.command('compile',path,self.bundle,output=live_elf))
            self.call(name+'-import-compile',self.command('compile',path,imported=self.root/(name+'-static.stdout'),output=import_elf))
            require(live_elf.read_bytes()==import_elf.read_bytes(), name+': imported native identity')
            canonical=self.call(name+'-canonical-native',[canonical_elf],status=None)
            live=self.call(name+'-live-native',[live_elf],status=canonical.returncode)
            require((live.stdout,live.stderr)==(canonical.stdout,canonical.stderr), name+': native equality')

    def all(self):
        base=b'fn main()->i32{return 0;}'
        for length in [len(base),255,256,257,510,511]:
            path, opa, ast, wire=self.program('length-'+str(length), b' '*(length-len(base))+base)
        loop=b'fn main()->i32{let mut x:i32=0;while x<3{x=x+1;}return x-3;}'
        self.program('loop-511', b' '*(511-len(loop))+loop)
        call=b'fn id(x:i32)->i32{return x;}fn main()->i32{return id(0);}'
        self.program('call-511', b' '*(511-len(call))+call)
        near_rows=b'fn main()->i32{if '+b'!'*46+b'true&&'+b'!'*46+b'true{return 0;}else{return 1;}}'
        near_path, near_opa, _, near_wire=self.program('near-rows-511', b' '*(511-len(near_rows))+near_rows)
        require(near_opa[10]==105, 'near-row fixture changed')
        functions=b''.join(f'fn f{i}()->(){{return;}}'.encode() for i in range(9))+base
        self.program('near-functions-511', b' '*(511-len(functions))+functions)
        for name, raw in [('row-token-overflow', b'fn main()->i32{if '+b'!'*51+b'true&&'+b'!'*50+b'true{return 0;}else{return 1;}}'),
                          ('function-token-overflow',b'fn extra()->(){return;}'+functions)]:
            source=b' '*(511-len(raw))+raw
            badpath=self.root/(name+'.ox');badpath.write_bytes(source)
            failed=self.call(name+'-parser',[self.bundle/'parser'],source,status=64)
            require(not failed.stdout, name+': failed parser output')
            self.rejected(name,badpath,self.bundle)
        # The frozen static closure exhausts its own execution fuel at this
        # deeper113-row input. Preserve this genuine partial-output failure;
        # neither the host nor this harness repairs or enlarges its fuel budget.
        # This is an unresolved acceptance finding, not a lower contract cap.
        raw=b'fn main()->i32{if '+b'!'*50+b'true&&'+b'!'*50+b'true{return 0;}else{return 1;}}'
        source=b' '*(511-len(raw))+raw
        badpath=self.root/'static-fuel-bound.ox';badpath.write_bytes(source)
        self.program('optimized-113-511', source)
        syntax=self.call('static-fuel-bound-parser',[self.old_bundle/'parser'],source).stdout
        failed=self.call('static-fuel-bound-static',[self.old_bundle/'static'],
                         b'AST3'+(511).to_bytes(2,'little')+source+syntax,status=1)
        require(0<len(failed.stdout)<2356 and b'E0601' in failed.stderr,
                'frozen static fuel-bound behavior changed')
        self.rejected('static-fuel-bound-live',badpath,self.old_bundle)
        # Full output is also untrusted when the genuine producer subsequently
        # exhausts fuel. This original frozen source at430 bytes gives a complete
        #2356-byte frame, exit1/E0601; the host must preserve prior output.
        source=b' '*(430-len(raw))+raw
        badpath=self.root/'static-full-output-exit1.ox';badpath.write_bytes(source)
        self.program('optimized-113-430', source)
        syntax=self.call('static-full-output-exit1-parser',[self.old_bundle/'parser'],source).stdout
        failed=self.call('static-full-output-exit1-static',[self.old_bundle/'static'],
                         b'AST3'+(430).to_bytes(2,'little')+source+syntax,status=1)
        require(len(failed.stdout)==2356 and b'E0601' in failed.stderr,
                'frozen full-output exit1 behavior changed')
        self.rejected('static-full-output-exit1-live',badpath,self.old_bundle)

        # Malicious near-cap mutations are untrusted, never generated proofs.
        for name,offset,value in [('inactive-row',14+128,1), ('inflated-row-count',10,128),
                                   ('out-of-range-next',14+129,32)]:
            mutated=bytearray(near_wire);mutated[offset]=value
            observation=self.root/(name+'.wire');observation.write_bytes(mutated)
            self.rejected(name, near_path, imported=observation)
        # Standalone framing failures remain standalone failures; no host repair.
        for name, bad in [('ast2', b'AST2'+ast[4:]), ('short',ast[:-1]), ('trailing',ast+b'!'),
                          ('high-length',b'AST3'+(512).to_bytes(2,'little')+ast[6:])]:
            result=self.call('static-'+name, [self.bundle/'static'], bad, status=64)
            require(not result.stdout, 'static failed output leaked')
        for name, source in [('too-long',b' '*512), ('non-ascii',b'//\xc3\xa9\n'+base)]:
            badpath=self.root/(name+'.ox');badpath.write_bytes(source)
            result=self.rejected(name,badpath,self.bundle)
            require(b'"spawned":true' not in result.stdout, name+': spawned despite source bound')
        for name,bad in [('short',wire[:-1]), ('trailing',wire+b'!'), ('legacy-sized',wire+bytes(251)),
                         ('mixed-opa',b'OPA2'+wire[4:]), ('unknown-version',b'OPA4'+wire[4:]),
                         ('parser-diagnostic',b'OPA3'+bytes([1,1])+bytes(8)), ('mixed-stf',wire[:1307]+b'2'+wire[1308:]),
                         ('high-source',wire[:12]+b'\xff\xff'+wire[14:])]:
            obs=self.root/(name+'.wire');obs.write_bytes(bad)
            self.rejected('import-'+name,path,imported=obs)
        for name,p,t in [('parser-short',opa[:-1],wire), ('parser-trailing',opa+b'!',wire),
                         ('mixed-parser',b'OPA2'+opa[4:],wire), ('parser-used',opa[:12]+b'\xff\xff'+opa[14:],wire),
                         ('static-short',opa,wire[:-1]), ('static-trailing',opa,wire+b'!'),
                         ('mixed-static',opa,wire[:1307]+b'2'+wire[1308:]),
                         ('opa-echo',opa,wire[:14]+bytes([wire[14]^1])+wire[15:]),
                         ('parser-diagnostic',b'OPA3'+bytes([1,1])+bytes(8),wire)]:
            self.rejected(name,path,self.helper(name,p,t))
        for mode in ['stderr','timeout']:
            self.rejected(mode,path,self.helper(mode,opa,wire,mode))
        self.call('fragmented-check',self.command('check',path,self.helper('fragmented',opa,wire,'fragmented')))
        for version in [0,1,2,4]:
            bundle=self.root/('manifest-'+str(version));shutil.copytree(self.bundle,bundle)
            self.manifest(bundle,version)
            self.rejected('manifest-'+str(version),self.root/('length-'+str(len(base))+'.ox'),bundle)
        bundle=self.root/'hash-mismatch';shutil.copytree(self.bundle,bundle)
        with (bundle/'parser').open('ab') as f:f.write(b'changed')
        self.rejected('hash-mismatch',path,bundle)
        bundle=self.root/'symlink';shutil.copytree(self.bundle,bundle)
        (bundle/'parser').unlink();(bundle/'parser').symlink_to(self.bundle/'parser')
        self.rejected('symlink',path,bundle)
        changed=self.root/'changed-source.ox';changed.write_bytes(path.read_bytes().replace(b'return 0',b'return 1'))
        self.rejected('source-binding',changed,self.helper('source-binding',opa,wire))
        # Use the existing broad scalar roster unchanged, padded to put all
        # primary and secondary spans beyond255. Parser diagnostics remain
        # standalone; every successful syntax frame reaches the real live path.
        roster=Path(__file__).resolve().parents[1]/'tests/fixtures/bounded_typed_static/cases.json'
        diagnostic_kinds=set()
        for case in json.loads(roster.read_text())['cases']:
            name='roster-'+case['name']
            raw=case['source'].encode();source=b' '*(511-len(raw))+raw
            p=self.root/(name+'.ox');p.write_bytes(source)
            syntax=self.call(name+'-parser',[self.bundle/'parser'],source).stdout
            if len(syntax)==14:
                require(case['name'] in ['canonical-036','canonical-037','canonical-038','canonical-039'],
                        name+': unexpected syntax diagnostic')
                continue
            require(len(syntax)==1304,name+': incomplete syntax')
            static=self.call(name+'-static',[self.bundle/'static'],b'AST3'+(511).to_bytes(2,'little')+source+syntax).stdout
            require(len(static) in [1324,2356],name+': incomplete static frame')
            status=1 if len(static)==1324 else 0
            if status and static[1310] not in diagnostic_kinds:
                # A live short diagnostic is never admitted as a supplied
                # success observation, even when it agrees with canonical errors.
                self.rejected(name+'-supplied-diagnostic', p, imported=self.root/(name+'-static.stdout'))
                diagnostic_kinds.add(static[1310])
            canonical=self.call(name+'-canonical',self.command('check',p),status=status)
            actual=self.call(name+'-live',self.command('check',p,self.bundle),status=status)
            filtered=b'\n'.join(line for line in actual.stdout.splitlines() if json.loads(line).get('kind')!='hir-producer')+b'\n'
            require(filtered==canonical.stdout, name+': canonical result mismatch')
        require(diagnostic_kinds==set(range(1,15)),f'missing live diagnostic families: {diagnostic_kinds}')
        self.retained_capacity()
        require(digest(self.compiler.read_bytes())==self.compiler_hash, 'compiler changed during qualification')
        (self.root/'experimental-summary.json').write_text(json.dumps(dict(experimental=True,status='finite-controls-passed',
            scope='finite retained controls; no global capacity guarantee',
            old_bundle_manifest_sha256=digest((self.old_bundle/'manifest.txt').read_bytes()),
            compiler_sha256=self.compiler_hash, harness_sha256=digest(Path(__file__).read_bytes()),
            manifest_sha256=digest((self.bundle/'manifest.txt').read_bytes()),
            executable_sha256={role:digest((self.bundle/role).read_bytes()) for role in ['parser','static']},
            receipts_sha256=digest((self.root/'experimental-receipts.json').read_bytes()), receipts=len(self.receipts)),indent=2)+'\n')
        print(f'{len(self.receipts)} V3 production-path command controls completed: {self.root}')



if __name__=='__main__':
    parser=argparse.ArgumentParser(description=__doc__)
    for name in ['compiler','llvm-bin','bundle','output','old-bundle']:
        parser.add_argument('--'+name,type=Path,required=True)
    args=parser.parse_args()
    Run(args.compiler,args.llvm_bin,args.bundle,args.output,args.old_bundle).all()
