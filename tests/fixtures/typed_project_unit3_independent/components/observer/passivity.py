#!/usr/bin/env python3
"""Compare actual instrumented results with the existing uninstrumented control.

Reads source-only hand smokes and actual observation artifacts; no oracle input.
"""
import argparse,gzip,hashlib,json,pathlib,subprocess
HERE=pathlib.Path(__file__).resolve().parent
CONTROL=HERE.parent/'typed-project-unit3-implementation-review'
def identity(path):
    data=path.read_bytes();return {'path':str(path),'bytes':len(data),'sha256':hashlib.sha256(data).hexdigest()}
def read(path):return gzip.decompress(path.with_name(path.name+'.gz').read_bytes())if not path.exists()else path.read_bytes()
def main():
    p=argparse.ArgumentParser();p.add_argument('--profile',choices=('debug','release'),required=True);p.add_argument('--requests',type=pathlib.Path,required=True);p.add_argument('--observations',type=pathlib.Path,required=True);p.add_argument('--output',type=pathlib.Path,required=True);a=p.parse_args()
    # Every frontend compiler source is identical to core-v1 except the reviewed
    # append-only outer observer entrypoint. No runtime/audit hooks are present.
    manifest=HERE.parent/'typed-project-unit3-evidence/source-inputs-core-v1.json'
    instrumentation=json.loads((CONTROL/'adapter-instrumentation.json').read_text())
    assert identity(manifest)['sha256']==instrumentation['source_manifest']
    checked=[]
    for item in json.loads(manifest.read_text())['files']:
        if not item['path'].startswith('src/frontend/'):continue
        path=CONTROL/'source'/item['path'];data=path.read_bytes()
        if item['path']=='src/frontend/mod.rs':
            assert hashlib.sha256(data).hexdigest()==instrumentation['frontend_mod_instrumented_sha256']
            assert hashlib.sha256(data[:item['bytes']]).hexdigest()==item['sha256']
        else:assert hashlib.sha256(data).hexdigest()==item['sha256'],item['path']
        checked.append(identity(path))
    binary=CONTROL/('observer-'+a.profile);out=a.output.resolve();assert not out.exists();out.mkdir(parents=True)
    mapping=json.loads(a.observations.read_text());results=[]
    for request in map(json.loads,a.requests.read_text().splitlines()):
        name=request['id'];observed=pathlib.Path(mapping[name]);receipt=json.loads((observed/'receipt.json').read_text());assert receipt['status']=='observed'and receipt['profile']==a.profile
        actual=json.loads(read(observed/'observations.json'));entry=(a.requests.resolve().parent/request['source_root']/request['entry']).resolve()
        dump=out/(name+'.checked.debug');cmd=[str(binary),str(entry),str(dump)]
        result=subprocess.run(cmd,text=True,capture_output=True,timeout=30);assert result.returncode==0
        (out/(name+'.stdout')).write_text(result.stdout);(out/(name+'.stderr')).write_text(result.stderr)
        rows=[json.loads(line)for line in result.stdout.splitlines()]
        row={'case':name,'argv':cmd,'control_exit_code':result.returncode,'instrumented_receipt':identity(observed/'receipt.json')}
        if actual.get('checked_immutable'):
            row['checked_debug_equal']=dump.read_bytes()==read(observed/'checked.debug')
            llvm=dump.with_name(dump.name+'.ll');row['llvm_equal']=llvm.read_bytes()==(observed/'native-default.ll').read_bytes()
            result_rows=[r for r in rows if r.get('event')=='run'];assert len(result_rows)==1
            row['reference_result_equal']={'result':result_rows[0]['result']}==actual['reference']['reference-default']['outcome']
            assert row['checked_debug_equal']and row['llvm_equal']and row['reference_result_equal'],row
            row['control_checked']=identity(dump);row['control_llvm']=identity(llvm)
        else:
            diagnostics=[r for r in rows if r.get('kind')=='diagnostic']
            row['diagnostics_equal']=diagnostics==actual['diagnostics'];assert row['diagnostics_equal']
        results.append(row)
    report={'scope':'three hand-prescribed source controls; no full corpus or actual native execution','profile':a.profile,'core_manifest':identity(manifest),'control_binary':identity(binary),'control_entrypoint':identity(CONTROL/'observer.rs'),'control_instrumentation':identity(CONTROL/'adapter-instrumentation.json'),'verified_frontend_inputs':checked,'results':results}
    (out/'results.json').write_text(json.dumps(report,indent=2)+'\n');print(json.dumps({'profile':a.profile,'cases':len(results),'all_equal':True}))
if __name__=='__main__':main()
