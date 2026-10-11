"""Fixed original-build provenance; serialized evidence is never a live owner.

Only a reviewed produce_build could issue BuildArtifact from an actual
supervised unchanged Cargo build; this source slice issues none. Unreviewed roles, policies, workflow wiring and transport stay
explicitly blocked. This source slice performs no build at import or planning.
"""
import base64
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import re
import stat
import sys
import types
import time
import weakref

SOURCE_KEY = '_oxid_package_dependency_ci_source_v1'
SOURCE_SHA256 = 'a202686c177c128628420c4f208aff67cf33830878af2df6132c739ef9cf081d'  # Set only when the canonical source owner is sealed.
BUILD_SCHEMA = 'oxid-package-ci-build-artifact-v1'
TRANSPORT_SCHEMA = 'oxid-package-ci-build-transport-v1'
ROLES = ('production', 'test', 'observer', 'container-image')
ORIGINAL_BUILDS = frozenset(('ci.yml:L80', 'ci.yml:L188', 'ci.yml:L490'))
ORIGINAL_ARGV = ['cargo', 'build', '--release', '--locked']
MAX_DOCUMENT = 128 * 1024 ** 2
MAX_CACHE_FILES = 20000
MAX_CACHE_BYTES = 2 * 1024 ** 3
_MAX_PROCESS_SECONDS = 7200
_MAX_STREAM = 32 * 1024 ** 2
_LIVE = weakref.WeakKeyDictionary()
PRODUCTION_ADAPTER_REVIEWED = False
_ENV_KEYS = frozenset(('PATH', 'HOME', 'USERPROFILE', 'CARGO_HOME', 'RUSTUP_HOME',
                       'RUSTUP_TOOLCHAIN', 'SYSTEMROOT', 'WINDIR', 'TEMP', 'TMP',
                       'TMPDIR', 'LANG', 'LC_ALL', 'CARGO_TERM_COLOR'))
_FORBIDDEN_ENV = ('RUSTFLAGS', 'CARGO_ENCODED_RUSTFLAGS', 'RUSTC', 'RUSTC_WRAPPER',
                  'RUSTC_WORKSPACE_WRAPPER', 'RUSTDOCFLAGS', 'CARGO_TARGET_DIR',
                  'CARGO_BUILD_TARGET', 'CARGO_BUILD_RUSTFLAGS', 'CARGO_BUILD_RUSTC')


class Rejected(ValueError):
    pass


def need(value, message):
    if not value:
        raise Rejected(message)


def canonical(value):
    return json.dumps(value, sort_keys=True, separators=(',', ':'), allow_nan=False).encode()


def decode(raw):
    need(type(raw) is bytes and len(raw) <= MAX_DOCUMENT, 'bounded build JSON required')
    def pairs(items):
        out = {}
        for key, value in items:
            need(key not in out, 'duplicate JSON key')
            out[key] = value
        return out
    try:
        return json.loads(raw, object_pairs_hook=pairs,
                          parse_constant=lambda _: need(False, 'nonfinite JSON'))
    except (UnicodeError, json.JSONDecodeError, RecursionError) as error:
        raise Rejected('invalid build JSON') from error


def clone(value):
    return decode(canonical(value))


def _source():
    owner = globals().get('_SOURCE_OWNER')
    need(type(owner) is types.ModuleType and sys.modules.get(SOURCE_KEY) is owner,
         'artifacts requires its authenticated canonical source owner')
    need(type(SOURCE_SHA256) is str and re.fullmatch('[0-9a-f]{64}', SOURCE_SHA256),
         'artifact source owner pin is not sealed')
    owner.owner()
    need(owner.identity(Path(__file__).resolve().with_name('source.py'))['sha256'] == SOURCE_SHA256,
         'artifact source owner body drift')
    need(owner.owns_sibling(sys.modules.get(__name__)), 'unissued artifacts sibling')
    return owner


def _relative(value):
    need(type(value) is str and value and '\\' not in value and ':' not in value and '\x00' not in value,
         'safe fixed relative path required')
    path = PurePosixPath(value)
    need(not path.is_absolute() and str(path) == value and all(part not in ('.', '..') for part in path.parts),
         'safe fixed relative path required')
    return value


