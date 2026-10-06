#!/usr/bin/env python3
"""Closed four-row enum Enabled successor; never rewrites frozen observations."""
import base64
import copy
import gzip
import hashlib
import json
from pathlib import Path

IDENTITY = 'enum-enabled-qualified-values-v1'
DESCRIPTOR_SHA = '18dc17439c6ccc0e76f8a6bbbae5707c7ad826159292e2c548da8fc44280886f'
CASE_IDS = (
    'parser/bare-relative-prefix',
    'parser/qualified-function-value',
    'parser/self-relative-prefix',
    'parser/super-relative-prefix',
)


def need(condition, message):
    if not condition:
        raise ValueError(IDENTITY + ': ' + message)


def sha(data):
    return hashlib.sha256(data).hexdigest()


def canonical(value):
    return json.dumps(value, ensure_ascii=False, sort_keys=True,
                      separators=(',', ':'), allow_nan=False).encode('utf-8')


def bound_bytes(path, expected, label):
    path = Path(path)
    need(path.is_file() and not path.is_symlink(), label + ' regular file')
    data = path.read_bytes()
    need(len(data) == expected['bytes'] and sha(data) == expected['sha256'], label + ' identity')
    return data


class Amendment:
    def __init__(self, source_manifest, corpus_path, descriptor_path=None):
        descriptor_path = Path(descriptor_path) if descriptor_path else Path(__file__).with_name(IDENTITY + '.json')
        raw = descriptor_path.read_bytes()
        need(not descriptor_path.is_symlink() and sha(raw) == DESCRIPTOR_SHA, 'descriptor pin')
        self.descriptor = json.loads(raw)
        need(self.descriptor['schema'] == 'oxid-' + IDENTITY and self.descriptor['identity'] == IDENTITY,
             'descriptor identity')
        need(tuple(row['id'] for row in self.descriptor['rows']) == CASE_IDS, 'exact four-row roster')
        self.rows = {row['id']: row for row in self.descriptor['rows']}
        manifest = json.loads(bound_bytes(source_manifest, self.descriptor['source_manifest'], 'source manifest'))
        need(manifest['reviewed_source_head'] == self.descriptor['source_manifest']['reviewed_source_head']
             and len(manifest['files']) == self.descriptor['source_manifest']['members'], 'source checkpoint')
        corpus = bound_bytes(corpus_path, self.descriptor['unit2_corpus'], 'frozen Unit2 corpus')
        self.corpus = {}
        for line in gzip.decompress(corpus).splitlines(keepends=True):
            row = json.loads(line)
            case_id = row['id']
            need(case_id not in self.corpus, 'duplicate corpus row')
            self.corpus[case_id] = row
            if case_id in self.rows:
                need(sha(line) == self.rows[case_id]['frozen_line_sha256'], 'frozen line identity')
                self._unit2_row(row, row['expected'])
        need(len(self.corpus) == self.descriptor['unit2_corpus']['rows']
             and all(case_id in self.corpus for case_id in CASE_IDS), 'complete frozen corpus')
        self._unit2_seen = set()

    def _unit2_row(self, corpus_row, expected):
        case_id = corpus_row['id']
        need(case_id in self.rows, 'unapproved amended case')
        row = self.rows[case_id]
        need(sha(canonical(corpus_row)) == row['frozen_row_canonical_sha256'], 'frozen whole row identity')
        need(sha(canonical(expected)) == row['frozen_expected_canonical_sha256']
             and expected == row['old_unit2_expected'], 'frozen expectation identity')
        need(corpus_row['cohort'] == 'parser' and corpus_row['mode'] == 'project-parser', 'loader-only route')
        source = row['source']
        data = source['text'].encode('utf-8')
        need(len(data) == source['bytes'] and sha(data) == source['sha256'], 'literal source pin')
        need(len(corpus_row['sources']) == 1
             and base64.b64decode(corpus_row['sources'][0]['base64'], validate=True) == data, 'literal source bytes')
        return row

    def receipt(self):
        return {
            'identity': IDENTITY,
            'descriptor_sha256': DESCRIPTOR_SHA,
            'source_manifest': copy.deepcopy(self.descriptor['source_manifest']),
            'unit2_corpus': copy.deepcopy(self.descriptor['unit2_corpus']),
            'base_public_freeze_sha256': self.descriptor['base_public_freeze_sha256'],
            'public_predecessors': copy.deepcopy(self.descriptor['public_predecessors']),
            'cases': list(CASE_IDS),
            'frozen_authorities_unchanged': True,
            'observations_unchanged': True,
            'historical_qualification_claim': False,
        }

    def project_unit2(self, corpus_row, expected, item, failures):
        """The copied parser comparator calls this after its expected deepcopy."""
        case_id = item['case']
        if case_id not in self.rows:
            need(corpus_row['id'] == case_id and case_id in self.corpus
                 and corpus_row == self.corpus[case_id]
                 and canonical(expected) == canonical(corpus_row['expected']), 'unchanged corpus row identity')
            return expected
        need(corpus_row['id'] == case_id, 'observation/corpus case identity')
        row = self._unit2_row(corpus_row, expected)
        need(case_id not in self._unit2_seen, 'duplicate amended observation')
        self._unit2_seen.add(case_id)
        wanted = row['current_unit2_result']
        actual = item['result']
        if set(actual) != set(wanted):
            failures.append({'at': IDENTITY + '/result_fields', 'expected': sorted(wanted), 'actual': sorted(actual)})
        for field, value in wanted.items():
            if field not in actual or canonical(actual[field]) != canonical(value):
                failures.append({'at': IDENTITY + '/' + field, 'expected': value, 'actual': actual.get(field)})
        if item.get('mode') != 'project-parser':
            failures.append({'at': IDENTITY + '/mode', 'expected': 'project-parser', 'actual': item.get('mode')})
        reads = item.get('program_read_source_entries')
        display_files = None if not isinstance(reads, list) else [entry.get('display_file') for entry in reads]
        if display_files != ['root.ox']:
            failures.append({'at': IDENTITY + '/source_reads', 'expected': ['root.ox'], 'actual': display_files})
        return copy.deepcopy(row['current_unit2_expected'])

    def unit2_report(self, results):
        need(self._unit2_seen == set(CASE_IDS), 'all four amendments applied exactly once')
        ids = [row['case'] for row in results]
        need(len(ids) == len(set(ids)) == len(self.corpus) and set(ids) == set(self.corpus),
             'complete unique comparison roster')
        return {**self.receipt(), 'rows': [
            {'case': case_id,
             'frozen_expected_canonical_sha256': self.rows[case_id]['frozen_expected_canonical_sha256'],
             'old_expected': copy.deepcopy(self.rows[case_id]['old_unit2_expected']),
             'current_expected': copy.deepcopy(self.rows[case_id]['current_unit2_expected']),
             'current_result': copy.deepcopy(self.rows[case_id]['current_unit2_result'])}
            for case_id in CASE_IDS]}

    def admit_public_rows(self, rows):
        selected = [row for row in rows if row['family'] == 'Unit2' and row['id'] in self.rows]
        need(len(selected) == len(CASE_IDS) and {row['id'] for row in selected} == set(CASE_IDS),
             'exact public amendment roster')
        for row in selected:
            need(sha(canonical(row)) == self.rows[row['id']]['public_row_canonical_sha256'],
                 'frozen whole public row identity')

    def public_comparison(self, tuple_row, corpus_row, expected, sources, status, diagnostics):
        """Compare only the frozen public status/first-code-stage claim domain."""
        case = tuple_row['case']
        if case['id'] not in self.rows:
            return None
        need(case['family'] == 'Unit2', 'public predecessor family')
        row = self._unit2_row(corpus_row, expected)
        need(sha(canonical(case)) == row['public_row_canonical_sha256'], 'frozen whole public row identity')
        need(tuple_row['profile'] in ('debug', 'release') and tuple_row['host'] == 'Linux x86_64'
             and tuple_row['execution_host'] == 'Linux x86_64' and tuple_row['scope'] == 'execute', 'public profile/host')
        need(tuple_row['argv_template'] == ['{oxid}', 'check', 'root.ox', '--edition', 'typed-preview', '--message-format=json'],
             'public check route')
        need(sources == {'root.ox': row['source']['text'].encode('utf-8')}, 'public literal source')
        need(type(status) is int, 'public process status type')
        first = {key: diagnostics[0][key] for key in ('code', 'stage')} if diagnostics else None
        observed = {'status': status, 'first_diagnostic': first}
        return {
            'identity': IDENTITY, 'descriptor_sha256': DESCRIPTOR_SHA,
            'case': case['id'], 'profile': tuple_row['profile'], 'host': tuple_row['host'],
            'frozen_expected_projection_sha256': row['frozen_expected_canonical_sha256'],
            'frozen_public_row_canonical_sha256': row['public_row_canonical_sha256'],
            'observed_projection': observed,
            'historical': {'expected': copy.deepcopy(row['old_public_expected']),
                           'status': 'match' if observed == row['old_public_expected'] else 'mismatch'},
            'current': {'expected': copy.deepcopy(row['current_public_expected']),
                        'status': 'match' if observed == row['current_public_expected'] else 'mismatch'},
        }
