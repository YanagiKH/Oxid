"""Actual closed historical grammar controls, separate from frozen observations."""
import json
from pathlib import Path
import re
import uuid

CASES = {
    'narrow': 'fn main() -> i32 { let x = 255; let b = x.to_u8_checked(); return 0; }',
    'widen': 'fn f(b: u8) -> i32 { return b.to_i32(); }',
    'trivia': 'fn main() -> i32 { let x = 1; x /*a*/ . /*b*/ to_u8_checked /*c*/ ( ); return 0; }',
    'numeric': 'fn main() -> i32 { return 255.to_u8_checked(); }',
}
OBSERVER_TEST = 'frontend::parser::unit4_observer::observe_request'
PUBLIC_TEST = ('frontend::parser::u8_syntax_tests::'
               'u8_named_conversion_has_a_real_child_and_exact_trivia_origins_on_every_mode')


def execute(api, session_path, a):
    session, root = api.session_at(session_path, a)
    out = api.fresh(root / 'u8-policy-controls')
    rows = []
    for profile in ('debug', 'release'):
        for control in (False, True):
            build, _ = api.verify_build(root, session_path, profile, a, control)
            for name, source in CASES.items():
                directory = out / (profile + ('-control-' if control else '-observer-') + name)
                directory.mkdir()
                path = directory / 'source.ox'; path.write_text(source, encoding='utf-8')
                nonce = uuid.uuid4().hex
                env = api.minimal_env(root / 'home')
                env.update(UNIT4_SOURCE=str(path), UNIT4_RAW_OUTPUT=str(directory / 'raw.json'),
                           UNIT4_DISPLAY_PATH='u8-policy.ox', UNIT4_NONCE=nonce,
                           UNIT4_CASE_ID=name, UNIT4_CONTROL=str(int(control)),
                           UNIT4_ORIGINAL='1', UNIT4_REJECT_KIND='', UNIT4_REJECT_OCCURRENCE='0',
                           UNIT4_RESERVE_FAIL_AT='0', UNIT4_DIRECT='0',
                           UNIT4_TOKEN_LIMIT='200000', UNIT4_NODE_LIMIT='200000')
                api.invoke([build['binary']['path'], '--exact', OBSERVER_TEST,
                            '--ignored', '--nocapture'], root, env, directory, session['host'])
                rows.append({'profile': profile, 'control': control, 'case': name, 'nonce': nonce,
                             'binary': build['binary'], 'source': api.identity(path),
                             'invocation': api.identity(directory / 'invocation.json'),
                             'raw': api.identity(directory / 'raw.json')})
            if control:
                directory = out / (profile + '-enabled-public'); directory.mkdir()
                api.invoke([build['binary']['path'], '--exact', PUBLIC_TEST, '--nocapture'], root,
                           api.minimal_env(root / 'home'), directory, session['host'])
                rows.append({'profile': profile, 'control': True, 'case': 'enabled-public',
                             'binary': build['binary'],
                             'invocation': api.identity(directory / 'invocation.json')})
    receipt = {'schema': 'oxid-unit4-u8-policy-controls-v1', 'session': api.identity(session_path),
               'closed_policy': a['current']['u8_closed_policy'], 'cases': rows,
               'historical_observations_changed': False, 'closed_refusal_observations': 32,
               'enabled_public_tests': 2}
    verify(api, receipt, api.artifact, expected_session=session)
    api.write(out / 'receipt.json', receipt)
    return receipt


def verify(api, receipt, raw, expected_binaries=None, expected_session=None):
    api.same(receipt['schema'], 'oxid-unit4-u8-policy-controls-v1', 'u8 policy control schema')
    expected = [(profile, control, case) for profile in ('debug', 'release')
                for control in (False, True)
                for case in [*CASES, *(['enabled-public'] if control else [])]]
    api.same([(r['profile'], r['control'], r['case']) for r in receipt['cases']], expected,
             'complete ordered closed/enabled policy controls')
    nonces = set()
    for row in receipt['cases']:
        if expected_binaries is not None:
            api.same(row['binary'], expected_binaries[(row['profile'], row['control'])],
                     'u8 policy controls use exact fresh parser build')
        invocation = json.loads(raw(row['invocation']))
        public = row['case'] == 'enabled-public'
        api.same(invocation['argv'], [row['binary']['path'], '--exact',
                 PUBLIC_TEST if public else OBSERVER_TEST,
                 *([] if public else ['--ignored']), '--nocapture'], 'exact u8 policy invocation')
        api.same(invocation['exit_code'], 0, 'u8 policy execution success')
        api.same(raw(invocation['stderr']), b'', 'u8 policy execution stderr')
        if expected_session is not None:
            api.same(invocation['host'], expected_session['host'], 'u8 policy actual session host')
            api.same(invocation['cwd'], expected_session['root'], 'u8 policy actual session cwd')
            api.require(expected_session['prepared_ns'] <= invocation['started_ns'] <= invocation['finished_ns'],
                        'u8 policy execution follows current preparation')
            expected_env = api.minimal_env(Path(expected_session['root']) / 'home')
            if not public:
                expected_env.update(UNIT4_SOURCE=row['source']['path'], UNIT4_RAW_OUTPUT=row['raw']['path'],
                    UNIT4_DISPLAY_PATH='u8-policy.ox', UNIT4_NONCE=row['nonce'], UNIT4_CASE_ID=row['case'],
                    UNIT4_CONTROL=str(int(row['control'])), UNIT4_ORIGINAL='1', UNIT4_REJECT_KIND='',
                    UNIT4_REJECT_OCCURRENCE='0', UNIT4_RESERVE_FAIL_AT='0', UNIT4_DIRECT='0',
                    UNIT4_TOKEN_LIMIT='200000', UNIT4_NODE_LIMIT='200000')
            api.same(invocation['environment'], expected_env, 'exact u8 policy request environment')
        stdout = raw(invocation['stdout']).decode()
        api.require(re.search(r'test result: ok\. 1 passed; 0 failed; 0 ignored;', stdout) is not None,
                    'u8 policy test actually executed')
        if public:
            continue
        api.same(raw(row['source']), CASES[row['case']].encode(), 'exact u8 policy source')
        nonce = row['nonce']
        api.require(re.fullmatch('[0-9a-f]{32}', nonce) is not None and nonce not in nonces,
                    'unique u8 policy execution nonce')
        nonces.add(nonce)
        api.same(stdout.count('UNIT4_EXECUTED ' + nonce), 1, 'actual u8 policy observation marker')
        observed = json.loads(raw(row['raw']))
        api.same(observed['nonce'], nonce, 'u8 policy raw nonce')
        api.same(observed['case_id'], row['case'], 'u8 policy raw case')
        api.same(observed['display_path'], 'u8-policy.ox', 'u8 policy exact display path')
        api.same(observed['source_utf8'], CASES[row['case']], 'u8 policy raw source')
        api.same([r['mode'] for r in observed['observations']], ['ProjectCandidate', 'OwnedCandidate'],
                 'both historical parser modes')
        for result in observed['observations']:
            api.require(result['executed'] is True and result['result'] == 'parse_error'
                        and result['ast'] is None and len(result['diagnostics']) > 0,
                        'closed historical grammar must reject new conversion syntax')
    api.same(receipt['closed_refusal_observations'], 32, 'complete u8 closed refusal count')
    api.same(receipt['enabled_public_tests'], 2, 'both enabled public profiles')
    api.same(receipt['historical_observations_changed'], False, 'historical observations remain separate')
    return {'closed_refusal_observations': 32, 'enabled_public_tests': 2}
