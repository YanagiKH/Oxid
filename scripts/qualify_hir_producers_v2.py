#!/usr/bin/env python3
"""Targeted actual-source v2 qualification; every run keeps its evidence."""
import argparse
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import time

import build_hir_producers_v2 as builder


def require(ok, message):
    if not ok:
        raise RuntimeError(message)


# This is the existing real recipe, including two actual end=255 mutations.
# Keep its exact names and expected outcomes, not merely a successful count.
EXPECTED_RECEIPTS = []
for _name in ('leading-129', 'leading-254', 'leading-255', 'comment-255', 'loop-255'):
    EXPECTED_RECEIPTS.extend((_name + '-' + stage, 0)
                             for stage in ('parser', 'static', 'check', 'run', 'compile', 'native', 'import'))
    if _name in ('leading-255', 'loop-255'):
        EXPECTED_RECEIPTS.append((_name + '-bad-end', 1))
EXPECTED_RECEIPTS += [
    ('byte256', 64), ('non-ascii', 64), ('tokens129', 64),
    ('empty', 0), ('tokens128', 0), ('read-error', 74),
    ('wrong-outer', 64), ('wrong-inner', 64), ('trailing', 64), ('short', 64),
    ('mixed-static', 1), ('mixed-parser', 1),
    ('v1-build-parser', 0), ('v1-build-static', 0),
    ('v1-check', 0), ('v1-run', 0), ('v1-compile', 0), ('v1-native', 0),
    ('v1-refuses129', 1), ('mixed-bundle-parser', 1), ('mixed-bundle-static', 1),
]


def input_identities(compiler):
    source = {}
    for key, revision in (('source_head', 'HEAD'), ('source_tree', 'HEAD^{tree}')):
        value = subprocess.check_output(['git', 'rev-parse', revision], cwd=builder.ROOT).decode().strip()
        require(re.fullmatch(r'[0-9a-f]{40}', value) is not None, 'invalid source identity')
        source[key] = value
    return dict(source, compiler_sha256=builder.digest(compiler.read_bytes()),
                harness_sha256=builder.digest(Path(__file__).read_bytes()),
                builder_sha256=builder.digest(Path(builder.__file__).read_bytes()),
                source_manifest_sha256=builder.digest(builder.MANIFEST.read_bytes()))


def validate_receipts(root):
    receipts = json.loads((root / 'receipts.json').read_text())
    require(isinstance(receipts, list) and len(receipts) == 58, 'v2 recipe requires exactly 58 receipts')
    for row, (name, status) in zip(receipts, EXPECTED_RECEIPTS):
        require(isinstance(row, dict) and row.get('name') == name, 'v2 receipt recipe/order mismatch')
        require(type(row.get('status')) is int and row['status'] == status, 'v2 receipt status mismatch: ' + name)
        require(isinstance(row.get('argv'), list) and row['argv']
                and all(isinstance(arg, str) for arg in row['argv']), 'invalid v2 receipt command')
        for stream in ('stdout', 'stderr'):
            require(row.get(stream + '_sha256') == builder.digest((root / (name + '.' + stream)).read_bytes()),
                    'v2 receipt stream identity mismatch: ' + name)
    return receipts


def validate_build(root, expected):
    directory = root / 'v2-build'
    build = json.loads((directory / 'build-evidence.json').read_text())
    require(isinstance(build, dict) and type(build.get('schema_version')) is int
            and build['schema_version'] == 2, 'invalid v2 build evidence')
    for key in ('compiler_sha256', 'source_manifest_sha256'):
        require(build.get(key) == expected[key], 'v2 build identity mismatch: ' + key)
    manifest_data = (directory / 'sources/source-manifest.json').read_bytes()
    require(builder.digest(manifest_data) == expected['source_manifest_sha256'], 'materialized source manifest identity mismatch')
    manifest = json.loads(manifest_data)
    require(type(build.get('source_count')) is int and build['source_count'] == len(manifest['sources']),
            'v2 build source count mismatch')
    hashes = {role: builder.digest((directory / 'bundle' / role).read_bytes()) for role in ('parser', 'static')}
    require(build.get('executable_sha256') == hashes, 'v2 producer executable identity mismatch')
    require((directory / 'bundle/manifest.txt').read_text() == 'OXID-HIR-PRODUCERS-2\n' + ''.join(
        role + ' ' + hashes[role] + '\n' for role in ('parser', 'static')), 'v2 bundle manifest identity mismatch')
    return hashes


