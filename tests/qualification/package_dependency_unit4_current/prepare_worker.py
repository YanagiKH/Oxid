"""Installed, bounded source-only Unit4 preparation worker.

Only ``prepare`` invokes genuine retained admission and preparation. ``plan`` and
``verify_output`` are read-only. The caller must authenticate this worker,
configuration, complete input inventory, process policy, and resource guard
before spawning isolated, bytecode-disabled Python. The CLI requires an explicit caller-sealed configuration path.
"""
from contextlib import contextmanager
import hashlib
import json
import os
import platform
from pathlib import Path, PurePosixPath
import re
import stat
import sys
import struct
import time
import types

STAGE1 = 'tests/qualification/package_dependency_unit4_source_v1/'
STAGE1_MANIFEST_SHA = '83460b8bddaeb59d2af8416ffe0d5548f51d3f36a7b3700611e3871c0ae47118'
STAGE1_PINS = {
    'package-manifest.json': '83460b8bddaeb59d2af8416ffe0d5548f51d3f36a7b3700611e3871c0ae47118',
    'parser-candidate-source.json': 'f04152f3d8ae1c74ae83e3ffe16f89fe6bd2f921d0d08e341dab96a4ae5368bb',
    'parser-source-successor-authority.json': '8473ef19379f0c12995d4e8ac10b71abc5af99023f68af1add493ae852adc0be',
    'parser_maps.py': '0cb1497c59b0ccc65fd8cfad2fbec32405494903de0d7c57d30a0692bfaf040b',
    'public-source-successor-authority.json': '392071a60bccbf00882a9419ca1ab7ec5197455d803189a9638abcf792650ac3',
    'public_maps.py': '381028469c64d724b9871d5ee3daa43f87d0f02a5bdd193f438d12855e570742',
    'source.py': 'ddffc38c26098f6727d4d253c08af0423027f0536efb97905938208320a3c7d9',
    'test_parser_maps.py': '676aec03ae8e6223231acc205ccb395998461fe6fb38fe9aa5fe6570232743ee',
    'test_public_maps.py': '21004e37706a34e1083e120aa0e7a175b0e7c28943e87065dfb4aeb944253320',
    'test_source.py': '86578a603ac97e79fbfb0e257d9fdc23b84694b06e239faef2100bb1fc1745d4',
}
PUBLIC = 'tests/qualification/unit4_public_v3/'
PARSER = 'tests/qualification/unit4_parser_current/'
FROZEN = 'tests/fixtures/typed_project_unit4_parser_portable/frozen/v3/'
ADAPTER = 'tests/qualification/lexer_reservation_current/adapters.py'
CORE_PINS = {
    PUBLIC + 'authority.py': '346485ecb8afed13bd784190f57f8e86a4d6d065437c2220e99928ade425aac7',
    PUBLIC + 'lexer_reservation_lifecycle.py': '155471428cc243fbb7845ec2dcd21fb982f51e7e8264ccf804f1df07338c91ec',
    ADAPTER: '0520fbf2c651f9b22897a19937c54749b0153ee234bae424dc4132d48e198e2a',
    PARSER + 'portable.py': '3366c49f267ab98bd792a1cd1b01ebbaaca548bc19685d841eb688cf39f99162',
    PARSER + 'authority.json': 'd8a02a6e970417b2108fcb8ff5e9eb2b32b068d562a7042d73eabaf504dae35c',
    FROZEN + 'authority.json': '02b72b3dcf45c695e5c523d71bb1c83e15c082556cf36029829fefc7a71571b0',
}
MAX_FILE = 64 * 1024 ** 2
MAX_OUTPUT = 128 * 1024 ** 2
METADATA_CAP = 2 * 1024 ** 2
FIELDS = ('current_derived_files', 'current_control_derived_files')
VIEW_FIELDS = {'public/ordinary': 'new_ordinary376', 'public/lifecycle': 'new_lifecycle377',
               'parser/observed': FIELDS[0], 'parser/control': FIELDS[1]}
SCOPES = {'public': ('public/ordinary', 'public/lifecycle'),
          'parser': ('parser/observed', 'parser/control'),
          'combined': tuple(VIEW_FIELDS)}
REQUIRED_HOSTS = {('linux', 'x86_64'), ('macos', 'x86_64'),
                  ('macos', 'aarch64'), ('windows', 'x86_64')}
ORIGINAL_HEAD = '0fef9bf4c664c52f441e1c8b842edfcb57fa15d0'
ORIGINAL_TREE = 'fc5515d09dce7e8cdb67a43c890272683f7b4755'
HISTORICAL_HEAD = 'd9e6b9bf172abd5e15da7212c9e6224e29ccc768'


class Rejected(ValueError):
    pass


def need(value, message):
    if not value:
        raise Rejected(message)


def sha(raw):
    return hashlib.sha256(raw).hexdigest()


def row(name, raw):
    return {'path': str(name), 'bytes': len(raw), 'sha256': sha(raw)}


