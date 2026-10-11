"""Current-source CI live-to-transport boundary.

A capsule is inert evidence. Only the canonical in-process consumer owners can
issue a LiveSeal; no deserializer, report, archive or local terminal can do so.
"""
import copy
import contextlib
import contextvars
import math
import hashlib
import io
import json
import lzma
import os
from pathlib import Path, PurePosixPath, PureWindowsPath
import re
import stat
import sys
import tarfile
import tempfile
import threading
import time
import types
import weakref

sys.dont_write_bytecode = True
COMMON_SHA256 = '017a0b034ac120b246d2f9b88600cc3515534fe97c3e033b73a9f363e7b76e3d'
COMMON_KEY = '_oxid_package_dependency_unit4_ci_common_v1'
HOST_SCHEMA = 'oxid-package-unit4-ci-host-v1'
SEAL_SCHEMA = 'oxid-package-unit4-ci-seal-v1'
CAPSULE_SCHEMA = 'oxid-package-unit4-ci-capsule-v1'
COMPACT_LIMIT = 32 * 1024 ** 2
DECODED_LIMIT = 768 * 1024 ** 2
MAX_MEMBER = 256 * 1024 ** 2
MAX_MEMBERS = 200000
FULL_LIMIT = 32 * 1024 ** 3
PROFILES = ('debug', 'release')
SECTIONS = ('public', 'original', 'predecessors', 'lifecycle', 'guards')
DOMAINS = ('linux-public', 'linux-parser', 'macos-x86_64-public', 'macos-arm64-public', 'windows-x86_64-public')
PARSER_SEQUENCE = ('passivity-debug', 'collect-debug', 'passivity-release', 'collect-release', 'u8', 'phase', 'compare')
IDENTITY_KEYS = {'path', 'bytes', 'sha256'}
_LIVE = weakref.WeakKeyDictionary()
_DEADLINE = contextvars.ContextVar('unit4_ci_evidence_deadline', default=None)


def tick():
    deadline = _DEADLINE.get()
    need(deadline is None or time.monotonic() < deadline, 'CI evidence phase deadline exceeded')


@contextlib.contextmanager
def budget(deadline):
    need(type(deadline) in (int, float) and math.isfinite(deadline) and deadline > time.monotonic(), 'finite future evidence deadline')
    previous = _DEADLINE.get()
    token = _DEADLINE.set(min(previous, deadline) if previous is not None else deadline)
    try:
        tick(); yield; tick()
    finally:
        _DEADLINE.reset(token)


class _BoundedIO:
    def __init__(self, stream): self.stream = stream
    def read(self, size=-1):
        tick(); value = self.stream.read(size); tick(); return value
    def write(self, raw):
        tick(); value = self.stream.write(raw); tick(); return value
    def __getattr__(self, name): return getattr(self.stream, name)



class Rejected(ValueError):
    pass


def need(value, message):
    if not value:
        raise Rejected(message)


def sha(raw):
    return hashlib.sha256(raw).hexdigest()


def canonical(value):
    return json.dumps(value, sort_keys=True, separators=(',', ':'), allow_nan=False).encode()


def decode(raw):
    def pairs(items):
        value = {}
        for key, item in items:
            need(key not in value, 'duplicate JSON key')
            value[key] = item
        return value
    return json.loads(raw, object_pairs_hook=pairs,
                      parse_constant=lambda _: need(False, 'nonfinite JSON'))


def clone(value):
    return decode(canonical(value))


def _common():
    owner = globals().get('_COMMON_OWNER')
    need(type(owner) is types.ModuleType and sys.modules.get(COMMON_KEY) is owner,
         'evidence requires its authenticated common owner')
    need(type(COMMON_SHA256) is str and sha(_bytes(Path(__file__).resolve().with_name('common.py'))) == COMMON_SHA256,
         'unsealed common body')
    owner.owner()
    need(owner.owns_sibling(sys.modules.get(__name__)), 'unissued evidence sibling')
    return owner


def _local(path, exists=True):
    path = Path(path)
    need(path.is_absolute() and path.resolve(strict=exists) == path and
         not any(p.is_symlink() for p in (path, *path.parents)), 'canonical local path required')
    return path


def _bytes(path, limit=MAX_MEMBER):
    path = _local(path)
    before = path.stat()
    need(stat.S_ISREG(before.st_mode) and before.st_size <= limit, 'bounded regular evidence required')
    parts, size = [], 0
    with path.open('rb') as stream:
        for block in iter(lambda: stream.read(1024 ** 2), b''):
            tick(); size += len(block)
            need(size <= limit and size <= before.st_size, 'growing evidence file')
            parts.append(block)
    raw = b''.join(parts)
    tick()
    after = path.stat()
    keys = ('st_dev', 'st_ino', 'st_mode', 'st_size', 'st_mtime_ns', 'st_ctime_ns')
    need(len(raw) == before.st_size and all(getattr(before, k) == getattr(after, k) for k in keys),
         'evidence changed during read')
    return raw


def identity(path):
    path = _local(path)
    before = path.stat()
    need(stat.S_ISREG(before.st_mode) and before.st_size <= FULL_LIMIT, 'bounded regular identity')
    digest, size = hashlib.sha256(), 0
    with path.open('rb') as stream:
        for raw in iter(lambda: stream.read(1024 ** 2), b''):
            tick(); digest.update(raw); size += len(raw)
            need(size <= FULL_LIMIT, 'identity byte bound')
    after = path.stat()
    need(before == after and size == before.st_size, 'identity changed during read')
    return {'path': str(path), 'bytes': size, 'sha256': digest.hexdigest()}


