"""Sealed current-only lexer/budget hooks, around retained public composers."""
from pathlib import Path
import types

HERE = Path(__file__).resolve().parent
PREDECESSOR_SHA = '446f9c022dd7ae3070a9fe8a07e4973fdbb12f4623b8a293b6826ed3ef576c5b'
FIELDS = ('parser_predecessor', 'source_predecessor', 'source_authority', 'source_patch', 'observer_adapter', 'helper')


def previous(p, active):
    binding = active['lexer_reservation']
    p.same(set(binding), set(FIELDS), 'exact lexer parser bridge fields')
    p.same(binding['parser_predecessor']['path'], 'tests/qualification/unit4_parser_current/byte-storage-authority-v1.json', 'exact lexer parser predecessor path')
    p.same(binding['parser_predecessor']['sha256'], PREDECESSOR_SHA, 'immutable byte storage parser predecessor')
    p.verify_map(p.REPOSITORY, list(binding.values()))
    before = p.read(p.REPOSITORY / binding['parser_predecessor']['path'])
    omit = {'current_base_files', 'current_derived_files', 'current_control_derived_files',
            'current_candidate_source_manifest_sha256', 'current_source_manifest', 'reviewed_source_head',
            'source_only_tree', 'source_binding_runner', 'source_delta', 'lexer_reservation'}
    p.same({k: v for k, v in active.items() if k not in omit},
           {k: v for k, v in before.items() if k not in omit}, 'lexer changes unrelated parser authority')
    before['source_binding_runner'] = active['source_binding_runner']
    return before


def restore(p, active, inputs):
    prior = previous(p, active)
    binding = active['lexer_reservation']
    api = p.u8_binding_api(active)
    patch = (p.REPOSITORY / binding['source_patch']['path']).read_bytes()
    transition = p.read(p.REPOSITORY / binding['source_authority']['path'])
    p.same(transition['current_source_sha256'], active['current_source_manifest']['sha256'], 'parser outer source binding')
    p.same(transition['reviewed_source_head'], active['reviewed_source_head'], 'parser outer source head')
    p.same(transition['source_only_tree'], active['source_only_tree'], 'parser outer source tree')
    try:
        restored, touched = api.apply_inverse_patch(inputs, patch, p.sha(patch), len(patch), tuple(transition['transition_paths']))
    except api.BindingError as error:
        raise p.Rejected('lexer parser inverse rejected: ' + str(error)) from error
    p.same(list(touched), transition['transition_paths'], 'parser outer inverse scope')
    source = p.read(p.REPOSITORY / binding['source_predecessor']['path'])
    p.same([{'path': n, 'bytes': len(b), 'sha256': p.sha(b)} for n, b in sorted(restored.items())],
           source['files'], 'complete lexer parser inverse identity')
    p.same(source['reviewed_source_head'], prior['reviewed_source_head'], 'parser predecessor checkpoint')
    return restored


def body(p, a, name, raw):
    active = a['current']
    prior = previous(p, active)
    before = next(r for r in prior['current_base_files'] if r['path'] == name)
    after = next(r for r in active['current_base_files'] if r['path'] == name)
    p.same({'path': name, 'bytes': len(raw), 'sha256': p.sha(raw)}, after, 'current lexer composition identity')
    if before != after:
        api = p.u8_binding_api(active)
        patch = (p.REPOSITORY / active['lexer_reservation']['source_patch']['path']).read_bytes()
        prefix = ('a/' + name + ' b/' + name + '\n').encode()
        sections = [b'diff --git ' + part for part in patch.split(b'diff --git ')[1:] if part.startswith(prefix)]
        p.same(len(sections), 1, 'exact lexer composition section')
        section = sections[0]
        try:
            restored, touched = api.apply_inverse_patch({name: raw}, section, p.sha(section), len(section), (name,))
        except api.BindingError as error:
            raise p.Rejected('lexer parser body inverse rejected: ' + str(error)) from error
        p.same(list(touched), [name], 'exact lexer composition inverse member')
        raw = restored[name]
    p.same({'path': name, 'bytes': len(raw), 'sha256': p.sha(raw)}, before, 'lexer composition predecessor identity')
    return dict(a, current=prior), raw


def adapter(p, active):
    previous(p, active)
    path = p.REPOSITORY / active['lexer_reservation']['observer_adapter']['path']
    module = types.ModuleType('unit4_current_lexer_hooks')
    exec(compile(path.read_bytes(), str(path), 'exec'), module.__dict__)
    return module


def compose(p, a, name, raw):
    prior, old_raw = body(p, a, name, raw)
    old = (p.compose_division_lexer(prior, old_raw) if name == 'src/frontend/lexer.rs'
           else p.compose_array_instrumentation(prior, name, old_raw))
    old_expected = next(r for r in prior['current']['current_derived_files'] if r['path'] == name)
    p.same({'path': name, 'bytes': len(old), 'sha256': p.sha(old)}, old_expected,
           'current lexer hooks preserve full predecessor composition')
    base = next(r for r in a['current']['current_base_files'] if r['path'] == name)
    expected = next(r for r in a['current']['current_derived_files'] if r['path'] == name)
    api = adapter(p, a['current'])
    try:
        if name == 'src/frontend/lexer.rs':
            derived, _ = api.instrument_lexer(raw, base, expected, 'token')
        else:
            p.same(name, 'src/frontend/project/budget.rs', 'current lexer hook scope')
            derived, _ = api.instrument_budget(raw, base, expected)
    except api.Reject as error:
        raise p.Rejected('unapproved current derived map: ' + str(error)) from error
    return derived
