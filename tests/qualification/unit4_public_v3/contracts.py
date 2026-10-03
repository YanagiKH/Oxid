#!/usr/bin/env python3
"""New Unit4-v3 source-only contract binding and exhaustive tuple inventory.

Historical helpers, expectation assets and their constants are never modified.
This module does not execute a compiler. Relocation is explicit and hash checked.
"""
import sys
sys.dont_write_bytecode = True
import gzip
import copy
import hashlib
import json
import platform
from pathlib import Path

FREEZE_SHA = '12b40d321014719805f4864f297ba3598a3c4c5ec6f416de99a0218de68f2a85'
NAMES = {
    'public': 'public-cli-contract-replacement-v3.json',
    'original': 'original-compatibility-contract-replacement-v3.json.gz',
    'predecessors': 'predecessor-public-projections-replacement-v3.json.gz',
    'lifecycle': 'lifecycle-contract-replacement-v3.json.gz',
    'guards': 'output-guards-contract-replacement-v3.json',
}
HOSTS = ('Linux x86_64', 'macOS x86_64', 'macOS arm64', 'Windows x86_64')
EXCLUDED_REASON = 'host outside frozen required_hosts'
REMOTE_REASON = 'required host unavailable on this execution host'
CAPABILITY_REASON = 'posix_regular_file_mode_denies_open_for_effective_identity unavailable'
AMENDMENT_SHA = 'e4c3b886ec2d0cf6448a1fc1d4f15bac5ee081d05e80876b13055587a421974d'
DESCRIPTOR_SHA = '68c310e4b0d1f63b3cab25839dade5f4cdfa55cecf752409314dba7311ef2eb7'
EFFECTIVE_SHA = 'bb31b5b4ddd5467170ade55e7cc81f92378570f860ff17bfa86d821feab651ec'
EFFECTIVE_FIELDS = ('effective_contract_identity', 'effective_contract_descriptor_sha256', 'base_public_freeze_sha256', 'ordered_amendment_sha256', 'effective_public_document_canonical_sha256')

class Reject(Exception):
    pass

def need(condition, message):
    if not condition:
        raise Reject(message)

def sha(data):
    return hashlib.sha256(data).hexdigest()

def load(path):
    path = Path(path)
    data = path.read_bytes()
    return json.loads(gzip.decompress(data) if path.suffix == '.gz' else data)

def save(path, value):
    Path(path).write_text(json.dumps(value, indent=2, sort_keys=True) + '\n')

def binding(path):
    path = Path(path).resolve()
    data = path.read_bytes()
    return {'path': str(path), 'bytes': len(data), 'sha256': sha(data)}

def verify(path, expected):
    data = Path(path).read_bytes()
    need(len(data) == expected['bytes'] and sha(data) == expected['sha256'], 'changed binding: ' + str(path))
    return data

def host(system=None, machine=None):
    aliases = {'Darwin': 'macOS', 'AMD64': 'x86_64', 'aarch64': 'arm64'}
    system = system or platform.system()
    machine = machine or platform.machine()
    return aliases.get(system, system) + ' ' + aliases.get(machine, machine)