def serial(value):
    return (json.dumps(value, sort_keys=True, indent=2, allow_nan=False) + '\n').encode()


def canonical(value):
    return json.dumps(value, sort_keys=True, separators=(',', ':'), allow_nan=False).encode()


def decode(raw):
    def pairs(items):
        result = {}
        for key, value in items:
            need(key not in result, 'duplicate JSON key')
            result[key] = value
        return result
    def reject(_):
        raise Rejected('nonfinite JSON')
    return json.loads(raw, object_pairs_hook=pairs, parse_constant=reject)


def safe(name):
    need(type(name) is str and name and ':' not in name and '\\' not in name and
         str(PurePosixPath(name)) == name and not name.startswith('/') and
         all(x not in ('.', '..', '') for x in PurePosixPath(name).parts), 'unsafe relative path')
    return name


def real_path(path, exists=True):
    path = Path(path)
    need(path.is_absolute() and path == path.resolve(strict=exists), 'canonical absolute path required')
    need(not any(p.is_symlink() for p in (path, *path.parents)), 'symlink input ancestry')
    return path


def regular(path, *, executable=False, tick=lambda: None):
    path = real_path(path)
    info = path.lstat()
    need(stat.S_ISREG(info.st_mode) and (executable or not info.st_mode & 0o111), 'nonregular/executable data')
    need(info.st_size <= MAX_FILE, 'input per-file bound')
    chunks = []
    with path.open('rb') as stream:
        while True:
            tick()
            chunk = stream.read(1024 ** 2)
            if not chunk:
                break
            chunks.append(chunk)
    raw = b''.join(chunks)
    need(len(raw) == info.st_size, 'input size changed while reading')
    return raw


def check_rows(records):
    need(type(records) is list, 'file map list required')
    names = []
    for record in records:
        need(type(record) is dict and set(record) == {'path', 'bytes', 'sha256'}, 'closed file row')
        names.append(safe(record['path']))
        need(type(record['bytes']) is int and 0 <= record['bytes'] <= MAX_FILE, 'file length bound')
        need(type(record['sha256']) is str and re.fullmatch('[0-9a-f]{64}', record['sha256']), 'file digest shape')
    need(len(names) == len(set(names)), 'duplicate file map member')
    need(not any(str(parent) in set(names) for n in names for parent in PurePosixPath(n).parents), 'file/directory collision')
    return {r['path']: r for r in records}


def directory_names(names):
    return {str(parent) for name in names for parent in PurePosixPath(name).parents if str(parent) != '.'}


def scan(root, *, allow_executable=False, tick=lambda: None):
    root = real_path(root)
    need(root.is_dir(), 'directory required')
    files, directories = {}, set()
    for parent, dirs, names in os.walk(root, followlinks=False):
        for name in dirs + names:
            tick()
            path = Path(parent) / name
            mode = path.lstat().st_mode
            need(stat.S_ISREG(mode) or stat.S_ISDIR(mode), 'symlink/special output member')
            relative = path.relative_to(root).as_posix()
            if stat.S_ISDIR(mode):
                directories.add(relative)
            else:
                files[relative] = row(relative, regular(path, executable=allow_executable, tick=tick))
    return files, directories


def measured_host():
    """Read the actual host without invoking tools or native probes."""
    system, machine = platform.system(), platform.machine().lower()
    return {'os': {'Linux': 'linux', 'Darwin': 'macos', 'Windows': 'windows'}.get(system, system.lower()),
            'architecture': {'amd64': 'x86_64', 'arm64': 'aarch64'}.get(machine, machine),
            'pointer_width': struct.calcsize('P') * 8}


