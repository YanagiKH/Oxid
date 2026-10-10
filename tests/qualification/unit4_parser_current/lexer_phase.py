"""Inactive source-only public parser lexical-phase successor.

Composition is pure and reversible. No authority, map, source tree, binary, or
receipt is generated or activated by importing this module. Actual typed layout
and independent source/whole-map admission remain mandatory external gates.
"""
from __future__ import annotations

import hashlib
import json
import re

SCHEMA = 'oxid-unit4-lexer-phase-observation-v1'
ENVELOPE_SCHEMA = 'oxid-unit4-lexer-phase-envelope-v1'
EVENT_LIMIT = 16
ROW_BYTES = 8
SERIALIZED_BYTE_LIMIT = 1912
CONTROL_BYTE_LIMIT = 572
FRAME_PREFIX = b'UNIT4_LEXER_PHASE_V1 '
FRAME_BYTE_LIMIT = 1966
ENVELOPE_BYTE_LIMIT = 225509
ENVELOPES_BYTE_LIMIT = 478706
ROLES = ('observed', 'not_observed')
SIDECAR_KEYS = ('schema', 'nonce', 'case_id', 'role', 'runtime_os',
    'runtime_architecture', 'pointer_width', 'token_bytes', 'lex_attempts',
    'lex_result', 'phase', 'closed', 'mode_indices', 'event_limit', 'event_count',
    'fixed_row_bytes', 'fixed_rows_bytes', 'occupied_row_bytes', 'ledger_bytes',
    'row_byte_limit', 'serialized_byte_limit', 'reserve_failed', 'rows')
ROW_KEYS = ('seq', 'kind', 'length', 'element_bytes', 'success')
ENVELOPE_KEYS = ('schema', 'scope', 'profile', 'session_sha256', 'authority_sha256',
    'adapter_sha256', 'parent_receipt', 'record_count', 'records')
RECORD_KEYS = ('case_id', 'nonce', 'role', 'process_index',
    'process_receipt_sha256', 'binary_sha256', 'source_sha256', 'raw_sha256',
    'stdout_sha256', 'stderr_sha256', 'sidecar')
IDENTITY_KEYS = ('path', 'bytes', 'sha256')
PREDECESSOR_IDENTITIES = {
    'budget': (9432, '81fd8a3f2d2f9dbcd6966aeb8ad977f4129f7e785b164eebf63d63589bf416fd'),
    'lexer': (35594, 'c3a56f40587fa40ceed56a15e27861039673a98eab8b9d3e3a471a3ab26a518c'),
    'observer': (20052, '8b7efa8484d9636dda5329a4708fa762d09c53bd76b42c485b871973410c7928'),
}


class Reject(ValueError):
    pass


def need(value, why):
    if not value:
        raise Reject(why)


def sha256(raw):
    return hashlib.sha256(raw).hexdigest()


def canonical_bytes(value):
    try:
        return (json.dumps(value, ensure_ascii=False, separators=(',', ':'),
                           allow_nan=False) + '\n').encode('ascii')
    except (ValueError, TypeError, UnicodeError) as error:
        raise Reject('noncanonical JSON value') from error


def process_projection(value):
    """Native-row projection only; never substitutes for exact container bytes."""
    try:
        return json.dumps(value, ensure_ascii=False, sort_keys=True,
                          separators=(',', ':'), allow_nan=False).encode('utf-8')
    except (ValueError, TypeError, UnicodeError) as error:
        raise Reject('invalid process projection') from error


def _pairs(pairs):
    result = {}
    for key, value in pairs:
        need(key not in result, 'duplicate JSON key')
        result[key] = value
    return result


def decode_canonical(raw):
    need(type(raw) is bytes and raw.endswith(b'\n') and raw.count(b'\n') == 1,
         'exact final LF required')
    try:
        value = json.loads(raw.decode('ascii'), object_pairs_hook=_pairs,
                           parse_constant=lambda value: (_ for _ in ()).throw(Reject('nonfinite JSON')))
    except (UnicodeError, ValueError) as error:
        raise Reject('invalid closed ASCII JSON') from error
    need(canonical_bytes(value) == raw, 'noncanonical JSON bytes')
    return value


def _keys(value, names):
    need(type(value) is dict and tuple(value) == names, 'closed ordered keys differ')


def _uint(value, maximum=2**64-1, minimum=0):
    need(type(value) is int and minimum <= value <= maximum, 'unsigned integer bound/type')


def _hex(value, length=64):
    need(type(value) is str and re.fullmatch('[0-9a-f]{%d}' % length, value) is not None,
         'lowercase hexadecimal identity')


def _case(value):
    need(type(value) is str and re.fullmatch('[a-z0-9][a-z0-9_-]{0,63}', value) is not None,
         'closed case identifier')


def _path(value, maximum):
    need(type(value) is str and 1 <= len(value) <= maximum
         and re.fullmatch('[A-Za-z0-9_./-]+', value) is not None
         and not value.startswith('/') and all(p not in ('', '.', '..') for p in value.split('/')),
         'canonical relative path')


def _identity(value, maximum_path, maximum_bytes=2**64-1, minimum_bytes=0):
    _keys(value, IDENTITY_KEYS)
    _path(value['path'], maximum_path)
    _uint(value['bytes'], maximum_bytes, minimum_bytes)
    _hex(value['sha256'])


def validate_sidecar(raw, *, nonce=None, case_id=None, role=None,
                     mode_indices=None, lex_result=None):
    need(type(raw) is bytes and 1 <= len(raw) <= SERIALIZED_BYTE_LIMIT, 'sidecar byte limit')
    value = decode_canonical(raw)
    _keys(value, SIDECAR_KEYS)
    _hex(value['nonce'], 32)
    _case(value['case_id'])
    need(value['schema'] == SCHEMA and value['role'] in ROLES, 'sidecar schema/role')
    need(value['runtime_os'] == 'linux' and value['runtime_architecture'] == 'x86_64', 'sidecar ABI')
    for key, expected in (('pointer_width', 64), ('token_bytes', 32), ('lex_attempts', 1),
                          ('event_limit', 16), ('fixed_row_bytes', 8), ('fixed_rows_bytes', 128),
                          ('ledger_bytes', 248), ('row_byte_limit', 128), ('serialized_byte_limit', 1912)):
        _uint(value[key])
        need(value[key] == expected, 'sidecar ' + key)
    need(value['lex_result'] in ('ok', 'err') and value['phase'] == 'finished'
         and value['closed'] is True, 'sidecar phase closure')
    need(type(value['mode_indices']) is list and value['mode_indices'] in ([0], [0, 1])
         and all(type(n) is int for n in value['mode_indices']), 'exact mode sequence')
    _uint(value['event_count'], 16)
    _uint(value['occupied_row_bytes'], 128)
    need(type(value['rows']) is list and len(value['rows']) == value['event_count']
         and value['occupied_row_bytes'] == 8 * value['event_count'], 'row count/charge')
    previous = 0
    failed = False
    for index, row in enumerate(value['rows']):
        _keys(row, ROW_KEYS)
        _uint(row['seq'], 16, 1)
        _uint(row['length'], 131072, 4)
        _uint(row['element_bytes'])
        need(row['seq'] == index + 1 and row['kind'] == 'lexer token tape'
             and row['element_bytes'] == 32 and type(row['success']) is bool, 'row fields')
        need(not failed and row['length'] & (row['length'] - 1) == 0
             and row['length'] > previous and (index != 0 or row['length'] == 4), 'row order/terminal')
        previous = row['length']
        failed = not row['success']
    if value['role'] == 'observed':
        need(type(value['reserve_failed']) is bool and value['reserve_failed'] == failed,
             'observed failure flag')
        need(not failed or value['lex_result'] == 'err', 'failed reserve cannot lex ok')
        need(value['lex_result'] != 'ok' or bool(value['rows']), 'observed EOF needs reserve')
    else:
        need(not value['rows'] and value['reserve_failed'] is None and len(raw) <= CONTROL_BYTE_LIMIT,
             'control is not observed')
    for key, expected in (('nonce', nonce), ('case_id', case_id), ('role', role),
                          ('mode_indices', mode_indices), ('lex_result', lex_result)):
        if expected is not None:
            need(value[key] == expected, 'sidecar association: ' + key)
    return value


