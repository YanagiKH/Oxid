#!/usr/bin/env python3
"""Verify float text/JSON, exact bit round-trips and isolated native output."""
import linux_native_target as native_target
import argparse
import hashlib
import json
import os
from pathlib import Path
import random
import re
import shutil
import struct
import subprocess
import sys

import verify_floats as model


def sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def values(ty):
    width = int(ty[1:])
    rng = random.Random(0xF074 + width)
    return sorted(set(model.boundary_values(ty)) | {rng.getrandbits(width) for _ in range(256)})


def verify_text(output, bits, ty):
    if output.returncode != 0 or output.stderr:
        raise ValueError("float output command failed")
    text = output.stdout.decode('ascii')
    if not re.fullmatch(r'(?:-?(?:[0-9]+(?:\.[0-9]+)?|inf)|NaN)\n', text):
        raise ValueError("float output is not canonical decimal text")
    width = int(ty[1:])
    kind, sign, _ = model.classify(bits, width)
    if kind == 'nan':
        if text != 'NaN\n':
            raise ValueError("wrong NaN presentation")
    elif kind == 'inf':
        if text != ('-inf\n' if sign else 'inf\n'):
            raise ValueError("wrong infinity presentation")
    else:
        encoded = struct.pack('<f' if width == 32 else '<d', float(text))
        if int.from_bytes(encoded, 'little') != bits:
            raise ValueError("decimal result does not round-trip to the exact float bits")
    return text