def original_path(value):
    """Validate a producer path lexically, independently of the join's OS."""
    need(type(value) is str and value and '\x00' not in value, 'original path string')
    cls = PureWindowsPath if re.match(r'^[A-Za-z]:[\\/]', value) else PurePosixPath
    path = cls(value)
    need(path.is_absolute() and '..' not in path.parts and '.' not in path.parts and str(path) == value,
         'noncanonical original path')
    if cls is PureWindowsPath:
        need(not value.startswith('\\\\') and not any(':' in x for x in path.parts[1:]), 'Windows device/stream path')
    return path


def child(root, *parts):
    return str(original_path(root).joinpath(*parts))


def safe_member(value):
    need(type(value) is str and value and '\\' not in value and '\x00' not in value,
         'unsafe transport member')
    path = PurePosixPath(value)
    need(not path.is_absolute() and str(path) == value and
         all(part not in ('', '.', '..') for part in path.parts), 'unsafe transport relative path')
    return path


def member_name(original):
    path = original_path(original)
    if isinstance(path, PureWindowsPath):
        parts = ('windows', path.drive[0].upper(), *path.parts[1:])
    else:
        parts = ('posix', *path.parts[1:])
    return 'raw/' + PurePosixPath(*parts).as_posix()


def exact_identity(row):
    need(type(row) is dict and set(row) == IDENTITY_KEYS, 'closed evidence identity')
    original_path(row['path'])
    need(type(row['bytes']) is int and 0 <= row['bytes'] <= FULL_LIMIT and
         type(row['sha256']) is str and re.fullmatch('[0-9a-f]{64}', row['sha256']), 'evidence identity value')
    return row


def _identities(value):
    """Enumerate only explicit file identities, never arbitrary path strings."""
    if type(value) is dict:
        if IDENTITY_KEYS <= set(value) and type(value.get('path')) is str:
            try:
                original_path(value['path'])
            except Rejected:
                pass
            else:
                row = {k: value[k] for k in IDENTITY_KEYS}
                exact_identity(row)
                yield row
        for item in value.values():
            yield from _identities(item)
    elif type(value) is list:
        for item in value:
            yield from _identities(item)


def _activation(document):
    activation = document.get('activation')
    need(type(activation) is dict and activation.get('approved') is True,
         'CI activation has not been independently reviewed')
    need(activation.get('job_limit_seconds') == 21600,
         'reviewed GitHub-hosted job ceiling required')


class LiveSeal:
    __slots__ = ('__weakref__',)

    def __new__(cls, *args, **kwargs):
        raise TypeError('LiveSeal is issued only from original terminal live consumers')

    def document(self):
        return clone(_live(self)['document'])


def _live(seal):
    need(type(seal) is LiveSeal and seal in _LIVE, 'original live seal required')
    state = _LIVE[seal]
    need(state['pid'] == os.getpid() and state['thread'] == threading.get_ident() and not state['invalidated'],
         'live seal original process/thread required')
    return state


def _public_snapshot(api, result, source_context, machine, invocation, policy):
    runtime, build, hosted = api['public_runtime'], api['build'], api['hosted']
    if policy['hosted_required']:
        need(type(result) is runtime.JoinedResult, 'Linux public requires original root twelve-gap join')
        runtime.verify_join(result)
        joined = runtime._JOINS[result]
        collection = joined['collection']
        hosted_inputs = hosted.result_inputs(joined['hosted'])
        hosted_record = hosted._RESULTS[joined['hosted']]
        transport = hosted._TRANSPORTS[hosted_record['transport']]
        shard = {'report': hosted_inputs['report'], 'report_binding': hosted_inputs['report_binding'],
                 'worker_process': hosted_inputs['worker_process'], 'ledger': clone(transport['ledger']),
                 'ledger_binding': hosted_inputs['transport_binding'], 'runtime_tools': transport['traps'].document()}
        terminal_binding = clone(joined['binding'])
    else:
        need(type(result) is runtime.CollectionResult, 'portable public requires original complete collection')
        runtime.verify_collection(result)
        collection, shard, terminal_binding = result, None, None
    owned = runtime._COLLECTIONS[collection]
    state = runtime._state(owned['admission'])
    need(state['invocation'] is invocation and state['machine'] is machine,
         'public seal cross-invocation or cross-machine')
    configs, builds = {}, {}
    for role in ('ordinary', 'lifecycle'):
        live = build.runtime_inputs(state[role])
        need(live['context'] is source_context and live['machine'] is machine and live['invocation'] is invocation,
             'public build belongs to another source/machine/invocation')
        body = build.read_build_binding(state[role])
        need(set(body['profiles']) == set(PROFILES) and body['role'] == role,
             'public complete role/profile inventory')
        builds[role] = {'binding': clone(build._result(state[role])['receipt']), 'document': body}
        configs[role] = clone(state['cfgs'][role])
    document = result.document()
    need(document['status'] == 'COMPLETE_LOCAL_OBSERVATIONS' and document['execution_qualified'] is False,
         'public terminal must preserve its non-global local status')
    primary = collection.document()
    need(set(primary['reports']) == set(SECTIONS), 'all five public sections required')
    if policy['hosted_required']:
        need(primary['status'] == 'INCOMPLETE_LOCAL_CAPABILITY' and
             primary['mandatory_hosted_join_required'] is True, 'genuine root primary gaps required')
        keys = document['replaced_capability_keys']
        need(set(keys) == set(SECTIONS) and {k: len(v) for k, v in keys.items()} ==
             {'public': 6, 'original': 0, 'predecessors': 0, 'lifecycle': 6, 'guards': 0},
             'exact twelve root replacements required')
    else:
        need(primary['status'] == 'COMPLETE_LOCAL_OBSERVATIONS' and
             primary['mandatory_hosted_join_required'] is False, 'portable incomplete collection')
    return {'document': document, 'terminal_binding': terminal_binding or clone(owned['binding']),
            'primary': primary, 'primary_binding': clone(owned['binding']), 'builds': builds, 'configs': configs,
            'execution_context': clone(state['execution_context']), 'root': str(state['root']),
            'collection_root': str(state['collection_root']), 'input_paths': clone(state['input_paths']),
            'contracts': clone(runtime._contract_snapshot(state['contracts'])),
            'rosters': clone(state['rosters']), 'runtime_tools': state['runtime_tools'].document(),
            'hosted': shard}