def validate_summary(root, expected):
    """Re-admit retained evidence against the wrapper's exact-build identities."""
    summary = json.loads((root / 'summary.json').read_text())
    require(isinstance(summary, dict) and type(summary.get('schema_version')) is int
            and summary['schema_version'] == 2 and summary.get('status') == 'passed', 'invalid v2 summary')
    require(type(summary.get('receipts')) is int and summary['receipts'] == 58, 'incomplete v2 summary recipe')
    for key, value in expected.items():
        require(summary.get(key) == value, 'v2 qualification identity mismatch: ' + key)
    require(summary.get('receipts_sha256') == builder.digest((root / 'receipts.json').read_bytes()),
            'v2 receipt file identity mismatch')
    require(summary.get('build_evidence_sha256') == builder.digest((root / 'v2-build/build-evidence.json').read_bytes()),
            'v2 build evidence identity mismatch')
    validate_receipts(root)
    require(summary.get('executable_sha256') == validate_build(root, expected), 'v2 summary producer identity mismatch')
    return summary


def validate_execution(result, source, ast, opa, wire, hashes):
    rows = [json.loads(line) for line in result.stdout.splitlines()]
    require(rows and all(isinstance(row, dict) for row in rows), 'invalid producer JSON evidence')
    records = [row for row in rows if row.get('kind') == 'hir-producer']
    require(rows[-1].get('success') is True and len(records) == 2 and not result.stderr, 'missing producer success')
    for role, record, data, output in zip(('parser', 'static'), records, (source, ast), (opa, wire)):
        require(record.get('role') == role and record.get('executable_sha256') == hashes[role]
                and record.get('input_sha256') == builder.digest(data)
                and record.get('captured_stdout_sha256') == builder.digest(output)
                and type(record.get('input_written')) is int and record['input_written'] == len(data)
                and type(record.get('captured_stdout_bytes')) is int and record['captured_stdout_bytes'] == len(output)
                and type(record.get('captured_stderr_bytes')) is int and record['captured_stderr_bytes'] == 0
                and record.get('spawned') is True and record.get('leader_reaped') is True
                and record.get('stop') == 'exited' and type(record.get('exit_status')) is int
                and record['exit_status'] == 0 and 'signal' in record and record['signal'] is None,
                'execution identity mismatch: ' + role)