def authenticate(config, tick=lambda: None):
    """Independently hash the caller-sealed closure before importing any helper."""
    need(type(config) is dict, 'configuration object required')
    need(not sys.flags.optimize and __debug__ and sys.dont_write_bytecode, 'normal bytecode-disabled Python required')
    scope = config.get('scope')
    need(type(scope) is str and scope in SCOPES, 'exact preparation scope required')
    python = real_path(config['python'])
    need(str(python) == sys.executable, 'canonical configured interpreter must equal sys.executable')
    host = measured_host()
    need(host['pointer_width'] == 64 and (host['os'], host['architecture']) in REQUIRED_HOSTS,
         'unsupported preparation host')
    if scope != 'public':
        need((host['os'], host['architecture']) == ('linux', 'x86_64'), 'parser preparation requires Linux x86_64')
    root_names = ('current_repo', 'stage1_root') if scope == 'public' else (
        'original_repo', 'historical_repo', 'current_repo', 'stage1_root')
    roots = {name: real_path(config[name]) for name in root_names}
    need(all(path.is_dir() for path in roots.values()), 'input roots must be directories')
    need(roots['stage1_root'] == roots['current_repo'], 'Stage1 must be installed in current repository')
    if scope != 'public':
        need(len({roots[n] for n in ('original_repo', 'historical_repo', 'current_repo')}) == 3,
             'parser repository roots must be distinct')
    output = real_path(config['output'], exists=False)
    need(output.name == 'source-preparation', 'installed preparation output name')
    need(config.get('run_root', str(output.parent)) == str(output.parent), 'preparation invocation root association')
    if not output.parent.exists():
        need(config.get('run_root') == str(output.parent) and output.parent.parent.is_dir(),
             'only the named fresh run root may be absent during planning')
    need(all(output != root and root not in output.parents and output not in root.parents
             for root in roots.values()), 'output overlaps input root')
    records = config['input_files']
    need(type(records) is list and records, 'sealed input inventory required')
    inventory = {}
    for record in records:
        tick()
        need(type(record) is dict and set(record) == {'path', 'bytes', 'sha256'}, 'closed absolute input row')
        need(type(record['bytes']) is int and 0 <= record['bytes'] <= MAX_FILE and
             type(record['sha256']) is str and re.fullmatch('[0-9a-f]{64}', record['sha256']), 'input identity shape')
        path = real_path(record['path'])
        need(str(path) not in inventory, 'duplicate input identity')
        need(row(path, regular(path, executable=True, tick=tick)) == record, 'input identity changed: ' + str(path))
        inventory[str(path)] = record
    need(str(python) in inventory, 'configured interpreter absent from sealed inventory')
    seen_routes = set()
    for route in config.get('input_routes', []):
        tick()
        need(type(route) is dict and set(route) == {'path', 'target'}, 'closed system tool route')
        path, target = Path(route['path']), Path(route['target'])
        need(str(path) not in seen_routes, 'duplicate input route')
        seen_routes.add(str(path))
        need(path.is_absolute() and (scope == 'public' or
             str(path).startswith(('/usr/bin/', '/bin/', '/usr/local/bin/')) or
             target == python), 'non-system tool symlink route')
        need(path.resolve(strict=True) == target and str(target) in inventory, 'changed tool symlink route')
    return roots, output, inventory


def input_body(inventory, path, digest=None, *, executable=False):
    path = real_path(path)
    need(str(path) in inventory, 'input absent from sealed closure: ' + str(path))
    raw = regular(path, executable=executable)
    need(row(path, raw) == inventory[str(path)], 'input changed after closure authentication')
    if digest is not None:
        need(sha(raw) == digest, 'fixed input pin mismatch: ' + str(path))
    return raw


def module(raw, path, name):
    result = types.ModuleType(name)
    result.__file__ = str(path)
    result.__package__ = ''
    exec(compile(raw, str(path), 'exec'), result.__dict__)
    return result


@contextmanager
def authority_import(authority):
    """Expose only the authenticated original under its required normal name."""
    marker = object()
    previous = sys.modules.get('authority', marker)
    sys.modules['authority'] = authority
    try:
        yield
    finally:
        if previous is marker:
            sys.modules.pop('authority', None)
        else:
            sys.modules['authority'] = previous


