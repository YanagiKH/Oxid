#!/usr/bin/env python3
"""Comparison predicates for the separately approved replacement-v3 contract."""
import sys
sys.dont_write_bytecode = True
import json
from contracts import Reject, need, sha

DIAGNOSTIC_FIELDS = {'schema_version', 'edition', 'kind', 'severity', 'code', 'stage', 'message', 'primary', 'secondary', 'notes'}

def envelope(process, operation):
    need(process['status'] in (0, 1) and not process['timed_out'], 'process completion')
    need(process['stderr'] == '', 'JSON stderr')
    for stream in ('stdout', 'stderr'):
        need(sha(process[stream].encode()) == process[stream + '_sha256'], 'stream identity')
    lines = process['stdout'].splitlines(keepends=True)
    need(lines and all(line.endswith('\n') and line.strip() for line in lines), 'JSON line framing')
    objs = [json.loads(line) for line in lines]
    for obj in objs:
        need(type(obj['schema_version']) is int and obj['schema_version'] == 1 and obj['edition'] == 'typed-preview', 'JSON schema/edition')
    summary, diagnostics = objs[-1], objs[:-1]
    field = {'check': 'functions', 'run': 'result', 'compile': 'output'}[operation]
    need(set(summary) == {'schema_version', 'edition', 'kind', 'success', 'errors', field}, 'summary fields')
    need(summary['kind'] == operation + '-summary' and type(summary['errors']) is int and summary['errors'] == len(diagnostics), 'summary kind/error count')
    need(summary['success'] is (process['status'] == 0), 'summary success')
    need((not diagnostics) == summary['success'], 'diagnostic/summary consistency')
    if not summary['success']:
        need(summary[field] is None, 'failure payload')
    elif operation == 'check':
        need(type(summary[field]) is int and summary[field] >= 0, 'function count type/range')
    elif operation == 'run':
        result = summary[field]
        need(isinstance(result, dict) and result.get('type') in ('i32', 'bool', 'unit'), 'run result type')
        if result['type'] == 'unit':
            need(set(result) == {'type'}, 'unit result fields')
        else:
            need(set(result) == {'type', 'value'}, 'run result fields')
            need(type(result['value']) is (bool if result['type'] == 'bool' else int), 'run scalar type')
            if result['type'] == 'i32':
                need(-2147483648 <= result['value'] <= 2147483647, 'i32 result range')
    for diagnostic in diagnostics:
        need(set(diagnostic) == DIAGNOSTIC_FIELDS, 'diagnostic fields')
        need(diagnostic['kind'] == 'diagnostic' and diagnostic['severity'] == 'error', 'diagnostic kind/severity')
        need(all(isinstance(diagnostic[k], str) and diagnostic[k] for k in ('code', 'stage')), 'diagnostic code/stage schema')
        need(isinstance(diagnostic['message'], str) and diagnostic['message'], 'diagnostic message')
        need(isinstance(diagnostic['secondary'], list) and isinstance(diagnostic['notes'], list), 'diagnostic labels/notes')
    return summary, diagnostics

def location(actual, expected):
    if expected is None:
        need(actual is None, 'expected absent location')
        return
    need(actual is not None, 'missing location')
    names = {'file': 'file_id', 'path': 'path', 'start': 'start', 'end': 'end', 'line': 'line',
             'scalar_column': 'column', 'end_line': 'end_line', 'end_scalar_column': 'end_column'}
    need(set(expected) <= set(names), 'unimplemented expected location field')
    need({key: actual[names[key]] for key in expected} == expected, 'diagnostic source location')

def diagnostic_vector(actual, expected):
    need(len(actual) == len(expected), 'diagnostic count')
    for got, want in zip(actual, expected):
        need(got['code'] == want['code'] and got['stage'] == want['stage'], 'diagnostic code/stage')
        location(got['primary'], want['location'])
        need(len(got['secondary']) == len(want['related_locations']), 'secondary count')
        for label, span in zip(got['secondary'], want['related_locations']):
            location(label['span'], span)
        if 'literal_message' in want:
            need(got['message'] == want['literal_message'], 'literal diagnostic message')

def policy_bounds(diagnostics, policy):
    pairs = {(r['code'], r['stage']) for r in policy['applies_to']}
    for got in diagnostics:
        if (got['code'], got['stage']) not in pairs:
            continue
        need(0 < len(got['message'].encode()) <= policy['message_utf8_bytes_max'], 'policy message byte bound')
        need(len(got['secondary']) <= policy['secondary_label_count_max'], 'policy secondary count')
        need(all(isinstance(r['message'], str) and len(r['message'].encode()) <= policy['secondary_label_message_utf8_bytes_max'] for r in got['secondary']), 'policy label byte bound')
        need(len(got['notes']) <= policy['note_count_max'], 'policy note count')
        need(all(isinstance(note, str) and len(note.encode()) <= policy['note_utf8_bytes_max'] for note in got['notes']), 'policy note byte bound')