def _local(path, exists=True):
    path = Path(path)
    need(path.is_absolute() and path.resolve(strict=exists) == path
         and not any(part.is_symlink() for part in (path, *path.parents)), 'canonical nonsymlink build path required')
    return path


def _body(row):
    need(type(row) is dict and type(row.get('bytes')) is int and row['bytes'] >= 0
         and type(row.get('sha256')) is str and re.fullmatch('[0-9a-f]{64}', row['sha256']), 'body identity required')
    return {'bytes': row['bytes'], 'sha256': row['sha256']}


class BuildArtifact:
    __slots__ = ('__weakref__',)
    def __new__(cls, *args, **kwargs):
        raise Rejected('BuildArtifact is issued only by the fixed actual producer')
    def document(self):
        _source()
        need(type(self) is BuildArtifact and self in _LIVE and not _LIVE[self].get('invalidated'), 'unissued or invalidated build artifact')
        return clone(_LIVE[self]['document'])


def plan_build(context, fixed_recipe_id):
    source = _source()
    document = source.verify_job(context)
    registry = source.registry(context)
    need(type(fixed_recipe_id) is str and fixed_recipe_id in registry['build_recipes'], 'unknown fixed build recipe')
    recipe = registry['build_recipes'][fixed_recipe_id]
    need(recipe.get('id') == fixed_recipe_id and recipe.get('role') in ROLES, 'fixed typed build recipe required')
    need(document['job_recipe_id'] in recipe['job_recipe_ids'], 'build recipe belongs to another job/matrix')
    source.projection(context, recipe['projection_id'])
    # Every role is explicitly typed; only the three original production build
    # commands have a producer implementation in this first source slice.
    supported = (recipe['original_id'] in ORIGINAL_BUILDS and recipe['role'] == 'production'
                 and recipe['profile'] == 'release' and recipe['argv'] == ORIGINAL_ARGV)
    if supported:
        need(recipe['package_name'] == 'oxid' and recipe['manifest_path'] == 'Cargo.toml'
             and recipe['target_name'] == 'oxid' and recipe['target_kind'] == 'bin'
             and recipe['target_path'] == 'src/cli.rs', 'fixed package/entrypoint build identity')
        need(recipe['output_relative'] in ('target/release/oxid', 'target/release/oxid.exe'), 'fixed selected production output')
    route = source.route(context, recipe['original_id'])
    # Registry must preserve the original command byte-for-byte. No flags,
    # message-format switch, output directory, or wrapper command is inserted.
    original = route.get('original_record', {}).get('original', route.get('original', {}))
    if supported:
        need(original.get('command') == 'cargo build --release --locked', 'original build body changed')
    return {'schema': 'oxid-package-ci-build-plan-v1', 'authority': 'inert-plan',
            'recipe': clone(recipe), 'source': clone(document), 'supported': supported,
            'operational': supported and recipe.get('operational') is True and document['activation'].get('approved') is True,
            'blockers': clone(recipe.get('blockers', []))}


def _policies(recipe):
    tool, env, config, resource = (recipe.get(key) for key in
                                   ('tool_policy', 'environment_policy', 'config_policy', 'resource_policy'))
    need(type(tool) is dict and set(tool) == {'reviewed', 'cargo_version', 'rustc_version', 'rustc_host'}
         and tool['reviewed'] is True and all(type(tool[key]) is str and tool[key] for key in
                                             ('cargo_version', 'rustc_version', 'rustc_host')), 'exact tool policy awaits review')
    need(type(env) is dict and set(env) == {'reviewed', 'inherit', 'fixed', 'forbidden'} and env['reviewed'] is True,
         'exact environment policy awaits review')
    need(type(env['inherit']) is list and len(set(env['inherit'])) == len(env['inherit'])
         and set(env['inherit']) <= _ENV_KEYS and type(env['fixed']) is dict and set(env['fixed']) <= _ENV_KEYS,
         'closed build environment keys')
    need(type(env['forbidden']) is list and set(_FORBIDDEN_ENV) <= set(env['forbidden']), 'complete forbidden build overrides required')
    need(type(config) is dict and set(config) == {'reviewed', 'files', 'absent'} and config['reviewed'] is True
         and type(config['files']) is list and type(config['absent']) is list, 'exact Cargo config policy awaits review')
    need(type(resource) is dict and set(resource) == {'reviewed', 'timeout_seconds', 'stream_limit_bytes'}
         and resource['reviewed'] is True, 'exact build resource policy awaits review')
    need(type(resource['timeout_seconds']) is int and 1 <= resource['timeout_seconds'] <= _MAX_PROCESS_SECONDS
         and type(resource['stream_limit_bytes']) is int and 1 <= resource['stream_limit_bytes'] <= _MAX_STREAM,
         'bounded supervised build resources')
    return tool, env, config, resource


