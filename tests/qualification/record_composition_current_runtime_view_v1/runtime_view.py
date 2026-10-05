#!/usr/bin/env python3
"""Reversible representation-only view of published scalar-record observations.

Never edit source observations, expectations, costs, outcomes, spans, instruction
payloads or traces. This module only unwraps four current record identity carriers
at declared raw descriptor/result sites. It is not an execution/comparison gate.
"""
import copy
import hashlib
import json

VERSION = 'oxid-record-current-runtime-view-v1'
MAX_BYTES = 256 * 1024 * 1024
U32_MAX = (1 << 32) - 1
USIZE_MAX = (1 << 64) - 1
RAW_KEYS = {'tag', 'records', 'functions'}
FUNCTION_KEYS = {'tag', 'id', 'span', 'result', 'parameters', 'locals', 'places',
                 'owners', 'references', 'calls', 'loans', 'entry', 'blocks'}
INSTRUCTIONS = {
    'Scalar': {'scalar'}, 'StorageLive': {'owner'}, 'StorageEnd': {'owner'},
    'Construct': {'destination', 'fields'}, 'MoveInitialize': {'destination', 'source'},
    'Replace': {'destination', 'source'}, 'Discard': {'owner'},
    'ReadField': {'destination', 'base', 'field'}, 'WriteField': {'base', 'field', 'value'},
    'OpenCall': {'call'}, 'PrepareScalar': {'call', 'argument', 'value'},
    'PrepareOwned': {'call', 'argument', 'source'}, 'PrepareBorrow': {'call', 'argument', 'loan'},
}
TERMINATORS = {'Branch': {'condition', 'then_block', 'else_block'}, 'Goto': {'target'},
               'Invoke': {'call', 'continuation'}, 'ReturnScalar': {'value'}, 'ReturnOwned': {'owner'}}
ROOT_REQUIRED = {'schema', 'loaded', 'index', 'namespace_work', 'audit', 'reference', 'native'}
ROOT_OPTIONAL = {'route', 'raw', 'raw_before_audit', 'entry', 'checked_immutable', 'diagnostics'}


def require(condition, message):
    if not condition:
        raise ValueError(message)


def digest(data):
    return hashlib.sha256(data).hexdigest()


def canonical(value):
    return json.dumps(value, sort_keys=True, separators=(',', ':'), ensure_ascii=False, allow_nan=False).encode('utf-8')


def identity(data):
    return {'bytes': len(data), 'sha256': digest(data)}


def shape(value, keys, label):
    require(type(value) is dict and set(value) == set(keys), label + ': exact keys')


def tagged(value, tag, keys, label):
    shape(value, {'tag', *keys}, label)
    require(value['tag'] == tag, label + ': exact tag')


def ordinal(value, maximum, label):
    # Preserve invalid declaration references. Representability is the only check.
    require(type(value) is int and 0 <= value <= maximum, label + ': encoded ID range/type')
    return value


def vector(value, label):
    require(type(value) is list, label + ': list required')
    return value


def tuple_item(value, tag, label):
    tagged(value, tag, {'items'}, label)
    items = vector(value['items'], label)
    require(len(items) == 1, label + ': exact tuple arity')
    return items[0]


def record_id(value, maximum, label):
    return ordinal(tuple_item(value, 'Record', label), maximum, label)


def scalar_type(value, label):
    tagged(value, 'Scalar', {'scalar'}, label)
    shape(value['scalar'], {'tag'}, label)
    require(value['scalar']['tag'] in ('Bool', 'I32', 'Unit'), label + ': scalar type required')


def instruction(value, label):
    require(type(value) is dict and value.get('tag') in INSTRUCTIONS, label + ': outside historical instruction domain')
    tagged(value, value['tag'], INSTRUCTIONS[value['tag']], label)


def kind(value, label):
    shape(value, {'tag'}, label)
    require(value['tag'] in ('Shared', 'Exclusive'), label + ': borrow kind')


def owner_kind(value, label):
    kinds = {'Parameter': {'position'}, 'Local': {'mutable'}, 'Temporary': set(),
             'StagedArgument': {'call', 'argument'}, 'CallResult': {'call'}}
    require(type(value) is dict and value.get('tag') in kinds, label + ': owner kind')
    tagged(value, value['tag'], kinds[value['tag']], label)


