#!/usr/bin/env python3
"""Frozen public projections with one explicit, source-bound enum successor."""
import sys
sys.dont_write_bytecode = True
import base64
import copy
import gzip
import importlib.util
import io
import json
import os
import shutil
import subprocess
import sys
import tarfile
from pathlib import Path
from contracts import need, sha, load, binding, verify, save
from compare import envelope, diagnostic_vector, valid_origins, policy_bounds
from authority import CURRENT_SOURCE_SHA, ENUM_SOURCE_SHA

QUALIFIED_PATHS_HELPER_SHA = '999e9f8cd75ae2010a11d20a40c357bf28d42b658d6293e93cb73a36cb665288'

def authority_path(contracts, suffix):
    matches = list({r['path']: r for r in contracts.verified if r.get('authority_path', '').endswith(suffix)}.values())
    need(len(matches) == 1, 'authority suffix not unique: ' + suffix)
    return Path(matches[0]['path'])

def original_manifest_transport(raw, expected, platform_name):
    """Only the generator's JSON text-file newline transport can be projected."""
    need(platform_name in ('nt', 'posix'), 'unknown manifest writer platform')
    projected = raw
    if platform_name == 'nt':
        projected = raw.replace(b'\r\n', b'\n')
        need(b'\r' not in projected, 'unexpected lone CR in generated intent transport')
    need(projected == expected, 'generated complete original intent differs')
    return projected

def original_generator_check(contracts, output, repository):
    generator = authority_path(contracts, '/typed_project_unit1_compatibility/generate_fixtures.py')
    package_root = Path(repository) / 'tests/fixtures/typed_project_unit1_compatibility'
    manifest_path = package_root / 'package-manifest.json'
    need(sha(manifest_path.read_bytes()) == '0f331a077a7e67b8e8943dfd463273f61ebbb64f08e2a60e0545b391479d3215', 'unchanged Unit1 compatibility package manifest')
    intent_path = authority_path(contracts, '/typed_project_unit1_compatibility/suite-intent.json')
    package = load(manifest_path)
    for name, row in package['files'].items():
        verify(package_root / name, row)
    output.mkdir()
    (output / 'fixtures').mkdir()
    shutil.copy2(generator, output / generator.name)
    p = subprocess.run([sys.executable, str(output / generator.name)], stdout=subprocess.PIPE, stderr=subprocess.PIPE, timeout=60)
    need(p.returncode == 0 and not p.stderr, 'original source generator failed')
    raw_intent = output / 'suite-intent.json'
    projected_intent = output / 'suite-intent.frozen-newline.json'
    projected_intent.write_bytes(original_manifest_transport(raw_intent.read_bytes(), intent_path.read_bytes(), os.name))
    intent = load(intent_path)
    need(len(intent['cases']) == 35, 'complete original fixture count')
    for case in intent['cases']:
        verify(output / case['path'], case)
    need({p.relative_to(output).as_posix() for p in (output / 'fixtures').rglob('*') if p.is_file()} == {c['path'] for c in intent['cases']}, 'complete original source membership')
    for case in contracts.tables['original']['literal_cases']:
        for source in case['source_files']:
            need((output / source['path']).read_bytes() == contracts.source_bytes(case)[source['path']], 'original generated source binding')
    receipt = {'generator': binding(generator), 'package_manifest': binding(manifest_path), 'intent': binding(intent_path),
               'sources': [{'path': c['path'], 'bytes': c['bytes'], 'sha256': c['sha256']} for c in intent['cases']],
               'manifest_newline_transport': {'platform': os.name, 'raw_generated': binding(raw_intent), 'frozen_newline_projection': binding(projected_intent), 'scope': 'Only generated suite-intent JSON platform newlines; no compiler stream/source normalization'},
               'compiler_invocations': 0, 'status': 'SOURCE_IDENTITIES_VERIFIED'}
    save(output / 'verification.json', receipt)
    return receipt