def _environment(policy):
    need(all(key not in os.environ for key in policy['forbidden']), 'unreviewed inherited build override')
    # A caller may not inject additional Cargo configuration through environment.
    need(not any(key.startswith(('CARGO_TARGET_', 'CARGO_PROFILE_', 'CARGO_BUILD_', 'RUSTUP_DIST_', 'RUSTUP_UPDATE_'))
                 for key in os.environ), 'unreviewed Cargo or toolchain environment')
    result = {key: os.environ[key] for key in policy['inherit'] if key in os.environ}
    need(all(type(key) is str and type(value) is str and '\x00' not in value for key, value in policy['fixed'].items()),
         'fixed environment strings required')
    result.update(policy['fixed'])
    need('PATH' in result and ('HOME' in result or 'USERPROFILE' in result), 'reviewed PATH and home required')
    return result


def _config(source, repository, environment, policy):
    home = _local(Path(environment.get('HOME', environment.get('USERPROFILE'))))
    cargo_home = _local(Path(environment.get('CARGO_HOME', str(home / '.cargo'))))
    roots = {'repository': repository, 'home': home, 'cargo_home': cargo_home}
    seen, rows = set(), []
    for absent, collection in ((False, policy['files']), (True, policy['absent'])):
        for row in collection:
            need(type(row) is dict and set(row) == ({'root', 'path'} if absent else {'root', 'path', 'bytes', 'sha256'}),
                 'closed Cargo configuration row')
            need(row['root'] in roots, 'known config root required')
            path = _local(roots[row['root']] / _relative(row['path']), exists=not absent)
            need(str(path) not in seen, 'duplicate Cargo config path')
            seen.add(str(path))
            if absent:
                need(not os.path.lexists(path), 'expected absent Cargo config appeared')
                rows.append({'path': str(path), 'absent': True})
            else:
                identity = source.identity(path)
                need(_body(identity) == _body(row), 'Cargo config body drift')
                rows.append(identity)
    # All Cargo config search locations and rustup directory overrides must be
    # covered explicitly, including ancestors outside the checkout.
    locations = {cargo_home / name for name in ('config', 'config.toml')}
    for parent in (repository, *repository.parents):
        locations.update(parent / '.cargo' / name for name in ('config', 'config.toml'))
        locations.update(parent / name for name in ('rust-toolchain', 'rust-toolchain.toml'))
    need(all(str(path) in seen or not os.path.lexists(path) for path in locations), 'unreviewed Cargo/rustup configuration')
    return sorted(rows, key=lambda row: row['path'])


def _stream(row):
    need(type(row) is dict and set(row) == {'base64', 'bytes', 'sha256'}, 'closed raw build stream')
    try:
        raw = base64.b64decode(row['base64'], validate=True)
    except (ValueError, TypeError) as error:
        raise Rejected('invalid raw build stream') from error
    need(type(row['bytes']) is int and len(raw) == row['bytes'] <= _MAX_STREAM
         and hashlib.sha256(raw).hexdigest() == row['sha256'], 'raw build stream body mismatch')
    return raw


def _metadata(raw, repository, recipe):
    metadata = decode(raw)
    need(type(metadata) is dict and metadata.get('version') == 1, 'Cargo metadata v1 required')
    need(metadata.get('workspace_root') == str(repository) and metadata.get('target_directory') == str(repository / 'target'),
         'Cargo workspace or target directory changed')
    selected = [package for package in metadata.get('packages', [])
                if package.get('name') == recipe['package_name']
                and package.get('manifest_path') == str(repository / recipe['manifest_path'])]
    need(len(selected) == 1 and selected[0].get('source') is None, 'exact local Cargo package required')
    targets = [target for target in selected[0].get('targets', []) if target.get('name') == recipe['target_name']
               and target.get('kind') == [recipe['target_kind']]
               and target.get('src_path') == str(repository / recipe['target_path'])]
    need(len(targets) == 1, 'exact Cargo target/entrypoint required')
    need(selected[0].get('id') in metadata.get('workspace_members', []), 'selected package is not a workspace member')
    return {'metadata': metadata, 'package_id': selected[0]['id'], 'target': targets[0]}


