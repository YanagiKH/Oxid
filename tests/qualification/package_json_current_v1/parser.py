"""Finite two-source parser transport preserving the complete formatter stage."""
import copy
import json
from pathlib import PurePosixPath

import generation as g
import predecessor

KEY = 'package_json_parser'
OLD_AUTHORITY_SHA = '900f9e7cfb32027b4406e2860c5bc39abb0d831c89931efaf166911905c35ca4'
FIELDS = ('schema', 'reviewed_source_head', 'source_only_tree', 'current_source_manifest',
          'source_binding_runner', 'source_delta', 'current_base_files', 'current_derived_files',
          'current_control_derived_files', 'current_candidate_source_manifest_sha256')


def ordinary_sources(old, inserted_roster, accounting_paths):
    g.need(tuple(accounting_paths) == g.ACCOUNTING_PATHS, 'exact accounting roster')
    for name in g.PATHS:
        for field in ('instrumentation', 'control_instrumentation'):
            g.need(name not in {r['path'] for r in old[field]}, 'ordinary source in ' + field)
        g.need(name not in {r if isinstance(r, str) else r['path'] for r in inserted_roster},
               'ordinary source in observer inserted_roster')
        g.need(name not in accounting_paths, 'ordinary source in ACCOUNTING_PATHS')


def candidate(a):
    current = a['current']
    g.need(KEY in current, 'explicit package JSON parser generation')
    return {'schema': 'oxid-unit4-package-json-candidate-source-v1',
            'historical_commit': a['base_commit'],
            'reviewed_source_head': g.CHECKPOINT['head'], 'source_only_tree': g.CHECKPOINT['tree'],
            'current_source_manifest_sha256': current['current_source_manifest']['sha256'],
            'source_provenance': copy.deepcopy(current[KEY]['source_provenance']),
            'files': copy.deepcopy(current['current_base_files'])}


def transport(old, admitted, dispatcher, inserted_roster, stage='phase'):
    g.need(stage in ('phase', 'prephase') and KEY not in old['current'], 'exact parser stage')
    if stage == 'phase':
        g.need(g.sha(g.encoded(old['current'])) == OLD_AUTHORITY_SHA, 'unchanged accepted parser authority')
    ordinary_sources(old, inserted_roster, g.ACCOUNTING_PATHS)
    g.pin(admitted['artifacts'][g.CURRENT_NAME],
          (76328, 'd33ecc4e41975a062790540ba9e9a270d7c6043a31f8f4b9a8d7a044c79530b0'),
          'fixed complete parser successor source')
    g.need(g.encoded(admitted['current']) == admitted['artifacts'][g.CURRENT_NAME],
           'parser source capture differs from sealed bytes')
    previous = old['current_source']
    g.pin(g.encoded(previous), (74567, g.PREDECESSOR_SHA), 'genuine formatter parser source')
    before = g.rows_map(previous['files']); after = g.rows_map(admitted['current']['files'])
    g.need(list(before) == list(after) and len(after) == 376
           and [n for n in before if before[n] != after[n]] == list(g.PATHS), 'exact two parser sources')
    for name in before:
        g.need(g.identity(name, admitted['predecessor_inputs'][name]) == before[name]
               and g.identity(name, admitted['inputs'][name]) == after[name], 'complete parser admitted bytes')
    original = g.rows_map(old['original_files'])
    def delta(rows):
        return [{'path': n, 'before': original.get(n), 'after': r}
                for n, r in rows.items() if original.get(n) != r]
    g.need(delta(before) == old['current']['source_delta'], 'complete historical delta inverse')
    old_base = original | before; new_base = original | after
    g.need([old_base[n] for n in sorted(old_base)] == old['current']['current_base_files'],
           'complete parser base inverse')
    g.need(set(new_base) == set(old_base), 'base membership changed')
    current = copy.deepcopy(old['current'])
    source_raw = admitted['artifacts'][g.CURRENT_NAME]
    current.update(schema='oxid-unit4-package-json-parser-authority-v1',
                   reviewed_source_head=g.CHECKPOINT['head'], source_only_tree=g.CHECKPOINT['tree'],
                   current_source_manifest=g.identity(g.PACKAGE+'current-source.json', source_raw),
                   source_binding_runner=copy.deepcopy(dispatcher), source_delta=delta(after),
                   current_base_files=[new_base[n] for n in sorted(new_base)])
    current[KEY] = {'schema': 'oxid-package-json-parser-transport-v1', 'stage': stage,
                    'predecessor_authority_sha256': g.sha(g.encoded(old['current'])),
                    'source_provenance': copy.deepcopy(admitted['current']['package_json_source'])}
    result = dict(copy.deepcopy(old), current=current, current_source=copy.deepcopy(admitted['current']))
    raw = g.encoded(candidate(result)); current['current_candidate_source_manifest_sha256'] = g.sha(raw)
    for field in ('current_derived_files', 'current_control_derived_files'):
        rows = {r['path']: r for r in old['current'][field]}
        g.need(len(rows) == len(old['current'][field]), 'duplicate parser derived row')
        new = dict(rows)
        for name in g.PATHS:
            g.need(rows[name] == before[name], 'source unexpectedly instrumented in derived map')
            new[name] = after[name]
        new['candidate-source-manifest.json'] = g.identity('candidate-source-manifest.json', raw)
        ordered = sorted(new, key=lambda n: PurePosixPath(n).parts)
        current[field] = [new[n] for n in ordered]
        restored = [rows[n] if n in (*g.PATHS, 'candidate-source-manifest.json') else new[n] for n in ordered]
        g.need(restored == old['current'][field], 'complete parser derived map inverse')
    restored = copy.deepcopy(current); del restored[KEY]
    for field in FIELDS:
        restored[field] = old['current'][field]
    g.need(restored == old['current'], 'unrelated parser authority changed')
    return result


