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
original_expected=list(expected)
current_controls=['frontend::project::reviewer_unit1::current_lexer_storage_failure_full_span_and_eof']
if set(original_expected)&set(current_controls):raise SystemExit('Current controls overlap original case identities')
if hashlib.sha256(roster.read_bytes()).hexdigest()!='331bf92273fb3f9904aeff24e279ff546387a211239e0ae7ce1ac20c04468f79':raise SystemExit('Changed original Unit1 case roster')
roster_sha=hashlib.sha256(roster.read_bytes()).hexdigest()
expected=original_expected+current_controls
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
    if sorted(tests)!=sorted(expected):raise RuntimeError(f'Test roster mismatch: expected69 original names plus1 current control, discovered{len(tests)}')
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
    if hashlib.sha256(roster.read_bytes()).hexdigest()!=roster_sha:raise RuntimeError('Frozen roster changed during execution')
except Exception as exc:error=str(exc)
passed=sum(x['passed'] for x in records)
success=error is None and passed==len(records)==len(expected)==70
manifest={'status':'passed' if success else 'failed','binary':str(b),'binary_sha256':binary_sha,'roster_sha256':hashlib.sha256(roster.read_bytes()).hexdigest(),'tests':records,'passed':passed,'total':len(records),'discovered':len(tests),'expected':len(expected),'error':error}
original_records=[row for row in records if row['test'] in original_expected]
control_records=[row for row in records if row['test'] in current_controls]
manifest.update({'schema':'oxid-unit1-lexer-current-results-v1','tests':original_records,'passed':sum(row['passed'] for row in original_records),'total':len(original_records),'expected':69,'current_controls':{'tests':control_records,'passed':sum(row['passed'] for row in control_records),'total':len(control_records),'expected':1},'all_passed':passed,'all_total':len(records),'all_expected':70})
(root/(args.tag+'-results.json')).write_text(json.dumps(manifest,indent=2)+'\n')
print('RESULT',passed,len(records),flush=True)
if error:print(error,file=sys.stderr)
raise SystemExit(0 if success else 1)
