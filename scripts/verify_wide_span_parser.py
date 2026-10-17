#!/usr/bin/env python3
"""Differential implementation checks, not independent corpus qualification."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import shutil
import subprocess
import time

import build_hir_producers_v2 as v2
import build_wide_span_parser as wide
import build_typed_parser_observer as parser_observer
import build_typed_lexer_observer as lexer_observer
import parser_ast_observation as projection


def sha(data):
    return hashlib.sha256(data).hexdigest()


def require(ok, message):
    if not ok:
        raise RuntimeError(message)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--compiler', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--llvm-bin', type=Path)
    args = parser.parse_args()
    compiler, output = args.compiler.resolve(strict=True), args.output.resolve()
    output.mkdir(parents=True, exist_ok=False)
    baseline = output / 'baseline'
    v2.materialize(baseline)
    candidate = output / 'candidate'
    wide.materialize(candidate)
    env = dict(os.environ)
    if args.llvm_bin:
        env['OXID_LLVM_BIN'] = str(args.llvm_bin.resolve(strict=True))
    report = dict(passed=False, scope='Same-domain differential and synthetic wide carrier controls',
                  compiler_sha256=sha(compiler.read_bytes()), harness_sha256=sha(Path(__file__).read_bytes()),
                  source_head=subprocess.check_output(['git','rev-parse','HEAD'],cwd=v2.ROOT).decode().strip(),
                  source_sha256={str(p.relative_to(output)): sha(p.read_bytes()) for p in sorted(output.rglob('*.ox'))},
                  receipts=[], comparisons=[], controls=[])

    def save():
        (output / 'report.json').write_text(json.dumps(report, indent=2) + '\n')

    save()

    def call(name, command, data=b''):
        command = [str(x) for x in command]
        (output / (name + '.stdin')).write_bytes(data)
        start = time.monotonic()
        try:
            result = subprocess.run(command, input=data, capture_output=True, cwd=output, env=env, timeout=120)
        except subprocess.TimeoutExpired as error:
            result = subprocess.CompletedProcess(command, 124, error.stdout or b'', error.stderr or b'')
        row = dict(name=name, argv=command, status=result.returncode,
                   elapsed_seconds=time.monotonic()-start, stdin_sha256=sha(data))
        for stream in ('stdout', 'stderr'):
            raw = getattr(result, stream)
            (output / (name + '.' + stream)).write_bytes(raw)
            row[stream + '_sha256'] = sha(raw)
        report['receipts'].append(row)
        save()
        return result

    def reference(entry):
        return [compiler, 'run', entry, '--edition=typed-preview', '--entry-mode=process']

    commands = {'baseline-reference':reference(baseline/'parser_main.ox'),
                'candidate-reference':reference(candidate/'parser_main.ox')}
    control_commands = {'reference':reference(candidate/'control_main.ox')}
    if args.llvm_bin:
        for name, entry in [('baseline',baseline/'parser_main.ox'), ('candidate',candidate/'parser_main.ox'),
                            ('controls',candidate/'control_main.ox')]:
            binary=output/(name+'.elf')
            result=call('build-'+name,[compiler,'compile',entry,'--edition=typed-preview',
                        '--entry-mode=process','--backend=llvm','--output',binary])
            require(result.returncode==0, name+' native build failed')
            require(binary.read_bytes()[:4]==b'\x7fELF','not an ELF')
            report.setdefault('binary_sha256',{})[name]=sha(binary.read_bytes())
            if name=='controls': control_commands['native']=[binary]
            else: commands[name+'-native']=[binary]
    # Existing independent builders copy canonical Rust sources unchanged.
    observers = {kind: Path(builder.build(output / ('canonical-' + kind))['observer'])
                 for kind, builder in [('parser', parser_observer), ('lexer', lexer_observer)]}
    report['canonical_observers'] = {kind: sha(path.read_bytes()) for kind,path in observers.items()}
    roster=json.loads((v2.ROOT/'tests/fixtures/bounded_typed_parser/cases.json').read_bytes())
    report['roster_sha256'] = sha((v2.ROOT/'tests/fixtures/bounded_typed_parser/cases.json').read_bytes())
    cases=[(r['name'],bytes.fromhex(r['input_hex'])) for r in roster['cases']]
    # Boundary inputs stay within the existing v2 contract, except explicit refusals.
    for n in (254,255,256):
        for label,tail in [('function',b'fn main()->i32{return 0;}'),('diagnostic',b'fn main()->i32{return 0;')]:
            cases.append((label+'-'+str(n),b' '*(n-len(tail))+tail))
    cases += [('empty',b''),('non-ascii',b'\xff'),('token-capacity',b';'*129)]
    for i,(name,data) in enumerate(cases):
        baseline_result=None
        for mode,command in commands.items():
            result=call(f'case-{i:03}-{mode}',command,data)
            value=(result.returncode,result.stdout,result.stderr)
            if baseline_result is None: baseline_result=value
            require(value==baseline_result, name+' differs: '+mode)
        if i < len(roster['cases']):
            facts = {}
            for kind, observer in observers.items():
                observed = call(f'canonical-{i:03}-{kind}', [observer], data)
                require(observed.returncode == 0 and not observed.stderr, 'canonical observer failed')
                facts[kind] = json.loads(observed.stdout)
            case = roster['cases'][i]
            require(facts['parser']['status'] == case['parser_status'], 'canonical status changed')
            expected = facts['parser']
            if case['role'] == 'refusal':
                refusal = case['refusal']
                expected = dict(status='domain_refusal', family=refusal['family'],
                                span=projection.span(refusal['start'], refusal['end']))
            # Same-domain OPA2 has exactly the OPA1 payload: adapt magic only,
            # never widen the independent adapter's 128-byte source contract.
            require(baseline_result[1][:4] == b'OPA2', 'unexpected observation version')
            got = projection.decode(b'OPA1' + baseline_result[1][4:], data, facts['lexer'].get('tokens'))
            require(got == expected, 'full canonical AST/diagnostic mismatch: ' + name)
            (output / f'canonical-{i:03}-projected.json').write_text(json.dumps(got, indent=2)+'\n')
        report['comparisons'].append(dict(name=name, source_sha256=sha(data), modes=list(commands),
                                          status=baseline_result[0],stdout_bytes=len(baseline_result[1])))
        save()
    for mode in range(22):
        status=0 if mode<6 or mode in (15,16) else (64 if mode in (6,7,14,20,21) else 70)
        expected=b''
        if mode==15:expected=b'OPA2'+bytes([1,11,255,255,0,0,255])
        if mode==16:
            headers=[1+64*(255+256*255)+4194304*(i+2 if i<127 else 0) for i in range(128)]+[0]
            columns=[headers,[0]*129,[0]*129]
            expected=b'OPA2'+bytes([0,0,0,0,128,1,255])+b''.join(
                bytes((v>>(8*plane))&255 for v in col) for col in columns for plane in range(4))
        for execution,command in control_commands.items():
            result=call(f'control-{mode:02}-{execution}',command,bytes([mode]))
            require((result.returncode,result.stdout,result.stderr)==(status,expected,b''),
                    f'control {mode} failed in {execution}')
            report['controls'].append(dict(mode=mode, execution=execution, status=status))
            save()
    require(report['source_sha256']=={str(p.relative_to(output)):sha(p.read_bytes()) for p in sorted(output.rglob('*.ox'))},
            'source snapshots changed')
    report['passed']=True
    save()
    print(output/'report.json')


if __name__=='__main__':
    main()
