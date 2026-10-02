#!/usr/bin/env python3
"""Explicit portable paths around immutable reviewed mutation controllers."""
from common import *
from assemble import apply_patch
import argparse, importlib.util, shutil, time

CORE = '53ed114674e59266c8e9a809f6cba5ef35c9ea819318ab64a7ae6d9aebd20910'
OBSERVER_PATCH = '62a2b6382f7684c5aaf03c355580f2e8d582c6c2688d7bab970f0b50f9c0ed54'
OBSERVER_MANIFEST = 'f0d330def24e2e9832d665f364f0227efa41cbd8fd2f9500af0cf0bde32287e7'
RUSTC = 'f3834d26669b03f6855fa54bd2381443123e860167bc0c7a8dd137e5cf6e5e4f'
STALE = 'source-association-stale_parser_generation'
EFFECTIVE = 'raw-wrong_scalar_result'
VARIANTS = {
    'v2': {
        'manifest': 'd6bc5a6bc88a51be9d109faaebe9eb5471e3977ea56cfb04cc8397eff219c874',
        'patch': 'c3a9ddc7d95fce4ea25b07ac9de2fdbdcefb43e052dcaeb1042bcf59e87536f9',
        'controllers': {'run.py': 'e0553a398c61eebe06e3b7f9a763d0313c2f41528b677b153cd979d7d3345510',
                        'run-effective.py': '77e49d23432d8b1f9f0d99f84c3b39115c569fd159b376a1c4d8fd5e7a2cc1a2'},
    },
    'v3': {
        'manifest': '6c77a0105d587dccb939f7251c09f99378b63ddeca90ed7dbbdb72e32fabcf83',
        'patch': '3c50051b6ee73c11643158fddcdb113bea7efb2e16fa0234cf02047dd3e1fa60',
        'controllers': {'run.py': '33bec550d9ccc15a054937d54ae448c0fb7200b7e9f727d9dda17ac3cc93159d'},
    },
}
INPUTS = {
    'oracle_manifest': ('pre-execution-manifest.json', 'b89c8c5b00b13de9573513ba0f96d28fc1046f68c55d32d4ac1aad03d15c24ba'),
    'mutation_requests': ('mutation-requests.json', '228f51304751f2a08d962d95aecbe066fdfa813fdf10b46b144f9cf7ee51846b'),
    'effective_requests': ('effective-requests.json', '628d08d0500d9b98acac658ad3c75ac311272cd493913b55c2e2598a48240bd8'),
    'applicability_manifest': ('applicability-manifest.json', 'a01be26791455c85790abcee37f05373da9a0fe7f77aaae3fbb635378cebb987'),
}
OBSERVER_FILES = {'run.py': 'ff51288e0bf90733eafe4690c02feacac06e61ab0608642423fd700e7fca42fc',
                  'parse_debug.py': '638fb39b814f637bf21902738edb35bbe775956d2c00a0e307957668e38c4264'}
HELPERS = {'src/frontend/unit3_mutations.rs', 'src/frontend/oir/unit3_mutations_scalar.rs',
           'src/frontend/oir/owned/source/unit3_mutations.rs'}
OBSERVER_HELPERS = {'src/frontend/unit3_observer.rs', 'src/frontend/oir/unit3_raw_scalar.rs',
                    'src/frontend/oir/owned/unit3_raw_owned.rs'}


def pinned(path, digest):
    require(identity(path)['sha256'] == digest, 'frozen identity differs: ' + str(path))
    return bound_file(path)


