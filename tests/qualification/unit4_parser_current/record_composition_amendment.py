"""Sealed six-case current parser expectations; no observation transformation."""
import copy
import hashlib
import json
from pathlib import Path

DATA_SHA = 'f0ec986bb6192df43e6e1248d08ba26df6b1eb8efc920222044b981974082c25'
CASE_IDS = tuple('retained-migration-' + family + '-scalar-' + style
                 for family in ('qualified', 'relative')
                 for style in ('plain', 'unicode-lf', 'unicode-crlf'))


def require(value, message):
    if not value:
        raise ValueError(message)


def canonical(value):
    return json.dumps(value, ensure_ascii=False, sort_keys=True,
                      separators=(',', ':'), allow_nan=False).encode('utf-8')


def digest(raw):
    return hashlib.sha256(raw).hexdigest()


def apply(effective, predecessor_receipt, data=None):
    """Replace only six diagnostics_exact predicates after exact predecessor admission."""
    raw = (Path(__file__).with_name('record-composition-diagnostics-v1.json').read_bytes()
           if data is None else data)
    require(digest(raw) == DATA_SHA, 'unapproved composition parser amendment')
    amendment = json.loads(raw)
    require(amendment['schema'] == 'oxid-owned-record-composition-parser-amendment-v1'
            and amendment['amendment_id'] == 'owned-record-composition-parser-v1'
            and amendment['case_ids'] == list(CASE_IDS)
            and [row['id'] for row in amendment['cases']] == list(CASE_IDS),
            'composition parser amendment scope differs')
    require(digest(canonical(effective)) == amendment['predecessor_document_canonical_sha256'],
            'composition parser predecessor document differs')
    require(canonical(predecessor_receipt) == canonical(amendment['predecessor_effective_receipt']),
            'composition parser predecessor receipt differs')
    result = copy.deepcopy(effective)
    for row in amendment['cases']:
        candidates = [case for case in result['cases'] if case['id'] == row['id']]
        require(len(candidates) == 1, 'missing or duplicate composition parser case')
        case = candidates[0]
        require(case['source'] == row['source']
                and digest(canonical(case['expected'])) == row['old_expected_canonical_sha256']
                and case['expected']['diagnostics_exact'] == row['old_diagnostics_exact']
                and {key: value for key, value in case['expected'].items() if key != 'diagnostics_exact'}
                    == row['unchanged_expected']
                and 'relation_to_original' not in case['expected'],
                'composition parser source or old predicates differ')
        case['expected']['diagnostics_exact'] = copy.deepcopy(row['new_diagnostics_exact'])
    require(digest(canonical(result)) == amendment['current_document_canonical_sha256'],
            'composition parser derived document differs')
    # Exact round trip proves no case, source, mode, limit, or other predicate changed.
    restored = copy.deepcopy(result)
    for row in amendment['cases']:
        case = next(case for case in restored['cases'] if case['id'] == row['id'])
        case['expected']['diagnostics_exact'] = copy.deepcopy(row['old_diagnostics_exact'])
    require(canonical(restored) == canonical(effective), 'composition parser predicate reversal differs')
    receipt = {
        **predecessor_receipt,
        'effective_contract_identity': 'oxid-owned-record-composition-parser-v1',
        'effective_contract_descriptor_sha256': DATA_SHA,
        'ordered_amendment_sha256': [*predecessor_receipt['ordered_amendment_sha256'], DATA_SHA],
        'effective_parser_document_canonical_sha256': amendment['current_document_canonical_sha256'],
        'predecessor_effective_contract_receipt': copy.deepcopy(predecessor_receipt),
        'current_parser_amendment': {
            'amendment_id': amendment['amendment_id'], 'sha256': DATA_SHA,
            'case_ids': list(CASE_IDS), 'mode': 'ProjectCandidate',
            'changed_predicate': 'diagnostics_exact',
            'raw_observations_changed': False, 'semantic_predicate_handlers_changed': False,
        },
    }
    return result, receipt