class Run:
    def __init__(self, compiler, llvm, output):
        self.compiler, self.llvm = compiler.resolve(strict=True), llvm.resolve(strict=True)
        self.root = output.resolve()
        self.root.mkdir()
        self.env = dict(os.environ, OXID_LLVM_BIN=str(self.llvm))
        self.receipts = []
        self.identities = input_identities(self.compiler)

    def call(self, name, argv, data=None, status=0, stdin=None):
        start = time.monotonic()
        command = [str(x) for x in argv]
        try:
            result = subprocess.run(command, input=data, stdin=stdin, cwd=builder.ROOT,
                                    env=self.env, capture_output=True, timeout=120)
            stdout, stderr, actual = result.stdout, result.stderr, result.returncode
        except subprocess.TimeoutExpired as error:
            stdout, stderr, actual = error.stdout or b'', error.stderr or b'', 'outer-timeout'
        except OSError as error:
            stdout, stderr, actual = b'', str(error).encode(), 'spawn-failed'
        for stream, content in (('stdout', stdout), ('stderr', stderr)):
            (self.root / (name + '.' + stream)).write_bytes(content)
        self.receipts.append(dict(name=name, argv=command, status=actual,
                                 elapsed_seconds=time.monotonic()-start,
                                 stdin_sha256=builder.digest(data) if data is not None else None,
                                 stdout_sha256=builder.digest(stdout), stderr_sha256=builder.digest(stderr)))
        (self.root / 'receipts.json').write_text(json.dumps(self.receipts, indent=2)+'\n')
        require(actual == status, f'{name}: expected {status}, got {actual}: {stdout!r} {stderr!r}')
        return result

    def command(self, op, source, option, value, output):
        argv = [self.compiler, op, source, '--edition=typed-preview', option, value, '--message-format=json']
        if op == 'compile':
            argv += ['--backend=llvm', '--output', output]
        return argv

    def program(self, name, source, value, bundle):
        path = self.root / (name+'.ox')
        path.write_bytes(source)
        opa = self.call(name+'-parser', [bundle/'parser'], source).stdout
        require(len(opa)==1559 and opa[:4]==b'OPA2' and opa[10]==len(source), 'invalid parser frame')
        ast = b'AST2'+bytes([len(source)])+source+opa
        wire = self.call(name+'-static', [bundle/'static'], ast).stdout
        require(len(wire)==2607 and wire[:1559]==opa and wire[1559:1564]==b'STF2\0', 'invalid static frame')
        observation=self.root/(name+'-static.stdout')
        for op in ['check','run','compile']:
            executable=self.root/(name+'.elf')
            result=self.call(name+'-'+op, self.command(op,path,'--experimental-hir-producers',bundle,executable))
            validate_execution(result, source, ast, opa, wire,
                               {role: builder.digest((bundle / role).read_bytes()) for role in ('parser', 'static')})
        self.call(name+'-native',[executable])
        require((self.root/(name+'-native.stdout')).read_bytes()==str(value).encode()+b'\n','wrong native result')
        imported=self.root/(name+'-import.elf')
        self.call(name+'-import',self.command('compile',path,'--experimental-hir-import',observation,imported))
        require(executable.read_bytes()==imported.read_bytes(),'producer/import native identity mismatch')
        # Mutate the actual parser's end=255 boundary while retaining well-formed framing.
        if len(source)==255 and source.endswith(b'}'):
            mutated=bytearray(wire)
            changed=False
            for cell in range(opa[8]):
                header=sum(mutated[11+plane*129+cell]<<(8*plane) for plane in range(4))
                if (header>>14)&255==255:
                    header=(header & ~(255<<14)) | (254<<14)
                    for plane in range(4): mutated[11+plane*129+cell]=(header>>(8*plane))&255
                    changed=True
                    break
            require(changed,'missing actual255 endpoint mutation')
            bad=self.root/(name+'-bad-end.bin');bad.write_bytes(mutated)
            self.call(name+'-bad-end',self.command('check',path,'--experimental-hir-import',bad,imported),status=1)

    def qualify(self):
        bundle=builder.build(self.compiler,self.llvm,self.root/'v2-build')
        code=b'fn main()->i32{return 7;}'
        for length in [129,254,255]:
            self.program('leading-'+str(length), b'\n'*(length-len(code))+code,7,bundle)
        self.program('comment-255',code+b'/*'+b'x'*(255-len(code)-4)+b'*/',7,bundle)
        loop=b'fn main()->i32{let mut n=0;while n<3{n=n+1;}return n;}'
        self.program('loop-255',b' '*(255-len(loop))+loop,3,bundle)
        for name,data in [('byte256',b' '*256),('non-ascii',b'\xff'),('tokens129',b'+'*129)]:
            result=self.call(name,[bundle/'parser'],data,status=64)
            require(not result.stdout and not result.stderr,'refusal emitted partial frame')
        for name,data in [('empty',b''),('tokens128',b'+'*128)]:
            self.call(name,[bundle/'parser'],data)
        descriptor=os.open(self.root,os.O_RDONLY)
        try:
            self.call('read-error',[bundle/'parser'],status=74,stdin=descriptor)
        finally:
            os.close(descriptor)
        source=(self.root/'leading-255.ox').read_bytes()
        opa=(self.root/'leading-255-parser.stdout').read_bytes()
        for name,data in [('wrong-outer',b'AST1'+b'\xff'+source+opa),
                          ('wrong-inner',b'AST2'+b'\xff'+source+b'OPA1'+opa[4:]),
                          ('trailing',b'AST2'+b'\xff'+source+opa+b'X'),
                          ('short',b'AST2'+b'\xff'+source+opa[:-1])]:
            self.call(name,[bundle/'static'],data,status=64)
        for name,index in [('mixed-static',1562),('mixed-parser',3)]:
            wire=bytearray((self.root/'leading-255-static.stdout').read_bytes());wire[index]=ord('1')
            path=self.root/(name+'.bin');path.write_bytes(wire)
            self.call(name,self.command('check',self.root/'leading-255.ox','--experimental-hir-import',path,self.root/'unused'),status=1)
        # Authentic v1 is built from the untouched historical roots on this same compiler.
        v1=self.root/'v1-bundle';v1.mkdir()
        for role,root in [('parser','parser_main'),('static','ast_static_main')]:
            self.call('v1-build-'+role,[self.compiler,'compile',builder.ROOT/'fixtures/typed-lexer-samples'/(root+'.ox'),
                      '--edition=typed-preview','--backend=llvm','--entry-mode=process','--output',v1/role])
        (v1/'manifest.txt').write_text('OXID-HIR-PRODUCERS-1\n'+''.join(role+' '+builder.digest((v1/role).read_bytes())+'\n' for role in ['parser','static']))
        short=self.root/'v1.ox';short.write_bytes(code)
        for op in ['check','run','compile']:
            self.call('v1-'+op,self.command(op,short,'--experimental-hir-producers',v1,self.root/'v1.elf'))
        self.call('v1-native',[self.root/'v1.elf'])
        require((self.root/'v1-native.stdout').read_bytes()==b'7\n','v1 native mismatch')
        self.call('v1-refuses129',self.command('check',self.root/'leading-129.ox','--experimental-hir-producers',v1,self.root/'unused'),status=1)
        # Explicit v2 manifest cannot silently run a v1 parser or static consumer.
        for role in ['parser','static']:
            mixed=self.root/('mixed-bundle-'+role);shutil.copytree(bundle,mixed)
            shutil.copyfile(v1/role,mixed/role)
            (mixed/'manifest.txt').write_text('OXID-HIR-PRODUCERS-2\n'+''.join(r+' '+builder.digest((mixed/r).read_bytes())+'\n' for r in ['parser','static']))
            self.call('mixed-bundle-'+role,self.command('check',short,'--experimental-hir-producers',mixed,self.root/'unused'),status=1)
        self.finish()
        print('Bounded v2 actual-source check/run/compile and v1 compatibility: PASS')

    def finish(self):
        require(input_identities(self.compiler) == self.identities, 'qualification inputs changed during execution')
        validate_receipts(self.root)
        hashes = validate_build(self.root, self.identities)
        summary = dict(self.identities, schema_version=2, status='passed', receipts=len(self.receipts),
                       receipts_sha256=builder.digest((self.root / 'receipts.json').read_bytes()),
                       build_evidence_sha256=builder.digest((self.root / 'v2-build/build-evidence.json').read_bytes()),
                       executable_sha256=hashes,
                       limits='Linux x86_64 local qualification; no default provider, grammar, global budget or self-hosting claim')
        (self.root / 'summary.json').write_text(json.dumps(summary, indent=2)+'\n')
        validate_summary(self.root, self.identities)


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    for name in ['compiler','llvm-bin','output']:parser.add_argument('--'+name,required=True,type=Path)
    args=parser.parse_args()
    Run(args.compiler,args.llvm_bin,args.output).qualify()


if __name__=='__main__':main()
