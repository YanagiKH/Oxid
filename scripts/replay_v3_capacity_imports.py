#!/usr/bin/env python3
"""Replay supplied imports against exact canonical execution receipts."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import subprocess


def digest(data):
    return hashlib.sha256(data).hexdigest()


def replay(compiler, llvm_bin, run, output):
    compiler=compiler.resolve(strict=True);run=run.resolve(strict=True)
    output.mkdir();compiler_hash=digest(compiler.read_bytes())
    summary=json.loads((run/'experimental-summary.json').read_text())
    assert summary['compiler_sha256']==compiler_hash
    receipts={row['name']:row for row in json.loads((run/'experimental-receipts.json').read_text())}
    corpus=Path(__file__).resolve().parents[1]/'tests/fixtures/v3_retained_capacity'
    results=[]
    for case in json.loads((corpus/'manifest.json').read_text()):
        name='retained-'+case
        # Producer success is independent of user-program runtime success.
        for role in ['parser','static']:
            assert receipts[name+'-'+role]['status']==0
            assert not (run/(name+'-'+role+'.stderr')).read_bytes()
        for op in ['check','run','native']:
            reference=receipts[name+'-canonical-'+op]
            argv=([str(run/(name+'.import.elf'))] if op=='native' else
                  [str(compiler),op,str(run/(name+'.ox')),'--edition=typed-preview','--message-format=json',
                   '--experimental-hir-import',str(run/(name+'-static.stdout'))])
            result=subprocess.run(argv,capture_output=True,timeout=30,
                                  env=dict(os.environ,OXID_LLVM_BIN=str(llvm_bin.resolve(strict=True))))
            assert result.returncode==reference['status'],(name,op,result.returncode,reference['status'])
            for stream in ['stdout','stderr']:
                raw=getattr(result,stream);expected=(run/(name+'-canonical-'+op+'.'+stream)).read_bytes()
                (output/(name+'-'+op+'.'+stream)).write_bytes(raw)
                assert raw==expected,(name,op,stream)
            results.append(dict(name=name,operation=op,argv=argv,status=result.returncode,
                                stdout_sha256=digest(result.stdout),stderr_sha256=digest(result.stderr)))
            (output/'receipts.json').write_text(json.dumps(results,indent=2)+'\n')
    assert digest(compiler.read_bytes())==compiler_hash
    (output/'summary.json').write_text(json.dumps(dict(status='finite-controls-passed',
        controls=len(results),compiler_sha256=compiler_hash,harness_sha256=digest(Path(__file__).read_bytes()),
        main_summary_sha256=digest((run/'experimental-summary.json').read_bytes()),
        corpus_manifest_sha256=digest((corpus/'manifest.json').read_bytes()),
        receipts_sha256=digest((output/'receipts.json').read_bytes())),indent=2)+'\n')
    print(len(results),'supplied-import execution controls passed')


if __name__=='__main__':
    p=argparse.ArgumentParser(description=__doc__)
    for arg in ['compiler','llvm-bin','run','output']:p.add_argument('--'+arg,type=Path,required=True)
    a=p.parse_args();replay(a.compiler,a.llvm_bin,a.run,a.output)