def _materials(config, tick=lambda: None):
    roots, output, inventory = authenticate(config, tick)
    scope = config['scope']
    stage = roots['stage1_root'] / STAGE1
    raw_manifest = input_body(inventory, stage / 'package-manifest.json', STAGE1_MANIFEST_SHA)
    manifest = decode(raw_manifest)
    need(set(manifest) == {'current_execution_status', 'execution_qualified', 'files',
                          'physical_preparation_performed', 'schema', 'scope'}, 'closed installed Stage1 manifest')
    need(manifest['schema'] == 'package-unit4-source-only-derivation-inventory-v1' and
         manifest['current_execution_status'] == 'NotReady' and manifest['execution_qualified'] is False and
         manifest['physical_preparation_performed'] is False, 'source-only Stage1 manifest')
    wanted = check_rows(manifest['files'])
    need(len(wanted) == 9 and {n: r['sha256'] for n, r in wanted.items()} ==
         {n: digest for n, digest in STAGE1_PINS.items() if n != 'package-manifest.json'},
         'fixed installed Stage1 nine-row roster')
    actual, directories = scan(stage, tick=tick)
    expected = dict(wanted, **{'package-manifest.json': row('package-manifest.json', raw_manifest)})
    need(actual == expected and len(actual) == 10 and directories == directory_names(actual),
         'closed installed Stage1 ten files/directories')
    stage_bodies = {n: input_body(inventory, stage / n, digest) for n, digest in STAGE1_PINS.items()}
    current = roots['current_repo']
    # Public admission always loads the immutable original helper at the current
    # installed path. Its source root is separately chosen, never relabelled.
    core = {n: input_body(inventory, current / n, digest) for n, digest in CORE_PINS.items()
            if n.startswith(PUBLIC) or n == ADAPTER} if scope != 'parser' else {}
    original = roots.get('original_repo')
    if scope != 'public':
        core.update({n: input_body(inventory, original / n, digest) for n, digest in CORE_PINS.items()
                     if n.startswith((PARSER, FROZEN))})
    source = module(stage_bodies['source.py'], stage / 'source.py', 'unit4_stage1_source')
    tick()
    context = source.capture(current)
    tick()
    public = public_proof = parser = parser_proof = old = historical = None
    if scope != 'parser':
        public = module(stage_bodies['public_maps.py'], stage / 'public_maps.py', 'unit4_stage1_public')
        public_inputs = {n: input_body(inventory, current / n, digest) for n, digest in public.PINS.items()}
        public_proof = public.validate_authority(stage_bodies['public-source-successor-authority.json'], context, public_inputs)
    if scope != 'public':
        parser = module(stage_bodies['parser_maps.py'], stage / 'parser_maps.py', 'unit4_stage1_parser')
        parser_inputs = {n: input_body(inventory, original / n, digest) for n, digest in parser.PINS.items()}
        parser_proof = parser.verify({n: stage_bodies[n] for n in parser.OUTPUTS}, context, parser_inputs)
        repositories = config.get('repositories')
        need(type(repositories) is list, 'sealed repository identities required')
        identities = {}
        for entry in repositories:
            need(type(entry) is dict and set(entry) == {'path', 'identity'} and
                 type(entry['path']) is str and entry['path'] not in identities and
                 type(entry['identity']) is dict, 'closed unique repository identity')
            identities[entry['path']] = entry['identity']
        git = identities.get(str(original), {})
        need(git.get('head') == ORIGINAL_HEAD and git.get('tree') == ORIGINAL_TREE,
             'original checkout must be the exact retained 0fef source')
        need(identities.get(str(roots['historical_repo']), {}).get('head') == HISTORICAL_HEAD,
             'historical repository must be the exact retained d9 source')
        old = decode(core[PARSER + 'authority.json'])
        historical = decode(core[FROZEN + 'authority.json'])
        need(historical['base_commit'] == 'd9e6b9bf172abd5e15da7212c9e6224e29ccc768', 'historical commit identity')
        for record in historical['original_files']:
            tick()
            need(row(record['path'], input_body(inventory, roots['historical_repo'] / safe(record['path']), executable=True)) == record,
                 'historical selected source identity')
        for name, body in context['predecessor_inputs'].items():
            tick()
            need(input_body(inventory, original / safe(name)) == body, 'original source differs from captured predecessor')
    for name, body in context['current_inputs'].items():
        tick()
        need(input_body(inventory, current / safe(name)) == body, 'captured current source differs from inventory')
    return dict(roots=roots, output=output, inventory=inventory, core=core, stage_bodies=stage_bodies,
                context=context, config=config, scope=scope, public=public, parser=parser,
                public_proof=public_proof, parser_proof=parser_proof, old=old, historical=historical)


def _overlay(materials, view, records):
    c = materials['context']
    key = 'public-source-successor-authority.json' if view.startswith('public/') else 'parser-source-successor-authority.json'
    return serial({'schema': 'oxid-package-unit4-source-preparation-overlay-v1',
        'status': 'NotReady', 'execution_qualified': False, 'view': view,
        'source_checkpoint': c['source_checkpoint'], 'current_repository': str(materials['roots']['current_repo']),
        'current_source_manifest': c['current_manifest_binding'],
        'predecessor_source_manifest': c['predecessor_manifest_binding'],
        'successor_authority': row(key, materials['stage_bodies'][key]),
        'source_map_sha256': sha(canonical(records)), 'files': records,
        'predecessor_evidence': ('predecessor/public-admission.json' if materials['scope'] == 'public'
                                 else 'predecessor/session.json'),
        'instrumentation_semantics': 'unchanged retained instrumentation; execution not qualified'})