def prepare(a):
    selected = VARIANTS[a.variant]
    parent_path = pathlib.Path(a.observer_prepared).resolve()
    parent = read_json(parent_path)
    require(parent['schema'] == SCHEMA + '-prepared-source' and parent['status'] == 'prepared', 'wrong observer preparation')
    require(parent['core_manifest']['sha256'] == CORE and parent['patch_sha256'] == OBSERVER_PATCH, 'wrong core/observer generation')
    source = parent_path.parent / relative(parent['source_directory'])
    adapter = parent_path.parent / relative(parent['adapter_directory'])
    check_manifest(source, parent['files'], exact=True)
    check_manifest(adapter, parent['adapter_files'], exact=True)
    for name, digest in OBSERVER_FILES.items():
        pinned(adapter / name, digest)
    pinned(adapter / 'overlay-manifest-v5.json', OBSERVER_MANIFEST)
    core_path = parent_path.parent / relative(parent['core_manifest']['path'])
    pinned(core_path, CORE)
    core = read_json(core_path)
    require({r['path'] for r in parent['files']} == {r['path'] for r in core['files']} | OBSERVER_HELPERS, 'observer source membership differs')
    observer_rows = {r['path']: r for r in read_json(adapter / 'overlay-manifest-v5.json')['files']}
    for row in parent['files']:
        require(row == observer_rows.get(row['path']), 'observer source differs from frozen bytes')
    component = pathlib.Path(a.component_root).resolve()
    manifest_name = 'mutation-manifest-' + a.variant + '.json'
    patch_name = 'mutation-overlay-' + a.variant + '.patch'
    original_manifest = pinned(component / manifest_name, selected['manifest'])
    original = read_json(component / manifest_name)
    pinned(component / patch_name, selected['patch'])
    require(original['core_manifest_sha256'] == CORE and original['observer_patch_sha256'] == OBSERVER_PATCH, 'wrong mutation generation')
    for name, digest in selected['controllers'].items():
        pinned(component / name, digest)
    for flag, (_, digest) in INPUTS.items():
        pinned(getattr(a, flag), digest)
    out = fresh(a.output)
    target = out / ('source-' + a.variant)
    shutil.copytree(source, target)
    touched = apply_patch(target, (component / patch_name).read_text())
    actual = files_manifest(target)
    expected = {r['path']: r for r in original['files']}
    require({r['path'] for r in actual} == {r['path'] for r in parent['files']} | HELPERS, 'unexpected mutation source membership')
    for item in actual:
        require(item == expected.get(item['path']), 'mutation source differs from frozen body')
    shutil.copytree(adapter, out / 'frozen-observer')
    controllers = out / 'frozen-controllers'
    controllers.mkdir()
    for name in selected['controllers']:
        shutil.copyfile(component / name, controllers / name)
    inputs = out / 'frozen-inputs'
    inputs.mkdir()
    for flag, (name, _) in INPUTS.items():
        shutil.copyfile(getattr(a, flag), inputs / name)
    shutil.copyfile(component / manifest_name, inputs / manifest_name)
    shutil.copyfile(core_path, inputs / 'core-source-manifest.json')
    shutil.copyfile(component / patch_name, out / patch_name)
    derived = {**original, 'source': str(target), 'files': actual,
               'portable_original_manifest': bound_file(inputs / manifest_name),
               'status': 'derived paths and compiler-input subset; unchanged frozen source bytes'}
    write_json(out / manifest_name, derived)
    result = {'schema': SCHEMA + '-prepared-source', 'status': 'prepared', 'mutation_variant': a.variant,
              'assertion_mode': assertion_mode(), 'observer_prepared': bound_file(parent_path),
              'core_manifest': {'path': 'frozen-inputs/core-source-manifest.json', **identity(inputs / 'core-source-manifest.json')}, 'patch_sha256': selected['patch'],
              'original_mutation_manifest': bound_file(inputs / manifest_name),
              'derived_mutation_manifest': bound_file(out / manifest_name),
              'source_directory': target.name, 'adapter_directory': 'frozen-observer',
              'files': actual, 'adapter_files': files_manifest(out / 'frozen-observer'),
              'controller_files': files_manifest(controllers), 'input_files': files_manifest(inputs),
              'patched_files': touched,
              'uncompiled_auxiliary_fixture_files_omitted': [expected[name] for name in sorted(set(expected) - {r['path'] for r in actual})],
              'controller': bound_file(pathlib.Path(__file__))}
    write_json(out / 'prepared-mutation.json', result)
    return result


class RoutedInputs:
    def __init__(self, paths):
        self.paths = paths

    def __truediv__(self, name):
        require(name in self.paths, 'unreviewed input route: ' + str(name))
        return self.paths[name]