def _parser_snapshot(api, handle, source_context, machine, invocation):
    parser = api['parser']
    report = parser.verify_runtime(handle)
    runtime = parser._runtime_api()
    need(type(handle) is runtime.RuntimeSession, 'canonical parser RuntimeSession required')
    state = runtime._live(handle)
    need(state['invocation'] is invocation and state['machine'] is machine and state['a']._context is source_context,
         'parser seal cross-context/machine/invocation')
    need(report['stages'] == list(PARSER_SEQUENCE) and report['local_semantics_status'] == 'pass',
         'complete parser sequence required')
    builds = []
    for profile in PROFILES:
        for control in (False, True):
            result = state['builds'][(profile, control)]
            native, outer = parser.verify_build(result)
            live = parser._result(result)
            need(live['invocation'] is invocation and live['machine'] is machine and live['a'] is state['a'],
                 'parser original four-build association')
            builds.append({'native': native, 'document': outer, 'binding': clone(live['checkpoint'])})
    need(len({b['native']['binary']['sha256'] for b in builds}) == 4, 'four distinct parser binaries')
    return {'report': report, 'root': str(state['root']), 'session': clone(state['session']),
            'session_binding': clone(state['session_identity']), 'recipe': clone(state['recipe']),
            'builds': builds, 'contract_root': str(state['contract_root']),
            'runtime_inputs': state['runtime_inputs'].document(),
            'processes': [clone(state['processes'][name]['document']) for name in sorted(state['processes'])],
            'sealed_runtime': [clone(state['sealed'][name]) for name in sorted(state['sealed'])]}


def _terminal(state):
    q = _common()
    ci = q.verify_ci(state['ci_context'])
    _activation(ci)
    q.require_activation(state['ci_context'])
    api = q.apis(state['ci_context'])
    invocation = q.invocation(state['ci_context'])
    source, machine = state['source_context'], state['machine']
    need(type(source) is api['context'].AdmittedContext and source.invocation() is invocation,
         'canonical original prepared source required')
    api['context'].verify_context(source)
    need(type(machine) is api['machine'].ToolAdmission and machine.invocation() is invocation,
         'canonical original machine required')
    api['machine'].verify_machine(machine, invocation)
    api['preparation'].full_verify_invocation(invocation)
    policy = q.host_policy(ci['domain'])
    need(source.document()['scope'] == policy['scope'] and
         source.document()['invocation'] == invocation.document(), 'CI source scope/invocation')
    public = (_public_snapshot(api, state['public_result'], source, machine, invocation, policy)
              if policy['public_required'] else None)
    parser = (_parser_snapshot(api, state['parser_handle'], source, machine, invocation)
              if policy['parser_required'] else None)
    staging = (q.load_sibling('staging').verify_staging(state['staging'], state['ci_context'], invocation)
               if policy['staging_required'] else None)
    api['context'].verify_context(source)
    api['machine'].verify_machine(machine, invocation)
    api['preparation'].full_verify_invocation(invocation)
    need(q.verify_ci(state['ci_context']) == ci, 'CI changed during terminal readmission')
    return {'schema': HOST_SCHEMA, 'ci_context': ci, 'policy': policy, 'invocation': invocation.document(),
            'source_context': source.document(), 'preparation': api['context']._preparation(invocation), 'machine': machine.document(),
            'public': public, 'parser': parser, 'staging': staging,
            'status': 'TERMINAL_LIVE_SEALED', 'ci_runtime_proof': True,
            'terminal_readmission': ['source', 'machine', 'invocation', 'applicable-public',
                                    'applicable-parser', 'applicable-staging', 'committed-ci-inputs'],
            'qualification_boundary': 'This domain alone is not four-host qualification.'}


def seal_live_host(ci_context, *, source_context, machine, public_result=None, parser_handle=None, staging=None):
    q = _common()
    ci = q.verify_ci(ci_context)
    _activation(ci)
    q.require_activation(ci_context)
    policy = q.host_policy(ci['domain'])
    for flag, value in (('public_required', public_result), ('parser_required', parser_handle), ('staging_required', staging)):
        need((value is not None) is policy[flag], 'missing or foreign live domain object: ' + flag)
    state = dict(ci_context=ci_context, source_context=source_context, machine=machine,
                 public_result=public_result, parser_handle=parser_handle, staging=staging,
                 pid=os.getpid(), thread=threading.get_ident(), invalidated=False)
    started = time.time_ns()
    host = _terminal(state)
    journal = Path(ci['outer_root']) / 'ci-host-stages.json'
    need(journal.is_file(), 'completed immutable gate stage journal required')
    host['journal'] = identity(journal)
    inventory = _inventory(host)
    need(_terminal(state) == {k: v for k, v in host.items() if k != 'journal'},
         'live inputs changed during seal inventory')
    need(identity(journal) == host['journal'], 'gate journal changed during seal')
    state['document'] = {'schema': SEAL_SCHEMA, 'host': host, 'inventory': inventory,
                         'started_ns': started, 'completed_ns': time.time_ns(), 'status': 'sealed'}
    seal = object.__new__(LiveSeal)
    _LIVE[seal] = state
    return seal