def _old(p, admitted, seal, operation):
    with predecessor.view(p.REPOSITORY, admitted['predecessor_inputs'], seal['predecessor_view']) as view:
        parser = predecessor.load(view, 'tests/qualification/unit4_parser_current/portable.py',
                                  'package_json_predecessor_parser')
        lifecycle = json.loads(g.regular(view, 'tests/qualification/unit4_public_v3/published-lifecycle-v1.json'))
        inserted = lifecycle['inserted_roster']
        return operation(parser, inserted)


def authority(p, admitted, seal, dispatcher):
    result = _old(p, admitted, seal, lambda old, inserted: transport(old.authority(), admitted, dispatcher, inserted))
    raw = g.regular(p.REPOSITORY, 'tests/qualification/unit4_parser_current/authority.json')
    g.need(g.sha(raw) == p.AUTHORITY_SHA and json.loads(raw) == result['current'], 'sealed parser authority')
    return result


def phase_previous(p, a, admitted, seal, dispatcher):
    def run(old, inserted):
        previous = old.authority()
        g.need(a == transport(previous, admitted, dispatcher, inserted), 'exact complete phase input')
        return transport(old.phase_previous(previous), admitted, dispatcher, inserted, 'prephase')
    return _old(p, admitted, seal, run)


def compose(p, a, raw, name, control, operation, admitted, seal, dispatcher):
    allowed = {'lexer': {('src/frontend/lexer.rs', False)},
               'array': {('src/frontend/ast.rs', False), ('src/frontend/ast.rs', True),
                         ('src/frontend/parser.rs', False), ('src/frontend/parser.rs', True),
                         ('src/frontend/project/budget.rs', False)},
               'namespace': {('src/frontend/declaration_index/resource.rs', False)},
               'source': {('src/frontend/source.rs', False)}}
    g.need(type(control) is bool and (name, control) in allowed.get(operation, set()), 'closed composer role')
    def run(old, inserted):
        previous = old.phase_previous(old.authority())
        expected = transport(previous, admitted, dispatcher, inserted, 'prephase')
        g.need(a == expected and raw == admitted['inputs'][name] == admitted['predecessor_inputs'][name],
               'composer stage and unchanged source body')
        if operation == 'lexer': result = old.compose_current_lexer(previous, raw)
        elif operation == 'array': result = old.compose_array_instrumentation(previous, name, raw, control)
        elif operation == 'namespace': result = old.compose_namespace_resource(previous, raw)
        else: result = old.compose_source_read(previous, raw)
        field = 'current_control_derived_files' if control else 'current_derived_files'
        wanted = next(r for r in expected['current'][field] if r['path'] == name)
        g.need(g.identity(name, result) == wanted, 'complete current composer result')
        return result
    return _old(p, admitted, seal, run)