def _plan(materials, tick=lambda: None):
    """Freeze every output name and exact source identity before any writes."""
    m, exact, caps = materials, {}, {}
    def add(record, prefix=''):
        tick()
        value = dict(record, path=prefix + record['path'])
        safe(value['path'])
        need(value['path'] not in exact and value['path'] not in caps, 'duplicate planned output')
        exact[value['path']] = value
    def body(name, raw):
        add(row(name, raw))
    def bounded(name):
        safe(name)
        need(name not in exact and name not in caps, 'duplicate planned metadata')
        caps[name] = METADATA_CAP
    if m['scope'] == 'public':
        for record in m['public_proof']['maps']['old_ordinary376']:
            add(record, 'predecessor/public-source/')
        body('predecessor/public-admission.json', input_body(m['inventory'],
             m['roots']['current_repo'] / PUBLIC / 'lexer-reservation-lifecycle-v2.json'))
    else:
        h, old = m['historical'], m['old']
        observer = canonical(h['helper_files'])
        need(sha(observer) == h['helper_manifest_sha256'], 'exact helper source manifest')
        old_candidate = serial({'schema': 'oxid-unit4-candidate-source-manifest-v1', 'commit': h['base_commit'], 'files': h['original_files']})
        need(sha(old_candidate) == h['candidate_source_manifest_sha256'], 'historical candidate reconstruction')
        for record in h['helper_files']:
            raw = input_body(m['inventory'], m['roots']['original_repo'] / FROZEN / 'frozen/helpers' / safe(record['path']))
            need(row(record['path'], raw) == record, 'historical helper closure')
            add(record, 'predecessor/helpers/')
        for control, field in zip((False, True), FIELDS):
            role = 'control-source' if control else 'source'
            invocation = 'prepare-control' if control else 'prepare'
            check_rows(old[field])
            need(len(old[field]) == 542, 'complete old542 count')
            for record in old[field]:
                add(record, 'predecessor/' + role + '/')
            body('predecessor/' + role + '/observer-source-manifest.json', observer)
            bounded('predecessor/' + role + '/overlay-manifest.json')
            body('predecessor/' + invocation + '/historical-candidate-source-manifest.json', old_candidate)
            bounded('predecessor/' + invocation + '/historical-overlay-manifest.json')
            stdout = (json.dumps({'overlay_manifest': str(m['output'] / 'predecessor' / role / 'overlay-manifest.json'),
                                  'files': 283, 'control': control}) + '\n').encode()
            body('predecessor/' + invocation + '/driver.stdout', stdout)
            body('predecessor/' + invocation + '/driver.stderr', b'')
            bounded('predecessor/' + invocation + '/invocation.json')
        for name, raw in (('current-authority.json', m['core'][PARSER + 'authority.json']),
                          ('historical-authority.json', m['core'][FROZEN + 'authority.json']),
                          ('current-source-manifest.json', m['context']['predecessor_manifest'])):
            body('predecessor/' + name, raw)
        bounded('predecessor/session.json')
    views, metadata = {}, {}
    for view in SCOPES[m['scope']]:
        field = VIEW_FIELDS[view]
        proof = m['public_proof'] if view.startswith('public/') else m['parser_proof']
        records = proof['maps'][field]
        check_rows(records)
        views[view] = records
        for record in records:
            add(record, view + '/')
        name = view + '/package-preparation-overlay.json'
        metadata[name] = _overlay(m, view, records)
        body(name, metadata[name])
    bounded('report.json')
    check_rows(list(exact.values()))
    names = set(exact) | set(caps)
    need(not any(str(parent) in names for n in names for parent in PurePosixPath(n).parents), 'planned path collision')
    exact_bytes = sum(r['bytes'] for r in exact.values())
    source_bytes = sum(r['bytes'] for view in views.values() for r in view)
    source_bytes += (sum(r['bytes'] for r in m['public_proof']['maps']['old_ordinary376'])
                     if m['scope'] == 'public' else
                     sum(r['bytes'] for field in FIELDS for r in m['old'][field]))
    budget = exact_bytes + sum(caps.values())
    need(budget <= MAX_OUTPUT, 'derived aggregate source/metadata envelope exceeds ceiling')
    result = {'schema': 'oxid-package-unit4-source-preparation-plan-v1', 'status': 'NotReady',
        'execution_qualified': False, 'scope': m['scope'], 'output': str(m['output']),
        'expected_files': [exact[n] for n in sorted(exact)],
        'metadata_caps': [{'path': n, 'max_bytes': caps[n]} for n in sorted(caps)],
        'directories': sorted(directory_names(names) | (set() if m['scope'] == 'public' else {'predecessor/home'})),
        'file_count': len(names), 'exact_bytes': exact_bytes, 'source_bytes': source_bytes,
        'metadata_max_bytes': sum(caps.values()), 'max_bytes': budget, 'maximum_bytes': budget,
        'views': views,
        'old_map_sha256': ({'old_ordinary376': sha(canonical(m['public_proof']['maps']['old_ordinary376'])),
                            'old_lifecycle377': sha(canonical(m['public_proof']['maps']['old_lifecycle377']))}
                           if m['scope'] == 'public' else
                           {field: sha(canonical(m['old'][field])) for field in FIELDS}),
        'new_map_sha256': {view: sha(canonical(records)) for view, records in views.items()},
        'old_source': m['context']['predecessor_manifest_binding'],
        'new_source': m['context']['current_manifest_binding'],
        'source_checkpoint': m['context']['source_checkpoint'],
        'stage1_manifest_sha256': STAGE1_MANIFEST_SHA,
        'original_public_lifecycle_proof': (row(PUBLIC + 'lexer-reservation-lifecycle-v2.json',
            input_body(m['inventory'], m['roots']['current_repo'] / PUBLIC / 'lexer-reservation-lifecycle-v2.json'))
            if m['scope'] != 'parser' else None),
        'input_inventory_sha256': sha(canonical(sorted(m['inventory'].values(), key=lambda r: r['path'])))}
    return result, metadata


def plan(config, tick=lambda: None):
    """Read/hash/pure-map planning only; does not run retained admission."""
    return _plan(_materials(config, tick), tick)[0]