def _classification(host, transport=None):
    """Omissions come from exact admitted role maps and executable identities."""
    full, caches, routes = {}, {}, {}
    read = (lambda path: transport.raw(path)) if transport is not None else _bytes
    def full_record(row, role):
        row = {k: row[k] for k in IDENTITY_KEYS}
        exact_identity(row)
        if row['path'] in full:
            need(full[row['path']]['identity'] == row, 'conflicting full-only identity')
        full[row['path']] = {'identity': row, 'role': role}
    for role, view in host['source_context']['views'].items():
        for row in view['files']:
            # The generated candidate body is semantic input even in compact form.
            if row['path'] != 'candidate-source-manifest.json':
                full_record(dict(row, path=child(view['path'], row['path'])), 'prepared-source:' + role)
    public = host['public']
    if public is not None:
        for role, record in public['builds'].items():
            build = record['document']
            plan = decode(read(build['plan']['path']))
            caches[plan['target']] = 'public-cargo-target:' + role
            for profile in PROFILES:
                for key in ('binary', 'emitted_binary'):
                    full_record(build['profiles'][profile][key], 'public-binary:' + role + ':' + profile + ':' + key)
        # Emitted runtime programs are retained in full. Their exact receipt paths,
        # rather than filename or directory tests, define this finite omission set.
        for section in SECTIONS:
            rows = _json_lines(read(child(public['collection_root'], section, 'observations.jsonl.gz')), compressed=True)
            for row in rows:
                values = [row]
                if row.get('ordinary_receipt'):
                    values.append(decode(read(row['ordinary_receipt']['path'])))
                for value in values:
                    for key in ('original_elf', 'copied_elf'):
                        if value.get('native_execution'):
                            full_record(value['native_execution'][key], 'public-native-binary')
                    if value.get('linked_artifact'):
                        full_record(value['linked_artifact'], 'public-linked-artifact')
        if public['hosted'] is not None:
            ledger = public['hosted']['ledger']
            logical_binaries = {r['document']['profiles'][p]['binary']['path']
                                for r in public['builds'].values() for p in PROFILES}
            for row in ledger['entries']:
                if row['logical_path'] in logical_binaries:
                    full_record({'path': row['physical_path'], 'bytes': row['bytes'], 'sha256': row['sha256']},
                                'hosted-readonly-compiler')
    parser = host['parser']
    if parser is not None:
        for role, dirname in (('parser_observed', 'source'), ('parser_control', 'control-source')):
            for row in host['source_context']['views'][role]['files']:
                if row['path'] != 'candidate-source-manifest.json':
                    full_record(dict(row, path=child(parser['root'], dirname, row['path'])), 'parser-source:' + role)
        for record in parser['builds']:
            outer = record['document']
            full_record(record['native']['binary'], 'parser-binary:' + outer['profile'] + ':' + str(outer['control']))
            for name in ('cargo-home', 'result/target'):
                caches[child(outer['output'], name)] = 'parser-cache:' + outer['profile'] + ':' + str(outer['control']) + ':' + name
            for name in ('cargo', 'rustc'):
                route = child(outer['output'], 'rust-bin', name)
                routes[route] = {'kind': 'executable-view', 'target': host['machine']['snapshot']['rust']['root'] + '/bin/' + name}
    staging = host['staging']
    if staging is not None:
        for row in staging['original_receipt']['content']['entries']:
            if row['kind'] == 'file':
                full_record({'path': child(staging['library_root'], row['path']), 'bytes': row['bytes'], 'sha256': row['sha256']}, 'staged-llvm-library')
    return full, caches, routes


def _json_lines(raw, compressed=False):
    if compressed:
        import gzip
        parts, size = [], 0
        with gzip.GzipFile(fileobj=io.BytesIO(raw)) as stream:
            while True:
                tick(); block = stream.read(1024 ** 2)
                if not block: break
                size += len(block)
                need(size <= DECODED_LIMIT, 'decoded observation bound')
                parts.append(block)
        raw = b''.join(parts)
    rows = []
    with io.BytesIO(raw) as stream:
        while True:
            tick(); line = stream.readline(MAX_MEMBER + 1)
            if not line: break
            need(len(line) <= MAX_MEMBER and len(rows) < MAX_MEMBERS, 'bounded raw observation row')
            if line.strip(): rows.append(decode(line))
    return rows


