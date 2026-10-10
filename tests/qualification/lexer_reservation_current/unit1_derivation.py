"""Exact reversible derivation of the separately named current Unit1 runners.

The frozen original runner/controller bytes and their guards remain authoritative.
No compiler or fixture execution occurs here. Every seam is single-occurrence.
"""

RUNNER_SEAMS = ((b'from rust_strings import rust_string_literal\nPACKAGE=Path(__file__).resolve().parent\n',
  b'import unit1_current_support as current\nrust_string_literal = current.rust_string_literal\nPA'
  b'CKAGE=current.original_package()\n'),
 (b'def main():\n    ap=', b'def main():\n    current.authenticate()\n    ap='),
 (b'    repo=args.repo.resolve()\n',
  b'    repo=args.repo.resolve()\n    current_admission=current.admit(repo)\n'),
 (b"        for name in ['materialize_fixtures.py','materialize_resource_fixtures.py','generate_"
  b"reviewer_tests.py','reviewer_additional.rs','reviewer_v2.rs','scalar_schedule_heldout.rs','c"
  b"heck_resource_inventory.py','run_reviewer.py','origin_oracle.py','expectation-provenance.jso"
  b"n','expected-test-roster.json','rust_strings.py']:\n            shutil.copyfile(PACKAGE/name,"
  b'q/name)\n',
  b'        for name in current.ORIGINAL_NAMES:\n            shutil.copyfile(PACKAGE/name,q/name)'
  b'\n        current.stage(q)\n        current.check_admission(source,current_admission)\n'),
 (b"        try:\n            for name in ['materialize_fixtures.py'",
  b'        summary.update(current.summary_fields(current_admission))\n        try:\n            f'
  b"or name in ['materialize_fixtures.py'"),
 (b"            project=source/'src/frontend/project.rs'\n",
  b"            current.include_control(test)\n            project=source/'src/frontend/project.r"
  b"s'\n"),
 (b"                checked([sys.executable,str(q/'check_resource_inventory.py'),'--output',f'be"
  b"fore-{profile}-inventory.json'],q)\n",
  b'                current.profile_guard(repo,current_admission,source,compiled_sources,q)\n    '
  b"            checked([sys.executable,str(q/'check_resource_inventory.py'),'--output',f'before"
  b"-{profile}-inventory.json'],q)\n"),
 (b"                checked([sys.executable,str(q/'run_reviewer.py'),'--binary',str(binary),'--t"
  b"ag',profile],q)\n",
  b"                checked([sys.executable,str(q/'unit1_run_reviewer_current.py'),'--binary',st"
  b"r(binary),'--tag',profile],q)\n"),
 (b"                assert digest(binary)==binary_sha,'Test executable changed during the run'\n",
  b'                current.check_profile_results(results)\n                assert digest(binary)'
  b"==binary_sha,'Test executable changed during the run'\n"),
 (b"                summary['profiles'][profile]={'passed':69,'total':69,'binary_sha256':binary_"
  b"sha,'elapsed_seconds':time.monotonic()-begin}\n",
  b'                current.profile_guard(repo,current_admission,source,compiled_sources,q)\n    '
  b"            summary['profiles'][profile]={'passed':69,'total':69,'current_controls':{'passed"
  b"':1,'total':1},'binary_sha256':binary_sha,'elapsed_seconds':time.monotonic()-begin}\n"),
 (b"                print(f'{profile}:69/69 passed',flush=True)\n",
  b"                print(f'{profile}:69/69 original cases; 1/1 current control passed',flush=Tr"
  b'ue)\n'),
 (b'            # Check exact supplied code bytes again; no source edit is permitted.\n',
  b'            current.check_admission(repo,current_admission)\n            # Check exact suppli'
  b'ed code bytes again; no source edit is permitted.\n'),
 (b"            summary['status']='passed';summary['test_functions_executed']=sum(x['total'] for"
  b" x in summary['profiles'].values())\n",
  b"            summary['status']='passed';summary['test_functions_executed']=sum(x['total'] for"
  b" x in summary['profiles'].values())\n            summary['current_control_test_functions_exec"
  b"uted']=sum(x['current_controls']['total'] for x in summary['profiles'].values())\n"))

REVIEWER_SEAMS = ((b"if len(expected)!=69 or len(set(expected))!=69:raise SystemExit('Invalid frozen69-test roste"
  b"r')\n",
  b"if len(expected)!=69 or len(set(expected))!=69:raise SystemExit('Invalid frozen69-test roste"
  b"r')\noriginal_expected=list(expected)\ncurrent_controls=['frontend::project::reviewer_unit1::c"
  b"urrent_lexer_storage_failure_full_span_and_eof']\nif set(original_expected)&set(current_contr"
  b"ols):raise SystemExit('Current controls overlap original case identities')\nif hashlib.sha256"
  b"(roster.read_bytes()).hexdigest()!='331bf92273fb3f9904aeff24e279ff546387a211239e0ae7ce1ac20c"
  b"04468f79':raise SystemExit('Changed original Unit1 case roster')\nroster_sha=hashlib.sha256(r"
  b'oster.read_bytes()).hexdigest()\nexpected=original_expected+current_controls\n'),
 (b"if sorted(tests)!=sorted(expected):raise RuntimeError(f'Test roster mismatch: expected69 exa"
  b"ct names, discovered{len(tests)}')",
  b"if sorted(tests)!=sorted(expected):raise RuntimeError(f'Test roster mismatch: expected69 ori"
  b"ginal names plus1 current control, discovered{len(tests)}')"),
 (b"    if hashlib.sha256(b.read_bytes()).hexdigest()!=binary_sha:raise RuntimeError('Test execu"
  b"table changed during execution')\n",
  b"    if hashlib.sha256(b.read_bytes()).hexdigest()!=binary_sha:raise RuntimeError('Test execu"
  b"table changed during execution')\n    if hashlib.sha256(roster.read_bytes()).hexdigest()!=ros"
  b"ter_sha:raise RuntimeError('Frozen roster changed during execution')\n"),
 (b'success=error is None and passed==len(records)==len(expected)==69\n',
  b'success=error is None and passed==len(records)==len(expected)==70\n'),
 (b"(root/(args.tag+'-results.json')).write_text(json.dumps(manifest,indent=2)+'\\n')\n",
  b"original_records=[row for row in records if row['test'] in original_expected]\ncontrol_record"
  b"s=[row for row in records if row['test'] in current_controls]\nmanifest.update({'schema':'oxi"
  b"d-unit1-lexer-current-results-v1','tests':original_records,'passed':sum(row['passed'] for ro"
  b"w in original_records),'total':len(original_records),'expected':69,'current_controls':{'test"
  b"s':control_records,'passed':sum(row['passed'] for row in control_records),'total':len(contro"
  b"l_records),'expected':1},'all_passed':passed,'all_total':len(records),'all_expected':70})\n(r"
  b"oot/(args.tag+'-results.json')).write_text(json.dumps(manifest,indent=2)+'\\n')\n"))

def transform(raw, seams):
    result = raw
    for before, after in seams:
        if before == after or result.count(before) != 1 or after in result:
            raise ValueError("Missing, duplicate, or already adapted Unit1 runner seam")
        result = result.replace(before, after, 1)
    if inverse(result, seams) != raw:
        raise ValueError("Unit1 runner inverse lost original bytes")
    return result


def inverse(derived, seams):
    restored = derived
    for before, after in reversed(seams):
        if restored.count(after) != 1:
            raise ValueError("Missing or duplicate Unit1 runner inverse seam")
        restored = restored.replace(after, before, 1)
    return restored
