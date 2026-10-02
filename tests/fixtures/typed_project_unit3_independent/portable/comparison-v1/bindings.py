"""Explicit approved source/path translations; no expected semantic facts."""
from protocol import *
import tomllib

CORE = '53ed114674e59266c8e9a809f6cba5ef35c9ea819318ab64a7ae6d9aebd20910'
OBSERVER_MANIFEST = 'f0d330def24e2e9832d665f364f0227efa41cbd8fd2f9500af0cf0bde32287e7'
OBSERVER_HELPERS = {'src/frontend/unit3_observer.rs','src/frontend/oir/unit3_raw_scalar.rs','src/frontend/oir/owned/unit3_raw_owned.rs'}
MUTATION_HELPERS = {'src/frontend/unit3_mutations.rs','src/frontend/oir/unit3_mutations_scalar.rs','src/frontend/oir/owned/source/unit3_mutations.rs'}
MUTATION_MANIFESTS = {'v2':'d6bc5a6bc88a51be9d109faaebe9eb5471e3977ea56cfb04cc8397eff219c874',
                      'v3':'6c77a0105d587dccb939f7251c09f99378b63ddeca90ed7dbbdb72e32fabcf83'}
CONTROLLERS = {'v2':{'run.py':'e0553a398c61eebe06e3b7f9a763d0313c2f41528b677b153cd979d7d3345510',
                    'run-effective.py':'77e49d23432d8b1f9f0d99f84c3b39115c569fd159b376a1c4d8fd5e7a2cc1a2'},
               'v3':{'run.py':'33bec550d9ccc15a054937d54ae448c0fb7200b7e9f727d9dda17ac3cc93159d'}}
NATIVE_CONTROLLER = '05c96a02ba8ff2d5069a45e737aeefd37dfabf8d65fd16c9cffd8b5147089306'
WRAPPERS = {
    'source':{'14dfc7142d5ce044afe60caaa85b538c3ae8fdcf21878256f2e1d8a44f7d5fb2'},
    'native':{'fcc7f17dd26de7ee20078db8fd6215bc9a62594eb9a5d731bb1cb8676a907501','49e857b57b0cf589d4fdf667b4f0b689340e3357122135fa0a4e8786c28245dd'},
    'mutation-v2':{'24ddd3bb3b85c1e04a3daf82ac2e1001d71e6e28fa8c8b24f077ba545ee2cac7'},
    'mutation-v3':{'24ddd3bb3b85c1e04a3daf82ac2e1001d71e6e28fa8c8b24f077ba545ee2cac7'},
}
RUSTC = 'f3834d26669b03f6855fa54bd2381443123e860167bc0c7a8dd137e5cf6e5e4f'

def pin(path, digest):
    row = identity(path); verified(row); require(row['sha256'] == digest, 'unapproved input: ' + str(path)); return row
def observer_files(home, prepared):
    adapter = home / relative(prepared['adapter_directory'])
    regular_rows(adapter, prepared['adapter_files'], exact=True)
    pin(adapter / 'run.py', OBSERVER_SHA); pin(adapter / 'parse_debug.py', PARSER_SHA)
    pin(adapter / 'overlay-manifest-v5.json', OBSERVER_MANIFEST)
    return adapter
def source_preparation(home, prepared):
    require(prepared['schema'] == 'oxid-unit3-portable-v1-prepared-source' and prepared['status'] == 'prepared' and 'mutation_variant' not in prepared, 'wrong source preparation')
    assertions(prepared['assertion_mode']); adapter = observer_files(home, prepared)
    core_path = home / relative(prepared['core_manifest']['path']); pin(core_path, CORE)
    require({k:prepared['core_manifest'][k] for k in ('bytes','sha256')} == {k:identity(core_path)[k] for k in ('bytes','sha256')}, 'core identity changed')
    core = read(core_path); required = {x['path'] for x in core['files']} | OBSERVER_HELPERS
    require({x['path'] for x in prepared['files']} == required and len(required) == 120, 'wrong source input membership')
    frozen = {x['path']:x for x in read(adapter / 'overlay-manifest-v5.json')['files']}
    require(all(row == frozen.get(row['path']) for row in prepared['files']), 'source body differs from frozen observer')
    regular_rows(home / relative(prepared['source_directory']), prepared['files'], exact=True)
    return adapter