def _inspect(output, proposed, *, report_required=True, tick=lambda: None):
    exact = check_rows(proposed['expected_files'])
    caps = {r['path']: r['max_bytes'] for r in proposed['metadata_caps']}
    need(len(caps) == len(proposed['metadata_caps']) and not set(caps) & set(exact), 'closed planned metadata')
    actual, directories = scan(output, tick=tick)
    names = set(exact) | set(caps)
    if not report_required:
        names.remove('report.json')
    need(set(actual) == names, 'complete output file roster differs')
    need(directories == set(proposed['directories']), 'complete output directory roster differs')
    for name, expected in exact.items():
        tick()
        need(actual[name] == expected, 'prepared source/body identity differs: ' + name)
    for name, cap in caps.items():
        if name in actual:
            need(type(cap) is int and 0 < cap <= METADATA_CAP and actual[name]['bytes'] <= cap, 'metadata envelope exceeded')
    need(sum(r['bytes'] for r in actual.values()) <= proposed['max_bytes'] <= MAX_OUTPUT, 'aggregate output envelope exceeded')
    return actual


def _report(proposed, actual, predecessor):
    records = [actual[n] for n in sorted(actual) if n != 'report.json']
    scope = proposed['scope']
    report = {'schema': 'oxid-package-unit4-source-preparation-report-v1', 'status': 'NotReady',
        'scope': scope, 'execution_qualified': False, 'physical_source_preparation_performed': True,
        'compiler_invocations': 0, 'native_executions': 0, 'parser_prepare_calls': int(scope != 'public'),
        'genuine_public_admit_calls': int(scope != 'parser'),
        'original_public_lifecycle_proof': proposed['original_public_lifecycle_proof'],
        'predecessor_invocations': predecessor['invocations'],
        'plan_sha256': sha(serial(proposed)), 'input_inventory_sha256': proposed['input_inventory_sha256'],
        'files': records, 'file_count_excluding_report': len(records),
        'total_bytes_excluding_report': sum(r['bytes'] for r in records),
        'source_bytes': proposed['source_bytes'], 'max_bytes_including_report': proposed['max_bytes'],
        'old_source': proposed['old_source'], 'new_source': proposed['new_source'],
        'source_checkpoint': proposed['source_checkpoint'], 'old_map_sha256': proposed['old_map_sha256'],
        'new_map_sha256': proposed['new_map_sha256']}
    if scope == 'public':
        report['genuine_public_prepared_ns'] = predecessor['public_prepared_ns']
        report['genuine_public_admission'] = actual['predecessor/public-admission.json']
    else:
        report['predecessor_prepared_ns'] = predecessor['session']['prepared_ns']
        report['genuine_predecessor_session'] = actual['predecessor/session.json']
    return report


def verify_output(config, proposed, tick=lambda: None):
    """Read/hash-only acceptance; process provenance belongs to the producer.

    In particular a public report timestamp is a value checked by the trusted
    producer against its actual supervisor window, never execution proof alone.
    No admission, materializer, Git command or tool subprocess is called here.
    """
    materials = _materials(config, tick)
    need(proposed == _plan(materials, tick)[0], 'provided plan differs from deterministic input derivation')
    output = Path(config['output'])
    actual = _inspect(output, proposed, tick=tick)
    report_raw = regular(output / 'report.json', tick=tick)
    report = decode(report_raw)
    need(type(report) is dict, 'report object required')
    if materials['scope'] == 'public':
        prepared_ns = report.get('genuine_public_prepared_ns')
        need(type(prepared_ns) is int and prepared_ns > 0, 'genuine public timestamp shape')
        predecessor = {'public_prepared_ns': prepared_ns, 'invocations': []}
    else:
        original = materials['roots']['original_repo']
        portable = module(materials['core'][PARSER + 'portable.py'], original / PARSER / 'portable.py', 'unit4_original_readback')
        predecessor = _verify_predecessor(materials, portable, row(output / 'predecessor/session.json',
                            regular(output / 'predecessor/session.json', tick=tick)))
    tick()
    expected = _report(proposed, actual, predecessor)
    need(set(report) == set(expected) and report_raw == serial(expected), 'exact closed source-only report differs')
    return expected


def _write(root, name, raw):
    path = root / safe(name)
    path.parent.mkdir(parents=True, exist_ok=True)
    need(not any(p.is_symlink() for p in (path, *path.parents)), 'output symlink ancestry')
    with path.open('xb') as stream:
        stream.write(raw)
    need(regular(path) == raw, 'output readback differs')


