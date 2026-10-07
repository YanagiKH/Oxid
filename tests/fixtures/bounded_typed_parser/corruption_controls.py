"""39 unchanged malformed-wire controls and named Call/Let successors."""
import json

def run(projection, OUT):
    report = {"controls": []}
    source = (OUT / 'cases/full_initial_stage/input.bin').read_bytes()
    raw = (OUT / 'cases/full_initial_stage/candidate.stdout').read_bytes()
    tokens = json.loads((OUT / 'cases/full_initial_stage/lexer.stdout').read_bytes())['tokens']
    wire = projection.decode_wire(raw, source)
    
    def field(data, column, row, value):
        result = bytearray(data)
        for plane in range(4):
            result[11 + column * 516 + plane * 129 + row] = (value >> (8 * plane)) & 255
        return bytes(result)
    
    def header(data, row, **updates):
        entry = {**wire['rows'][row], **updates}
        return field(data, 0, row, entry['kind'] + 64 * (entry['start'] + 256 * entry['end']) + (entry['next'] << 22))
    
    def reject(name, data, src=source, ts=tokens, expected=projection.ObservationError):
        try:
            projection.decode(data, src, ts)
        except expected as error:
            if name in ('supported_call_bad_callee_successor', 'supported_let_bad_keyword_successor'):
                assert type(error) is projection.ObservationError
            report['controls'].append({'case': name, 'passed': True, 'error': str(error)})
        else:
            raise AssertionError(name + ' accepted corruption')
    
    reject('short_header', raw[:10])
    reject('bad_magic', b'FAIL' + raw[4:])
    reject('trailing_bytes', raw + b'\0')
    reject('missing_plane', raw[:-129])
    reject('source_length', raw, source + b' ')
    reject('source_nonascii', raw, b'\xff' + source[1:])
    reject('success_detail', raw[:5] + b'\1' + raw[6:])
    reject('nonzero_reserved129', field(raw, 0, 128, 1))
    reject('nonzero_inactive', field(raw, 1, len(wire['rows']), 1))
    reject('tag_zero', header(raw, 0, kind=0))
    reject('tag37', header(raw, 0, kind=37))
    reject('span_backwards', header(raw, 0, start=20, end=19))
    reject('span_outside_source', header(raw, 0, end=128))
    reject('next_outside_table', header(raw, 0, next=128))
    reject('payload_high_bits', field(raw, 1, 0, 1 << 16))
    reject('item_cycle', header(raw, 0, next=1))
    reject('wrong_item_kind', raw[:9] + b'\2' + raw[10:])
    reject('orphan_rows', header(raw, 0, next=0)[:9] + b'\0' + raw[10:])
    reject('function_name_span', header(raw, 0, start=0, end=3))
    reject('wrong_public_token', field(raw, 1, 0, 2 + wire['rows'][0]['b'] * 256))
    param = next(i for i,r in enumerate(wire['rows']) if r['kind'] == 2)
    reject('parameter_cycle', header(raw, param, next=param+1))
    reject('parameter_unused_field', field(raw, 2, param, 1))
    block = next(i for i,r in enumerate(wire['rows']) if r['kind'] == 5)
    reject('block_next_link', header(raw, block, next=1))
    reject('block_closing_token', field(raw, 1, block, 1 + wire['rows'][block]['b'] * 256))
    number = next(i for i,r in enumerate(wire['rows']) if r['kind'] == 15)
    reject('number_height', field(raw, 2, number, 2 * 256))
    reject('number_flag', field(raw, 1, number, wire['rows'][number]['a'] + 2 * 256))
    reject('number_token', field(raw, 1, number, 1 + wire['rows'][number]['b'] * 256))
    reject('leaf_next_link', header(raw, number, next=number+1))
    statement = next(i for i,r in enumerate(wire['rows']) if r['kind'] == 9)
    reject('shared_expression', field(raw, 1, statement, number+1))
    reject('supported_let_bad_keyword_successor', header(raw, statement, kind=6), expected=projection.ObservationError)
    assert report['controls'][-1]['error'] == 'AST/token kind mismatch: expected Let'
    reject('supported_call_bad_callee_successor', header(raw, number, kind=20), expected=projection.ObservationError)
    assert report['controls'][-1]['error'] == 'AST/token kind mismatch: expected Ident'
    bad_tokens = [dict(t) for t in tokens]; bad_tokens[0]['id'] = 5
    reject('token_kind_identity', raw, ts=bad_tokens)
    bad_tokens = [dict(t) for t in tokens]; bad_tokens[0]['file_id'] = 1
    reject('token_file_identity', raw, ts=bad_tokens)
    bad_tokens = [dict(t) for t in tokens]; bad_tokens[1]['start'] = 0
    reject('token_coverage', raw, ts=bad_tokens)
    bad_tokens = [dict(t) for t in tokens]; bad_tokens[-1]['kind'] = 'Ident'; bad_tokens[-1]['id'] = 2
    reject('token_eof', raw, ts=bad_tokens)
    reject('missing_tokens', raw, ts=None)
    error = b'OPA1' + bytes([1,17,0,1,0,0,len(source)])
    reject('failure_partial_ast', error + bytes(1548))
    reject('failure_nonzero_rows', error[:8] + b'\1' + error[9:])
    reject('internal_error9', error[:4] + b'\x09' + error[5:])
    reject('unknown_message', error[:5] + b'\xff' + error[6:])
    reject('unknown_error_detail', error[:4] + b'\x02\xff' + error[6:])
    assert len(report["controls"]) == 41
    return report["controls"]
