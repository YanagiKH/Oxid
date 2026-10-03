#!/usr/bin/env python3
"""Compare ordinary externally visible outcomes with parser hooks absent/present."""
import argparse,json,os,pathlib,subprocess,uuid
import sys
if sys.flags.optimize or not __debug__:
    raise SystemExit("OPTIMIZED_PYTHON_UNSUPPORTED: parser observer requires enabled admission checks")
from run import ENTRY,identity,read_json,verified
CONTROLS={
    'original':'fn main() -> i32 { return 7; }\n',
    'project':'pub mod child; use crate::answer; pub struct R { pub n: i32 } pub fn main() -> i32 { return crate::answer(); }\n',
    'malformed':'pub fn broken() {}\nuse ;\nmod ;\n',
}
COMPARE=['result','parse_attempts','diagnostics','json_diagnostic_renderings','human_diagnostic_rendering','token_inventory','reserve_attempts','reserve_trace','ast','syntax_flavor']
def main():
    ap=argparse.ArgumentParser();ap.add_argument('--instrumented',type=pathlib.Path,required=True);ap.add_argument('--control',type=pathlib.Path,required=True);ap.add_argument('--checkpoint',type=pathlib.Path,required=True);ap.add_argument('--output',type=pathlib.Path,required=True);a=ap.parse_args();assert a.checkpoint.is_file();out=a.output.resolve();assert not out.exists();out.mkdir(parents=True)
    builds=[read_json(a.instrumented),read_json(a.control)];assert builds[0]['control']is False and builds[1]['control']is True
    assert builds[0]['candidate_source_manifest_sha256']==builds[1]['candidate_source_manifest_sha256']and builds[0]['profile']==builds[1]['profile']
    bins=[verified(b['binary'])for b in builds];results=[]
    for name,text in CONTROLS.items():
        source=out/(name+'.ox');source.write_text(text);observations=[];receipts=[]
        for index,binary in enumerate(bins):
            work=out/(name+('-control'if index else'-instrumented'));work.mkdir();nonce=uuid.uuid4().hex
            env=os.environ.copy();env.update({'UNIT4_SOURCE':str(source),'UNIT4_DISPLAY_PATH':name+'.ox','UNIT4_NODE_LIMIT':'100000','UNIT4_TOKEN_LIMIT':'100000','UNIT4_RESERVE_FAIL_AT':'0','UNIT4_REJECT_KIND':'','UNIT4_REJECT_OCCURRENCE':'0','UNIT4_DIRECT':'0','UNIT4_SEGMENT_START':'0','UNIT4_SEGMENT_END':'0','UNIT4_ORIGINAL':'1','UNIT4_CONTROL':str(index),'UNIT4_NONCE':nonce,'UNIT4_CASE_ID':name,'UNIT4_RAW_OUTPUT':str(work/'raw.json')})
            argv=[str(binary),ENTRY,'--exact','--ignored','--nocapture','--test-threads=1'];r=subprocess.run(argv,env=env,capture_output=True,text=True,timeout=120);(work/'stdout.txt').write_text(r.stdout);(work/'stderr.txt').write_text(r.stderr);assert r.returncode==0 and r.stdout.count('UNIT4_EXECUTED '+nonce)==1 and 'test result: ok. 1 passed; 0 failed; 0 ignored;'in r.stdout
            raw=read_json(work/'raw.json');assert raw['nonce']==nonce and raw['source_utf8']==text;observations.append(raw['observations']);receipts.append({'argv':argv,'exit_code':r.returncode,'raw':identity(work/'raw.json'),'stdout':identity(work/'stdout.txt'),'stderr':identity(work/'stderr.txt')})
        assert len(observations[0])==len(observations[1])==2
        for left,right in zip(*observations):
            assert left['mode']==right['mode']
            for field in COMPARE:assert left[field]==right[field],(name,left['mode'],field)
            if left['result']=='ok':assert left['nodes_admitted']==right['nodes_admitted']
        results.append({'case':name,'source':identity(source),'pairs':2,'equal_fields':COMPARE,'receipts':receipts})
    report={'schema':'oxid-unit4-parser-passivity-v1','status':'pass','scope':'three hand-prescribed ordinary controls in both modes; no seam equivalence or full-corpus assertion','instrumented':identity(a.instrumented),'control':identity(a.control),'authority_checkpoint':identity(a.checkpoint),'results':results};(out/'report.json').write_text(json.dumps(report,sort_keys=True,indent=2)+'\n');print(json.dumps({'status':'pass','pairs':6,'report':str(out/'report.json')}))
if __name__=='__main__':main()