def raw_view(raw, reverse, root, changes):
    tagged(raw, 'RawOwnedProgram', RAW_KEYS - {'tag'}, root)
    for ri, record in enumerate(vector(raw['records'], root + '.records')):
        label = f'{root}.records[{ri}]'
        tagged(record, 'RawRecordDecl', {'id', 'span', 'fields'}, label)
        ordinal(record['id'], USIZE_MAX, label)
        for fi, field in enumerate(vector(record['fields'], label + '.fields')):
            label_field = f'{label}.fields[{fi}]'
            tagged(field, 'RawFieldDecl', {'id', 'span', 'ty'}, label_field)
            scalar_type(tuple_item(field['ty'], 'Value', label_field + '.ty'), label_field + '.ty')
    for fi, function in enumerate(vector(raw['functions'], root + '.functions')):
        label = f'{root}.functions[{fi}]'
        tagged(function, 'RawOwnedFunction', FUNCTION_KEYS - {'tag'}, label)
        result = function['result']
        require(type(result) is dict, label + '.result: type required')
        if result.get('tag') == 'Scalar':
            scalar_type(result, label + '.result')
        else:
            item = tuple_item(result, 'Owned', label + '.result')
            rid = ordinal(item, USIZE_MAX, label) if reverse else record_id(item, USIZE_MAX, label)
            function['result'] = {'tag': 'Owned', 'items': [{'tag': 'Record', 'items': [rid]}] if reverse else [rid]}
            changes.append({'path': label + '.result.items[0]', 'kind': 'owned-result', 'record': rid})
        for oi, owner in enumerate(vector(function['owners'], label + '.owners')):
            site = f'{label}.owners[{oi}]'
            carrier = 'record' if reverse else 'aggregate'
            tagged(owner, 'OwnerDecl', {carrier, 'kind', 'span'}, site)
            owner_kind(owner['kind'], site)
            rid = ordinal(owner[carrier], U32_MAX, site) if reverse else record_id(tuple_item(owner[carrier], 'AggregateSlot', site), U32_MAX, site)
            del owner[carrier]
            owner['aggregate' if reverse else 'record'] = {'tag': 'AggregateSlot', 'items': [{'tag': 'Record', 'items': [rid]}]} if reverse else rid
            changes.append({'path': site, 'kind': 'owner', 'record': rid})
        for collection, tag, keys in [('references', 'ReferenceDecl', {'kind', 'position', 'span'}),
                                      ('loans', 'LoanDecl', {'kind', 'call', 'argument', 'authority', 'span'})]:
            for di, descriptor in enumerate(vector(function[collection], label + '.' + collection)):
                site = f'{label}.{collection}[{di}]'
                carrier = 'record' if reverse else 'referent'
                tagged(descriptor, tag, {carrier, *keys}, site)
                kind(descriptor['kind'], site)
                rid = ordinal(descriptor[carrier], U32_MAX, site) if reverse else record_id(tuple_item(descriptor[carrier], 'BorrowedSlot', site), U32_MAX, site)
                del descriptor[carrier]
                descriptor['referent' if reverse else 'record'] = {'tag': 'BorrowedSlot', 'items': [{'tag': 'Record', 'items': [rid]}]} if reverse else rid
                changes.append({'path': site, 'kind': collection, 'record': rid})
        for bi, block in enumerate(vector(function['blocks'], label + '.blocks')):
            site = f'{label}.blocks[{bi}]'
            tagged(block, 'OwnedBlock', {'merge', 'span', 'statements', 'terminator'}, site)
            for si, statement in enumerate(vector(block['statements'], site + '.statements')):
                tagged(statement, 'OwnedStatement', {'kind', 'span', 'diagnostic_origins'}, site)
                instruction(statement['kind'], f'{site}.statements[{si}].kind')
            if block['terminator'] is not None:
                term = block['terminator']; tagged(term, 'OwnedTerminator', {'kind', 'span', 'diagnostic_origins'}, site)
                value = term['kind']
                require(type(value) is dict and value.get('tag') in TERMINATORS, site + ': historical terminator')
                tagged(value, value['tag'], TERMINATORS[value['tag']], site)


def transform(value, reverse=False):
    require(type(value) is dict and ROOT_REQUIRED <= set(value) <= ROOT_REQUIRED | ROOT_OPTIONAL, 'observation root keys')
    require(type(value['schema']) is int and value['schema'] == 1, 'observation schema')
    if 'route' in value:
        require(value['route'] in ('owned', 'scalar'), 'allowed observation route')
    result = copy.deepcopy(value); changes = []
    roots = [name for name in ('raw', 'raw_before_audit') if name in result]
    if roots:
        require(result.get('route') in ('owned', 'scalar'), 'raw route required')
    if result.get('route') == 'scalar':
        for name in roots:
            tagged(result[name], 'Program', {'functions'}, name)
    if result.get('route') == 'owned':
        for name in roots: raw_view(result[name], reverse, name, changes)
        for name, run in result['reference'].items():
            for index, row in enumerate(run['trace']):
                context = row.get('context')
                if context and type(context.get('operation')) is dict and context['operation'].get('tag') == 'OwnedStatement':
                    instruction(context['operation']['kind'], f'reference.{name}.trace[{index}].context')
                if row.get('kind') == 'existing_owned_event':
                    payload = row.get('payload')
                    require(not (type(payload) is dict and payload.get('tag') in ('ReadIndex', 'WriteIndex', 'ArrayLength')), 'array event outside historical domain')
    # No other field is eligible for projection, including complete trace trees.
    for name in set(value) - set(roots):
        require(result[name] == value[name], 'non-raw observation changed: ' + name)
    return result, changes


def decode(data):
    require(type(data) is bytes and len(data) <= MAX_BYTES, 'bounded bytes required')
    def pairs(rows):
        out = {}
        for key, value in rows:
            require(key not in out, 'duplicate JSON key'); out[key] = value
        return out
    def reject_constant(value): raise ValueError('nonfinite JSON: ' + value)
    value = json.loads(data, object_pairs_hook=pairs, parse_constant=reject_constant)
    require(canonical(value) == data, 'exact canonical source observation required')
    return value


def project_bytes(original):
    before = decode(original)
    after, paths = transform(before)
    restored, reversed_paths = transform(after, reverse=True)
    require(restored == before and reversed_paths == paths, 'every mapped site round trip')
    require(canonical(restored) == original, 'original byte identity round trip')
    derived = canonical(after)
    return derived, {'version': VERSION, 'original': identity(original), 'derived': identity(derived),
                     'mapped_sites': paths, 'round_trip_sha256': digest(canonical(restored)),
                     'semantics_normalized': False}


def reverse_bytes(derived):
    before = decode(derived)
    after, paths = transform(before, reverse=True)
    restored, reversed_paths = transform(after)
    require(restored == before and paths == reversed_paths, 'reverse every mapped site round trip')
    return canonical(after)