def portable_build(binding, build, prepared_path):
    require(build['status'] == 'built' and build['exit_code'] == 0 and build['profile'] == binding['profile'] and build['portable_schema'] == 'oxid-unit3-portable-v1-build', 'wrong/failed portable build')
    assertions(build['assertion_mode'])
    require(build['prepared_manifest'] == binding['prepared'] == identity(prepared_path), 'build/preparation association')
    require(build['overlay_manifest'] == build['prepared_manifest'], 'unexpected portable overlay association')
    for name in ('binary','compiler.stdout.jsonl','compiler.stderr'): verified(build[name])
    # Independently join the receipt to actual bound Cargo JSON and exact
    # package, target, selected profile and tool invocation. A receipt label
    # cannot turn a debug executable into a release one.
    keys(build['toolchain'],('rustc','cargo','rustdoc','cc','cxx','ar'),'build toolchain')
    for tool in build['toolchain'].values():verified(tool)
    require(build['toolchain']['rustc']['sha256']==RUSTC,'unqualified Rust binary')
    for name in ('version.stdout','version.stderr'):verified(build[name])
    require(build['version_exit_code']==0 and build['version_argv']==[build['toolchain']['rustc']['path'],'--version','--verbose'],'Rust version invocation')
    require(Path(build['version.stdout']['path']).read_text()==build['rustc'] and build['rustc'].startswith('rustc 1.99.0 '),'actual Rust version output')
    environment=build['environment'];jobs=environment['CARGO_BUILD_JOBS'];require(jobs in ('1','2'),'bounded build jobs')
    require(environment['CARGO_INCREMENTAL']=='0' and environment['PYTHONOPTIMIZE']=='0','build execution controls')
    for key in ('CARGO_ENCODED_RUSTFLAGS','RUSTC_WRAPPER','RUSTC_WORKSPACE_WRAPPER'):require(environment[key]=='','unqualified compiler override')
    for p in ('DEV','TEST','RELEASE'):
        values={'OPT_LEVEL':'3'if p=='RELEASE'else'0','DEBUG':'0','DEBUG_ASSERTIONS':'false'if p=='RELEASE'else'true','OVERFLOW_CHECKS':'false'if p=='RELEASE'else'true'}
        for suffix,value in values.items():require(environment['CARGO_PROFILE_'+p+'_'+suffix]==value,'unqualified compiler profile setting')
    wanted_argv=[build['toolchain']['cargo']['path'],'test','--offline','--no-run','--bin','oxid','--jobs',jobs,'--message-format=json']+(['--release']if binding['profile']=='release'else[])
    require(build['argv']==wanted_argv,'Cargo command/profile differs')
    log=Path(build['compiler.stdout.jsonl']['path']);require(log.stat().st_size<=64*1024*1024,'Cargo JSON size ceiling')
    rows=[strict_json(line)for line in log.read_bytes().splitlines()]
    terminal=[r for r in rows if r.get('reason')=='build-finished']
    require(len(terminal)==1 and terminal[0]=={'reason':'build-finished','success':True} and rows[-1]==terminal[0] and build['cargo_terminal']==terminal[0],'actual Cargo terminal mismatch')
    artifacts=[r for r in rows if r.get('reason')=='compiler-artifact' and r.get('executable') and r.get('profile',{}).get('test')is True]
    require(len(artifacts)==1 and artifacts[0]==build['cargo_artifact'],'actual Cargo artifact mismatch')
    artifact=artifacts[0];source=absolute(build['cwd']);package=tomllib.loads((source/'Cargo.toml').read_text())['package']
    require(artifact['package_id']==f"path+{source.as_uri()}#{package['name']}@{package['version']}",'actual Cargo package/source mismatch')
    target=artifact['target'];require(target['name']=='oxid' and target['kind']==['bin'] and target['crate_types']==['bin'] and target['src_path']==str(source/'src/cli.rs') and target.get('test')is True,'actual Cargo target/source mismatch')
    expected_profile={'opt_level':'0'if binding['profile']=='debug'else'3','debuginfo':0,'debug_assertions':binding['profile']=='debug','overflow_checks':binding['profile']=='debug','test':True}
    require(artifact['profile']==expected_profile and artifact.get('features')==[],'actual Cargo profile/features mismatch')
    binary=Path(build['binary']['path']);require(artifact['executable']==str(binary) and binary.parent.name=='deps' and binary.parent.parent.name==binding['profile'] and not binary.is_relative_to(source) and os.access(binary,os.X_OK),'actual executable location/profile/access')