def extract_witness(stdout, *, nonce, case_id=None, role=None, mode_indices=None, lex_result=None):
    """Caller must first admit the original successful helper/process receipt."""
    _hex(nonce, 32)
    need(type(stdout) is bytes and stdout.count(FRAME_PREFIX) == 1, 'one lexical frame required')
    start = stdout.index(FRAME_PREFIX)
    header = FRAME_PREFIX + nonce.encode('ascii') + b' '
    need(stdout[start:start + len(header)] == header, 'frame nonce/header')
    stop = stdout.find(b'\n', start, start + FRAME_BYTE_LIMIT + 1)
    need(stop >= 0 and stop + 1 - start <= FRAME_BYTE_LIMIT, 'bounded complete lexical frame')
    payload = stdout[start + len(header):stop + 1]
    completion = b'UNIT4_EXECUTED ' + nonce.encode('ascii') + b'\n'
    need(stdout.count(b'UNIT4_EXECUTED ') == 1 and stdout.count(completion) == 1
         and stdout.index(completion) > stop, 'exact completion after lexical frame')
    validate_sidecar(payload, nonce=nonce, case_id=case_id, role=role,
                     mode_indices=mode_indices, lex_result=lex_result)
    return payload


def validate_raw_prefix(raw):
    """Inspect at most the fixed metadata prefix; source text is never scanned."""
    need(type(raw) is bytes, 'raw bytes required')
    prefix = b'{"schema":"oxid-unit4-parser-raw-v1","nonce":"'
    need(raw.startswith(prefix), 'raw schema prefix')
    offset = len(prefix)
    need(len(raw) >= offset + 32 + len(b'","case_id":"'), 'truncated raw nonce')
    nonce = raw[offset:offset + 32]
    _hex(nonce.decode('ascii', errors='replace'), 32)
    offset += 32
    delimiter = b'","case_id":"'
    need(raw[offset:offset + len(delimiter)] == delimiter, 'raw nonce delimiter')
    offset += len(delimiter)
    end = raw.find(b'"', offset, offset + 65)
    need(end >= 0, 'raw case width/closure')
    case = raw[offset:end]
    _case(case.decode('ascii', errors='replace'))
    need(raw[end:end + len(b'","source_utf8":')] == b'","source_utf8":', 'raw case delimiter')
    return nonce.decode('ascii'), case.decode('ascii')


