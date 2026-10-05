"""Exact empty-path successor for the frozen scalar-record observation view.

Nonempty projected paths are outside that historical oracle domain and rejected.
No trace, cost, result, span, source, or diagnostic is rewritten.
"""
import copy
import hashlib
import importlib.util
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
PRIOR = ROOT / 'tests/qualification/record_composition_current_runtime_view_v1/runtime_view.py'
PRIOR_SHA = 'dadc167a65c8fec86c1856e2cd3ab4220595e824c9cf2873fdd19d4d92d31e0a'
if hashlib.sha256(PRIOR.read_bytes()).hexdigest() != PRIOR_SHA:
    raise ValueError('changed frozen scalar-record view')
spec = importlib.util.spec_from_file_location('_projected_predecessor_runtime_view', PRIOR)
prior = importlib.util.module_from_spec(spec)
spec.loader.exec_module(prior)


def paths(value, reverse=False):
    result = copy.deepcopy(value)
    changed = []
    if result.get('route') == 'owned':
        for root in ('raw', 'raw_before_audit'):
            if root not in result:
                continue
            for fi, function in enumerate(result[root]['functions']):
                for li, loan in enumerate(function['loans']):
                    name = f'{root}.functions[{fi}].loans[{li}].projection'
                    if reverse:
                        prior.require('projection' not in loan, 'projection already present')
                        loan['projection'] = []
                    else:
                        prior.require(type(loan.get('projection')) is list and loan['projection'] == [],
                                      'only exact empty loan paths map to historical scalar-record view')
                        del loan['projection']
                    changed.append(name)
    return result, changed


def project_bytes(original):
    value = prior.decode(original)
    reduced, sites = paths(value)
    derived, record = prior.project_bytes(prior.canonical(reduced))
    restored = reverse_bytes(derived)
    prior.require(restored == original, 'projected view original byte round trip')
    return derived, {'version': 'projected-slice-empty-path-runtime-view-v1',
                     'original': prior.identity(original), 'derived': prior.identity(derived),
                     'mapped_sites': record['mapped_sites'], 'empty_path_sites': sites,
                     'predecessor_view': record, 'round_trip_sha256': prior.digest(restored),
                     'semantics_normalized': False}


def reverse_bytes(derived):
    record = prior.reverse_bytes(derived)
    restored, _ = paths(prior.decode(record), reverse=True)
    return prior.canonical(restored)
