#!/usr/bin/env python3
"""Portable entrance to byte-identical independent source/native/mutation checks."""
import sys
if not sys.dont_write_bytecode:
    raise SystemExit('PROTOCOL_REFUSED: run with python3 -B')
from protocol import *
from bindings import validate_binding, source_receipt, native_receipt
import argparse, contextlib, io, traceback
from types import SimpleNamespace

def native_comparison(rows, actuals, ctx, output_directory):
    """Use the original whole-driver comparator through a byte-identical view.

    Its only path global is the admitted physical fixture prefix. Receipt and
    stream files are copied byte-for-byte; the view retains an explicit map to
    original artifact identities. No ELF, source corpus, or journal is copied.
    """
    root = output_directory / 'native-receipt-view'; root.mkdir()
    mappings = []
    for number, row in enumerate(rows):
        src = Path(row['receipt']).parent; dst = root / str(number); dst.mkdir()
        names = ['receipt.json','invocation.json','stdout','stderr']
        if 'execution' in actuals[number]: names += ['execution/stdout','execution/stderr']
        copied = []
        for name in names:
            source = src / name; target = dst / name
            verified(identity(source)); target.parent.mkdir(parents=True, exist_ok=True); shutil.copyfile(source, target)
            require(source.read_bytes() == target.read_bytes(), 'native comparison copy differs')
            copied.append(dict(original=identity(source), view=identity(target)))
        mappings.append(dict(row=number, files=copied))
    write(output_directory / 'native-receipt-view.json', mappings)
    native = importlib.import_module('compare_native')
    original_root = native.FIXTURE_ROOT; original_argv = sys.argv[:]
    native.FIXTURE_ROOT = ctx['sources'] / 'sources'
    destination = output_directory / 'native-frozen-comparison.json'
    captured = io.StringIO()
    try:
        sys.argv = [native.__file__, str(root), '--output', str(destination)]
        with contextlib.redirect_stdout(captured): native.main()
    finally:
        native.FIXTURE_ROOT = original_root; sys.argv = original_argv
    result = read(destination)
    require(result['receipts'] == len(rows), 'frozen native comparison membership differs')
    return dict(report=identity(destination), view=identity(output_directory / 'native-receipt-view.json'),
                receipts=result['receipts'], actual_elf_runs=result['actual_elf_runs'], status='MATCH')

def mutation_receipt(row, envelope, wrapper, admitted, ctx):
    binding = admitted['binding']
    require(wrapper['schema'] == 'oxid-unit3-portable-v1-mutation' and wrapper['status'] == 'observed' and wrapper['exit_status'] == 0, 'mutation wrapper failure')
    assertions(wrapper['assertion_mode'])
    require((wrapper['mutation_id'],wrapper['profile']) == (row['id'],row['profile']), 'mutation wrapper case/profile')
    for actual_key, expected in [('prepared_manifest',binding['prepared']),('materialization',ctx['prepared']['materialization']),
                                 ('portable_build',binding['portable_build']),('build_view',binding['build']),('path_wrapper',binding['wrapper'])]:
        require(wrapper[actual_key] == expected, 'mutation wrapper association: ' + actual_key); verified(expected)
    require(wrapper['observation_receipt'] == identity(row['receipt']), 'mutation inner receipt association')
    require(wrapper['derived_mutation_manifest'] == admitted['derived_overlay'] and wrapper['original_mutation_manifest'] == admitted['prepared']['original_mutation_manifest'], 'mutation original/derived association')
    require(envelope['mutation_id'] == row['id'] and envelope['profile'] == row['profile'], 'mutation envelope identity')
    require(envelope['build_receipt'] == binding['build'] and envelope['compiler_overlay'] == admitted['derived_overlay'], 'mutation envelope build association')
    require(envelope['source_requests'] == ctx['material']['requests'], 'mutation source materialization association')
    require(wrapper['frozen_controller'] == envelope['mutation_controller'], 'mutation controller association')