def _verify_public(materials, old_root):
    """Call the authenticated, unchanged original against real old source."""
    m = materials
    current = m['roots']['current_repo']
    authority = module(m['core'][PUBLIC + 'authority.py'], current / PUBLIC / 'authority.py', 'authority')
    lifecycle = module(m['core'][PUBLIC + 'lexer_reservation_lifecycle.py'],
                       current / PUBLIC / 'lexer_reservation_lifecycle.py', 'unit4_original_lifecycle')
    patch = input_body(m['inventory'], current / PUBLIC / 'observer-lexer-reservation-v2.patch')
    with authority_import(authority):
        old377, proof = lifecycle.admit(old_root, patch)
        prepared_ns = time.time_ns()
    expected = m['public_proof']
    proof_raw = input_body(m['inventory'], current / PUBLIC / 'lexer-reservation-lifecycle-v2.json')
    need(proof == decode(proof_raw) and serial(proof) == proof_raw,
         'genuine public admission must return the exact retained proof')
    need(lifecycle.rows(old377, True) == expected['maps']['old_lifecycle377'], 'genuine old lifecycle map differs')
    need(lifecycle.exact_patch(old377, patch, True) == m['context']['predecessor_inputs'], 'genuine old full inverse differs')
    new376 = m['context']['current_inputs']
    new377 = lifecycle.exact_patch(new376, patch)
    need(lifecycle.rows(new377, True) == expected['maps']['new_lifecycle377'], 'new lifecycle map differs')
    need(lifecycle.exact_patch(new377, patch, True) == new376, 'new full lifecycle inverse differs')
    need(proof['inserted_roster'] == expected['inserted_roster'] and
         proof['lexer_hook_correspondence'] == expected['lexer_hook_correspondence'], 'genuine lifecycle event correspondence differs')
    return {'public/ordinary': new376, 'public/lifecycle': new377}, proof_raw, prepared_ns


def _verify_predecessor(materials, portable, returned):
    """Read/hash-only original metadata checks; never session_at/verify_checkout."""
    m = materials
    root = m['output'] / 'predecessor'
    raw = regular(root / 'session.json')
    need(returned == row(root / 'session.json', raw), 'prepare returned a different session')
    session = decode(raw)
    expected_keys = {'schema', 'authority_sha256', 'run_id', 'root', 'prepared_ns', 'historical_repo',
        'checkout', 'host', 'prepare_invocation', 'control_prepare_invocation', 'overlay', 'control_overlay',
        'adapter', 'authority', 'transition_authority', 'historical_authority', 'current_source_manifest', 'transitions'}
    need(set(session) == expected_keys, 'closed genuine predecessor session fields')
    need(session['root'] == str(root) and session['historical_repo'] == str(m['roots']['historical_repo']) and
         session['checkout']['path'] == str(m['roots']['original_repo']), 'genuine session input/root association')
    need(session['authority_sha256'] == CORE_PINS[PARSER + 'authority.json'], 'old session authority identity')
    need(type(session['run_id']) is str and re.fullmatch('[0-9a-f]{32}', session['run_id']), 'genuine session nonce')
    need(type(session['prepared_ns']) is int and session['prepared_ns'] > 0, 'genuine prepared timestamp')
    a = dict(m['historical'], current=m['old'], current_source=decode(m['context']['predecessor_manifest']))
    # These unchanged original checks perform no Git verification or subprocess.
    portable.verify_transition_records(session, a)
    portable.verify_overlay(root, a)
    portable.verify_overlay(root, a, True)
    original = m['roots']['original_repo']
    repositories = {entry['path']: entry['identity'] for entry in m['config']['repositories']}
    need(str(original) in repositories, 'original Git identity missing from sealed config')
    git = repositories[str(original)]
    expected_checkout = {'path': str(original), 'head': git['head'], 'tree': git['tree'],
        'compiler_files': portable.compiler_map(a), 'historical_source_equivalent': False,
        'current_source_bound': True, 'current_source_manifest_sha256': m['context']['predecessor_manifest_binding']['sha256'],
        'reviewed_source_head': m['old']['reviewed_source_head'], 'source_only_tree': m['old']['source_only_tree']}
    need(session['checkout'] == expected_checkout, 'complete original checkout association')
    for key, path in (('adapter', original / PARSER / 'portable.py'),
                      ('authority', original / PARSER / 'authority.json')):
        need(session[key] == row(path, input_body(m['inventory'], path)), 'session retained helper binding')
    host = portable.measured_host()
    need(session['host'] == host, 'genuine session measured host')
    portable.check_host(host, a)
    env = portable.minimal_env(root / 'home')
    need(not {'CC', 'CXX', 'AR', 'CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER'} & set(env),
         'native tool invocation environment forbidden')
    tools = portable.process_tools(env)
    for tool in tools.values():
        identity = tool['binary']
        need(m['inventory'].get(identity['path']) == identity, 'invocation tool missing from sealed inventory')
    prior_finish = 0
    invocations = []
    for control, key in ((False, 'prepare_invocation'), (True, 'control_prepare_invocation')):
        role = 'prepare-control' if control else 'prepare'
        source = root / ('control-source' if control else 'source')
        invocation_root = root / role
        receipt_raw = regular(invocation_root / 'invocation.json')
        need(session[key] == row(invocation_root / 'invocation.json', receipt_raw), 'session invocation identity')
        receipt = decode(receipt_raw)
        need(set(receipt) == {'argv', 'cwd', 'environment', 'started_ns', 'finished_ns', 'exit_code',
                             'stdout', 'stderr', 'host', 'tools'}, 'closed preparation invocation fields')
        argv = [sys.executable, '-B', str(root / 'helpers/prepare.py'), '--repo',
                str(m['roots']['historical_repo']), '--output', str(source)]
        if control:
            argv.append('--control')
        need(receipt['argv'] == argv and receipt['cwd'] == str(root) and receipt['environment'] == env,
             'exact original preparation command/environment')
        need(type(receipt['exit_code']) is int and receipt['exit_code'] == 0, 'preparation success exit required')
        need(receipt['host'] == host and receipt['tools'] == tools, 'actual invocation host/tool identities')
        start, finish = receipt['started_ns'], receipt['finished_ns']
        need(type(start) is int and type(finish) is int and prior_finish <= start <= finish <= session['prepared_ns'],
             'ordered genuine preparation timestamps')
        prior_finish = finish
        for stream in ('stdout', 'stderr'):
            path = invocation_root / ('driver.' + stream)
            need(receipt[stream] == row(path, regular(path)), 'invocation stream binding')
        invocations.append({'role': role, 'started_ns': start, 'finished_ns': finish,
            'argv': argv, 'exit_code': 0,
            'receipt': row('predecessor/' + role + '/invocation.json', receipt_raw)})
    return {'session': session, 'invocations': invocations}