class Contracts:
    def __init__(self, root, path_map=None, amendment_root=None):
        self.root = Path(root).resolve()
        self.path_map = path_map
        need(path_map is None or isinstance(path_map, dict), 'path map must be explicit object')
        self.verified = []
        self.effective_identity = None
        freeze = self.root / 'PUBLIC-FREEZE-v3.json'
        need(sha(freeze.read_bytes()) == FREEZE_SHA, 'public-v3 freeze identity')
        self.freeze = load(freeze)
        for row in self.freeze['files']:
            p = self.root / Path(row['path']).name
            data = verify(p, row)
            if 'decoded_sha256' in row:
                decoded = gzip.decompress(data)
                need(len(decoded) == row['decoded_bytes'] and sha(decoded) == row['decoded_sha256'], 'decoded contract identity')
            self.verified.append(binding(p))
        for key in ('generator', 'validator'):
            self.verify_external(self.freeze[key])
        self.tables = {name: load(self.root / filename) for name, filename in NAMES.items()}
        for row in load(self.root / 'authority-bindings-replacement-v3.json')['bindings']:
            self.verify_external(row)
        self.verify_external(self.tables['original']['authority'])
        self.verify_external(self.tables['lifecycle']['source_authority'])
        for row in self.tables['guards']['source_authorities']:
            self.verify_external(row)
        self.verify_external(self.tables['predecessors']['source_only_retained_adapter_basis'])
        pilots = load(self.root / 'pilot-semantic-derivations-v3.json')
        self.verify_external(pilots['authority'])
        self.verify_external(pilots['source_transport'])
        self.cases = {}
        for section in ('public', 'original'):
            table = self.tables[section]
            need(table['profiles'] == ['debug', 'release'], 'contract profile roster')
            need(tuple(table['declared_hosts']) == HOSTS, 'contract host universe')
            for group in ('literal_cases', 'inherited_negative_and_route_cases'):
                for case in table[group]:
                    need(case['id'] not in self.cases, 'duplicate source case')
                    self.cases[case['id']] = case
                    root = self.root / case['source_root']
                    declared = {s['path'] for s in case['source_files']}
                    need(len(declared) == len(case['source_files']), 'duplicate source path')
                    actual = {p.relative_to(root).as_posix() for p in root.rglob('*') if p.is_file()}
                    need(actual == declared, 'source map missing/extra files: ' + case['id'])
                    for row in case['source_files']:
                        p = self.safe_path(root, row['path'])
                        verify(p, row)
        counts = self.freeze['counts']
        need(len(self.tables['original']['literal_cases']) == counts['original_compatibility_cases'] == 31, 'original cases')
        need(sum(len(c['observations']) for c in self.tables['original']['literal_cases']) == 124, 'original streams')
        need(len(self.tables['public']['literal_cases']) == counts['literal_activation_cases'] == 15, 'literal cases')
        need(len(self.tables['public']['inherited_negative_and_route_cases']) == counts['inherited_negative_route_host_cases'] == 26, 'inherited cases')
        need(len(self.tables['lifecycle']['heldout_cases']) == counts['heldout_lifecycle_cases'] == 15, 'heldout cases')
        need(len(self.tables['guards']['occupied_controls']) == counts['occupied_output_controls'] == 10, 'occupied cases')
        for family, count in (('Unit2', 3603), ('Unit3', 152)):
            need(sum(r['family'] == family for r in self.tables['predecessors']['rows']) == count, 'predecessor count')

        if amendment_root is not None:
            self.apply_amendment(amendment_root)

    def apply_amendment(self, root):
        need(self.effective_identity is None, 'duplicate amendment application')
        root = Path(root).resolve()
        amendment_path = root / 'artifacts/amendment.json'
        descriptor_path = root / 'artifacts/effective-contract.json'
        identity_path = root / 'receipt-identity.json'
        need(sha(amendment_path.read_bytes()) == AMENDMENT_SHA, 'amendment pin')
        need(sha(descriptor_path.read_bytes()) == DESCRIPTOR_SHA, 'effective descriptor pin')
        need(sha(identity_path.read_bytes()) == '512c38aa55693975afcbad7ecbad123a19b20df778fb3eacf22c0237808d00a9', 'effective receipt identity pin')
        amendment, descriptor = load(amendment_path), load(descriptor_path)
        canonical = lambda value: json.dumps(value, ensure_ascii=False, sort_keys=True, separators=(',', ':'), allow_nan=False).encode('utf-8')
        need(amendment['base_public_freeze_sha256'] == descriptor['base_public_freeze_sha256'] == FREEZE_SHA, 'amendment base freeze')
        verify(self.root / NAMES['public'], amendment['base_document'])
        need(descriptor['base_public_document_sha256'] == amendment['base_document']['sha256'], 'descriptor raw base binding')
        need(descriptor['ordered_amendments'] == [{'bytes': 2100, 'file': 'amendment.json', 'sha256': AMENDMENT_SHA}], 'additional or reordered amendments')
        need(amendment['case_pointer'] == '/inherited_negative_and_route_cases/13', 'amendment exact case index')
        case = self.tables['public']['inherited_negative_and_route_cases'][13]
        need(case['id'] == amendment['case_id'] == 'host-malformed-root-first', 'amendment exact case ID')
        need(sha(canonical(case)) == amendment['old_case_canonical_sha256'], 'amendment old whole case')
        diagnostic = case['expected_public_projection']['first_diagnostic']
        need(sha(canonical(diagnostic)) == amendment['old_diagnostic_canonical_sha256'], 'amendment old diagnostic subtree')
        verify(self.root / amendment['source']['path'], amendment['source'])
        operation = amendment['operation']
        need(operation['op'] == 'replace' and operation['pointer'] == amendment['case_pointer'] + '/expected_public_projection/first_diagnostic/location', 'amendment only permitted operation')
        need(diagnostic['location'] == operation['old_value'], 'amendment stale old location')
        changed = {key for key in operation['old_value'] if operation['old_value'][key] != operation['value'].get(key)}
        need(set(operation['old_value']) == set(operation['value']) and changed == {'start', 'end', 'scalar_column', 'end_scalar_column'}, 'amendment unlisted leaf changes')
        diagnostic['location'] = copy.deepcopy(operation['value'])
        need(sha(canonical(diagnostic)) == amendment['new_diagnostic_canonical_sha256'] and sha(canonical(case)) == amendment['new_case_canonical_sha256'], 'amendment new subtree identities')
        effective = canonical(self.tables['public'])
        need(len(effective) == descriptor['effective_public_document_canonical_bytes'] and sha(effective) == descriptor['effective_public_document_canonical_sha256'] == EFFECTIVE_SHA, 'whole effective document identity')
        identity = {'effective_contract_identity': 'oxid-unit4-public-v3-location-amendment-v1', 'effective_contract_descriptor_sha256': DESCRIPTOR_SHA, 'base_public_freeze_sha256': FREEZE_SHA, 'ordered_amendment_sha256': [AMENDMENT_SHA], 'effective_public_document_canonical_sha256': EFFECTIVE_SHA}
        need(load(identity_path) == identity and descriptor['identity'] == identity['effective_contract_identity'], 'effective receipt identity values')
        self.effective_identity = identity
        self.verified.extend([binding(amendment_path), binding(descriptor_path), binding(identity_path)])

    @staticmethod
    def safe_path(root, relative):
        p = Path(relative)
        need(not p.is_absolute() and '..' not in p.parts, 'unsafe relative path')
        dest = Path(root) / p
        need(not dest.is_symlink(), 'symlink source')
        return dest

    def resolve(self, original):
        if self.path_map is not None:
            need(str(original) in self.path_map, 'missing immutable path-map member: ' + str(original))
            return Path(self.path_map[str(original)])
        return Path(original)

    def verify_external(self, row):
        path = self.resolve(row['path'])
        verify(path, row)
        self.verified.append({'authority_path': row['path'], **binding(path)})
        return path

    def source_bytes(self, case):
        return {row['path']: verify(self.safe_path(self.root / case['source_root'], row['path']), row)
                for row in case['source_files']}

    def roster(self, section, actual_host=None):
        actual_host = actual_host or host()
        need(actual_host in HOSTS, 'execution host outside declared qualification universe: ' + actual_host)
        table = self.tables[section]
        rows = []
        def append(group, case, index, obs, profiles=None):
            argv = obs['argv']
            required = obs.get('required_hosts', case.get('required_hosts'))
            need(isinstance(required, list) and required and len(set(required)) == len(required) and set(required) <= set(HOSTS), 'invalid explicit host scope')
            profiles = obs.get('profiles', profiles or case.get('profiles', table.get('profiles', ['debug', 'release'])))
            need(isinstance(profiles, list) and profiles and len(set(profiles)) == len(profiles) and set(profiles) <= {'debug', 'release'}, 'invalid profile scope')
            for profile in profiles:
                for canonical_host in HOSTS:
                    scope = 'excluded-host' if canonical_host not in required else ('execute' if canonical_host == actual_host else 'unavailable-host')
                    key = json.dumps([table['identity'], group, case['id'], index, profile, canonical_host], separators=(',', ':'))
                    rows.append({'key': key, 'contract_identity': table['identity'], 'group': group, 'case_id': case['id'],
                                 'observation_index': index, 'profile': profile, 'host': canonical_host,
                                 'execution_host': actual_host, 'required_hosts': required, 'scope': scope,
                                 'format': obs.get('format', 'json' if any(a in ('--message-format=json', '--message-format') for a in argv) else 'text'),
                                 'argv_template': argv, 'required_capabilities': obs.get('required_capabilities', case.get('required_capabilities', [])),
                                 'case': case, 'observation': obs})
        if section in ('public', 'original'):
            for group in ('literal_cases', 'inherited_negative_and_route_cases'):
                for case in table[group]:
                    for index, obs in enumerate(case.get('observations', case.get('operations'))):
                        append(group, case, index, obs)
        elif section == 'predecessors':
            for case in table['rows']:
                for index, op in enumerate(case['operations']):
                    append(case['family'], case, index, {'argv': ['{oxid}', op, case['entry'], '--edition', 'typed-preview', '--message-format=json']})
        elif section == 'lifecycle':
            for case in table['public_cases']:
                for obs in case['observations']:
                    append('public', case, obs['observation_index'], obs)
            for case in table['heldout_cases']:
                append('heldout', case, 0, {'argv': ['{oxid}'] + case['argv']})
        elif section == 'guards':
            for case in table['occupied_controls']:
                args = ['{oxid}', 'compile', case['entry_template'], '--edition', 'typed-preview', '--backend', 'llvm', '--output', case['output_template']]
                if case['format'] == 'json':
                    args.append('--message-format=json')
                append('occupied', case, 0, {'argv': args, 'format': case['format']})
            case = table['concurrent_publication']
            append('race', case, 0, {'argv': ['{oxid}', 'compile', 'main.ox', '--edition', 'typed-preview', '--backend', 'llvm', '--output', 'out.bin', '--message-format=json']})
        if self.effective_identity is not None:
            for row in rows:
                row.update(copy.deepcopy(self.effective_identity))
        need(rows and len({r['key'] for r in rows}) == len(rows), 'empty/duplicate frozen tuple roster')
        return rows

