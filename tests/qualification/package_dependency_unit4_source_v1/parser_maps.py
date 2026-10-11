"""Pure public parser package-source map transport. Never runtime admission.

The immutable phase authority and correspondence remain historical inputs.
Only separately named source maps/manifests are derived. No Git, compiler,
preparation worker, historical body recovery or runtime is invoked here.
"""
import copy
import hashlib
import json
from pathlib import Path, PurePosixPath
import types

DIRECTORY = 'tests/qualification/unit4_parser_current/'
RUNNER = 'tests/fixtures/typed_project_source_binding_byte_storage_v1/run.py'
HISTORICAL = 'tests/fixtures/typed_project_unit4_parser_portable/frozen/v3/authority.json'
PINS = {
    DIRECTORY+'authority.json': 'd8a02a6e970417b2108fcb8ff5e9eb2b32b068d562a7042d73eabaf504dae35c',
    DIRECTORY+'lexer-reservation-authority-v2.json': '917431fe269d807c76b2f16ae7fef7f4e6c70aa301f9823f65de4fe039b94baf',
    DIRECTORY+'lexer-phase-composition-v1.json': '5cf6b9cc9a6fd7253fd2f04b7107dd5eb631bbb438c5ad94f2b63ecf73deef36',
    DIRECTORY+'lexer_phase.py': '1067d223defa8de2a6b0faa71910d9acc97d635491022216e5d59fd48c62ed0e',
    DIRECTORY+'lexer_phase_controls.py': 'cf8667d190c9b50c2f69b623783a33c8c931f795e359a43980ef2486ad44d8c9',
    DIRECTORY+'test_lexer_phase.py': '858e2809bde1dc52ca4f088c4131ce539ab007f2d7f70214816499e73961ec7c',
    RUNNER: '584e8ee937cb2d3ef62d59d3eb69a79bfb6afce23df86de3df305f81afae7c76',
    HISTORICAL: '02b72b3dcf45c695e5c523d71bb1c83e15c082556cf36029829fefc7a71571b0',
}
OLD_SOURCE = '9432c61fc4f63b760e5f55599aedb24067a206e40e8b44b0911392c00cda7261'
NEW_SOURCE = 'f67d373bf5e2f8e620e17fa3f8fc8234211338738d46d65bbdf5658e0e34eac5'
PATCH = '3b123cfa29136244e62038e885761e68858d2b9b472bc14231d2e7a60a13586d'
CHECKPOINT = dict(head='ce522e393646ce6478848cee31f307cb4b6ad5e8', tree='8c3f4837e1fc89a6272ebee3552ce617ce584131')
FIELDS = ('current_base_files', 'current_derived_files', 'current_control_derived_files')
OUTPUTS = ('parser-source-successor-authority.json', 'parser-candidate-source.json')

class Reject(ValueError):
    pass

class NotReady(RuntimeError):
    pass

def need(ok, message):
    if not ok:
        raise Reject(message)

def sha(raw):
    need(type(raw) is bytes, 'exact bytes required')
    return hashlib.sha256(raw).hexdigest()

def serial(value):
    return (json.dumps(value, sort_keys=True, indent=2, allow_nan=False)+'\n').encode()

def projection(value):
    return json.dumps(value, sort_keys=True, separators=(',', ':'), allow_nan=False).encode()

def decode(raw):
    def pairs(items):
        result = {}
        for key, value in items:
            need(key not in result, 'duplicate JSON key')
            result[key] = value
        return result
    return json.loads(raw, object_pairs_hook=pairs,
        parse_constant=lambda _: (_ for _ in ()).throw(Reject('nonfinite JSON')))

def row(name, raw):
    return dict(path=name, bytes=len(raw), sha256=sha(raw))

def ordered(values):
    return sorted(values, key=lambda r: PurePosixPath(r['path']).parts)

def admit_rows(rows, count):
    need(type(rows) is list and len(rows)==count, 'complete map count')
    need(all(type(r) is dict and set(r)=={'path','bytes','sha256'} for r in rows), 'closed map row')
    expected=sorted(rows,key=lambda r:r['path']) if count==539 else ordered(rows)
    need(count in (539,542) and len({r['path'] for r in rows})==count and rows==expected, 'duplicate/reordered map')
    return {r['path']:r for r in rows}

def replace_rows(rows, changes):
    old=admit_rows(rows,len(rows)); need(set(changes)<=set(old), 'unknown replacement path')
    return [changes.get(r['path'],r) for r in rows]