def _cache_inventory(source, repository, selected):
    target = repository / 'target'
    if not target.exists():
        return {'root': str(target), 'existed': False, 'files': []}
    _local(target)
    rows, size = [], 0
    for path in sorted(target.rglob('*')):
        source.tick()
        need(not path.is_symlink(), 'symlink in reused build cache')
        if path.is_dir():
            continue
        need(path != selected and stat.S_ISREG(path.stat().st_mode), 'selected artifact or special build cache member')
        size += path.stat().st_size
        need(len(rows) < MAX_CACHE_FILES and size <= MAX_CACHE_BYTES, 'build cache inventory exceeds bound')
        identity = source.identity(path)
        rows.append(dict(identity, path=path.relative_to(repository).as_posix()))
    return {'root': str(target), 'existed': True, 'files': rows}


def _assert_output_absent(repository, recipe):
    path = _local(repository / _relative(recipe['output_relative']), exists=False)
    need(not os.path.lexists(path), 'selected executable must be absent before the actual build')
    return path


def _same_source(source, context, before):
    after = source.verify_job(context)
    need(after == before, 'source context drift across actual build')
    return after


def _metadata_exact(raw, repository, recipe, source_files):
    selected = _metadata(raw, repository, recipe)
    value = selected['metadata']
    package_id = 'path+' + repository.as_uri() + '#oxid@0.9.0'
    need(value.get('resolve') is None and len(value['packages']) == 1,
         'complete single-package no-deps metadata required')
    package = value['packages'][0]
    need(package['id'] == package_id and package.get('version') == '0.9.0' and
         package.get('edition') == '2021', 'exact current production package/version/edition')
    targets = [
        {'kind': ['bin'], 'crate_types': ['bin'], 'name': 'oxid', 'src_path': str(repository / 'src/cli.rs'),
         'edition': '2021', 'doc': True, 'doctest': False, 'test': True},
        {'kind': ['custom-build'], 'crate_types': ['bin'], 'name': 'build-script-build',
         'src_path': str(repository / 'build.rs'), 'edition': '2021', 'doc': False, 'doctest': False, 'test': False}]
    need(type(source_files) is list and len(source_files) == 376 and
         len({row['path'] for row in source_files}) == 376, 'complete current-all376 metadata source map')
    for row in source_files:
        path = PurePosixPath(row['path'])
        if len(path.parts) == 2 and path.parts[0] == 'tests' and path.suffix == '.rs':
            targets.append({'kind': ['test'], 'crate_types': ['bin'], 'name': path.stem,
                'src_path': str(repository / path), 'edition': '2021', 'doc': False, 'doctest': False, 'test': True})
    need(sorted(package['targets'], key=lambda item: item['name']) == sorted(targets, key=lambda item: item['name']),
         'complete original production Cargo target roster differs')
    need(value.get('workspace_members') == [package_id] and
         value.get('workspace_default_members') == [package_id], 'complete original production workspace membership')
    return selected