def _inventory(host):
    full, caches, routes = _classification(host)
    records, links, directories, cache_records = {}, [], set(), []
    roots = {host['ci_context']['producer_root'], str(Path(host['journal']['path']).parent / 'staging') if host['staging'] else None}
    roots.discard(None)
    def add(path, role='raw-evidence'):
        row = identity(path)
        previous = records.get(row['path'])
        need(previous is None or previous['identity'] == row, 'duplicate evidence body differs')
        classification = full.get(row['path'])
        if classification:
            need(classification['identity'] == row, 'full-only actual identity differs')
        records[row['path']] = {'identity': row, 'role': classification['role'] if classification else role,
                                'storage': 'full-only' if classification else 'compact'}
    for root in sorted(roots):
        tick()
        root_path = _local(root)
        need(root_path.is_dir(), 'owned evidence root absent')
        pending = [root_path]
        while pending:
            tick(); directory = pending.pop()
            with os.scandir(directory) as entries:
                for entry in entries:
                    tick(); path = directory / entry.name
                    need(len(records) + len(links) + len(directories) < MAX_MEMBERS, 'evidence member bound')
                    if entry.is_dir(follow_symlinks=False):
                        if str(path) in caches:
                            cache_records.append({'path': str(path), 'role': caches[str(path)],
                                                  'retained_executables': sorted(p for p in full if path in Path(p).parents)})
                        else:
                            directories.add(str(path)); pending.append(path)
                    elif entry.is_symlink():
                        if str(path) in routes:
                            target = str(path.resolve(strict=True))
                            need(target == routes[str(path)]['target'], 'exact registered executable route')
                            links.append({'path': str(path), 'target': os.readlink(path), 'resolved': target,
                                          'role': 'parser-executable-view'})
                        elif entry.name == 'occupied-output' and host['public'] is not None:
                            need(os.readlink(path) == 'preserved-target' and path.parent.joinpath('preserved-target').is_file(),
                                 'guard symlink must retain exact target')
                            links.append({'path': str(path), 'target': 'preserved-target',
                                          'resolved': str(path.resolve(strict=True)), 'role': 'public-occupied-output'})
                        elif host['staging'] is not None and str(path) == child(host['staging']['library_root'], 'libLLVM-19.so'):
                            links.append({'path': str(path), 'target': os.readlink(path),
                                          'resolved': str(path.resolve(strict=True)), 'role': 'staged-llvm-alias'})
                        else:
                            raise Rejected('unlisted evidence symlink: ' + str(path))
                    else:
                        need(entry.is_file(follow_symlinks=False), 'special evidence file')
                        add(path)
    for row in full.values():
        add(row['identity']['path'])
    add(host['journal']['path'], 'gate-stage-journal')
    # Materialized contracts and path maps are raw authorities, never summaries.
    public = host['public']
    if public is not None:
        for row in public['contracts']['verified']:
            add(row['path'], 'public-contract-authority')
        add(child(public['input_paths']['contract_root'], 'PUBLIC-FREEZE-v3.json'), 'public-contract-body')
        for section in ('public', 'original'):
            table = public['contracts']['tables'][section]
            for group in ('literal_cases', 'inherited_negative_and_route_cases'):
                for case in table[group]:
                    for row in case['source_files']:
                        add(child(public['input_paths']['contract_root'], case['source_root'], row['path']), 'public-contract-body')
        add(public['input_paths']['path_map'], 'public-contract-path-map')
    parser = host['parser']
    if parser is not None:
        freeze_path = child(parser['contract_root'], 'package-freeze.json')
        freeze = decode(_bytes(freeze_path)); add(freeze_path, 'parser-contract-body')
        for row in freeze['files']:
            path = child(parser['contract_root'], row['path'])
            need(identity(path) == dict(row, path=path), 'parser frozen contract body')
            add(path, 'parser-contract-body')
    if host['staging'] is not None:
        for row in host['staging']['dependencies']['dpkg_metadata']:
            add(row['path'], 'staging-dpkg-metadata')
    compact = sum(r['identity']['bytes'] for r in records.values() if r['storage'] == 'compact')
    total = sum(r['identity']['bytes'] for r in records.values())
    need(compact <= DECODED_LIMIT and total <= FULL_LIMIT, 'evidence byte bounds')
    need(set(full) <= set(records), 'required full executable/source artifact missing')
    return {'files': [records[name] for name in sorted(records)],
            'links': sorted(links, key=lambda r: r['path']), 'directories': sorted(directories),
            'excluded_regenerable_caches': sorted(cache_records, key=lambda r: r['path'])}


def _tar(path, manifest, records, *, compact):
    with Path(path).open('xb') as file, tarfile.open(fileobj=_BoundedIO(file), mode='w:xz', preset=6) as sink:
        raw = canonical(manifest)
        header = tarfile.TarInfo('capsule.json' if compact else 'full-manifest.json')
        header.size, header.mode = len(raw), 0o644
        need(len(raw) <= MAX_MEMBER, 'bounded archive manifest')
        sink.addfile(header, _BoundedIO(io.BytesIO(raw)))
        for row in records:
            tick()
            identity_row = row['identity']
            data = _bytes(identity_row['path'])
            need(len(data) == identity_row['bytes'] and sha(data) == identity_row['sha256'],
                 'changed evidence during export')
            header = tarfile.TarInfo(member_name(identity_row['path']))
            header.size, header.mode = len(data), 0o644
            sink.addfile(header, _BoundedIO(io.BytesIO(data)))


def export_capsule(seal, fresh_directory, *, deadline=None):
    state = _live(seal)
    seconds = state['document']['host']['ci_context']['activation']['failure_preservation_seconds']
    need(type(seconds) is int and 0 < seconds <= 21600, 'trusted export preservation bound')
    maximum = time.monotonic() + seconds
    if deadline is None: deadline = maximum
    need(deadline <= maximum, 'export deadline exceeds trusted preservation reservation')
    with budget(deadline):
        return _export_capsule(seal, fresh_directory)