IDENTITY_FIELDS = ('key', 'contract_identity', 'group', 'case_id', 'observation_index', 'profile', 'host', 'execution_host', 'required_hosts', 'scope', 'format', 'argv_template', 'required_capabilities')

def receipt_identity(row):
    extra = EFFECTIVE_FIELDS if any(key in row for key in EFFECTIVE_FIELDS) else ()
    need(not extra or all(key in row for key in EFFECTIVE_FIELDS), 'incomplete effective authority identity')
    return {key: row[key] for key in IDENTITY_FIELDS + extra}

def check_inventory(rows, receipts, allow_capability_gap=True):
    need(rows and receipts, 'zero-execution inventory')
    keys = [r['key'] for r in receipts]
    need(len(keys) == len(set(keys)), 'duplicate receipt tuple')
    need(set(keys) == {r['key'] for r in rows}, 'missing/extra receipt tuple')
    lookup = {r['key']: r for r in receipts}
    executed = 0
    for row in rows:
        got = lookup[row['key']]
        need(all(got.get(k) == value for k, value in receipt_identity(row).items()), 'receipt tuple identity: ' + row['key'])
        if row['scope'] != 'execute':
            reason = EXCLUDED_REASON if row['scope'] == 'excluded-host' else REMOTE_REASON
            need(got['executed'] is False and got['status'] == row['scope'] and got['reason'] == reason and 'process' not in got, 'incorrect host exclusion')
        elif got['status'] == 'unsupported-capability':
            proof = got['capability_proof']
            need(allow_capability_gap and row['required_capabilities'] == ['posix_regular_file_mode_denies_open_for_effective_identity'], 'false capability skip')
            need(got['executed'] is False and got['reason'] == CAPABILITY_REASON and proof['read_open_succeeded'] is True and proof['mode_before_candidate'] == 0 and proof['source_restored'] is True, 'unsupported capability proof')
            need('process' not in got, 'unsupported capability executed')
        else:
            need(got['executed'] is True and got['status'] in ('pass', 'fail') and 'reason' not in got, 'false skip')
            executed += 1
    required_local = sum(row['scope'] == 'execute' for row in rows)
    need(executed > 0 or required_local == 0, 'no applicable rows executed')
    return {'executed': executed, 'required_local': required_local, 'excluded_host': sum(r['status'] == 'excluded-host' for r in receipts),
            'unavailable_host': sum(r['status'] == 'unavailable-host' for r in receipts),
            'unsupported_capability': sum(r['status'] == 'unsupported-capability' for r in receipts)}