def _emitted_artifact(path, host, process, tick):
    # Actual emitted bytes are inspected without executing the selected program.
    path = _local(path)
    tick()
    before = path.stat()
    need(stat.S_ISREG(before.st_mode) and 0 < before.st_size <= 1024 ** 3,
         'bounded regular selected production executable required')
    names = ('st_dev', 'st_ino', 'st_mode', 'st_size', 'st_mtime_ns', 'st_ctime_ns')
    signature = {name: getattr(before, name) for name in names}
    digest, header, size = hashlib.sha256(), bytearray(), 0
    with path.open('rb') as stream:
        need(all(getattr(os.fstat(stream.fileno()), name) == signature[name] for name in names),
             'selected production executable changed before open')
        while True:
            tick()
            chunk = stream.read(1024 ** 2)
            if not chunk:
                break
            size += len(chunk)
            need(size <= 1024 ** 3, 'selected executable grew beyond bound')
            digest.update(chunk)
            if len(header) < 1024 ** 2:
                header.extend(chunk[:1024 ** 2 - len(header)])
        need(all(getattr(os.fstat(stream.fileno()), name) == signature[name] for name in names),
             'selected production executable changed during read')
    need(size == before.st_size and all(getattr(path.stat(), name) == signature[name] for name in names),
         'selected production executable changed after read')
    need(process['started_ns'] <= before.st_mtime_ns <= process['completed_ns'],
         'selected executable emission time is outside original successful build')
    raw = bytes(header)
    if host == 'linux-x86_64':
        need(len(raw) >= 64 and raw[:7] == b'\x7fELF\x02\x01\x01' and
             int.from_bytes(raw[16:18], 'little') in (2, 3) and int.from_bytes(raw[18:20], 'little') == 62 and
             int.from_bytes(raw[20:24], 'little') == 1 and int.from_bytes(raw[52:54], 'little') == 64,
             'actual ELF64 x86_64 production executable required')
    elif host in ('macos-x86_64', 'macos-aarch64'):
        cpu = 0x01000007 if host == 'macos-x86_64' else 0x0100000c
        need(len(raw) >= 32 and raw[:4] == b'\xcf\xfa\xed\xfe' and
             int.from_bytes(raw[4:8], 'little') == cpu and int.from_bytes(raw[12:16], 'little') == 2,
             'actual matching Mach-O production executable required')
    else:
        raise Rejected('NotReady: Windows descendant containment remains unreviewed')
    need(before.st_mode & 0o111, 'selected production output is not executable')
    tick()
    return {'path': str(path), 'bytes': size, 'sha256': digest.hexdigest(), 'stat': signature}


def _operation_evidence(record, document):
    streams = {}
    for name in ('stdout', 'stderr'):
        raw = getattr(record, name)()
        need(len(raw) == document[name]['bytes'] and hashlib.sha256(raw).hexdigest() == document[name]['sha256'],
             'production operation raw evidence differs')
        streams[name] = {'base64': base64.b64encode(raw).decode('ascii'), 'bytes': len(raw),
                         'sha256': hashlib.sha256(raw).hexdigest()}
    return {'process': clone(document), 'streams': streams}


def produce_build(context, fixed_recipe_id):
    source = _source()
    plan = plan_build(context, fixed_recipe_id)
    source.require_operational(context, plan['recipe'])
    need(plan['supported'] and plan['operational'], 'this build role/recipe has no admitted operational producer')
    need(PRODUCTION_ADAPTER_REVIEWED is True,
         'NotReady: authenticated supervisor/machine production adapter awaits independent execution review')
    return _produce_build(source, context, plan)