def _export_capsule(seal, fresh_directory):
    state = _live(seal)
    root = _local(fresh_directory, False)
    need(not os.path.lexists(root) and root.parent.is_dir(), 'fresh evidence destination required')
    root.mkdir()
    try:
        document = state['document']
        host = document['host']
        need(_terminal(state) == {k: v for k, v in host.items() if k != 'journal'}, 'live terminal changed before export')
        need(identity(host['journal']['path']) == host['journal'], 'gate journal changed before export')
        need(_inventory(host) == document['inventory'], 'sealed evidence changed before export')
        records = document['inventory']['files']
        full_manifest = {'schema': 'oxid-package-unit4-ci-full-evidence-v1', 'seal': document,
                         'files': [{**row, 'member': member_name(row['identity']['path'])} for row in records]}
        _tar(root / 'full.tar.xz', full_manifest, records, compact=False)
        full_archive = identity(root / 'full.tar.xz')
        manifest = {'schema': CAPSULE_SCHEMA, 'status': 'sealed', 'seal': document,
                    'members': [{**row, 'member': member_name(row['identity']['path'])}
                                for row in records if row['storage'] == 'compact'],
                    'full_archive': {**full_archive, 'path': 'full.tar.xz'},
                    'replay_boundary': 'Transport verifies retained raw predicates and exact identities; it does not restore live capabilities or replay omitted binaries.'}
        _tar(root / 'compact.tar.xz', manifest, [r for r in records if r['storage'] == 'compact'], compact=True)
        need((root / 'compact.tar.xz').stat().st_size <= COMPACT_LIMIT, 'compact archive exceeds32MiB')
        need(_inventory(host) == document['inventory'], 'evidence drift during archives')
        need(_terminal(state) == {k: v for k, v in host.items() if k != 'journal'}, 'terminal drift during export')
        receipt = {'schema': 'oxid-package-unit4-ci-export-v1', 'status': 'exported',
                   'compact': identity(root / 'compact.tar.xz'), 'full': full_archive}
        with (root / 'export.json').open('xb') as output:
            output.write(canonical(receipt))
        return root / 'compact.tar.xz'
    except BaseException as error:
        state['invalidated'] = True
        failed = {'schema': 'oxid-package-unit4-ci-export-failure-v1', 'status': 'failed',
                  'original_error': {'type': type(error).__name__, 'message': str(error)[:4096]},
                  'qualifying_capsule': False}
        try:
            with (root / 'export-failed.json').open('xb') as output: output.write(canonical(failed))
        except BaseException as preservation:
            error.add_note('evidence export failure preservation failed: ' + str(preservation)[:1000])
        raise