def checked_preparation(path):
    prepared = read_json(path)
    require(prepared['schema'] == SCHEMA + '-prepared-source' and prepared['status'] == 'prepared', 'wrong preparation')
    variant = prepared['mutation_variant']
    home = pathlib.Path(path).resolve().parent
    selected = VARIANTS[variant]
    for folder, rows in [(prepared['source_directory'], prepared['files']), ('frozen-observer', prepared['adapter_files']),
                         ('frozen-controllers', prepared['controller_files']), ('frozen-inputs', prepared['input_files'])]:
        check_manifest(home / relative(folder), rows, exact=True)
    pinned(home / 'frozen-inputs' / ('mutation-manifest-' + variant + '.json'), selected['manifest'])
    pinned(home / ('mutation-overlay-' + variant + '.patch'), selected['patch'])
    verify(prepared['derived_mutation_manifest']['path'], prepared['derived_mutation_manifest'])
    for name, digest in selected['controllers'].items():
        pinned(home / 'frozen-controllers' / name, digest)
    for _, (name, digest) in INPUTS.items():
        pinned(home / 'frozen-inputs' / name, digest)
    for name, digest in OBSERVER_FILES.items():
        pinned(home / 'frozen-observer' / name, digest)
    original = read_json(home / 'frozen-inputs' / ('mutation-manifest-' + variant + '.json'))
    pinned(home / 'frozen-inputs/core-source-manifest.json', CORE)
    core = read_json(home / 'frozen-inputs/core-source-manifest.json')
    required = {r['path'] for r in core['files']} | OBSERVER_HELPERS | HELPERS
    require(len(prepared['files']) == len(required) == 123 and {r['path'] for r in prepared['files']} == required, 'compiled source roster drift')
    expected = {r['path']: r for r in original['files']}
    for row in prepared['files']:
        require(row == expected.get(row['path']), 'derived source is not a frozen source member')
    require(prepared['uncompiled_auxiliary_fixture_files_omitted'] == [expected[name] for name in sorted(set(expected) - required)], 'omitted auxiliary identities differ')
    derived = {**original, 'source': str(home / ('source-' + variant)), 'files': prepared['files'],
               'portable_original_manifest': prepared['original_mutation_manifest'],
               'status': 'derived paths and compiler-input subset; unchanged frozen source bytes'}
    require(read_json(home / ('mutation-manifest-' + variant + '.json')) == derived, 'unreviewed original/derived manifest mapping')
    require(pathlib.Path(prepared['derived_mutation_manifest']['path']) == home / ('mutation-manifest-' + variant + '.json'), 'derived manifest path substitution')
    verify(prepared['original_mutation_manifest']['path'], prepared['original_mutation_manifest'])
    require(pathlib.Path(prepared['original_mutation_manifest']['path']) == home / 'frozen-inputs' / ('mutation-manifest-' + variant + '.json'), 'original manifest path substitution')
    return prepared, home, variant


def build_view(prepared_path, prepared, home, variant, build_path, profile):
    build = read_json(build_path)
    require(build.get('portable_schema') == SCHEMA + '-build' and build['status'] == 'built' and build['profile'] == profile, 'wrong portable build')
    verify(prepared_path, build['prepared_manifest'])
    for tool in build['toolchain'].values():
        verify(tool['path'], tool)
    require(build['toolchain']['rustc']['sha256'] == RUSTC and build['rustc'].startswith('rustc 1.99.0 '), 'wrong qualified Rust binary')
    rustc = pathlib.Path(build['toolchain']['rustc']['path'])
    require(pathlib.Path(build['toolchain']['cargo']['path']) == rustc.with_name('cargo'), 'qualified Cargo/Rust paths must share the selected toolchain')
    require(build['cwd'] == str(home / ('source-' + variant)), 'build source differs')
    require(build['environment']['CARGO_INCREMENTAL'] == '0' and build['environment']['CARGO_BUILD_JOBS'] == '2', 'wrong build controls')
    for name in ('binary', 'compiler.stdout.jsonl', 'compiler.stderr'):
        verify(build[name]['path'], build[name])
    view = {'schema': 1, 'profile': profile, 'status': build['status'], 'exit_code': build['exit_code'],
            'argv': build['argv'], 'cwd': build['cwd'], 'rustc': build['rustc'],
            'toolchain_binary': build['toolchain']['rustc'], 'overlay_manifest': prepared['derived_mutation_manifest'],
            'environment': build['environment'], 'stdout': build['compiler.stdout.jsonl'],
            'stderr': build['compiler.stderr'], 'binary': build['binary'],
            'portable_build_receipt': bound_file(build_path)}
    # This stable per-build view permits verified baseline reuse; the original
    # portable build receipt is immutable and remains separately bound.
    path = pathlib.Path(build_path).resolve().parent / 'mutation-build-view.json'
    if path.exists():
        require(read_json(path) == view, 'existing build view differs')
    else:
        write_json(path, view)
    return path, rustc