def prepare(config):
    """Materialize one admitted scope using genuine retained preparation only.

    The trusted producer must supervise this API in a fresh process. Failure
    retains evidence and never retries preparation or removes files. The pure
    plan and verification APIs never call this function or retained admission.
    """
    materials = _materials(config)
    proposed, metadata = _plan(materials)
    root, scope = materials['output'], materials['scope']
    need(not root.exists() and not root.is_symlink(), 'fresh preparation output required')
    need(root.parent.is_dir(), 'preparation output parent must exist')
    root.mkdir()
    views = {}
    if scope == 'public':
        # These are all 376 actual exact-inverse bodies admitted by Stage1.
        # No historical parser repository or original Git checkout is used.
        for name, raw in sorted(materials['context']['predecessor_inputs'].items()):
            _write(root, 'predecessor/public-source/' + name, raw)
        views, proof_raw, prepared_ns = _verify_public(materials, root / 'predecessor/public-source')
        _write(root, 'predecessor/public-admission.json', proof_raw)
        predecessor = {'public_prepared_ns': prepared_ns, 'invocations': []}
    else:
        original = materials['roots']['original_repo']
        if scope == 'combined':
            views, _, _ = _verify_public(materials, original)
        portable = module(materials['core'][PARSER + 'portable.py'], original / PARSER / 'portable.py', 'unit4_original_portable')
        returned = portable.prepare(materials['roots']['historical_repo'], original, root / 'predecessor')
        predecessor = _verify_predecessor(materials, portable, returned)
        for view, old_role, field in (('parser/observed', 'source', FIELDS[0]), ('parser/control', 'control-source', FIELDS[1])):
            previous = {r['path']: regular(root / 'predecessor' / old_role / safe(r['path'])) for r in materials['old'][field]}
            need([row(n, previous[n]) for n in sorted(previous, key=lambda n: PurePosixPath(n).parts)] == materials['old'][field],
                 'complete genuine old parser map differs')
            current = dict(previous)
            current['src/main.rs'] = materials['context']['current_inputs']['src/main.rs']
            current['candidate-source-manifest.json'] = materials['stage_bodies']['parser-candidate-source.json']
            need([row(n, current[n]) for n in sorted(current, key=lambda n: PurePosixPath(n).parts)] == proposed['views'][view],
                 'complete transported parser map differs')
            restored = dict(current, **{n: previous[n] for n in ('src/main.rs', 'candidate-source-manifest.json')})
            need(restored == previous, 'complete parser source inverse differs')
            need({n for n in current if current[n] != previous[n]} == {'src/main.rs', 'candidate-source-manifest.json'},
                 'parser phase/instrumentation changed')
            views[view] = current
    need(set(views) == set(SCOPES[scope]), 'exact requested view scope')
    for view, bodies in views.items():
        need({r['path']: r for r in proposed['views'][view]} == {n: row(n, raw) for n, raw in bodies.items()}, 'view before-write identity')
        for name, raw in sorted(bodies.items()):
            _write(root, view + '/' + name, raw)
    for name, raw in sorted(metadata.items()):
        _write(root, name, raw)
    actual = _inspect(root, proposed, report_required=False)
    report = _report(proposed, actual, predecessor)
    need(len(serial(report)) <= METADATA_CAP, 'report metadata envelope')
    _write(root, 'report.json', serial(report))
    need(verify_output(config, proposed) == report, 'final source-only output readback')
    return report


if __name__ == '__main__':
    need(len(sys.argv) == 3 and sys.argv[1] == '--config', 'explicit --config PATH required')
    config = decode(regular(Path(sys.argv[2])))
    result = prepare(config)
    print(json.dumps({'schema': result['schema'], 'status': result['status'],
                      'execution_qualified': result['execution_qualified'],
                      'report': str(Path(config['output']) / 'report.json')}, sort_keys=True))