class Transport:
    """Inert bounded raw transport, deliberately unrelated to LiveSeal."""
    def __init__(self, manifest, payloads):
        need(type(manifest) is dict and set(manifest) ==
             {'schema', 'status', 'seal', 'members', 'full_archive', 'replay_boundary'}, 'closed capsule schema')
        need(manifest['schema'] == CAPSULE_SCHEMA and manifest['status'] == 'sealed', 'failed/old capsule')
        self.manifest, self.payloads = clone(manifest), dict(payloads)
        self.seal = self.manifest['seal']
        need(set(self.seal) == {'schema', 'host', 'inventory', 'started_ns', 'completed_ns', 'status'} and
             self.seal['schema'] == SEAL_SCHEMA and self.seal['status'] == 'sealed', 'closed transport seal')
        need(type(self.seal['started_ns']) is int and type(self.seal['completed_ns']) is int and
             0 < self.seal['started_ns'] <= self.seal['completed_ns'], 'seal time interval')
        rows = self.manifest['members']
        need(type(rows) is list and rows and len(rows) <= MAX_MEMBERS, 'capsule member bound')
        self.mapping, self.index = {}, {}
        seen_members, seen_paths = set(), set()
        full_archive = self.manifest['full_archive']
        need(type(full_archive) is dict and set(full_archive) == IDENTITY_KEYS and full_archive['path'] == 'full.tar.xz' and
             type(full_archive['bytes']) is int and 0 < full_archive['bytes'] <= FULL_LIMIT and
             type(full_archive['sha256']) is str and re.fullmatch('[0-9a-f]{64}', full_archive['sha256']), 'closed retained full archive identity')
        need(type(self.seal['inventory']) is dict and set(self.seal['inventory']) == {'files', 'links', 'directories', 'excluded_regenerable_caches'}, 'closed sealed evidence inventory')
        files = self.seal['inventory']['files']
        need(type(files) is list and len(files) <= MAX_MEMBERS, 'sealed inventory member bound')
        for row in files:
            tick()
            need(type(row) is dict and set(row) == {'identity', 'role', 'storage'} and
                 row['storage'] in ('compact', 'full-only') and type(row['role']) is str and row['role'],
                 'closed evidence classification')
            bound = exact_identity(row['identity'])
            normalized = bound['path'].casefold() if isinstance(original_path(bound['path']), PureWindowsPath) else bound['path']
            need(bound['path'] not in self.index and normalized not in seen_paths, 'duplicate original evidence path')
            seen_paths.add(normalized)
            self.index[bound['path']] = bound
        need([r['identity']['path'] for r in files] == sorted(self.index), 'ordered closed evidence inventory')
        wanted = [{**row, 'member': member_name(row['identity']['path'])}
                  for row in files if row['storage'] == 'compact']
        need(rows == wanted, 'compact omission inventory differs from sealed exact roles')
        for row in rows:
            tick()
            bound, member = row['identity'], row['member']
            safe_member(member)
            need(bound['path'] not in self.mapping and member not in seen_members and member in self.payloads, 'missing/duplicate archive member')
            seen_members.add(member)
            raw = self.payloads[member]
            need(type(raw) is bytes and len(raw) == bound['bytes'] and sha(raw) == bound['sha256'], 'altered capsule member')
            self.mapping[bound['path']] = row
        need(set(self.payloads) == {r['member'] for r in rows} and
             sum(len(raw) for raw in self.payloads.values()) <= DECODED_LIMIT, 'extra or unbounded capsule payload')
        links = self.seal['inventory']['links']
        need(type(links) is list and len(links) <= MAX_MEMBERS, 'bounded link metadata')
        link_names = []
        for row in links:
            need(type(row) is dict and set(row) == {'path', 'target', 'resolved', 'role'}, 'closed original link metadata')
            original_path(row['path']); original_path(row['resolved'])
            need(type(row['target']) is str and row['target'] and '\x00' not in row['target'] and
                 row['path'] not in self.index, 'link/file collision')
            normalized = row['path'].casefold() if isinstance(original_path(row['path']), PureWindowsPath) else row['path']
            need(normalized not in seen_paths, 'case-aliased original link collision')
            seen_paths.add(normalized); link_names.append(row['path'])
        need(link_names == sorted(set(link_names)), 'duplicate/unordered original link metadata')
        directories = self.seal['inventory']['directories']
        need(type(directories) is list and len(directories) <= MAX_MEMBERS and
             directories == sorted(set(directories)), 'closed original directory catalogue')
        for path in directories:
            tick(); original_path(path)
            normalized = path.casefold() if isinstance(original_path(path), PureWindowsPath) else path
            need(normalized not in seen_paths and path not in link_names, 'directory/member collision')
            seen_paths.add(normalized)
        need(len(files) + len(links) + len(directories) <= MAX_MEMBERS, 'total original member bound')
        caches = self.seal['inventory']['excluded_regenerable_caches']
        need(type(caches) is list and len(caches) <= MAX_MEMBERS, 'bounded cache metadata')
        cache_names = []
        for row in caches:
            need(type(row) is dict and set(row) == {'path', 'role', 'retained_executables'} and
                 type(row['role']) is str and type(row['retained_executables']) is list, 'closed excluded cache metadata')
            original_path(row['path']); cache_names.append(row['path'])
            need(row['retained_executables'] == sorted(set(row['retained_executables'])), 'ordered retained executable paths')
            for path in row['retained_executables']: original_path(path)
        need(cache_names == sorted(set(cache_names)), 'duplicate/unordered excluded cache roots')
        self.host = self.seal['host']
        self._temporary = None

    def named(self, path):
        need(str(path) in self.index, 'missing sealed evidence path: ' + str(path))
        return clone(self.index[str(path)])

    def raw(self, bound):
        tick()
        if type(bound) is str or isinstance(bound, (Path, PurePosixPath, PureWindowsPath)):
            bound = self.named(str(bound))
        exact_identity(bound)
        need(self.index.get(bound['path']) == bound and bound['path'] in self.mapping,
             'missing/full-only/altered raw evidence')
        raw = self.payloads[self.mapping[bound['path']]['member']]
        need(len(raw) == bound['bytes'] and sha(raw) == bound['sha256'], 'changed raw evidence')
        return raw

    def json(self, bound):
        return decode(self.raw(bound))

    def rows(self, bound, compressed=False):
        return _json_lines(self.raw(bound), compressed=compressed)

    def local_path(self, original):
        """Explicit read-only transport projection; never an original root."""
        original = str(original)
        if self._temporary is None:
            self._temporary = tempfile.TemporaryDirectory(prefix='unit4-ci-transport-')
            root = Path(self._temporary.name)
            for row in self.manifest['members']:
                tick()
                path = root / safe_member(row['member'])
                path.parent.mkdir(parents=True, exist_ok=True)
                with path.open('xb') as output: output.write(self.raw(row['identity']))
                path.chmod(0o444)
        root = Path(self._temporary.name)
        return root / safe_member(member_name(original))

    def close(self):
        if self._temporary is not None:
            self._temporary.cleanup()
            self._temporary = None


def _read_archive(path):
    raw = _bytes(path, COMPACT_LIMIT)
    payloads, manifest, total = {}, None, 0
    decoder = lzma.LZMADecompressor(format=lzma.FORMAT_XZ, memlimit=256 * 1024 ** 2)
    decoded, decoded_bytes = [], 0
    for offset in range(0, len(raw), 65536):
        tick(); pending = raw[offset:offset + 65536]
        while True:
            block = decoder.decompress(pending, max_length=1024 ** 2); pending = b''
            decoded_bytes += len(block); tick()
            need(decoded_bytes <= DECODED_LIMIT, 'decoded capsule archive exceeds768MiB')
            decoded.append(block)
            if decoder.eof:
                need(not decoder.unused_data and offset + 65536 >= len(raw), 'extra compressed stream or trailing bytes')
                break
            if decoder.needs_input: break
    need(decoder.eof, 'truncated compact compression stream')
    with tarfile.open(fileobj=_BoundedIO(io.BytesIO(b''.join(decoded))), mode='r:') as archive:
        names = set()
        for item in archive:
            tick()
            safe_member(item.name)
            need(item.isreg() and not item.issym() and not item.islnk() and item.name not in names,
                 'nonregular or duplicate archive member')
            names.add(item.name)
            need(len(names) <= MAX_MEMBERS + 1 and 0 <= item.size <= MAX_MEMBER, 'archive member bound')
            total += item.size
            need(total <= DECODED_LIMIT, 'decoded capsule exceeds768MiB')
            parts = []; remaining = item.size
            source = archive.extractfile(item)
            while remaining:
                tick(); block = source.read(min(remaining, 1024 ** 2))
                need(block, 'truncated archive payload'); parts.append(block); remaining -= len(block)
            body = b''.join(parts)
            need(len(body) == item.size, 'truncated archive member')
            if item.name == 'capsule.json': manifest = decode(body)
            else: payloads[item.name] = body
    need(manifest is not None, 'capsule manifest missing')
    return Transport(manifest, payloads)