def retained_inputs(repo):
    repo=Path(repo)
    result={}
    for name in PINS:
        path=repo/name
        need(path.is_file() and not any(p.is_symlink() for p in (path,*path.parents)), 'regular retained source')
        result[name]=path.read_bytes()
    return result

def source_admission(context, inputs):
    """Authenticate actual 376 bodies independently of caller metadata."""
    need(context['status']=='NotReady' and context['execution_qualified'] is False, 'source-only NotReady context required')
    need(context['source_checkpoint']==CHECKPOINT, 'wrong source checkpoint')
    old,new=decode(context['predecessor_manifest']),decode(context['current_manifest'])
    for key,name in (('current','tests/fixtures/package_dependency_source_v1/package-dependency-source-v1.json'),('predecessor','tests/fixtures/typed_project_source_binding/current-source.json')):
        need(context[key+'_manifest_binding']==row(name,context[key+'_manifest']), 'exact source manifest binding path/identity')
    need(sha(context['predecessor_manifest'])==OLD_SOURCE and sha(context['current_manifest'])==NEW_SOURCE, 'source manifest pin')
    need(new['reviewed_source_head']==CHECKPOINT['head'] and new['source_only_tree']==CHECKPOINT['tree'], 'manifest source association')
    before,after=context['predecessor_inputs'],context['current_inputs']
    for bodies,manifest in ((before,old),(after,new)):
        need(type(bodies) is dict and len(bodies)==376, '376 actual source bodies')
        need([row(n,bodies[n]) for n in sorted(bodies)]==manifest['files'], 'actual source map changed/missing/extra')
    need([n for n in sorted(before) if before[n]!=after[n]]==['src/main.rs'], 'exact main-only source change')
    patch=context['transition_patch']; need(len(patch)==752 and sha(patch)==PATCH, 'exact source inverse patch')
    api=types.ModuleType('public_parser_package_inverse'); api.__file__=RUNNER
    exec(compile(inputs[RUNNER], RUNNER, 'exec'),api.__dict__)
    restored,touched=api.apply_inverse_patch(after,patch,PATCH,752,('src/main.rs',))
    need(restored==before and tuple(touched)==('src/main.rs',), 'complete actual main inverse')
    for wrong in (before,restored):
        try:
            api.apply_inverse_patch(wrong,patch,PATCH,752,('src/main.rs',))
        except api.BindingError:
            continue
        raise Reject('wrong-stage/double inverse accepted')
    return old,new