def source(ty):
    text, _, _ = model.source(ty, 'identity')
    parser = text[:text.index('fn apply(')]
    conversion = '.to_u32_checked()' if ty == 'f32' else ''
    return parser + f'fn main()->{ty}{{let input=number(1);let bits=input{conversion};return {ty}::from_bits(bits);}}\n'


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('compilers', nargs='+', type=Path)
    parser.add_argument('--output', required=True, type=Path)
    parser.add_argument('--llvm-bin', required=True, type=Path)
    args = parser.parse_args()
    if native_target.host() is None:
        raise ValueError('requires Linux x86_64 or ARM64')
    root = args.output.resolve()
    root.mkdir(parents=True, exist_ok=False)
    compilers = [p.resolve(strict=True) for p in args.compilers]
    pins = [{'path': str(p), 'bytes': p.stat().st_size, 'sha256': sha(p)} for p in compilers]
    receipt = {'status':'running','controller_sha256':sha(__file__), 'compilers':pins,
               'model_sha256':sha(model.__file__), 'commands':0, 'reference_cases':0,
               'native_cases':0, 'broken_pipe_cases':0, 'native_elfs':[]}
    environment = dict(os.environ, OXID_LLVM_BIN=str(args.llvm_bin.resolve(strict=True)),
                       OXID_CACHE_DIR=str(root/'cache'))
    environment.pop('OXID_PATH',None)
    reference = {}
    elfs = {}
    with (root/'commands.jsonl').open('x') as transcript:
        def invoke(argv, cwd, metadata, native=False, closed=False):
            descriptor = None
            if closed:
                reader, descriptor = os.pipe()
                os.close(reader)
            try:
                output = subprocess.run([str(x) for x in argv], cwd=cwd, env={} if native else environment,
                    stdin=subprocess.DEVNULL, stdout=descriptor if closed else subprocess.PIPE,
                    stderr=subprocess.PIPE, timeout=90 if metadata['kind']=='build' else 15,
                    preexec_fn=model.predecessor.limited_native if native else None)
            finally:
                if descriptor is not None:
                    os.close(descriptor)
            transcript.write(json.dumps({**metadata,'argv':[str(x) for x in argv], 'cwd':str(cwd),
                'environment':{} if native else 'isolated compiler cache; OXID_PATH removed',
                'memory_limit_bytes':32*1024*1024 if native else None, 'status':output.returncode,
                'stdout_hex':(output.stdout or b'').hex(), 'stderr_hex':output.stderr.hex()},sort_keys=True)+'\n')
            transcript.flush();receipt['commands'] += 1
            return output
        try:
            for index, compiler in enumerate(compilers):
                for policy in ('bounded','hosted'):
                    for ty in ('f32','f64'):
                        name=f'compiler-{index}/{policy}/{ty}'
                        project=root/'projects'/name
                        (project/'src').mkdir(parents=True)
                        (project/'src/main.ox').write_text(source(ty))
                        (project/'oxid.toml').write_text('[project]\nname="value"\nedition="typed-preview"\nentry="src/main.ox"\n[build]\nentry-mode="result"\n'+f'[execution]\npolicy="{policy}"\n')
                        proof=root/'sources'/name;proof.mkdir(parents=True)
                        shutil.copy2(project/'src/main.ox',proof/'main.ox');shutil.copy2(project/'oxid.toml',proof/'oxid.toml')
                        meta={'compiler':index,'policy':policy,'type':ty}
                        for bits in values(ty):
                            output=invoke([compiler,'run','--',bits],project,{**meta,'kind':'reference','bits':str(bits)})
                            text=verify_text(output,bits,ty)
                            key=(ty,bits)
                            if key in reference and reference[key] != text:
                                raise ValueError('compiler profiles or execution policies disagree on float text')
                            reference[key]=text;receipt['reference_cases'] += 1
                        for bits in model.boundary_values(ty):
                            output=invoke([compiler,'run','--message-format=json','--',bits],project,{**meta,'kind':'json','bits':str(bits)})
                            if output.returncode != 0 or output.stderr:
                                raise ValueError('JSON float command failed')
                            value=json.loads(output.stdout)
                            if value.get('success') is not True or value.get('result') != {'type':ty,'value':reference[(ty,bits)].rstrip('\n')}:
                                raise ValueError('JSON float result differs from exact text/type')
                        deployments=[]
                        for profile in ('dev','release'):
                            for repeat in range(2):
                                output=invoke([compiler,'build','--profile',profile,'--reproducible','--incremental'],project,{**meta,'kind':'build','profile':profile,'repeat':repeat})
                                if output.returncode != 0 or output.stderr:
                                    raise ValueError('native float build failed')
                            deployment=root/'deployment'/name/profile;deployment.mkdir(parents=True)
                            binary=deployment/'value';shutil.copy2(project/'.oxid/bin/value',binary)
                            if binary.read_bytes()[:4] != b'\x7fELF':
                                raise ValueError('output is not an ELF')
                            key=(policy,ty,profile)
                            if key in elfs and elfs[key] != sha(binary):
                                raise ValueError('reproducible float ELF differs across compiler profiles or project locations')
                            elfs[key]=sha(binary)
                            pin={**meta,'profile':profile,'path':str(binary.relative_to(root)),'bytes':binary.stat().st_size,'sha256':sha(binary)}
                            receipt['native_elfs'].append(pin);deployments.append((deployment,binary,pin))
                        shutil.rmtree(project)
                        for deployment,binary,pin in deployments:
                            if list(deployment.iterdir()) != [binary]:
                                raise ValueError('deployment includes source or build inputs')
                            for bits in values(ty):
                                output=invoke([binary,bits],deployment,{**meta,'kind':'native','profile':pin['profile'],'bits':str(bits)},native=True)
                                if verify_text(output,bits,ty) != reference[(ty,bits)]:
                                    raise ValueError('native/reference float presentation differs')
                                receipt['native_cases'] += 1
                            output=invoke([binary,0],deployment,{**meta,'kind':'closed-output','profile':pin['profile']},native=True,closed=True)
                            if output.returncode != 74 or output.stderr:
                                raise ValueError('closed stdout did not produce status 74 without diagnostics')
                            receipt['broken_pipe_cases'] += 1
                            if sha(binary) != pin['sha256']:
                                raise ValueError('native ELF changed')
                        print(json.dumps({'completed':name,'commands':receipt['commands']}),flush=True)
            if pins != [{'path':str(p),'bytes':p.stat().st_size,'sha256':sha(p)} for p in compilers] or sha(__file__) != receipt['controller_sha256'] or sha(model.__file__) != receipt['model_sha256']:
                raise ValueError('compiler or controller changed during verification')
            receipt['status']='passed'
        except BaseException as error:
            receipt.update(status='failed',failure=f'{type(error).__name__}: {error}')
            raise
        finally:
            receipt['commands_sha256']=sha(root/'commands.jsonl')
            (root/'result.json').write_text(json.dumps(receipt,indent=2,sort_keys=True)+'\n')
    print(json.dumps({k:v for k,v in receipt.items() if k!='native_elfs'},sort_keys=True))


if __name__=='__main__':
    main()