def read_capsule(path, expected_context, *, deadline=None):
    maximum = time.monotonic() + 2700
    deadline = maximum if deadline is None else deadline
    need(deadline <= maximum, 'join capsule deadline exceeds fixed45-minute phase')
    with budget(deadline):
        return _read_capsule(path, expected_context)


def _read_capsule(path, expected_context):
    q = _common()
    expected = q.verify_join_context(expected_context)
    _activation(expected)
    q.require_activation(expected_context)
    transport = _read_archive(_local(path))
    try:
        join_api = q.load_sibling('join')
        transport.verification = join_api.verify_transport(expected_context, transport)
        need(q.verify_join_context(expected_context) == expected, 'join expectations changed during raw replay')
        return transport
    except BaseException as error:
        try: transport.close()
        except BaseException as preservation: error.add_note('transport cleanup failed: ' + str(preservation)[:1000])
        raise


def export_failure(ci_context, fresh_directory, *, original_error, deadline):
    """Bounded available full evidence only. This path never issues a LiveSeal.

    A failure can invalidate normal readmission, so the original registered CI
    document chooses the owned roots. Any readback failure remains explicit.
    No cache is discarded on this path: an incomplete build may not yet have
    identified its executable artifacts safely.
    """
    result = {'schema': 'oxid-package-unit4-ci-failure-export-v1', 'status': 'failed',
              'original_error': clone(original_error), 'qualification_complete': False,
              'ci_runtime_proof': False, 'archive': None, 'preservation_error': None}
    root = None
    try:
        q = _common()
        need(type(original_error) is dict and set(original_error) == {'type', 'message'} and
             all(type(v) is str for v in original_error.values()), 'closed original failure required')
        ci = q._record(ci_context)
        need(type(ci_context) is q.CIContext, 'original producer CI context required for failure preservation')
        seconds = ci['activation']['failure_preservation_seconds']
        need(type(seconds) is int and seconds > 0 and deadline <= time.monotonic() + seconds,
             'failure export exceeds original trusted preservation reservation')
        with budget(deadline):
            root = _local(fresh_directory, False)
            need(root == Path(ci['outer_root']) / 'failure-evidence' and root.parent.is_dir() and
                 not os.path.lexists(root), 'fresh fixed failure-evidence destination')
            root.mkdir()
            roots = [Path(ci['producer_root']), Path(ci['outer_root']) / 'staging']
            def snapshot():
                files, links, directories, total = {}, [], set(), 0
                for source in roots:
                    if not os.path.lexists(source): continue
                    _local(source); need(source.is_dir(), 'failure output root is not a directory')
                    pending = [source]
                    while pending:
                        tick(); directory = pending.pop()
                        with os.scandir(directory) as entries:
                            for entry in entries:
                                tick(); path = directory / entry.name
                                need(len(files) + len(links) + len(directories) < MAX_MEMBERS, 'failure evidence member bound')
                                if entry.is_symlink():
                                    links.append({'path': str(path), 'target': os.readlink(path)})
                                elif entry.is_dir(follow_symlinks=False):
                                    directories.add(str(path)); pending.append(path)
                                else:
                                    need(entry.is_file(follow_symlinks=False), 'failure special file cannot be archived safely')
                                    row = identity(path); total += row['bytes']
                                    need(total <= FULL_LIMIT and row['bytes'] <= MAX_MEMBER, 'failure full evidence byte bound')
                                    files[str(path)] = {'identity': row, 'role': 'available-failed-run-evidence', 'storage': 'full-only'}
                for name in ('ci-host-stages.json',):
                    path = Path(ci['outer_root']) / name
                    if path.exists(): files[str(path)] = {'identity': identity(path), 'role': 'failed-gate-journal', 'storage': 'full-only'}
                return {'files': [files[k] for k in sorted(files)], 'links': sorted(links, key=lambda r: r['path']),
                        'directories': sorted(directories)}
            before = snapshot()
            manifest = {'schema': 'oxid-package-unit4-ci-failed-full-evidence-v1', 'status': 'failed',
                        'original_error': clone(original_error), 'ci_context': ci, 'available': before,
                        'qualification_complete': False, 'ci_runtime_proof': False}
            _tar(root / 'failed-full.tar.xz', manifest, before['files'], compact=False)
            need(snapshot() == before, 'available failure evidence changed during archive')
            result.update(status='preserved', archive=identity(root / 'failed-full.tar.xz'))
            tick()
    except BaseException as error:
        result['status'] = 'failed'
        result['preservation_error'] = {'type': type(error).__name__, 'message': str(error)[:4096]}
    if root is not None:
        try:
            raw = canonical(result)
            need(len(raw) < 65536, 'bounded failure export outcome')
            with (root / 'failure-export.json').open('xb') as output: output.write(raw)
        except BaseException as error:
            result['status'] = 'failed'
            result['outcome_write_error'] = {'type': type(error).__name__, 'message': str(error)[:1000]}
    return result