def check_plan(args, ctx):
    selected = plan(args.plan, args.plan_sha256, ctx)
    admitted = {key:validate_binding(binding, ctx) for key,binding in selected['builds'].items()}
    if selected['kind'] == 'native':
        import native_plan
        roots = {str(v['native_root']) for v in admitted.values()}
        require(len(roots) == 1, 'one explicit native component required per plan')
        native_plan.validate_rows(selected['rows'], ctx['sources'], Path(next(iter(roots))), selected['scope'] == 'full')
    return selected, admitted

def attribute_driver(evidence, digest, ctx, output_directory):
    """Recheck the ten actual receipts for four existing driver boundaries."""
    evidence = Path(evidence).resolve(); require(identity(evidence)['sha256'] == digest, 'native evidence identity differs')
    report = read(evidence)
    require(report['schema'] == SCHEMA+'-comparison-v1' and report['kind'] == 'native' and report['scope'] == 'full' and report['status'] == 'MATCH', 'full successful native comparison required')
    require(report['count'] == report['matched'] == 300 and report['post_comparison_identity_check'] == 'UNCHANGED', 'full native evidence membership')
    require(report['wrapper'] == identity(Path(__file__).resolve()) and report['component_sha256'] == COMPONENT_SHA, 'native evidence comparator generation')
    require(report['prepared']['sha256'] == identity(Path(ctx['prepared']['component']['path']).parent / 'prepared-comparison.json')['sha256'], 'native evidence comparator view differs')
    verified(report['plan']); verified(report['prepared']); assertions(report['assertion_mode'])
    native_plan, admitted = check_plan(SimpleNamespace(plan=report['plan']['path'],plan_sha256=report['plan']['sha256']), ctx)
    require(native_plan['kind'] == 'native' and native_plan['scope'] == 'full', 'native evidence plan differs')
    observed = {r['planned_row']['receipt']:r for r in report['results']}
    require(len(observed) == 300 and all(r['status']=='MATCH' for r in observed.values()), 'native evidence result membership')
    source_requests = {x['id']:x for x in read(ctx['oracle'] / 'mutation-requests.json')['mutations'] if x['mutation']['kind']=='driver-boundary'}
    mapping = {
        'driver-snapshot_after_load':[('pilot-mechanical-batch','native-default','compile','json',None,None),('pilot-mechanical-batch','reference-fuel','run','text',201,None)],
        'driver-source_free_native':[('pilot-opaque-batch','native-default','compile','json',None,None)],
        'driver-recursion_occupied_output':[('negative-native_recursion-scalar-false','no-clobber','compile','json',None,'file')],
        'driver-recursion_symlink_output':[('negative-native_recursion-owned-false','no-clobber','compile','json',None,'symlink')],
    }
    require(set(mapping)==set(source_requests), 'driver boundary request inventory changed')
    selected_rows=[]; selected_actuals=[]; attributions=[]
    for id, selectors in mapping.items():
        for profile in ('debug','release'):
            receipts=[]
            for selector in selectors:
                matches=[r for r in native_plan['rows'] if r['profile']==profile and tuple(r[k] for k in ('id','group','operation','format','fuel','noclobber'))==selector]
                require(len(matches)==1, 'missing/duplicate native driver attribution')
                row=matches[0]; actual=read(row['receipt']); wrapper=read(row['wrapper_receipt'])
                previous=observed[row['receipt']]
                require(previous['planned_row']==row and previous['receipt']==identity(row['receipt']) and previous['wrapper']==identity(row['wrapper_receipt']), 'stale driver attribution')
                native_receipt(row,actual,wrapper,admitted[row['build']],ctx)
                state=actual.get('source_state')
                require(isinstance(state,list) and [s['phase'] for s in state]==['after_load_before_check','restored'], 'driver source snapshot transitions')
                require(state[0].get('all_source_paths_absent') is True and set(state[0]['files'])=={str(Path(actual['fixture_root'])/s['path']) for s in actual['source_files']}, 'driver did not hide exact loaded sources')
                if id=='driver-source_free_native':
                    execution=actual['execution']; require(execution['source_unavailable'] and execution['cwd_before_files']==['program'] and execution['environment']=={'PATH':'/usr/bin:/bin','LANG':'C','LC_ALL':'C'}, 'source-free native protocol')
                    elf=Path(row['receipt']).parent/'isolated/program'
                    require(identity(elf)['sha256']==actual['artifact_sha256']==execution['elf_sha256'] and elf.read_bytes()[:4]==b'\x7fELF', 'source-free executable identity')
                if row['noclobber']:
                    require(actual['noclobber_before']==actual['noclobber_after'] and actual['tools']==[] and actual['spy_calls']=='', 'recursion priority/output protection')
                selected_rows.append(row);selected_actuals.append(actual);receipts.append(identity(row['receipt']))
            attributions.append(dict(id=id,profile=profile,original_request=source_requests[id],receipts=receipts))
    require(len(selected_rows)==10 and len(attributions)==8, 'driver attribution cardinality')
    output_directory.mkdir()
    semantics=native_comparison(selected_rows,selected_actuals,ctx,output_directory)
    result=dict(status='MATCH',requests=4,profile_results=8,actual_receipt_attributions=10,additional_candidate_executions=0,
                native_evidence=identity(evidence),semantics=semantics,results=attributions)
    write(output_directory/'driver-attributions.json',result)
    return dict(status='MATCH',report=identity(output_directory/'driver-attributions.json'),requests=4,profile_results=8,additional_candidate_executions=0)