def derive(context, inputs):
    need(set(inputs)==set(PINS), 'closed retained authority/helper inputs')
    for name,digest in PINS.items():
        need(sha(inputs[name])==digest, 'changed retained input: '+name)
    old_source,new_source=source_admission(context,inputs)
    active=decode(inputs[DIRECTORY+'authority.json'])
    previous=decode(inputs[DIRECTORY+'lexer-reservation-authority-v2.json'])
    correspondence=decode(inputs[DIRECTORY+'lexer-phase-composition-v1.json'])
    phase=active['lexer_phase']
    for binding in (phase['predecessor'],phase['helper'],phase['source_controls'],phase['correspondence'],phase['controls']['driver'],active['historical_authority']):
        need(row(binding['path'],inputs[binding['path']])==binding, 'retained identity association')
    inverse=copy.deepcopy(active); del inverse['lexer_phase']
    for field in (*FIELDS[1:],'u8_policy_controls'):
        inverse[field]=copy.deepcopy(previous[field])
    need(inverse==previous, 'exact complete prephase authority inverse')
    maps={field:admit_rows(active[field],count) for field,count in zip(FIELDS,(539,542,542))}
    need(all(maps[FIELDS[0]].get(r['path'])==r for r in old_source['files']), 'old376 complete base association')
    historical=decode(inputs[HISTORICAL])
    candidate=dict(schema='oxid-unit4-current-candidate-source-manifest-v1', historical_commit=historical['base_commit'], reviewed_source_head=active['reviewed_source_head'],source_only_tree=active['source_only_tree'],current_source_manifest_sha256=OLD_SOURCE,files=active[FIELDS[0]])
    old_candidate=serial(candidate)
    need(sha(old_candidate)==active['current_candidate_source_manifest_sha256'], 'exact old candidate manifest reconstruction')
    old_candidate_row=row('candidate-source-manifest.json',old_candidate)
    need(all(maps[f]['candidate-source-manifest.json']==old_candidate_row for f in FIELDS[1:]), 'old manifest role association')
    main=row('src/main.rs',context['current_inputs']['src/main.rs'])
    new_base=replace_rows(active[FIELDS[0]],{'src/main.rs':main})
    new_candidate=serial(dict(candidate,reviewed_source_head=CHECKPOINT['head'],source_only_tree=CHECKPOINT['tree'],current_source_manifest_sha256=NEW_SOURCE,files=new_base))
    changes={'src/main.rs':main,'candidate-source-manifest.json':row('candidate-source-manifest.json',new_candidate)}
    derived={FIELDS[0]:new_base,**{f:replace_rows(active[f],changes) for f in FIELDS[1:]}}
    proofs=[]
    for role,field,record in zip(('observed','not_observed'),FIELDS[1:],correspondence['roles']):
        prior=previous[field]; admit_rows(prior,542)
        need(record['role']==role and record['predecessor_map_sha256']==sha(projection(prior)) and record['derived_map_sha256']==sha(projection(active[field])), 'historical complete-map digest association')
        phase_changes={m['path']:m['after'] for m in record['members']}
        need(not set(changes)&set(phase_changes), 'source/instrumentation overlap')
        prior_map={r['path']:r for r in prior}
        need(all(prior_map[m['path']]==m['before'] for m in record['members']), 'phase before row association')
        need(replace_rows(prior,phase_changes)==active[field], 'exact original phase correspondence')
        transported_prior=replace_rows(prior,changes)
        need(replace_rows(transported_prior,phase_changes)==derived[field], 'phase/source exact commutation')
        restored=replace_rows(derived[field],{n:maps[field][n] for n in changes})
        need(restored==active[field], 'complete source map inverse')
        need(replace_rows(derived[field],{m['path']:m['before'] for m in record['members']})==transported_prior, 'complete new phase inverse')
        proofs.append(dict(role=role,retained_phase_members=record['members'],new_prephase_map_sha256=sha(projection(transported_prior)),new_phase_map_sha256=sha(projection(derived[field])),source_then_phase_equals_phase_then_source=True,complete_source_inverse=True,complete_phase_inverse=True))
    need(replace_rows(new_base,{'src/main.rs':maps[FIELDS[0]]['src/main.rs']})==active[FIELDS[0]], 'base exact inverse')
    need(serial(dict(decode(new_candidate),reviewed_source_head=candidate['reviewed_source_head'],source_only_tree=candidate['source_only_tree'],current_source_manifest_sha256=OLD_SOURCE,files=active[FIELDS[0]]))==old_candidate, 'candidate complete inverse')
    for f in FIELDS:
        expected={'src/main.rs'} if f==FIELDS[0] else set(changes)
        need({r['path'] for r in derived[f] if r!=maps[f][r['path']]}==expected, 'exact allowed map changes')
    authority=dict(schema='oxid-unit4-public-parser-package-source-successor-v1',status='NotReady',execution_qualified=False,physical_preparation_performed=False,source_checkpoint=CHECKPOINT,current_source_manifest=context['current_manifest_binding'],predecessor_source_manifest=context['predecessor_manifest_binding'],retained_inputs=[row(n,inputs[n]) for n in sorted(inputs)],retained_phase_correspondence=correspondence,retained_phase_semantics=phase,new_candidate_manifest=row('parser-candidate-source.json',new_candidate),map_candidate_manifest=changes['candidate-source-manifest.json'],predecessor_candidate_manifest=old_candidate_row,maps=derived,map_sha256={f:sha(projection(derived[f])) for f in FIELDS},source_changes={f:['src/main.rs'] if f==FIELDS[0] else sorted(changes) for f in FIELDS},commutation=proofs,historical_body_status='not reread by pure derivation; authentic physical preparation remains required')
    return {OUTPUTS[0]:serial(authority),OUTPUTS[1]:new_candidate}

def verify(outputs, context, inputs):
    need(type(outputs) is dict and set(outputs)==set(OUTPUTS), 'closed named outputs')
    need(outputs==derive(context,inputs), 'changed successor authority/manifest bytes')
    return decode(outputs[OUTPUTS[0]])

def execution_context(*args,**kwargs):
    raise NotReady('Source-only public parser successor; no physical or runtime qualification')