def run(a):
    prepared_path = pathlib.Path(a.prepared_manifest).resolve()
    prepared, home, variant = checked_preparation(prepared_path)
    require((a.mutation_id == STALE) == (variant == 'v3'), 'case-restricted variant mismatch')
    material_path = pathlib.Path(a.materialization).resolve()
    material = read_json(material_path)
    require(material['schema'] == SCHEMA + '-materialized' and material['status'] == 'materialized', 'wrong materialization')
    source_root = material_path.parent
    require(str(source_root) == material['source_root'], 'source materialization moved')
    check_manifest(source_root, material['members'])
    require({p.relative_to(source_root).as_posix() for p in source_root.rglob('*.ox') if p.is_file()} == {r['path'] for r in material['members']}, 'source membership differs')
    requests = source_root / 'requests.jsonl'
    verify(requests, material['requests'])
    pinned(requests, 'f405bf46d8945027cd1d79d3c125692b4c00b8fe57b97c0d55c68b83b9795113')
    source_requests = [json.loads(line) for line in requests.read_text().splitlines()]
    expected_members = []
    for request in source_requests:
        for item in request['source_files']:
            expected_members.append({'path': (relative(request['source_root']) / relative(item['path'])).as_posix(),
                                     'bytes': item['bytes'], 'sha256': item['sha256']})
    require(len(source_requests) == 152 and len(expected_members) == len({r['path'] for r in expected_members}) == 445, 'original source roster differs')
    require(sorted(material['members'], key=lambda r: r['path']) == sorted(expected_members, key=lambda r: r['path']), 'materialized members differ from original fixed-SHA source requests')
    build_path = pathlib.Path(a.build_receipt).resolve()
    view_path, rustc = build_view(prepared_path, prepared, home, variant, build_path, a.profile)
    filename = 'run-effective.py' if a.mutation_id == EFFECTIVE else 'run.py'
    controller = home / 'frozen-controllers' / filename
    pinned(controller, VARIANTS[variant]['controllers'][filename])
    out = fresh(a.output)
    receipt = {'schema': SCHEMA + '-mutation', 'status': 'portable-mutation-failure',
               'assertion_mode': assertion_mode(), 'mutation_id': a.mutation_id, 'profile': a.profile,
               'prepared_manifest': bound_file(prepared_path), 'materialization': bound_file(material_path),
               'portable_build': bound_file(build_path), 'build_view': bound_file(view_path),
               'frozen_controller': bound_file(controller), 'path_wrapper': bound_file(pathlib.Path(__file__)),
               'original_mutation_manifest': prepared['original_mutation_manifest'],
               'derived_mutation_manifest': prepared['derived_mutation_manifest'],
               'python': bound_file(sys.executable), 'compiler_request_source': 'unchanged frozen source-only mutation controller'}
    write_json(out / 'invocation.json', receipt)
    initial_argv = sys.argv[:]
    optimization = os.environ.pop('PYTHONOPTIMIZE', None)
    started = time.monotonic()
    try:
        spec = importlib.util.spec_from_file_location('frozen_portable_mutation', controller)
        module = importlib.util.module_from_spec(spec)
        spec.loader.exec_module(module)
        old_here, old_oracles, old_observer = module.HERE, module.ORACLES, module.OBSERVER
        inputs = home / 'frozen-inputs'
        routes = {'pre-execution-manifest.json': inputs / 'pre-execution-manifest.json',
                  'mutation-requests.json': inputs / 'mutation-requests.json', 'requests.jsonl': requests,
                  'supplements/mutation-applicability-v1/requests.json': inputs / 'effective-requests.json',
                  'supplements/mutation-applicability-v1/supplement-manifest.json': inputs / 'applicability-manifest.json'}
        locations = {old_oracles / name: path for name, path in routes.items()}
        locations.update({old_observer / name: home / 'frozen-observer' / name for name in OBSERVER_FILES})
        manifest_name = 'mutation-manifest-' + variant + '.json'
        patch_name = 'mutation-overlay-' + variant + '.patch'
        locations.update({old_here / manifest_name: home / manifest_name, old_here / patch_name: home / patch_name})
        require(set(module.PINS) <= set(locations), 'unreviewed controller pin path')
        pins = {locations[path]: identity(locations[path])['sha256'] if path == old_here / manifest_name else digest for path, digest in module.PINS.items()}
        module.HERE, module.ORACLES, module.OBSERVER = home, RoutedInputs(routes), home / 'frozen-observer'
        module.RUSTC, module.PINS = rustc, pins
        # The unchanged controller requires its executable below HERE/target.
        # The portable builder is therefore required to use that exact directory.
        binary = pathlib.Path(read_json(view_path)['binary']['path'])
        require(binary.parent == home / 'target' / a.profile / 'deps', 'portable target must be prepared-home/target')
        sys.argv = [str(controller), '--id', a.mutation_id, '--profile', a.profile,
                    '--build-receipt', str(view_path), '--output', str(out / 'observed')]
        if a.baseline:
            sys.argv += ['--baseline', str(pathlib.Path(a.baseline).resolve())]
        receipt['controller_argv'] = sys.argv[:]
        receipt['relocated_pins'] = [{'path': str(path), 'sha256': digest} for path, digest in pins.items()]
        code = module.main()
        receipt['exit_status'] = code
        require(code == 0, 'frozen mutation controller failed')
        inner = read_json(out / 'observed/receipt.json')
        require(inner['status'] == 'observed' and inner['mutation_id'] == a.mutation_id and inner['profile'] == a.profile, 'wrong mutation completion')
        receipt.update(status='observed', observation_receipt=bound_file(out / 'observed/receipt.json'))
    except Exception as error:
        receipt['error'] = type(error).__name__ + ': ' + str(error)
    finally:
        sys.argv = initial_argv
        if optimization is not None:
            os.environ['PYTHONOPTIMIZE'] = optimization
        receipt['seconds'] = time.monotonic() - started
        try:
            checked_preparation(prepared_path)
            for key in ('prepared_manifest', 'materialization', 'portable_build', 'build_view', 'frozen_controller', 'path_wrapper'):
                verify(receipt[key]['path'], receipt[key])
        except Exception as error:
            receipt.update(status='portable-mutation-failure', post_run_identity_error=repr(error))
        if (out / 'observed/receipt.json').is_file():
            receipt['observation_receipt'] = bound_file(out / 'observed/receipt.json')
        write_json(out / 'portable-mutation.json', receipt)
    return receipt


def main():
    parser = argparse.ArgumentParser()
    commands = parser.add_subparsers(dest='command', required=True)
    prep = commands.add_parser('prepare')
    prep.add_argument('--variant', choices=VARIANTS, required=True)
    for name in ('observer-prepared', 'component-root', 'oracle-manifest', 'mutation-requests', 'effective-requests', 'applicability-manifest', 'output'):
        prep.add_argument('--' + name, required=True)
    execute = commands.add_parser('run')
    for name in ('prepared-manifest', 'materialization', 'build-receipt', 'mutation-id', 'output'):
        execute.add_argument('--' + name, required=True)
    execute.add_argument('--profile', choices=('debug', 'release'), required=True)
    execute.add_argument('--baseline')
    args = parser.parse_args()
    result = prepare(args) if args.command == 'prepare' else run(args)
    print(json.dumps({'status': result['status'], 'output': args.output}))
    return 0 if result['status'] in ('prepared', 'observed') else 1


if __name__ == '__main__':
    raise SystemExit(main())
