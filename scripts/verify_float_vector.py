#!/usr/bin/env python3
"""Check owned f64 vectors against independently encoded Python bit patterns."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import random
import resource
import shutil
import struct
import subprocess

ROOT = Path(__file__).resolve().parents[1]
EXAMPLE = Path('examples/typed-float-vector')
LIBRARY = Path('packages/float_vector')
SOURCES = (LIBRARY / 'oxid.toml', LIBRARY / 'src/lib.ox', EXAMPLE / 'oxid.toml',
           EXAMPLE / 'src/main.ox', EXAMPLE / 'tests/vector.ox')


def digest(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def bit_patterns():
    values = [0, 1, 2**52 - 1, 2**52, 0x3ff0000000000000,
              0x7fefffffffffffff, 0x7ff0000000000000, 0x7ff0000000000001,
              0x7ff8000000000001, 0x7fffffffffffffff]
    values += [n | (1 << 63) for n in values]
    rng = random.Random(0xf64ec70)
    values += [rng.getrandbits(64) for _ in range(128)]
    return values


def oracle_program(patterns):
    expected = b''.join(struct.pack('<Q', bits) for bits in patterns)
    bits_source = ', '.join(str(bits) + 'u64' for bits in patterns)
    bytes_source = ', '.join(str(byte) + 'u8' for byte in expected)
    return '''use alloc::bytes::GrowStatus;
use alloc::bytes::new as new_bytes;
use alloc::bytes::try_extend as extend_bytes;
use crate::vector::Vector;
use crate::vector::Read;
use crate::vector::FromBytes;
use crate::vector::new;
use crate::vector::len;
use crate::vector::try_push;
use crate::vector::get;
use crate::vector::into_bytes;
use crate::vector::from_bytes;
fn grown(status: GrowStatus) -> bool {
    match status {
        GrowStatus::Ok => { return true; },
        GrowStatus::CapacityOverflow => { return false; },
        GrowStatus::AllocationFailed => { return false; },
        GrowStatus::MemoryLimit => { return false; },
    }
}
fn same_bytes(input: &[u8], expected: &[u8]) -> bool {
    if input.len() != expected.len() { return false; }
    let mut index = 0;
    while index < input.len() {
        if input[index] != expected[index] { return false; }
        index = index + 1;
    }
    return true;
}
fn same_values(vector: &Vector, expected: &[u64]) -> bool {
    if len(&*vector) != expected.len() { return false; }
    let mut index = 0;
    while index < expected.len() {
        let found = get(&*vector, index);
        match found {
            Read::Value(value) => { if value.to_bits() != expected[index] { return false; } },
            Read::OutOfBounds => { return false; },
        }
        index = index + 1;
    }
    return true;
}
fn main() -> i32 {
    let patterns = [''' + bits_source + '''];
    let expected_bytes = [''' + bytes_source + '''];
    let mut vector = new();
    let mut index = 0;
    while index < patterns.len() {
        let bits = patterns[index];
        let value = f64::from_bits(bits);
        if !grown(try_push(&mut vector, value)) { return 1; }
        index = index + 1;
    }
    if !same_values(&vector, &patterns) { return 2; }
    let encoded = into_bytes(vector);
    if !same_bytes(&encoded, &expected_bytes) { return 3; }
    let mut external = new_bytes();
    if !grown(extend_bytes(&mut external, &expected_bytes)) { return 4; }
    let decoded = from_bytes(external);
    match decoded {
        FromBytes::Ok(restored) => { if !same_values(&restored, &patterns) { return 5; } },
        FromBytes::InvalidLength(rejected) => { return 6; },
    }
    return 0;
}
'''


def native_limits():
    resource.setrlimit(resource.RLIMIT_AS, (32 * 1024 * 1024,) * 2)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('compilers', nargs='+', type=Path)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--llvm-bin', type=Path)
    parser.add_argument('--reference-only', action='store_true')
    args = parser.parse_args()
    if not args.reference_only and not args.llvm_bin:
        raise ValueError('native verification requires --llvm-bin')
    output = args.output.resolve();output.mkdir(parents=True, exist_ok=False)
    compilers = [p.resolve(strict=True) for p in args.compilers]
    pins = [{'path':str(p),'bytes':p.stat().st_size,'sha256':digest(p)} for p in compilers]
    source_pins = {str(p):digest(ROOT/p) for p in SOURCES}
    patterns = bit_patterns();cases = [('mean',(ROOT/EXAMPLE/'src/main.ox').read_text(),b'4\n'),
                                     ('contracts',(ROOT/EXAMPLE/'tests/vector.ox').read_text(),b'0\n')]
    cases += [('bits-'+str(i//8),oracle_program(patterns[i:i+8]),b'0\n')
              for i in range(0,len(patterns),8)]
    receipt = {'status':'running','compilers':pins,'sources':source_pins,'controller_sha256':digest(__file__),
               'commands':0,'reference_cases':0,'native_cases':0,'bit_patterns':len(patterns),
               'reference_only':args.reference_only,'native_elfs':[]}
    env = dict(os.environ,OXID_CACHE_DIR=str(output/'cache'));env.pop('OXID_PATH',None)
    if args.llvm_bin:env['OXID_LLVM_BIN']=str(args.llvm_bin.resolve(strict=True))
    artifacts = {};all_elfs=[]
    with (output/'commands.jsonl').open('x') as transcript:
        def invoke(argv,cwd,kind,expected=None,native=False,expected_stderr=b''):
            result=subprocess.run([str(a) for a in argv],cwd=cwd,env={} if native else env,
                stdin=subprocess.DEVNULL,stdout=subprocess.PIPE,stderr=subprocess.PIPE,
                preexec_fn=native_limits if native else None,timeout=90)
            transcript.write(json.dumps({'argv':[str(a) for a in argv],'cwd':str(cwd),'kind':kind,
                'environment':{} if native else 'isolated compiler cache','status':result.returncode,
                'stdout_hex':result.stdout.hex(),'stderr_hex':result.stderr.hex()},sort_keys=True)+'\n');transcript.flush()
            receipt['commands']+=1
            if result.returncode or result.stderr!=expected_stderr or (expected is not None and result.stdout!=expected):
                raise ValueError('unexpected command result: '+kind)
            return result
        try:
            for ci,compiler in enumerate(compilers):
                for policy in ['bounded','hosted']:
                    graph=output/'projects'/f'compiler-{ci}'/policy
                    for source in SOURCES:
                        dst=graph/source;dst.parent.mkdir(parents=True,exist_ok=True);shutil.copy2(ROOT/source,dst)
                    project=graph/EXAMPLE;manifest=project/'oxid.toml';configuration=manifest.read_text()
                    if configuration.count('policy = "hosted"')!=1:raise ValueError('execution policy template changed')
                    manifest.write_text(configuration.replace('policy = "hosted"',f'policy = "{policy}"',1))
                    for name,body,expected in cases:
                        (project/'src/main.ox').write_text(body)
                        invoke([compiler,'check'],project,name+'-check')
                        invoke([compiler,'run'],project,name+'-reference',expected);receipt['reference_cases']+=1
                        if name=='mean':invoke([compiler,'test'],project,'public-contract-tests',b'',
                            expected_stderr=b'test "vector": ok (exit status: 0)\ntests: 1 passed; 0 failed\n')
                        if args.reference_only:continue
                        for profile in ['dev','release']:
                            for cache in ['cold','warm']:
                                built=invoke([compiler,'build','--profile',profile,'--incremental'],project,name+'-'+profile+'-'+cache)
                                if cache=='warm' and b'typed-preview native object cache: hit' not in built.stdout.splitlines():raise ValueError('missing warm object-cache hit')
                            executable=project/'.oxid/bin/float-vector-example'
                            dst=output/'executables'/f'compiler-{ci}'/policy/(name+'-'+profile);dst.parent.mkdir(parents=True,exist_ok=True);shutil.copy2(executable,dst)
                            key=(policy,name,profile);identity=digest(dst)
                            if key in artifacts and artifacts[key]!=identity:raise ValueError('compiler profile changes native bytes')
                            artifacts[key]=identity;receipt['native_elfs'].append({'path':str(dst),'sha256':identity});all_elfs.append((dst,expected))
                    preserved=output/'preserved-sources'/f'compiler-{ci}'/policy;preserved.parent.mkdir(parents=True,exist_ok=True);shutil.move(graph,preserved)
                    if graph.exists():raise ValueError('source graph remains at its build path')
            if not args.reference_only:
                for name,_,_ in cases:
                    for profile in ['dev','release']:
                        if artifacts[('bounded',name,profile)]==artifacts[('hosted',name,profile)]:raise ValueError('execution policy did not change native code')
                for executable,expected in all_elfs:
                    invoke([executable],output,'source-free-native',expected,True);receipt['native_cases']+=1
            if {str(p):digest(ROOT/p) for p in SOURCES}!=source_pins:raise ValueError('library or example changed during verification')
            for pin in pins:
                if digest(pin['path'])!=pin['sha256']:raise ValueError('compiler changed during verification')
            receipt['status']='passed'
        except BaseException as error:
            receipt.update(status='failed',failure=f'{type(error).__name__}: {error}');raise
        finally:
            receipt['commands_sha256']=digest(output/'commands.jsonl');(output/'result.json').write_text(json.dumps(receipt,indent=2)+'\n')
    print(json.dumps(receipt,sort_keys=True))


if __name__=='__main__':main()
