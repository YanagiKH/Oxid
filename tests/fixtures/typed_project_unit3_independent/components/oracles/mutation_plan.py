#!/usr/bin/env python3
"""Finite seam mutations, kept apart from source-only observer requests."""
from pathlib import Path
import json

HERE=Path(__file__).resolve().parent

def generate():
    rows=[]
    def add(id,base,action,authority,**expected):
        rows.append(dict(id=id,positive_control=base,mutation=action,expected=dict(authority=authority,**expected)))
    scalar='control-span-variants-scalar';owned='control-span-variants-owned'
    shared='owned-root_child-absolute-root-shared_nested';staging='owned-root_child-absolute-root-owned_staging'
    # One wrong-but-valid-file mutant for each stored span class in the complete
    # inventory. The selector takes the first occurrence in structural order.
    # It must fail explicitly if absent, never silently skip.
    scalars=['Function.span','locals.span','places.span','Block.span','BoolMerge.span','BoolMerge.operator_span',
             'BoolMerge.incoming[0].value.span','BoolMerge.incoming[1].value.span','Statement.Assign.span',
             'Statement.Initialize.span','Statement.Initialize.place.span','Statement.Initialize.value.span',
             'Statement.Store.span','Statement.Store.operator_span','Statement.Store.place.span','Statement.Store.value.span',
             'Rvalue.Load.place.span','Rvalue.NotBool.operand.span','Rvalue.NotBool.operator_span','Rvalue.Copy.operand.span',
             'Rvalue.CompareScalar.left.span','Rvalue.CompareScalar.right.span','Rvalue.CompareScalar.operator_span',
             'Rvalue.CheckedI32.left.span','Rvalue.CheckedI32.right.span','Rvalue.CheckedI32.operator_span',
             'Terminator.span','Branch.condition.span','Call.args.span','Return.value.span']
    owneds=['Function.span','locals.span','places.span','owners.span','references.span','calls.span','loans.span','Block.span',
            'BoolMerge.span','BoolMerge.operator_span','BoolMerge.incoming[0].value.span','BoolMerge.incoming[1].value.span',
            'Statement.span','DiagnosticOrigins.primary','DiagnosticOrigins.cause','Construct.fields.value.span',
            'WriteField.value.span','PrepareScalar.value.span','Terminator.span','Branch.condition.span','ReturnScalar.value.span',
            'RawRecordDecl.span','RawFieldDecl.span']
    for route,classes in [('scalar',scalars),('owned',owneds)]:
        for cls in classes:
            base=scalar if route=='scalar' else shared if cls in ('references.span','loans.span') else owned
            add('bind-wrong-file-'+route+'-'+cls,base,dict(kind='replace-span',category=route+'.'+cls,occurrence=0,
                replacement='another loaded file, valid UTF-8 range with same offsets where possible; otherwise its checked first identifier'),
                'association-audit-rejection',code='E0500',stage='oir-project-bind',raw_verifier='no rejection required solely for a valid foreign file')
    # Owned Scalar nesting is distinct from outer OwnedStatement.span.
    for cls in scalars[8:26]:
        add('bind-owned-nested-'+cls,owned,dict(kind='replace-span',category='scalar.'+cls,occurrence=0,replacement='valid range in different loaded file'),
            'association-audit-rejection',code='E0500',stage='oir-project-bind')
    for id,action in [
        ('invalid_file',dict(kind='replace-span-file',value='number_of_files')),
        ('reversed_range',dict(kind='replace-span-range',start=2,end=1)),
        ('past_eof',dict(kind='replace-span-range',end='file_length+1')),
        ('utf8_continuation',dict(kind='replace-span-range',start='first multibyte scalar start + 1')),
        ('function_id',dict(kind='set-function-id',index=0,value='function_count')),
        ('function_count',dict(kind='append-duplicate-function',index=0)),
        ('record_id',dict(kind='set-record-id',index=0,value=1)),
        ('field_id',dict(kind='set-field-id',record=0,field=0,value=[0,1])),
        ('field_count',dict(kind='append-duplicate-field',record=0,field=0)),
        ('entry_id',dict(kind='source-seal-seam-entry',value=0)),
        ('missing_span_visit',dict(kind='audit-visitor-suppress-one-visit',category='owned.Construct.fields.value.span')),
        ('duplicate_span_visit',dict(kind='audit-visitor-repeat-one-visit',category='owned.Construct.fields.value.span')),
        ('count_total',dict(kind='audit-count-seam',delta=1)),
        ('checked_count_overflow',dict(kind='audit-count-seam-overflow',value='usize::MAX+1')),
    ]:
        base='control-owned-entry-id1' if id=='entry_id' else 'origin-unicode-lf-owned_overflow' if id=='utf8_continuation' else owned
        add('bind-'+id,base,action,'association-audit-rejection',code='E0500',stage='oir-project-bind',origin='proven-safe origin or null; never the mutated unchecked origin')
    for id in ('same_bytes_new_map','same_path_new_map','same_offsets_other_map','stale_parser_generation'):
        add('source-association-'+id,'pilot-original-batch',dict(kind='original-source-constructor-substitution',substitution=id),
            'source-association-rejection',code='E0500',stage='resolve-project',primary=None,secondary=[],render_from_unchecked_map=False)
    add('source-nonzero-original-file-positive','control-original-scalar-pilot',dict(kind='original-source-map-file-id',value=1),
        'source-association-acceptance',result=10,entry=1,all_original_origins_file=1)
    for id,base,action in [
        ('invalid_target',scalar,dict(kind='call-target',value='function_count')),
        ('wrong_arity',scalar,dict(kind='remove-last-call-argument')),
        ('wrong_scalar_result',scalar,dict(kind='call-result-slot-type',value='opposite scalar type')),
        ('wrong_parameter_kind',shared,dict(kind='call-argument-mode',argument=0,value='Exclusive')),
        ('wrong_nominal_field','positive-nominal_reference',dict(kind='projection-field-record',record=1)),
        ('staging_owner_class',staging,dict(kind='owner-kind',select='first StagedArgument',value='Temporary')),
        ('return_transfer',staging,dict(kind='ReturnOwned-source',select='callee helper',value='already moved parameter owner of same record')),
        ('loan_parent',shared,dict(kind='nested-call-parent',value='self')),
        ('loan_acquisition_missing',shared,dict(kind='remove-outer-PrepareBorrow',retain='CallDecl Borrow argument and LoanDecl')),
        ('cleanup_continue_live','owned-root_child-absolute-root-loop_cleanup',dict(kind='remove-required-StorageEnd',edge='continue')),
        ('cleanup_duplicate_end','control-cleanup_return_join',dict(kind='duplicate-StorageEnd',edge='return')),
    ]:add('raw-'+id,base,action,'raw-verifier-rejection',verified_witness=False,classification='internal E0500; never invented user ownership permission')
    for id,base,edge in [
        ('return','control-cleanup_return_join','return'),
        ('break','owned-root_child-absolute-root-loop_cleanup','break'),
        ('join',owned,'if join')]:
        add('correspondence-missing-cleanup-'+id,base,dict(kind='remove-StorageEnd',edge=edge,
            select='lexical owner never accessed or StorageLive again on that exit path'),
            'semantic-correspondence-rejection',raw_verifier='accept',requirement='source lexical cleanup is mandatory even when raw memory safety permits retaining storage until return')
    for route,base in [('scalar','scalar-root_child-absolute-entry0-i32'),('owned',shared)]:
        add('correspondence-wrong-target-'+route,base,dict(kind='call-target',select='call to helper in left',replacement='same-signature helper original in right'),
            'semantic-correspondence-rejection',raw_verifier='accept',source_association_audit='accept',expected_target='left original',
            result_after_mutation=93 if route=='scalar' else 91)
    add('correspondence-reverse-cleanup', 'owned-root_child-absolute-root-loop_cleanup',
        dict(kind='swap-two-independent-StorageEnd',edge='continue',preserve='both owner lifetimes ended before edge'),
        'semantic-correspondence-rejection',raw_verifier='accept',requirement='reverse lexical cleanup order including moved owners')
    for key in ('primary','cause'):
        add('metadata-same-file-'+key,'negative-exclusive_then_read',dict(kind='replace-diagnostic-origin',field=key,replacement='other valid same-file span'),
            'semantic-correspondence-rejection',generic_raw_denial_kind='unchanged',source_denial_code='E0311',authority_not_changed=True)
    add('metadata-wrong-file','negative-exclusive_then_read',dict(kind='replace-diagnostic-origin',field='cause',replacement='valid child-file span'),
        'association-audit-rejection',code='E0500',stage='oir-project-bind',generic_raw_denial_kind='unchanged')
    for id,base,action in [
        ('snapshot_after_load','pilot-mechanical-batch','rename fixture root after real load, before reference/native IR construction'),
        ('source_free_native','pilot-opaque-batch','copy only ELF to fresh cwd; rename source root; execute with recorded minimal environment; restore source root'),
        ('recursion_occupied_output','negative-native_recursion-scalar-false','provide existing output file and spy LLVM tools'),
        ('recursion_symlink_output','negative-native_recursion-owned-false','provide existing output symlink and spy LLVM tools')]:
        add('driver-'+id,base,dict(kind='driver-boundary',action=action),'driver-consumer-boundary',
            expected='no source reopen; same result' if id in ('snapshot_after_load','source_free_native') else 'native E0700 before output collision; no clobber; zero LLVM invocations')
    assert len({x['id'] for x in rows})==len(rows)
    (HERE/'mutation-expectations.json').write_text(json.dumps(dict(schema='unit3-finite-mutations-v1',count=len(rows),mutations=rows),indent=2)+'\n')
    requests=[{k:v for k,v in row.items() if k!='expected'} for row in rows]
    (HERE/'mutation-requests.json').write_text(json.dumps(dict(schema='unit3-finite-mutation-requests-v1',count=len(rows),mutations=requests),indent=2)+'\n')
    print('mutation count',len(rows))

if __name__=='__main__':generate()