def compare(args):
    ctx = open_prepared(args.prepared)
    selected, admitted = check_plan(args, ctx)
    if selected['kind']=='mutation' and selected['scope']=='full':
        require(args.native_evidence and args.native_evidence_sha256, 'full mutation qualification requires separately completed full native evidence for eight driver attributions')
    out = Path(args.output).resolve(); require(not out.exists(), 'fresh comparison output directory required')
    # Roster and all declared apparatus/source identities have been admitted.
    out.mkdir(parents=True)
    comparison = importlib.import_module('compare')
    mutation = None; expectations = None
    if selected['kind'] == 'mutation':
        mutation = importlib.import_module('compare_mutations')
        expectations = {x['id']:x for x in read(ctx['oracle'] / 'supplements/mutation-applicability-v1/expectations.json')['mutations']
                        if x['expected']['authority'] != 'driver-consumer-boundary'}
    results = []; native_actuals = []
    for row in selected['rows']:
        result = dict(id=row['id'],profile=row['profile'],planned_row=row)
        try:
            receipt_path = verified(identity(absolute(row['receipt'])))
            wrapper_path = verified(identity(absolute(row['wrapper_receipt'])))
            result.update(receipt=identity(receipt_path),wrapper=identity(wrapper_path))
            actual = read(receipt_path); wrapper = read(wrapper_path); binding = admitted[row['build']]
            if selected['kind'] == 'source':
                source_receipt(row, actual, wrapper, binding, ctx)
                observations = comparison.read_actual(receipt_path, actual)
                root = ctx['sources'] / ctx['requests'][row['id']]['source_root']
                result['comparison'] = comparison.compare_case(ctx['cases'][row['id']], observations, root)
            elif selected['kind'] == 'native':
                native_receipt(row, actual, wrapper, binding, ctx)
                # Validate the immutable source-only invocation too, before
                # the legacy comparator's output substitution sees a receipt.
                invocation = read(receipt_path.parent / 'invocation.json')
                for name in ('id','profile','group','operation','format','fuel','entry','output','source_files','input_request_sha256'):
                    require(invocation[name] == actual[name], 'native invocation/receipt differs: ' + name)
                native_actuals.append(actual)
            else:
                mutation_receipt(row, actual, wrapper, binding, ctx)
                # Exactly two hash constants can move. Their derived files
                # have already passed the complete original/subset/path proof.
                prior_pin = mutation.PINS['compiler_overlay']; prior_parser = mutation.PARSER_OVERLAY_SHA
                try:
                    if binding['binding']['family'] == 'mutation-v2': mutation.PINS['compiler_overlay'] = binding['derived_overlay']['sha256']
                    else: mutation.PARSER_OVERLAY_SHA = binding['derived_overlay']['sha256']
                    result['comparison'] = mutation.compare_envelope(receipt_path, ctx['cases'], expectations)
                finally:
                    mutation.PINS['compiler_overlay'] = prior_pin; mutation.PARSER_OVERLAY_SHA = prior_parser
                result['overlay_translation'] = dict(original=binding['prepared']['original_mutation_manifest'],derived=binding['derived_overlay'])
            result['status'] = 'ADMITTED' if selected['kind']=='native' else 'MATCH'
        except Exception as error:
            result.update(status='MISMATCH',error=repr(error),traceback=traceback.format_exc())
        results.append(result)
    supplemental = {}
    if selected['kind'] == 'native' and all(r['status'] == 'ADMITTED' for r in results):
        try:
            supplemental['native'] = native_comparison(selected['rows'], native_actuals, ctx, out)
            for row in results:row['status']='MATCH'
        except Exception as error:
            supplemental['native'] = dict(status='MISMATCH',error=repr(error),traceback=traceback.format_exc())
            for row in results:row['status']='SEMANTICS_NOT_CONFIRMED'
    if selected['kind']=='mutation' and selected['scope']=='full' and all(r['status']=='MATCH' for r in results):
        try: supplemental['driver_attribution']=attribute_driver(args.native_evidence,args.native_evidence_sha256,ctx,out/'driver-boundaries')
        except Exception as error: supplemental['driver_attribution']=dict(status='MISMATCH',error=repr(error),traceback=traceback.format_exc())
    # Recheck material/view/build identities after all read-only comparisons.
    try:
        recheck_plan(args.plan,args.plan_sha256,selected)
        open_prepared(args.prepared)
        for binding in selected['builds'].values(): validate_binding(binding, ctx)
        post = 'UNCHANGED'
    except Exception as error:
        post = repr(error)
    successful = all(r['status'] == 'MATCH' for r in results) and post == 'UNCHANGED' and all(x['status'] == 'MATCH' for x in supplemental.values())
    report = dict(schema=SCHEMA+'-comparison-v1',status='MATCH' if successful else 'MISMATCH',kind=selected['kind'],scope=selected['scope'],
                  plan=identity(Path(args.plan).resolve()),prepared=identity(Path(args.prepared).resolve()),component_sha256=COMPONENT_SHA,
                  wrapper=identity(Path(__file__).resolve()),assertion_mode={'__debug__':__debug__,'optimize':sys.flags.optimize,'PYTHONOPTIMIZE':os.environ.get('PYTHONOPTIMIZE','')},
                  count=len(results),matched=sum(r['status']=='MATCH' for r in results),results=results,supplemental=supplemental,
                  post_comparison_identity_check=post,compiler_invocations=0,
                  claim='Exact source/model semantics via unchanged cleared comparators; bounded membership never counts as full qualification')
    if selected['kind'] == 'mutation':
        report['separate_driver_boundaries'] = supplemental.get('driver_attribution','NOT_REQUESTED_IN_BOUNDED_SCOPE')
        report['claim'] = 'Exact internal mutation application/authority semantics; full scope also requires eight separately rechecked native driver attributions'
    write(out / 'comparison.json', report)
    print(json.dumps({k:report[k] for k in ('status','kind','scope','count','matched')}))
    return 0 if successful else 1

def main():
    parser = argparse.ArgumentParser(); sub = parser.add_subparsers(dest='command', required=True)
    prep = sub.add_parser('prepare')
    for name in ('component-root','component-manifest','materialization','parser','output'): prep.add_argument('--'+name, required=True)
    check = sub.add_parser('compare')
    for name in ('prepared','plan','plan-sha256','output'): check.add_argument('--'+name, required=True)
    check.add_argument('--native-evidence');check.add_argument('--native-evidence-sha256')
    args = parser.parse_args()
    if args.command == 'prepare':
        result = prepare(args.component_root,args.component_manifest,args.materialization,args.parser,args.output)
        print(json.dumps({'status':result['status'],'output':str(Path(args.output).resolve())})); return 0
    return compare(args)

if __name__ == '__main__': raise SystemExit(main())