def _produce_build(source, context, plan):
    # Only the fixed authenticated owners can produce live operations. There is
    # no caller-owned build callback, dictionary receipt or alternative command.
    recipe = plan['recipe']
    before = source.verify_job(context)
    repository = _local(before['repository_path'])
    selected = _assert_output_absent(repository, recipe)
    cache = _cache_inventory(source, repository, selected)
    process = source.load_production_owner('process')
    machine = source.load_production_owner('machine')
    invocation = process.begin_production(context, recipe['id'])
    records, operations = {}, {}
    issued_artifact = None
    try:
        admission = machine.capture_production_machine(invocation)
        machine_before = machine.verify_production_machine(admission, invocation)
        _same_source(source, context, before)
        for operation_id in ('metadata-before', 'build', 'metadata-after'):
            if operation_id in ('metadata-before', 'build'):
                _assert_output_absent(repository, recipe)
            _same_source(source, context, before)
            need(machine.verify_production_machine(admission, invocation) == machine_before,
                 'complete machine drift before production operation')
            record = process.owned_production_operation(invocation, operation_id)
            document = process.verify_production_operation(record, invocation, operation_id)
            records[operation_id] = record
            operations[operation_id] = _operation_evidence(record, document)
            if operation_id.startswith('metadata-'):
                _metadata_exact(record.stdout(), repository, recipe, before['source_files'])
            _same_source(source, context, before)
            need(machine.verify_production_machine(admission, invocation) == machine_before,
                 'complete machine drift after production operation')
        need(operations['metadata-before']['streams']['stdout'] == operations['metadata-after']['streams']['stdout'],
             'actual Cargo metadata changed across original build')
        prior = invocation.document()['started_ns']
        for operation_id in ('metadata-before', 'build', 'metadata-after'):
            process_result = operations[operation_id]['process']
            need(prior <= process_result['started_ns'] <= process_result['completed_ns'], 'production operation window order')
            prior = process_result['completed_ns']
        emitted = _emitted_artifact(selected, before['host'], operations['build']['process'],
                                    lambda: process.production_tick(invocation))
        _same_source(source, context, before)
        need(machine.verify_production_machine(admission, invocation) == machine_before,
             'complete machine drift before final artifact issuance')
        document = {'schema': BUILD_SCHEMA, 'role': 'production', 'profile': 'release',
                    'recipe': clone(recipe), 'source': clone(before), 'invocation': invocation.document(),
                    'machine': machine_before, 'cache_before': cache, 'operations': operations,
                    'artifact': emitted, 'completed_ns': time.time_ns(), 'execution_qualified': False,
                    'cargo_offline': False, 'network_isolated': False,
                    'transport_status': 'NotReady: cross-step evidence remains separately held'}
        issued_artifact = object.__new__(BuildArtifact)
        _LIVE[issued_artifact] = {'document': clone(document), 'context': context, 'invocation': invocation,
                         'machine': admission, 'records': records, 'invalidated': False}
        verify_artifact(issued_artifact, role='production', profile='release')
        return issued_artifact
    except BaseException as error:
        if issued_artifact is not None:
            _LIVE.pop(issued_artifact, None)
        # Failure preservation is supplementary; it cannot mask the original error.
        try:
            process.retain_production_failure(invocation, error)
        except BaseException:
            pass
        try:
            process.cancel_production(invocation)
        except BaseException:
            pass
        raise


def verify_artifact(record, *, role, profile):
    source = _source()
    need(type(record) is BuildArtifact and record in _LIVE and not _LIVE[record].get('invalidated'),
         'original live BuildArtifact required; data is inert')
    need(role == 'production' and profile == 'release', 'exact production role/profile required')
    owned = _LIVE[record]
    document = owned['document']
    try:
        need(document['role'] == role and document['profile'] == profile, 'production artifact role/profile mismatch')
        need(source.verify_job(owned['context']) == document['source'], 'production artifact source context drift')
        process = source.load_production_owner('process')
        machine = source.load_production_owner('machine')
        invocation = owned['invocation']
        need(process.verify_production(invocation) == document['invocation'] and
             process.production_context(invocation) is owned['context'], 'production artifact original invocation mismatch')
        need(machine.verify_production_machine(owned['machine'], invocation) == document['machine'],
             'production artifact complete machine input drift')
        for operation_id in ('metadata-before', 'build', 'metadata-after'):
            value = process.verify_production_operation(owned['records'][operation_id], invocation, operation_id)
            need(_operation_evidence(owned['records'][operation_id], value) == document['operations'][operation_id],
                 'production artifact original raw operation drift')
        value = _emitted_artifact(Path(document['artifact']['path']), document['source']['host'],
                                 document['operations']['build']['process'], lambda: process.production_tick(invocation))
        need(value == document['artifact'], 'production selected executable body/stat drift')
        need(source.verify_job(owned['context']) == document['source'], 'production artifact final source drift')
        return clone(document)
    except BaseException:
        owned['invalidated'] = True
        raise


def read_artifact_evidence(path, expected_join, provider_outcomes):
    source = _source()
    join = source.verify_join(expected_join)
    source.require_operational(expected_join)
    jobs = source.load_sibling('jobs')
    # Requires the original fixed HTTP capture even when transport is unsupported.
    jobs._outcome_document(expected_join, provider_outcomes)
    value = decode(source.read(Path(path), cap=MAX_DOCUMENT))
    need(type(value) is dict and value.get('schema') == TRANSPORT_SCHEMA, 'closed actual-build transport required')
    # The original workflow has no reviewed complete build-step/raw-source
    # transport. Do not turn hashes, JSON, or successful job conclusions into a
    # build receipt, nor resurrect the producer registry in a different step.
    raise Rejected('cross-step build transport awaits exact raw evidence and provider build-step review')