def mutation_preparation(binding, home, prepared):
    variant = binding['family'].split('-')[-1]
    require(prepared['schema'] == 'oxid-unit3-portable-v1-prepared-source' and prepared['status'] == 'prepared' and prepared['mutation_variant'] == variant, 'wrong mutation preparation')
    assertions(prepared['assertion_mode']); observer_files(home, prepared)
    original_path = verified(prepared['original_mutation_manifest']); original = read(original_path)
    require(original_path == home / 'frozen-inputs' / ('mutation-manifest-' + variant + '.json'), 'original mutation manifest path')
    require(prepared['original_mutation_manifest']['sha256'] == MUTATION_MANIFESTS[variant], 'unapproved original mutation manifest')
    core_path = home / 'frozen-inputs/core-source-manifest.json'; pin(core_path, CORE)
    required = {x['path'] for x in read(core_path)['files']} | OBSERVER_HELPERS | MUTATION_HELPERS
    require(len(required) == len(prepared['files']) == 123 and {x['path'] for x in prepared['files']} == required, 'mutation input membership')
    originals = {x['path']:x for x in original['files']}
    require(all(x == originals.get(x['path']) for x in prepared['files']), 'derived mutation body differs')
    require(prepared['uncompiled_auxiliary_fixture_files_omitted'] == [originals[x] for x in sorted(set(originals)-required)], 'unaccounted original input omissions')
    source = home / ('source-' + variant)
    require(prepared['source_directory'] == source.name, 'mutation source path')
    regular_rows(source, prepared['files'], exact=True)
    regular_rows(home / 'frozen-controllers', prepared['controller_files'], exact=True)
    regular_rows(home / 'frozen-inputs', prepared['input_files'], exact=True)
    require({x['path'] for x in prepared['controller_files']} == set(CONTROLLERS[variant]), 'controller inventory')
    for name, digest in CONTROLLERS[variant].items(): pin(home / 'frozen-controllers' / name, digest)
    derived_path = verified(prepared['derived_mutation_manifest'])
    require(derived_path == home / ('mutation-manifest-' + variant + '.json'), 'derived mutation manifest path')
    expected = {**original, 'source':str(source), 'files':prepared['files'],
                'portable_original_manifest':prepared['original_mutation_manifest'],
                'status':'derived paths and compiler-input subset; unchanged frozen source bytes'}
    require(read(derived_path) == expected, 'unapproved original-to-derived mutation manifest mapping')
    portable = read_bound(binding['portable_build']); portable_build(binding, portable, Path(binding['prepared']['path']))
    require(portable['cwd'] == str(source), 'portable build source mismatch')
    expected_view = dict(schema=1, profile=binding['profile'], status=portable['status'], exit_code=portable['exit_code'],
                         argv=portable['argv'], cwd=portable['cwd'], rustc=portable['rustc'], toolchain_binary=portable['toolchain']['rustc'],
                         overlay_manifest=prepared['derived_mutation_manifest'], environment=portable['environment'],
                         stdout=portable['compiler.stdout.jsonl'], stderr=portable['compiler.stderr'], binary=portable['binary'],
                         portable_build_receipt=binding['portable_build'])
    require(read_bound(binding['build']) == expected_view, 'unapproved mutation build view delta')
    return prepared['derived_mutation_manifest']

def validate_binding(binding, ctx):
    family = binding.get('family'); require(family in ('source','native','mutation-v2','mutation-v3'), 'unknown build family')
    fields = {'family','profile','build','prepared','wrapper'}
    if family == 'native': fields.add('build_wrapper')
    if family.startswith('mutation-'): fields.add('portable_build')
    keys(binding, fields, 'build binding'); require(binding['profile'] in ('debug','release'), 'unknown build profile')
    verified(binding['wrapper']);require(binding['wrapper']['sha256'] in WRAPPERS[family],'unreviewed wrapper for family')
    if binding['wrapper']['sha256']=='49e857b57b0cf589d4fdf667b4f0b689340e3357122135fa0a4e8786c28245dd':
        pin(Path(binding['wrapper']['path']).with_name('native_groups.py'),'cd4a952a3cf72b1abd3f6c146e25fe103ed43eeccc070abf467b56126b7b93bf')
    prepared_path = verified(binding['prepared']); home = prepared_path.parent
    prepared = read(prepared_path); build = read_bound(binding['build'])
    result = dict(binding=binding, prepared=prepared, build=build, home=home)
    if family == 'source':
        source_preparation(home, prepared); portable_build(binding, build, prepared_path)
        require(build['cwd'] == str(home / prepared['source_directory']), 'source build cwd mismatch')
    elif family.startswith('mutation-'):
        result['derived_overlay'] = mutation_preparation(binding, home, prepared)
    else:
        require(prepared['schema'] == 'oxid-unit3-portable-v1-native-prepared' and prepared['status'] == 'prepared-not-executed', 'wrong native preparation')
        assertions(prepared['assertion_mode']); require(prepared['materialization'] == ctx['prepared']['materialization'], 'native uses another materialization')
        require(prepared['shared_source_root'] == str(ctx['sources']), 'native physical source root differs')
        regular_rows(home, prepared['prepared_files'])
        native_root = home / relative(prepared['native_component_directory']); result['native_root'] = native_root
        pin(native_root / 'run.py', NATIVE_CONTROLLER)
        require(build['status'] == 0 and build['profile'] == binding['profile'] and build['cwd'] == str(native_root), 'wrong native build/profile/root')
        require(verified(binding['build']) == native_root / ('build-' + binding['profile']) / 'verified-build.json', 'native build path')
        binary = identity(native_root / ('adapter-' + binding['profile'])); verified(binary)
        require(binary['sha256'] == build['binary_sha256'], 'native binary changed'); result['binary'] = binary
        for name in ('overlay-manifest-v2.json','main.rs','run.py','request-input-manifest.json'):
            require(identity(native_root / name)['sha256'] == build['bindings'][name], 'native build binding changed: ' + name)
        wrapper = read_bound(binding['build_wrapper']); assertions(wrapper['assertion_mode'])
        require(wrapper['status'] == 'completed' and wrapper['command'] == 'build' and wrapper['prepared'] == binding['prepared'] and wrapper['wrapper'] == binding['wrapper'], 'native build wrapper association')
        qualification = read_bound(wrapper['qualified_build_tools'])
        qprofile = qualification['profiles'][binding['profile']]
        require(qprofile['build_receipt'] == binding['build'] and qprofile['binary'] == binary, 'qualified native build mismatch')
        result['build_wrapper'] = wrapper
    return result

