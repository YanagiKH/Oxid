#!/usr/bin/env python3
"""Run a frozen libtest roster; exit0 requires one actual passing test per name."""
if not __debug__:
    raise SystemExit('Qualification refuses optimized Python because it can remove verification guards')
import argparse,hashlib,json,re,subprocess,sys,time
from pathlib import Path
root=Path(__file__).resolve().parent
ap=argparse.ArgumentParser();ap.add_argument('--binary',required=True);ap.add_argument('--tag',required=True);args=ap.parse_args()
if not re.fullmatch(r'[a-z][a-z0-9_-]*',args.tag):ap.error('tag must use lowercase letters, digits, underscores or hyphens')
b=Path(args.binary);logroot=root/(args.tag+'-logs');logroot.mkdir(exist_ok=True)
records=[];error=None;tests=[]
roster=root/'expected-test-roster.json'
expected=json.loads(roster.read_text())['tests']
if len(expected)!=69 or len(set(expected))!=69:raise SystemExit('Invalid frozen69-test roster')
binary_sha=hashlib.sha256(b.read_bytes()).hexdigest()
try:
    try:
        listing=subprocess.run([str(b),'frontend::project::reviewer_unit1::','--list','--color','never'],text=True,capture_output=True,timeout=25)
    except subprocess.TimeoutExpired as timeout:
        (logroot/'listing.stdout').write_bytes(timeout.stdout or b'')
        (logroot/'listing.stderr').write_bytes(timeout.stderr or b'')
        raise RuntimeError('Test listing timed out after25 seconds')
    (logroot/'listing.stdout').write_text(listing.stdout);(logroot/'listing.stderr').write_text(listing.stderr)
    if listing.returncode:raise RuntimeError(f'Test listing failed: exit{listing.returncode}')
    tests=[x.removesuffix(': test') for x in listing.stdout.splitlines() if x.endswith(': test')]
    if sorted(tests)!=sorted(expected):raise RuntimeError(f'Test roster mismatch: expected69 exact names, discovered{len(tests)}')
    for test in tests:
        start=time.monotonic()
        try:
            r=subprocess.run([str(b),test,'--exact','--nocapture','--test-threads=1','--color','never'],capture_output=True,timeout=25)
            code=r.returncode;out=r.stdout;err=r.stderr
        except subprocess.TimeoutExpired as e:
            code='timeout';out=e.stdout or b'';err=e.stderr or b''
        name=test.rsplit('::',1)[-1]
        (logroot/(name+'.stdout')).write_bytes(out);(logroot/(name+'.stderr')).write_bytes(err)
        text=out.decode(errors='replace')
        summaries=re.findall(r'^test result: ok\. (\d+) passed; (\d+) failed; (\d+) ignored; (\d+) measured; (\d+) filtered out;',text,re.M)
        one_summary=len(summaries)==1 and tuple(map(int,summaries[0][:4]))==(1,0,0,0)
        named_ok=bool(re.search(r'^test '+re.escape(test)+r' \.\.\. ok$',text,re.M))
        ran_one=bool(re.search(r'^running 1 test$',text,re.M))
        passed=code==0 and one_summary and named_ok and ran_one
        row={'test':test,'exit':code,'passed':passed,'one_named_test_verified':passed,'seconds':time.monotonic()-start,'stdout_sha256':hashlib.sha256(out).hexdigest(),'stderr_sha256':hashlib.sha256(err).hexdigest()};records.append(row)
        print(name,'PASS' if passed else 'FAIL',code,flush=True)
        if not passed:print('Required exactly1 passed/0 failed/0 ignored/0 measured and the named test record\n'+err.decode(errors='replace'),flush=True)
    if hashlib.sha256(b.read_bytes()).hexdigest()!=binary_sha:raise RuntimeError('Test executable changed during execution')
except Exception as exc:error=str(exc)
passed=sum(x['passed'] for x in records)
success=error is None and passed==len(records)==len(expected)==69
manifest={'status':'passed' if success else 'failed','binary':str(b),'binary_sha256':binary_sha,'roster_sha256':hashlib.sha256(roster.read_bytes()).hexdigest(),'tests':records,'passed':passed,'total':len(records),'discovered':len(tests),'expected':len(expected),'error':error}
(root/(args.tag+'-results.json')).write_text(json.dumps(manifest,indent=2)+'\n')
print('RESULT',passed,len(records),flush=True)
if error:print(error,file=sys.stderr)
raise SystemExit(0 if success else 1)