def validate_envelope(raw, *, scope, profile, session_sha256, authority_sha256,
                      adapter_sha256, parent_receipt, records):
    """Require independently authenticated, prescribed native rows from caller.

    The caller retains original admission of session, builds, maps, source, raw,
    stdout/stderr, native receipt container, process invocation and case roster.
    Matching a convenient row by digest is deliberately not an API operation.
    """
    need(type(raw) is bytes and 1 <= len(raw) <= ENVELOPE_BYTE_LIMIT, 'envelope byte bound')
    need(scope in ('collection', 'passivity', 'u8_controls') and profile in ('debug', 'release'),
         'envelope scope/profile')
    value = decode_canonical(raw)
    _keys(value, ENVELOPE_KEYS)
    need(value['schema'] == ENVELOPE_SCHEMA and value['scope'] == scope and value['profile'] == profile,
         'envelope selection')
    for key, expected in (('session_sha256', session_sha256), ('authority_sha256', authority_sha256),
                          ('adapter_sha256', adapter_sha256)):
        _hex(value[key]); _hex(expected)
        need(value[key] == expected, 'envelope exact ' + key)
    _identity(value['parent_receipt'], 64)
    expected_path = {'collection': f'collect-{profile}/result/execution-manifest.json',
                     'passivity': f'passivity-{profile}/result/report.json',
                     'u8_controls': 'u8-policy-controls/receipt.json'}[scope]
    need(value['parent_receipt']['path'] == expected_path and value['parent_receipt'] == parent_receipt,
         'exact parent receipt identity')
    count = {'collection': 248, 'passivity': 6, 'u8_controls': 8}[scope]
    _uint(value['record_count'], 248)
    need(value['record_count'] == count and type(value['records']) is list
         and len(value['records']) == count and type(records) is list and len(records) == count,
         'exact envelope roster')
    seen_nonce, seen_path = set(), set()
    for index, row in enumerate(value['records']):
        _keys(row, RECORD_KEYS)
        _case(row['case_id']); _hex(row['nonce'], 32)
        need(row['role'] in ROLES, 'envelope role')
        _uint(row['process_index'], 247)
        need(row['process_index'] == index + (9 if scope == 'u8_controls' and profile == 'release' else 0),
             'prescribed native process index')
        if scope == 'collection':
            need(row['role'] == 'observed', 'collection role')
            expected_sidecar = f"collect-{profile}/result/{row['case_id']}/lexer-phase.json"
        elif scope == 'passivity':
            need(row['role'] == ROLES[index % 2]
                 and row['case_id'] == ('original', 'project', 'malformed')[index // 2],
                 'flattened passivity role/case')
            suffix = '-instrumented' if index % 2 == 0 else '-control'
            expected_sidecar = f"passivity-{profile}/result/{row['case_id']}{suffix}/lexer-phase.json"
        else:
            need(row['role'] == ROLES[index // 4]
                 and row['case_id'] == ('narrow', 'widen', 'trivia', 'numeric')[index % 4],
                 'prescribed u8 role/case')
            label = 'observer' if index < 4 else 'control'
            expected_sidecar = f"u8-policy-controls/{profile}-{label}-{row['case_id']}/lexer-phase.json"
        for key in RECORD_KEYS[4:-1]:
            _hex(row[key])
        _identity(row['sidecar'], 128, 572 if row['role'] == 'not_observed' else 1912, 1)
        need(row['sidecar']['path'] == expected_sidecar, 'sidecar exact process path')
        need(row['nonce'] not in seen_nonce and row['sidecar']['path'] not in seen_path,
             'duplicate native process/sidecar')
        seen_nonce.add(row['nonce']); seen_path.add(row['sidecar']['path'])
        need(row == records[index], 'exact authenticated native process association')
    return value


def _verify(raw, kind):
    need(type(raw) is bytes and (len(raw), sha256(raw)) == PREDECESSOR_IDENTITIES[kind],
         'unapproved complete predecessor body: ' + kind)


def _replace(raw, seams):
    result = raw
    for before, after in seams:
        need(before != after and result.count(before) == 1 and after not in result,
             'missing/duplicate/wrong-stage/already-applied seam')
        result = result.replace(before, after, 1)
    need(_inverse(result, seams) == raw, 'exact whole-body inverse')
    return result


def _inverse(raw, seams):
    for before, after in reversed(seams):
        need(raw.count(after) == 1, 'missing/duplicate inverse seam')
        raw = raw.replace(after, before, 1)
    return raw


BUDGET_SEAMS = ((b'crate::frontend::parser::unit4_observer::reserve(kind, length, element_bytes, success);',
                 b'crate::frontend::parser::unit4_observer::lexer_phase::reserve(kind, length, element_bytes, success);'),)
LEXER_SEAMS = ((b'crate::frontend::parser::unit4_observer::lex_token(*tokens.last().unwrap());',
                b'crate::frontend::parser::unit4_observer::lexer_phase::lex_token(*tokens.last().unwrap());'),)


def compose_budget(raw):
    _verify(raw, 'budget')
    return _replace(raw, BUDGET_SEAMS)


def invert_budget(raw):
    result = _inverse(raw, BUDGET_SEAMS)
    _verify(result, 'budget')
    need(compose_budget(result) == raw, 'exact derived budget')
    return result


def compose_lexer(raw):
    _verify(raw, 'lexer')
    return _replace(raw, LEXER_SEAMS)


def invert_lexer(raw):
    result = _inverse(raw, LEXER_SEAMS)
    _verify(result, 'lexer')
    need(compose_lexer(result) == raw, 'exact derived lexer')
    return result


OBSERVER_SEAMS = (
    (b'    let lex = super::super::lexer::lex_with_limit(sources.get(id), number("UNIT4_TOKEN_LIMIT"));\n',
     b'    lexer_phase::request_modes(if number("UNIT4_ORIGINAL") == 1 { 2 } else { 1 });\n'
     b'    lexer_phase::begin_lex(control);\n'
     b'    let lex = super::super::lexer::lex_with_limit(sources.get(id), number("UNIT4_TOKEN_LIMIT"));\n'
     b'    lexer_phase::close_lex(&lex);\n'),
    (b'    let mut rows = vec![run_mode(\n',
     b'    lexer_phase::begin_mode(0);\n    let mut rows = vec![run_mode(\n'),
    (b'    )];\n    if number("UNIT4_ORIGINAL") == 1 {\n        rows.push(run_mode(\n',
     b'    )];\n    lexer_phase::close_mode(0);\n    if number("UNIT4_ORIGINAL") == 1 {\n'
     b'        lexer_phase::begin_mode(1);\n        rows.push(run_mode(\n'),
    (b'        ));\n    }\n    let out=format!',
     b'        ));\n        lexer_phase::close_mode(1);\n    }\n    let out=format!'),
    (b'    std::fs::write(env("UNIT4_RAW_OUTPUT"), out).expect("raw observation write");\n',
     b'    lexer_phase::finish(&out);\n'
     b'    std::fs::write(env("UNIT4_RAW_OUTPUT"), out).expect("raw observation write");\n'),
)


def compose_observer(raw, role):
    need(role in ROLES, 'exact observer role')
    _verify(raw, 'observer')
    return _replace(raw, OBSERVER_SEAMS) + rust_module(role)


def invert_observer(raw, role):
    need(role in ROLES, 'exact observer role')
    module = rust_module(role)
    need(raw.endswith(module) and raw.count(module) == 1, 'exact appended phase module')
    result = _inverse(raw[:-len(module)], OBSERVER_SEAMS)
    _verify(result, 'observer')
    need(compose_observer(result, role) == raw, 'exact derived observer')
    return result


def rust_module(role):
    need(role in ROLES, 'exact module role')
    return RUST_MODULE.replace(b'__CONTROL_ROLE__', b'true' if role == 'not_observed' else b'false')


RUST_MODULE = br'''
// Current-only phase adapter. Frozen helpers above remain byte-recoverable.
pub(in crate::frontend) mod lexer_phase {
    use super::*;
    use std::cell::{Ref, RefMut};
    use std::io::{StdoutLock, Write};
    use std::mem::{align_of, size_of};
    use std::ops::Deref;

    const CONTROL_ROLE: bool = __CONTROL_ROLE__;
    const INACTIVE: u8 = 0;
    const LEXING: u8 = 1;
    const LEXED: u8 = 2;
    const PARSING0: u8 = 3;
    const BETWEEN0: u8 = 4;
    const PARSING1: u8 = 5;
    const BETWEEN1: u8 = 6;
    const FINISHED: u8 = 7;
    const CAP: usize = 1912;
    const COUNT_ONLY: usize = 1usize << (usize::BITS - 1);
    const RAW_PREFIX: &[u8] = b"{\"schema\":\"oxid-unit4-parser-raw-v1\",\"nonce\":\"";

    #[repr(C)]
    #[derive(Clone, Copy)]
    struct Row { length: u32, element_bytes: u16, success: u8, seq: u8 }
    const EMPTY_ROW: Row = Row { length: 0, element_bytes: 0, success: 0, seq: 0 };
    #[repr(C)]
    struct Ledger {
        rows: [Row; 16],
        nonce: [u8; 32],
        case_id: [u8; 64],
        occupied_row_bytes: u32,
        token_count: u32,
        token_bytes: u16,
        serialized_length: u16,
        phase: u8,
        role: u8,
        row_count: u8,
        case_len: u8,
        requested_modes: u8,
        completed_modes: u8,
        lex_result: u8,
        failed: u8,
        os: u8,
        architecture: u8,
        pointer_width: u8,
        reserved: u8,
    }
    impl Ledger {
        const fn new() -> Self {
            Self { rows: [EMPTY_ROW; 16], nonce: [0; 32], case_id: [0; 64],
                occupied_row_bytes: 0, token_count: 0, token_bytes: 0,
                serialized_length: 0, phase: INACTIVE, role: 0, row_count: 0,
                case_len: 0, requested_modes: 0, completed_modes: 0,
                lex_result: 0, failed: 0, os: 0, architecture: 0,
                pointer_width: 0, reserved: 0 }
        }
    }
    thread_local! { static LEDGER: RefCell<Ledger> = const { RefCell::new(Ledger::new()) }; }

    #[repr(C)]
    struct CaptureCarriers<'a, 'k> {
        ledger: RefMut<'a, Ledger>, kind: &'k str,
        length: usize, element_bytes: usize,
        pending: Row, next_count: u32, next_bytes: u32, success: bool,
    }
    #[repr(C)]
    struct TokenForwardCarriers<'a> {
        input: Token, forwarded: Token, next_count: u32, ledger: RefMut<'a, Ledger>,
    }
    // Both passes instantiate these same fields. Ref and RefMut each have their
    // own mandatory measured layout. No second serializer or metadata snapshot.
    #[repr(C)]
    struct SerializeCarriers<'a, B, const N: usize> {
        ledger: B, raw: &'a [u8], nonce: &'a [u8], case_id: &'a [u8],
        destination: &'a mut [u8; N], used: usize, row_index: usize,
        decimal: [u8; 20], digits: usize,
    }
    #[repr(C)]
    struct FrameCarriers<'a> {
        nonce: &'a [u8], payload: &'a [u8], stdout: StdoutLock<'static>,
    }

    pub(in crate::frontend) fn begin_lex(control: bool) {
        LEDGER.with(|cell| {
            let mut ledger = cell.borrow_mut();
            assert!(ledger.phase != FINISHED, "UNIT4_PHASE_V1:phase.finished");
            assert!(ledger.phase == INACTIVE, "UNIT4_PHASE_V1:phase.nested_lex");
            assert!(control == CONTROL_ROLE, "UNIT4_PHASE_V1:phase.control");
            assert!(size_of::<Token>() == 32, "UNIT4_PHASE_V1:layout");
            ledger.token_bytes = u16::try_from(size_of::<Token>()).expect("UNIT4_PHASE_V1:layout");
            ledger.role = u8::from(control);
            ledger.phase = LEXING;
        });
    }
    pub(in crate::frontend) fn request_modes(count: u8) {
        LEDGER.with(|cell| {
            let mut ledger = cell.borrow_mut();
            assert!(ledger.phase == INACTIVE && ledger.requested_modes == 0 && (count == 1 || count == 2),
                "UNIT4_PHASE_V1:phase.mode_order");
            ledger.requested_modes = count;
        });
    }
    fn charge(c: &mut CaptureCarriers<'_, '_>) {
        assert!(c.ledger.phase == LEXING, "UNIT4_PHASE_V1:phase.unowned");
        c.next_count = u32::from(c.ledger.row_count).checked_add(1).expect("UNIT4_PHASE_V1:cap.events");
        assert!(c.next_count <= 16, "UNIT4_PHASE_V1:cap.events");
        c.next_bytes = c.ledger.occupied_row_bytes.checked_add(8).expect("UNIT4_PHASE_V1:cap.bytes");
        assert!(c.next_bytes <= 128, "UNIT4_PHASE_V1:cap.bytes");
    }
    pub(in crate::frontend) fn reserve(kind: &str, length: usize, element_bytes: usize, success: bool) {
        LEDGER.with(|cell| {
            let mut c = CaptureCarriers { ledger: cell.borrow_mut(), kind, length, element_bytes,
                pending: EMPTY_ROW, next_count: 0, next_bytes: 0, success };
            if c.ledger.phase == INACTIVE { return; }
            assert!(c.ledger.phase != FINISHED, "UNIT4_PHASE_V1:phase.finished");
            assert!(c.ledger.role == 0, "UNIT4_PHASE_V1:phase.control");
            match c.ledger.phase {
                LEXING => {
                    assert!(c.ledger.failed == 0, "UNIT4_PHASE_V1:phase.reserve_after_fail");
                    assert!(c.kind == "lexer token tape", "UNIT4_PHASE_V1:phase.nonlexer_lex");
                    charge(&mut c);
                    assert!(c.element_bytes == size_of::<Token>(), "UNIT4_PHASE_V1:row.width");
                    assert!(c.length >= 4 && c.length <= 131072 && c.length.is_power_of_two()
                        && c.length > c.ledger.token_count as usize, "UNIT4_PHASE_V1:row.length");
                    if c.ledger.row_count == 0 {
                        assert!(c.length == 4, "UNIT4_PHASE_V1:row.first");
                    } else {
                        assert!(c.length > c.ledger.rows[c.ledger.row_count as usize - 1].length as usize,
                            "UNIT4_PHASE_V1:row.order");
                    }
                    c.pending = Row { length: u32::try_from(c.length).expect("UNIT4_PHASE_V1:row.length"),
                        element_bytes: u16::try_from(c.element_bytes).expect("UNIT4_PHASE_V1:row.width"),
                        success: u8::from(c.success), seq: u8::try_from(c.next_count).expect("UNIT4_PHASE_V1:cap.events") };
                    // The two reviewed additional Row copy slots are the pending
                    // write value and the serializer's current row below.
                    let row = c.pending;
                    c.ledger.rows[(c.next_count - 1) as usize] = row;
                    c.ledger.row_count = c.next_count as u8;
                    c.ledger.occupied_row_bytes = c.next_bytes;
                    c.ledger.failed = u8::from(!c.success);
                }
                PARSING0 | PARSING1 => {
                    assert!(c.kind != "lexer token tape", "UNIT4_PHASE_V1:phase.lexer_parse");
                    super::reserve(c.kind, c.length, c.element_bytes, c.success);
                }
                _ => panic!("UNIT4_PHASE_V1:phase.unowned"),
            }
        });
    }
    // Accounting amendment: retain an additional size_of::<Token>() for this
    // by-value argument, independently of both Token fields in the carrier.
    pub(in crate::frontend) fn lex_token(token: Token) {
        LEDGER.with(|cell| {
            let mut c = TokenForwardCarriers { input: token, forwarded: token,
                next_count: 0, ledger: cell.borrow_mut() };
            if c.ledger.phase == INACTIVE { return; }
            assert!(c.ledger.phase != FINISHED, "UNIT4_PHASE_V1:phase.finished");
            assert!(c.ledger.role == 0, "UNIT4_PHASE_V1:phase.control");
            if c.ledger.phase == PARSING0 || c.ledger.phase == PARSING1 {
                panic!("UNIT4_PHASE_V1:phase.lexer_parse");
            }
            assert!(c.ledger.phase == LEXING, "UNIT4_PHASE_V1:phase.unowned");
            assert!(c.ledger.failed == 0, "UNIT4_PHASE_V1:phase.token_after_fail");
            c.next_count = c.ledger.token_count.checked_add(1).expect("UNIT4_PHASE_V1:cap.tokens");
            assert!(c.next_count <= 100001, "UNIT4_PHASE_V1:cap.tokens");
            c.forwarded = c.input;
            super::lex_token(c.forwarded);
            c.ledger.token_count = c.next_count;
        });
    }
    pub(in crate::frontend) fn close_lex(lex: &Result<Vec<Token>, Box<Diagnostic>>) {
        LEDGER.with(|cell| {
            let mut ledger = cell.borrow_mut();
            assert!(ledger.phase == LEXING, "UNIT4_PHASE_V1:phase.double_close");
            assert!(ledger.requested_modes == 1 || ledger.requested_modes == 2, "UNIT4_PHASE_V1:phase.mode_order");
            assert!(!(ledger.failed != 0 && lex.is_ok()), "UNIT4_PHASE_V1:phase.failed_ok");
            if ledger.role == 0 {
                assert!(ledger.occupied_row_bytes == u32::from(ledger.row_count) * 8,
                    "UNIT4_PHASE_V1:cap.bytes");
                STATE.with(|state| assert!(state.borrow().tokens.len() == ledger.token_count as usize,
                    "UNIT4_PHASE_V1:token.collector"));
                if let Ok(tokens) = lex {
                    assert!(tokens.len() == ledger.token_count as usize && ledger.row_count != 0,
                        "UNIT4_PHASE_V1:token.returned");
                }
            } else {
                assert!(ledger.row_count == 0 && ledger.token_count == 0 && ledger.failed == 0,
                    "UNIT4_PHASE_V1:phase.control");
            }
            ledger.lex_result = if lex.is_ok() { 1 } else { 2 };
            ledger.phase = LEXED;
            // Rows, counts, failure and lex_result are never changed after here.
        });
    }
    pub(in crate::frontend) fn begin_mode(index: u8) {
        LEDGER.with(|cell| {
            let mut ledger = cell.borrow_mut();
            assert!(ledger.phase != LEXING, "UNIT4_PHASE_V1:phase.unclosed_lex");
            assert!((index == 0 && ledger.phase == LEXED && ledger.completed_modes == 0)
                || (index == 1 && ledger.phase == BETWEEN0 && ledger.completed_modes == 1 && ledger.requested_modes == 2),
                "UNIT4_PHASE_V1:phase.mode_order");
            ledger.phase = if index == 0 { PARSING0 } else { PARSING1 };
        });
    }
    pub(in crate::frontend) fn close_mode(index: u8) {
        LEDGER.with(|cell| {
            let mut ledger = cell.borrow_mut();
            assert!((index == 0 && ledger.phase == PARSING0 && ledger.completed_modes == 0)
                || (index == 1 && ledger.phase == PARSING1 && ledger.completed_modes == 1),
                "UNIT4_PHASE_V1:phase.mode_order");
            ledger.completed_modes = index + 1;
            ledger.phase = if index == 0 { BETWEEN0 } else { BETWEEN1 };
        });
    }

    fn metadata<B: Deref<Target = Ledger>, const N: usize>(s: &mut SerializeCarriers<'_, B, N>) -> Result<(), ()> {
        if !s.raw.starts_with(RAW_PREFIX) { return Err(()); }
        s.used = RAW_PREFIX.len();
        s.nonce = s.raw.get(s.used..s.used.checked_add(32).ok_or(())?).ok_or(())?;
        if !s.nonce.iter().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(b)) { return Err(()); }
        s.used += 32;
        if s.raw.get(s.used..s.used + 13) != Some(b"\",\"case_id\":\"") { return Err(()); }
        s.used += 13;
        s.row_index = s.used;
        s.digits = 0;
        while s.digits <= 64 {
            if s.raw.get(s.used) == Some(&b'"') { break; }
            if s.digits == 64 { return Err(()); }
            if !s.raw.get(s.used).is_some_and(|b| b.is_ascii_lowercase() || b.is_ascii_digit()
                || (s.digits != 0 && (*b == b'_' || *b == b'-'))) { return Err(()); }
            s.used += 1;
            s.digits += 1;
        }
        if s.digits == 0 || s.raw.get(s.used..s.used + 16) != Some(b"\",\"source_utf8\":") { return Err(()); }
        s.case_id = s.raw.get(s.row_index..s.used).ok_or(())?;
        Ok(())
    }
    impl<B: Deref<Target = Ledger>, const N: usize> SerializeCarriers<'_, B, N> {
        fn put(&mut self, bytes: &[u8]) -> Result<(), ()> {
            if bytes.len() > N.checked_sub(self.used & !COUNT_ONLY).ok_or(())? { return Err(()); }
            if self.used & COUNT_ONLY == 0 {
                self.destination[self.used..self.used + bytes.len()].copy_from_slice(bytes);
            }
            self.used = self.used.checked_add(bytes.len()).ok_or(())?;
            Ok(())
        }
        fn uint(&mut self) -> Result<(), ()> {
            // The priced integer slots are reused for quotient and decimal
            // cursor. A current Row copy preserves its sequence during a row.
            self.row_index = 20;
            loop {
                self.row_index -= 1;
                self.decimal[self.row_index] = b'0' + (self.digits % 10) as u8;
                self.digits /= 10;
                if self.digits == 0 { break; }
            }
            self.digits = 20 - self.row_index;
            if self.digits > N.checked_sub(self.used & !COUNT_ONLY).ok_or(())? { return Err(()); }
            if self.used & COUNT_ONLY == 0 {
                self.destination[self.used..self.used + self.digits]
                    .copy_from_slice(&self.decimal[self.row_index..]);
            }
            self.used = self.used.checked_add(self.digits).ok_or(())?;
            Ok(())
        }
    }
    fn serialize<B: Deref<Target = Ledger>, const N: usize>(s: &mut SerializeCarriers<'_, B, N>) -> Result<(), ()> {
        s.put(b"{\"schema\":\"oxid-unit4-lexer-phase-observation-v1\",\"nonce\":\"")?;
        s.put(s.nonce)?; s.put(b"\",\"case_id\":\"")?; s.put(s.case_id)?;
        s.put(b"\",\"role\":\"")?;
        s.put(if s.ledger.role == 0 { b"observed" } else { b"not_observed" })?;
        s.put(b"\",\"runtime_os\":\"linux\",\"runtime_architecture\":\"x86_64\",\"pointer_width\":64,\"token_bytes\":32,\"lex_attempts\":1,\"lex_result\":\"")?;
        s.put(if s.ledger.lex_result == 1 { b"ok" } else { b"err" })?;
        s.put(b"\",\"phase\":\"finished\",\"closed\":true,\"mode_indices\":[0")?;
        if s.ledger.requested_modes == 2 { s.put(b",1")?; }
        s.put(b"],\"event_limit\":16,\"event_count\":")?; s.digits = usize::from(s.ledger.row_count); s.uint()?;
        s.put(b",\"fixed_row_bytes\":8,\"fixed_rows_bytes\":128,\"occupied_row_bytes\":")?;
        s.digits = s.ledger.occupied_row_bytes as usize; s.uint()?;
        s.put(b",\"ledger_bytes\":248,\"row_byte_limit\":128,\"serialized_byte_limit\":1912,\"reserve_failed\":")?;
        s.put(if s.ledger.role != 0 { b"null" } else if s.ledger.failed != 0 { b"true" } else { b"false" })?;
        s.put(b",\"rows\":[")?;
        s.row_index = 0;
        while s.row_index < s.ledger.row_count as usize {
            let row = s.ledger.rows[s.row_index];
            if s.row_index != 0 { s.put(b",")?; }
            s.put(b"{\"seq\":")?; s.digits = usize::from(row.seq); s.uint()?;
            s.put(b",\"kind\":\"lexer token tape\",\"length\":")?; s.digits = row.length as usize; s.uint()?;
            s.put(b",\"element_bytes\":")?; s.digits = usize::from(row.element_bytes); s.uint()?;
            s.put(b",\"success\":")?; s.put(if row.success == 1 { b"true" } else { b"false" })?;
            s.put(b"}")?;
            s.row_index = usize::from(row.seq);
        }
        s.put(b"]}\n")
    }
    fn seal(raw: &str, destination: &mut [u8; CAP]) {
        LEDGER.with(|cell| {
            {
                let mut s = SerializeCarriers { ledger: cell.borrow_mut(), raw: raw.as_bytes(),
                    nonce: &[], case_id: &[], destination, used: 0,
                    row_index: 0, decimal: [0; 20], digits: 0 };
                assert!((s.ledger.phase == BETWEEN0 && s.ledger.requested_modes == 1)
                    || (s.ledger.phase == BETWEEN1 && s.ledger.requested_modes == 2), "UNIT4_PHASE_V1:phase.mode_order");
                assert!(s.ledger.completed_modes == s.ledger.requested_modes && s.ledger.serialized_length == 0,
                    "UNIT4_PHASE_V1:phase.mode_order");
                assert!(std::env::consts::OS == "linux" && std::env::consts::ARCH == "x86_64"
                    && usize::BITS == 64 && size_of::<Ledger>() == 248, "UNIT4_PHASE_V1:layout");
                metadata(&mut s).expect("UNIT4_PHASE_V1:metadata");
                s.ledger.nonce.copy_from_slice(s.nonce);
                s.ledger.case_id[..s.case_id.len()].copy_from_slice(s.case_id);
                s.ledger.case_len = u8::try_from(s.case_id.len()).expect("UNIT4_PHASE_V1:metadata");
                s.ledger.os = 1; s.ledger.architecture = 1; s.ledger.pointer_width = 64;
                s.used = COUNT_ONLY;
                serialize(&mut s).expect("UNIT4_PHASE_V1:serialize");
                s.ledger.serialized_length = u16::try_from(s.used & !COUNT_ONLY).expect("UNIT4_PHASE_V1:serialize");
                assert!(s.ledger.serialized_length as usize <= CAP, "UNIT4_PHASE_V1:serialize");
                s.ledger.phase = FINISHED;
            }
        });
    }
    pub(in crate::frontend) fn finish(raw: &str) {
        // No destination carrier exists until ownership and closure are checked.
        LEDGER.with(|cell| {
            let ledger = cell.borrow();
            assert!((ledger.phase == BETWEEN0 && ledger.requested_modes == 1)
                || (ledger.phase == BETWEEN1 && ledger.requested_modes == 2), "UNIT4_PHASE_V1:phase.mode_order");
            assert!(ledger.completed_modes == ledger.requested_modes && ledger.serialized_length == 0,
                "UNIT4_PHASE_V1:phase.mode_order");
        });
        let mut destination = [0u8; CAP];
        seal(raw, &mut destination);
        LEDGER.with(|cell| {
            let mut s = SerializeCarriers { ledger: cell.borrow(), raw: raw.as_bytes(),
                nonce: &[], case_id: &[], destination: &mut destination, used: 0,
                row_index: 0, decimal: [0; 20], digits: 0 };
            metadata(&mut s).expect("UNIT4_PHASE_V1:metadata");
            assert!(s.ledger.phase == FINISHED && s.ledger.token_bytes == 32 && s.ledger.reserved == 0
                && s.ledger.os == 1 && s.ledger.architecture == 1 && s.ledger.pointer_width == 64,
                "UNIT4_PHASE_V1:phase.finished");
            assert!(s.nonce == s.ledger.nonce.as_slice()
                && s.case_id == &s.ledger.case_id[..usize::from(s.ledger.case_len)], "UNIT4_PHASE_V1:metadata");
            s.used = 0;
            serialize(&mut s).expect("UNIT4_PHASE_V1:serialize");
            assert!(s.used == usize::from(s.ledger.serialized_length), "UNIT4_PHASE_V1:serialize.length");
            let mut frame = FrameCarriers { nonce: s.nonce, payload: &s.destination[..s.used], stdout: std::io::stdout().lock() };
            frame.stdout.write_all(b"UNIT4_LEXER_PHASE_V1 ").expect("UNIT4_PHASE_V1:write");
            frame.stdout.write_all(frame.nonce).expect("UNIT4_PHASE_V1:write");
            frame.stdout.write_all(b" ").expect("UNIT4_PHASE_V1:write");
            frame.stdout.write_all(frame.payload).expect("UNIT4_PHASE_V1:write");
            frame.stdout.flush().expect("UNIT4_PHASE_V1:write");
        });
    }
'''

RUST_MODULE += br'''

    // Everything below is reachable only from phase_controls. No ordinary
    // request calls these fixtures, and no fixture calls reset/run_mode/observe_request.
    fn probe_state(tokens: Vec<Token>) {
        STATE.with(|state| {
            *state.borrow_mut() = State {
                enabled: !CONTROL_ROLE, tokens, position: None,
                reject_kind: String::new(), reject_occurrence: 0,
                ..State::default()
            };
        });
    }
    fn probe_raw_lex(text: &'static str) -> Result<Vec<Token>, Box<Diagnostic>> {
        let mut sources = SourceMap::new();
        let id = sources.add("phase-probe.ox".into(), text.into());
        crate::frontend::lexer::lex_with_limit(sources.get(id), 100000)
    }
    fn probe_lex(text: &'static str) -> Result<Vec<Token>, Box<Diagnostic>> {
        probe_state(Vec::new());
        request_modes(1);
        begin_lex(CONTROL_ROLE);
        let lex = probe_raw_lex(text);
        close_lex(&lex);
        lex
    }
    fn probe_parser() {
        let tokens = probe_lex("a").expect("actual parser fixture lex");
        // Only the old collector resets. The already sealed ledger is untouched.
        probe_state(tokens);
    }
    const PROBE_RAW: &str = "{\"schema\":\"oxid-unit4-parser-raw-v1\",\"nonce\":\"00000000000000000000000000000000\",\"case_id\":\"probe\",\"source_utf8\":\"\"}";
    const MAX_RAW: &str = "{\"schema\":\"oxid-unit4-parser-raw-v1\",\"nonce\":\"00000000000000000000000000000000\",\"case_id\":\"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa\",\"source_utf8\":\"\"}";
    fn probe_metadata(raw: &str) -> bool {
        let cell = RefCell::new(Ledger::new());
        let mut destination = [0u8; CAP];
        let mut s = SerializeCarriers { ledger: cell.borrow(), raw: raw.as_bytes(), nonce: &[], case_id: &[],
            destination: &mut destination, used: 0, row_index: 0, decimal: [0; 20], digits: 0 };
        metadata(&mut s).is_ok()
    }
    fn probe_render<const N: usize>(cell: &RefCell<Ledger>, raw: &str, destination: &mut [u8; N]) -> Result<usize, ()> {
        let mut s = SerializeCarriers { ledger: cell.borrow(), raw: raw.as_bytes(), nonce: &[], case_id: &[],
            destination, used: 0, row_index: 0, decimal: [0; 20], digits: 0 };
        metadata(&mut s)?;
        s.used = COUNT_ONLY;
        serialize(&mut s)?;
        s.used = 0;
        serialize(&mut s)?;
        Ok(s.used)
    }
    fn probe_serialize_max() {
        let cell = RefCell::new(Ledger::new());
        {
            let mut ledger = cell.borrow_mut();
            ledger.role = 0; ledger.phase = FINISHED; ledger.lex_result = 2;
            ledger.requested_modes = 2; ledger.completed_modes = 2;
            ledger.row_count = 16; ledger.occupied_row_bytes = 128;
            for index in 0..16 {
                ledger.rows[index] = Row { length: 4u32 << index, element_bytes: 32,
                    success: 1, seq: (index + 1) as u8 };
            }
        }
        let mut destination = [0xa5u8; CAP];
        assert_eq!(probe_render(&cell, MAX_RAW, &mut destination), Ok(1912));
        assert_eq!(destination[CAP - 1], b'\n');
        let mut undersized = [0xa5u8; CAP - 1];
        assert_eq!(probe_render(&cell, MAX_RAW, &mut undersized), Err(()));
        assert!(undersized.iter().all(|byte| *byte == 0xa5));
        {
            let mut ledger = cell.borrow_mut();
            ledger.rows[15].success = 0; ledger.failed = 1;
        }
        assert_eq!(probe_render(&cell, MAX_RAW, &mut destination), Ok(1912));
        assert!(std::str::from_utf8(&destination).unwrap().contains("\"reserve_failed\":true"));
        {
            let mut ledger = cell.borrow_mut();
            ledger.role = 1; ledger.row_count = 0; ledger.occupied_row_bytes = 0; ledger.failed = 0;
        }
        assert_eq!(probe_render(&cell, MAX_RAW, &mut destination), Ok(572));
    }
    fn probe_metadata_bounds() {
        assert!(probe_metadata(PROBE_RAW) && probe_metadata(MAX_RAW));
        for case in ["a", "0", "a_", "a-", "0123456789abcdefghijklmnopqrstuvwxyz_-"] {
            assert!(probe_metadata(&PROBE_RAW.replace("\"probe\"", &format!("\"{case}\""))));
        }
        let prefix_end = PROBE_RAW.find("\"source_utf8\":").unwrap() + "\"source_utf8\":".len();
        for end in 0..prefix_end { assert!(!probe_metadata(&PROBE_RAW[..end])); }
        for nonce in ["0".repeat(31), "0".repeat(33), "A".repeat(32), "g".repeat(32),
                      "\u{e9}".repeat(16), format!("{}\\\"", "0".repeat(30))] {
            assert!(!probe_metadata(&PROBE_RAW.replace(&"0".repeat(32), &nonce)));
        }
        assert!(probe_metadata(&PROBE_RAW.replace(&"0".repeat(32), "0123456789abcdef0123456789abcdef")));
        for case in [String::new(), "a".repeat(65), "_a".into(), "-a".into(), "A".into(),
                     "\u{e9}".into(), "a\\\"".into(), "a\\b".into(), "a\nb".into(),
                     "a\rb".into(), "a\tb".into(), "a\0b".into(), "a.b".into()] {
            assert!(!probe_metadata(&PROBE_RAW.replace("\"probe\"", &format!("\"{case}\""))));
        }
        for (before, after) in [("parser-raw-v1", "parser-raw-v2"), ("\",\"nonce\"", "\", \"nonce\""),
                                ("case_id", "case_ix"), ("source_utf8", "source_utf9"),
                                ("\"case_id\":", "\"case_id\" :"), ("\"source_utf8\":", "\"source_utf8\" :")] {
            assert!(!probe_metadata(&PROBE_RAW.replace(before, after)));
        }
        // Escaped alternate spellings must reject even if JSON decodes alike.
        assert!(!probe_metadata(&PROBE_RAW.replace("probe", "\\u0070robe")));
        assert!(!probe_metadata(&PROBE_RAW.replace(&"0".repeat(32), &format!("\\u0030{}", "0".repeat(31)))));
    }
    fn armed(nonce: &str, probe: &str, boundary: &str) {
        println!("UNIT4_PHASE_PROBE_ARMED_V1 {nonce} {probe} {boundary}");
        std::io::stdout().flush().expect("phase probe marker flush");
    }
    fn probe_layout(nonce: &str, probe: &str) {
        assert_eq!(size_of::<SerializeCarriers<'static, Ref<'static, Ledger>, CAP>>(),
            size_of::<SerializeCarriers<'static, RefMut<'static, Ledger>, CAP>>());
        assert_eq!(align_of::<SerializeCarriers<'static, Ref<'static, Ledger>, CAP>>(),
            align_of::<SerializeCarriers<'static, RefMut<'static, Ledger>, CAP>>());
        println!(concat!("UNIT4_PHASE_PROBE_LAYOUT_V1 {} {} layout ",
            "{{\"row_bytes\":{},\"row_align\":{},\"rows_bytes\":{},\"rows_align\":{},",
            "\"ledger_bytes\":{},\"ledger_align\":{},\"refcell_bytes\":{},\"refcell_align\":{},",
            "\"ref_bytes\":{},\"ref_align\":{},\"refmut_bytes\":{},\"refmut_align\":{},",
            "\"token_bytes\":{},\"token_align\":{},\"capture_bytes\":{},\"capture_align\":{},",
            "\"serialize_bytes\":{},\"serialize_align\":{},\"token_forward_bytes\":{},\"token_forward_align\":{},",
            "\"stdout_lock_bytes\":{},\"stdout_lock_align\":{},\"frame_bytes\":{},\"frame_align\":{},\"named_total_bytes\":{}}}"),
            nonce, probe, size_of::<Row>(), align_of::<Row>(), size_of::<[Row; 16]>(), align_of::<[Row; 16]>(),
            size_of::<Ledger>(), align_of::<Ledger>(), size_of::<RefCell<Ledger>>(), align_of::<RefCell<Ledger>>(),
            size_of::<Ref<'static, Ledger>>(), align_of::<Ref<'static, Ledger>>(),
            size_of::<RefMut<'static, Ledger>>(), align_of::<RefMut<'static, Ledger>>(),
            size_of::<Token>(), align_of::<Token>(), size_of::<CaptureCarriers<'static, 'static>>(), align_of::<CaptureCarriers<'static, 'static>>(),
            size_of::<SerializeCarriers<'static, Ref<'static, Ledger>, CAP>>(), align_of::<SerializeCarriers<'static, Ref<'static, Ledger>, CAP>>(),
            size_of::<TokenForwardCarriers<'static>>(), align_of::<TokenForwardCarriers<'static>>(),
            size_of::<StdoutLock<'static>>(), align_of::<StdoutLock<'static>>(), size_of::<FrameCarriers<'static>>(), align_of::<FrameCarriers<'static>>(),
            size_of::<RefCell<Ledger>>() + CAP + size_of::<CaptureCarriers<'static, 'static>>()
                + size_of::<SerializeCarriers<'static, Ref<'static, Ledger>, CAP>>()
                + size_of::<TokenForwardCarriers<'static>>() + 2 * size_of::<Row>() + size_of::<FrameCarriers<'static>>()
                + size_of::<Token>());
    }
    #[test]
    #[ignore]
    fn phase_controls() {
        let probe = std::env::var("UNIT4_PHASE_PROBE").expect("closed probe ID");
        let nonce = std::env::var("UNIT4_PHASE_NONCE").expect("closed probe nonce");
        let role = std::env::var("UNIT4_PHASE_ROLE").expect("closed probe role");
        assert!(nonce.len() == 32 && nonce.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)));
        assert_eq!(role, if CONTROL_ROLE { "not_observed" } else { "observed" });
        let boundary = match probe.as_str() {
            "layout" => { probe_layout(&nonce, &probe); return; }
            "inactive_noop" => {
                let tokens = probe_raw_lex("a").expect("actual inactive fixture");
                probe_state(tokens);
                STATE.with(|state| {
                    // Retaining an immutable old-State borrow makes any attempt
                    // by either inactive hook to mutate that State fail the test.
                    let original = state.borrow();
                    reserve("inactive probe", 7, 11, false);
                    lex_token(original.tokens[0]);
                    assert!(original.position.is_none() && original.events.is_empty() && original.bytes == 0);
                });
                LEDGER.with(|cell| {
                    let ledger = cell.borrow();
                    assert!(ledger.phase == INACTIVE && ledger.row_count == 0 && ledger.token_count == 0);
                });
                "inactive"
            }
            "metadata_bounds" => { probe_metadata_bounds(); "metadata.bounds" }
            "control_not_observed" if CONTROL_ROLE => {
                assert!(probe_lex("").is_ok()); begin_mode(0); close_mode(0);
                let mut destination = [0u8; CAP]; seal(PROBE_RAW, &mut destination);
                LEDGER.with(|cell| {
                    let ledger = cell.borrow();
                    assert!(ledger.role == 1 && ledger.row_count == 0 && ledger.token_count == 0
                        && ledger.occupied_row_bytes == 0 && ledger.failed == 0 && ledger.phase == FINISHED);
                    drop(ledger);
                    let count = probe_render(cell, PROBE_RAW, &mut destination).unwrap();
                    let payload = std::str::from_utf8(&destination[..count]).unwrap();
                    assert!(payload.contains("\"role\":\"not_observed\"") && payload.contains("\"reserve_failed\":null"));
                });
                "control.empty"
            }
            "control_callback_reject" if CONTROL_ROLE => {
                probe_state(Vec::new()); request_modes(1); begin_lex(true);
                armed(&nonce, &probe, "phase.control"); reserve("lexer token tape", 4, size_of::<Token>(), true);
                panic!("expected phase rejection did not occur");
            }
            _ if CONTROL_ROLE => panic!("probe is ineligible for control role"),
            "lex_ok_seal" => {
                let tokens = probe_lex("").expect("empty source lex");
                assert_eq!(tokens.len(), 1); assert_eq!(tokens[0].kind, Kind::Eof);
                LEDGER.with(|cell| {
                    let ledger = cell.borrow();
                    assert!(ledger.phase == LEXED && ledger.lex_result == 1 && ledger.token_count == 1
                        && ledger.row_count == 1 && ledger.rows[0].length == 4 && ledger.rows[0].success == 1);
                }); "lex.ok"
            }
            "lex_err_empty" => {
                assert!(probe_lex("/*").is_err());
                LEDGER.with(|cell| {
                    let ledger = cell.borrow();
                    assert!(ledger.phase == LEXED && ledger.lex_result == 2 && ledger.row_count == 0
                        && ledger.token_count == 0 && ledger.failed == 0);
                }); "lex.err"
            }
            "lex_failed_terminal" => {
                probe_state(Vec::new()); request_modes(1); begin_lex(false);
                reserve("lexer token tape", 4, size_of::<Token>(), false);
                let lex = probe_raw_lex("/*"); assert!(lex.is_err()); close_lex(&lex);
                LEDGER.with(|cell| {
                    let ledger = cell.borrow();
                    assert!(ledger.phase == LEXED && ledger.failed == 1 && ledger.row_count == 1
                        && ledger.rows[0].success == 0 && ledger.token_count == 0 && ledger.lex_result == 2);
                }); "lex.failed"
            }
            "parser_forward" => {
                probe_parser(); begin_mode(0);
                STATE.with(|state| { let token = state.borrow().tokens[0]; super::position(0, token); });
                reserve("phase probe", 7, 11, true);
                STATE.with(|state| {
                    let state = state.borrow();
                    assert_eq!(state.reserve_calls, 1); assert_eq!(state.events.len(), 1);
                    assert!(state.events[0].contains("\"detail\":{\"kind\":\"phase probe\",\"length\":7,\"element_bytes\":11,\"success\":true}"));
                    assert_eq!(state.position.unwrap().0, 0);
                    assert_eq!(state.position.unwrap().1.kind, state.tokens[0].kind);
                    assert_eq!(state.position.unwrap().1.span, state.tokens[0].span);
                }); close_mode(0); "parser.forward"
            }
            "parser_missing_position" => {
                probe_parser(); begin_mode(0); armed(&nonce, &probe, "parser.position");
                reserve("phase probe", 7, 11, true); panic!("expected original position rejection did not occur");
            }
            "nonlexer_in_lex" => {
                probe_state(Vec::new()); request_modes(1); begin_lex(false); armed(&nonce, &probe, "phase.nonlexer_lex");
                reserve("phase probe", 4, size_of::<Token>(), true); panic!("expected phase rejection did not occur");
            }
            "lexer_in_parse" => {
                probe_parser(); begin_mode(0); armed(&nonce, &probe, "phase.lexer_parse");
                reserve("lexer token tape", 4, size_of::<Token>(), true); panic!("expected phase rejection did not occur");
            }
            "unowned_active" => {
                probe_parser(); armed(&nonce, &probe, "phase.unowned");
                reserve("phase probe", 7, 11, true); panic!("expected phase rejection did not occur");
            }
            "nested_lex" => {
                probe_state(Vec::new()); request_modes(1); begin_lex(false); armed(&nonce, &probe, "phase.nested_lex");
                begin_lex(false); panic!("expected phase rejection did not occur");
            }
            "parse_before_close" => {
                probe_state(Vec::new()); request_modes(1); begin_lex(false); armed(&nonce, &probe, "phase.unclosed_lex");
                begin_mode(0); panic!("expected phase rejection did not occur");
            }
            "after_fail_reserve" => {
                probe_state(Vec::new()); request_modes(1); begin_lex(false); reserve("lexer token tape", 4, size_of::<Token>(), false);
                armed(&nonce, &probe, "phase.reserve_after_fail"); reserve("lexer token tape", 8, size_of::<Token>(), false);
                panic!("expected phase rejection did not occur");
            }
            "after_fail_token" => {
                let tokens = probe_raw_lex("a").expect("actual terminal-token fixture");
                probe_state(Vec::new()); request_modes(1); begin_lex(false); reserve("lexer token tape", 4, size_of::<Token>(), false);
                armed(&nonce, &probe, "phase.token_after_fail"); lex_token(tokens[0]);
                panic!("expected phase rejection did not occur");
            }
            "failed_then_ok" => {
                let tokens = probe_raw_lex("").expect("actual failed-ok fixture");
                probe_state(Vec::new()); request_modes(1); begin_lex(false); reserve("lexer token tape", 4, size_of::<Token>(), false);
                armed(&nonce, &probe, "phase.failed_ok"); close_lex(&Ok(tokens));
                panic!("expected phase rejection did not occur");
            }
            "event_cap" => {
                probe_state(Vec::new()); request_modes(1); begin_lex(false);
                for index in 0..16 { reserve("lexer token tape", 4usize << index, size_of::<Token>(), true); }
                armed(&nonce, &probe, "cap.events"); reserve("lexer token tape", 262144, size_of::<Token>(), true);
                panic!("expected sink rejection did not occur");
            }
            "byte_cap" => {
                probe_state(Vec::new()); request_modes(1); begin_lex(false);
                LEDGER.with(|cell| {
                    let mut c = CaptureCarriers { ledger: cell.borrow_mut(), kind: "lexer token tape", length: 4,
                        element_bytes: size_of::<Token>(), pending: EMPTY_ROW, next_count: 0, next_bytes: 0, success: true };
                    // Private checked-charge overflow; no normal-entrypoint seam.
                    c.ledger.occupied_row_bytes = u32::MAX - 3;
                    armed(&nonce, &probe, "cap.bytes"); charge(&mut c);
                }); panic!("expected sink rejection did not occur");
            }
            "serialize_max" => { probe_serialize_max(); "serialize.max" }
            "mode_order" => {
                probe_parser(); armed(&nonce, &probe, "phase.mode_order"); begin_mode(1);
                panic!("expected phase rejection did not occur");
            }
            "double_close" => {
                let lex = probe_lex(""); armed(&nonce, &probe, "phase.double_close"); close_lex(&lex);
                panic!("expected phase rejection did not occur");
            }
            "finished_callback" => {
                probe_parser(); begin_mode(0); close_mode(0);
                let mut destination = [0u8; CAP]; seal(PROBE_RAW, &mut destination);
                armed(&nonce, &probe, "phase.finished"); reserve("phase probe", 7, 11, true);
                panic!("expected phase rejection did not occur");
            }
            _ => panic!("unknown closed probe ID"),
        };
        println!("UNIT4_PHASE_PROBE_OK_V1 {nonce} {probe} {boundary}");
    }
}
'''