class Predecessors:
    def __init__(self, contracts, source_manifest=None, amendment_root=None):
        self._qualified_paths = None
        self.qualified_paths_amendment = None
        need((source_manifest is None) == (amendment_root is None), 'enum amendment needs explicit source and authority')
        u2 = authority_path(contracts, '/typed_project_unit2_independent/semantic/corpus.jsonl.gz')
        u3 = authority_path(contracts, '/typed_project_unit3_independent/components/oracles/expected.jsonl.gz')
        requests = authority_path(contracts, '/typed_project_unit3_independent/components/oracles/requests.jsonl')
        archive = authority_path(contracts, '/typed_project_unit3_independent/source-transport/sources.tar.gz')
        self.data = {}
        self.lines = {}
        for family, path in (('Unit2', u2), ('Unit3', u3)):
            for line in gzip.open(path, 'rb'):
                d = json.loads(line)
                key = family, d['id']
                need(key not in self.data, 'duplicate predecessor row')
                self.data[key], self.lines[key] = d, sha(line)
        reqs = {r['id']: r for r in (json.loads(line) for line in requests.read_text().splitlines())}
        tar_data = gzip.decompress(archive.read_bytes())
        need(len(tar_data) <= 32 * 1024 * 1024, 'source archive expansion limit')
        payloads = {}
        with tarfile.open(fileobj=io.BytesIO(tar_data), mode='r:') as tar:
            for member in tar:
                path = Path(member.name)
                need(member.isreg() and not path.is_absolute() and '..' not in path.parts and member.name not in payloads, 'source transport unsafe member')
                need(member.size <= 1048576, 'source transport member bound')
                payloads[member.name] = tar.extractfile(member).read()
        self.rows = {}
        used = set()
        for row in contracts.tables['predecessors']['rows']:
            key, auth = (row['family'], row['id']), (row['family'], row['authority_id'])
            need(self.lines[auth] == row['expected_authority_line_sha256'], 'predecessor expectation line identity')
            if row['family'] == 'Unit2':
                need(self.lines[key] == row['source_line_sha256'], 'predecessor source line identity')
            expected = self.data[auth]['expected']
            need(sha(json.dumps(expected, sort_keys=True, separators=(',', ':')).encode()) == row['expected_projection_sha256'], 'predecessor expected identity')
            if row['family'] == 'Unit2':
                d = self.data[key]
                request = json.loads(base64.b64decode(d['request_base64']))
                sources = {r['path']: base64.b64decode(r['base64']) for r in d['sources']}
            else:
                request = reqs[row['id']]
                sources = {}
                for source in request['source_files']:
                    name = (Path(request['source_root']) / source['path']).as_posix()
                    used.add(name)
                    sources[source['path']] = payloads[name]
            need(request['entry'] == row['entry'], 'predecessor entry')
            need(set(sources) == {r['path'] for r in row['source_files']}, 'predecessor source membership')
            for source in row['source_files']:
                data = sources[source['path']]
                need(len(data) == source['bytes'] and sha(data) == source['sha256'], 'predecessor source bytes')
            self.rows[key] = {'sources': sources, 'expected': expected, 'corpus': self.data[key]}
        need(used == set(payloads) and len(used) == 445, 'Unit3 complete transport membership')
        prose = authority_path(contracts, '/typed_project_unit2_independent/semantic/original93-prose-expectations.json')
        self.prose = {(r['case'], r['diagnostic_index']): r['expected'] for r in load(prose)['diagnostics']}

        if amendment_root is not None:
            root = Path(amendment_root)
            execution_raw = Path(source_manifest).read_bytes()
            need(not Path(source_manifest).is_symlink() and sha(execution_raw) == CURRENT_SOURCE_SHA,
                 'public predecessor current execution source identity')
            execution = load(source_manifest)
            need(execution['enum_source_sha256'] == ENUM_SOURCE_SHA and len(execution['files']) == 252,
                 'public predecessor exact semantic source link')
            semantic_manifest = root / 'enum-source.json'
            need(not semantic_manifest.is_symlink() and sha(semantic_manifest.read_bytes()) == ENUM_SOURCE_SHA,
                 'public predecessor enum semantic source identity')
            helper = root / 'enum_enabled_qualified_values_v1.py'
            need(helper.is_file() and not helper.is_symlink() and sha(helper.read_bytes()) == QUALIFIED_PATHS_HELPER_SHA,
                 'enum qualified-path helper identity')
            spec = importlib.util.spec_from_file_location('enum_enabled_qualified_values_v1', helper)
            module = importlib.util.module_from_spec(spec)
            spec.loader.exec_module(module)
            amendment = module.Amendment(semantic_manifest, u2, root / 'enum-enabled-qualified-values-v1.json')
            module.bound_bytes(contracts.root / 'predecessor-public-projections-replacement-v3.json.gz',
                               amendment.descriptor['public_predecessors'], 'frozen public predecessors')
            amendment.admit_public_rows(contracts.tables['predecessors']['rows'])
            self._qualified_paths = amendment
            self.qualified_paths_amendment = {**amendment.receipt(), 'implementation_sha256': QUALIFIED_PATHS_HELPER_SHA,
                'execution_source_manifest': {'bytes': len(execution_raw), 'sha256': sha(execution_raw),
                    'members': len(execution['files']), 'reviewed_source_head': execution['reviewed_source_head']},
                'rows': [{'case': case_id,
                          'frozen_public_row_canonical_sha256': amendment.rows[case_id]['public_row_canonical_sha256'],
                          'frozen_expected_projection_sha256': amendment.rows[case_id]['frozen_expected_canonical_sha256'],
                          'historical_expected': copy.deepcopy(amendment.rows[case_id]['old_public_expected']),
                          'current_expected': copy.deepcopy(amendment.rows[case_id]['current_public_expected'])}
                         for case_id in module.CASE_IDS]}

    def _qualified_paths_projection(self, row, status, diagnostics):
        if self._qualified_paths is None:
            return None
        data = self.rows[row['case']['family'], row['case']['id']]
        return self._qualified_paths.public_comparison(row, data['corpus'], data['expected'], data['sources'], status, diagnostics)

    def qualified_paths_comparison(self, row, process):
        """Separate old/current projections for receipts; never alters process data."""
        if self._qualified_paths is None or row['case']['family'] != 'Unit2' or row['case']['id'] not in self._qualified_paths.rows:
            return None
        _, diagnostics = envelope(process, row['argv_template'][1])
        return self._qualified_paths_projection(row, process['status'], diagnostics)

    def compare(self, row, process, policy, *, historical=False):
        case = row['case']
        data = self.rows[case['family'], case['id']]
        expected = data['expected']
        op = row['argv_template'][1]
        summary, diagnostics = envelope(process, op)
        valid_origins(diagnostics, data['sources'])
        policy_bounds(diagnostics, policy)
        kind = case['kind']
        if case['family'] == 'Unit2':
            if kind == 'schema-and-source-only':
                return
            if kind == 'first-diagnostic-only':
                need(process['status'] == 1 and diagnostics, 'Unit2 first failure absent')
                amendment = None if historical else self._qualified_paths_projection(row, process['status'], diagnostics)
                if amendment is not None:
                    need(amendment['current']['status'] == 'match', 'Unit2 enum-enabled-qualified-values-v1 first diagnostic')
                    return amendment
                need({k: diagnostics[0][k] for k in expected['first_diagnostic']} == expected['first_diagnostic'], 'Unit2 first diagnostic')
                return
            if kind == 'front-end-prefix-only':
                blocked = {'source', 'lex', 'parse'} if 'no-source-lex-or-parse-diagnostic' in case['claims'] else {'lex', 'parse'}
                need(not any(d['stage'] in blocked for d in diagnostics), 'Unit2 frontend prefix')
                return
            need(kind == 'complete-check-projection', 'unknown Unit2 projection kind')
            wanted = copy.deepcopy(expected['diagnostics'])
            if data['corpus']['cohort'] == 'original93':
                for i, d in enumerate(wanted):
                    d.update(self.prose.get((data['corpus']['case'], i), {}))
            need(len(diagnostics) == len(wanted) and process['status'] == (1 if wanted else 0), 'Unit2 status/count')
            def span(s):
                return None if s is None else {'file': s['path'], 'start': s['start'], 'end': s['end']}
            for got, want in zip(diagnostics, wanted):
                projection = {'code': got['code'], 'stage': got['stage'], 'primary': span(got['primary']),
                              'secondary': [span(s['span']) for s in got['secondary']], 'message': got['message'],
                              'labels': [s['message'] for s in got['secondary']], 'notes': got['notes']}
                compare = {k: v for k, v in want.items() if k not in ('phase', 'order')}
                need({k: projection[k] for k in compare} == compare, 'Unit2 complete projection')
            if not wanted:
                need(summary['functions'] == len(expected['function_declarations']), 'Unit2 accepted function count')
        else:
            rejection = expected if expected.get('status') == 'reject' or expected.get('check') == 'reject' else expected.get(op)
            if isinstance(rejection, dict):
                need(process['status'] == 1, 'Unit3 expected rejection')
                wanted = rejection.get('reachable_diagnostics', [rejection])
                if kind == 'first-source-failure-check-run-projection':
                    need(diagnostics, 'Unit3 first failure absent')
                    diagnostic_vector(diagnostics[:1], wanted[:1])
                else:
                    need(kind == 'complete-check-run-projection', 'unknown Unit3 projection kind')
                    diagnostic_vector(diagnostics, wanted)
            else:
                need(process['status'] == 0 and not diagnostics, 'Unit3 expected acceptance')
                if op == 'check':
                    need(summary['functions'] == data['corpus']['function_count'], 'Unit3 functions')
                else:
                    need(summary['result'] == {'type': expected['result_type'], 'value': expected['result']}, 'Unit3 result')