def source_receipt(row, receipt, wrapper, admitted, ctx):
    binding = admitted['binding']; request = ctx['requests'][row['id']]
    require(wrapper['schema'] == 'oxid-unit3-portable-v1-observe' and wrapper['status'] == 'observed' and wrapper['exit_code'] == 0, 'source wrapper failure')
    assertions(wrapper['assertion_mode'])
    require((wrapper['case'],wrapper['profile']) == (row['id'],row['profile']), 'source wrapper case/profile')
    require(wrapper['build_receipt'] == binding['build'] and wrapper['prepared_manifest'] == binding['prepared'] and wrapper['materialization'] == ctx['prepared']['materialization'], 'source wrapper input association')
    require(wrapper['observation_receipt'] == identity(row['receipt']), 'source inner receipt association')
    probe = wrapper['selected_assertion_probe']; require(probe['exit_code'] == 0 and probe['stdout'].strip() == '1 0', 'source selected interpreter assertions')
    require((receipt['case'],receipt['profile']) == (row['id'],row['profile']), 'source receipt case/profile')
    require(receipt['source_request'] == request and receipt['source_request_sha256'] == sha(json.dumps(request,sort_keys=True,separators=(',',':')).encode()), 'complete source request differs')
    require(receipt['request_file'] == ctx['material']['requests'], 'source request materialization association')
    verified(receipt['request_file']); require(receipt['build_receipt'] == binding['build'], 'source actual build differs')
    for key, pin_sha, wrapper_key in [('observer_controller',OBSERVER_SHA,'frozen_controller'),('normalizer',PARSER_SHA,'frozen_normalizer')]:
        verified(receipt[key]); require(receipt[key]['sha256'] == pin_sha and wrapper[wrapper_key] == receipt[key], 'source observer/parser differs')
    require(receipt['argv'] == [admitted['build']['binary']['path'],'frontend::unit3_observer::observe_request','--exact','--ignored','--nocapture','--test-threads=1'], 'source binary/entrypoint differs')

def native_receipt(row, receipt, wrapper, admitted, ctx):
    binding = admitted['binding']; require(wrapper['status'] == 'completed' and wrapper['command'] == 'invoke', 'native wrapper failure')
    assertions(wrapper['assertion_mode'])
    require(wrapper['prepared'] == binding['prepared'] and wrapper['wrapper'] == binding['wrapper'] and wrapper['build_wrapper_receipt'] == binding['build_wrapper'], 'native wrapper input association')
    require(wrapper['qualified_build_tools']==admitted['build_wrapper']['qualified_build_tools'],'native invoke/build tool qualification differs')
    verified(wrapper['qualified_build_tools'])
    require(wrapper['actual'] == receipt, 'native wrapper/inner receipt differ')
    require(receipt['binary_sha256'] == admitted['binary']['sha256'] and receipt['verified_build_sha256'] == binding['build']['sha256'], 'native actual build differs')
    require(receipt['overlay_manifest_sha256'] == admitted['build']['bindings']['overlay-manifest-v2.json'], 'native actual overlay differs')
    for name in ('id','profile','group','operation','format','fuel','entry','output','input_request_sha256'):
        require(receipt[name] == row[name], 'native invocation field differs: ' + name)
    require(receipt['fixture_root'] == row['source_root'] and receipt['source_files'] == ctx['requests'][row['id']]['source_files'], 'native source identity differs')
    require((receipt['noclobber_before'] is not None) == (row['noclobber'] is not None), 'native protection mode differs')
    if row['noclobber']:
        require(receipt['noclobber_before'] == receipt['noclobber_after'] and receipt['noclobber_before']['is_symlink'] == (row['noclobber']=='symlink'), 'native output protection differs')
    require(not receipt.get('qualification_failure') and receipt['source_restored'] and receipt['source_restored_after_execution'], 'native source restoration failure')