def public(row, process, policy):
    case, obs = row['case'], row['observation']
    operation = row['argv_template'][1]
    if row['group'] == 'literal_cases':
        for field in ('status', 'stdout', 'stderr'):
            need(process[field] == obs[field], 'complete literal ' + field)
        need(not process['timed_out'], 'literal timeout')
        if row['format'] == 'json':
            _, diagnostics = envelope(process, operation)
            policy_bounds(diagnostics, policy)
        return process['status'] == 0
    summary, diagnostics = envelope(process, operation)
    policy_bounds(diagnostics, policy)
    if 'expected_public_projection' in case:
        expected = case['expected_public_projection']
        need(expected['kind'] == 'first-diagnostic-only', 'unknown root projection')
        need(process['status'] == expected['status'] == 1 and 1 <= len(diagnostics) <= expected['diagnostic_cap'], 'root parse failure/cap')
        diagnostic_vector(diagnostics[:1], [expected['first_diagnostic']])
        need(summary['success'] is False, 'root parse summary')
        return False
    expected = case['literal_accepted_expected_projection']
    rejection = expected if expected.get('status') == 'reject' else expected.get('native' if operation == 'compile' else operation)
    if isinstance(rejection, dict):
        need(process['status'] == 1, 'expected rejection')
        diagnostic_vector(diagnostics, rejection.get('reachable_diagnostics', [rejection]))
        return False
    need(process['status'] == 0 and not diagnostics, 'expected acceptance')
    if operation == 'check':
        need(summary['functions'] == case['expected_function_count'], 'accepted function count')
    elif operation == 'run':
        need(summary['result'] == {'type': expected['result_type'], 'value': expected['result']}, 'accepted run result')
    else:
        need(summary['output'] == 'out.bin', 'accepted compile output')
    return True

def lifecycle(trace, expected, counters, file_counters):
    need(isinstance(trace, list), 'trace not list')
    phases = {key: sum(event['event'] == key for event in trace) for key in counters}
    need(phases == expected['expected_lifecycle'], 'exact logical phase counters')
    files = {}
    for event in trace:
        need(set(event) == {'event', 'subject'}, 'trace fields')
        kind, subject = event['event'], event['subject']
        need(kind in counters + file_counters + ['consumer_entry'], 'unknown event')
        if kind in file_counters:
            files.setdefault(subject, {key: 0 for key in file_counters})[kind] += 1
    wanted_files = {name: counts for name, counts in expected['file_counts'].items() if any(counts.values())}
    need(files == wanted_files, 'exact per-source logical counters')
    for subject, counts in files.items():
        actual = [e['event'] for e in trace if e['subject'] == subject and e['event'] in file_counters]
        wanted = [key for key in file_counters for _ in range(counts[key])]
        need(actual == wanted, 'per-source stage ordering')
    order = ['checker_attempts', 'route_attempts', 'route_completions', 'index_attempts', 'index_completions', 'checked_program_completions', 'consumer_entry']
    counts = {**phases, 'consumer_entry': phases['checked_program_completions']}
    need([e['event'] for e in trace if e['event'] in order] == [key for key in order for _ in range(counts[key])], 'phase/consumer ordering')
    if phases['checker_attempts']:
        first = next(i for i, event in enumerate(trace) if event['event'] == 'checker_attempts')
        need(not any(event['event'] in file_counters for event in trace[first:]), 'consumer source reopen or second parse')
    if expected['expected_route'] is not None:
        need([event['subject'] for event in trace if event['event'] == 'route_completions'] == [expected['expected_route']], 'route subject')
    if expected['expected_flavor'] is not None:
        need([event['subject'] for event in trace if event['event'] == 'checker_attempts'] == [expected['expected_flavor']], 'flavor subject')
    return {'phase_counts': phases, 'file_counts': files, 'consumer_source_reopens': 0, 'semantics': 'logical stage counters; not syscalls'}

def valid_origins(diagnostics, sources):
    """Independently validate every emitted source coordinate, including partial rows."""
    ids = {}
    paths = {}
    for diagnostic in diagnostics:
        spans = [diagnostic['primary']]
        for label in diagnostic['secondary']:
            need(set(label) == {'span', 'message'} and label['span'] is not None and isinstance(label['message'], str), 'secondary label shape')
            spans.append(label['span'])
        need(all(isinstance(note, str) for note in diagnostic['notes']), 'note shape')
        for span in spans:
            if span is None:
                continue
            need(set(span) == {'file_id', 'path', 'start', 'end', 'line', 'column', 'end_line', 'end_column'}, 'source origin shape')
            name = span['path']
            need(name in sources, 'origin names undeclared source')
            need(all(type(span[k]) is int for k in ('file_id', 'start', 'end', 'line', 'column', 'end_line', 'end_column')), 'origin integer fields')
            need(type(span['file_id']) is int and 0 <= span['file_id'] < len(sources), 'origin file ID range')
            need(ids.setdefault(span['file_id'], name) == name and paths.setdefault(name, span['file_id']) == span['file_id'], 'origin file/path consistency')
            data = sources[name]
            start, end = span['start'], span['end']
            need(type(start) is int and type(end) is int and 0 <= start <= end <= len(data), 'origin byte range')
            for offset, line, column in ((start, span['line'], span['column']), (end, span['end_line'], span['end_column'])):
                prefix = data[:offset].decode('utf-8')
                need(line == prefix.count('\n') + 1 and column == len(prefix.rsplit('\n', 1)[-1]) + 1, 'origin scalar coordinates')
