#!/usr/bin/env python3
"""Exact source-only current token/reserve controls, no semantic qualification."""
import sys
sys.dont_write_bytecode = True
import ast
import copy
import json
from pathlib import Path
import unittest
from unittest.mock import patch
import portable as p

_PARSER_CONTROL_PINS = {'authority': {'path': 'tests/qualification/unit4_parser_current/authority.json', 'bytes': 526829, 'sha256': '900f9e7cfb32027b4406e2860c5bc39abb0d831c89931efaf166911905c35ca4'}, 'byte_storage_portable': {'bytes': 211696, 'path': 'tests/qualification/unit4_parser_current/byte_storage_portable_v1.py', 'sha256': '2efcbd2f11ee0b9fee041c19b362459b15f7d843b16bc55f8743a938c58210cd'}, 'derivation': {'bytes': 9886, 'path': 'tests/qualification/published_formatter_current/derive_parser_successor.py', 'sha256': '2d5427b6f3bd4dfa0cfec6ad7723725c0122a70ccc033a34c41d728470d76bf6'}, 'fixture': {'path': 'tests/qualification/published_formatter_current/parser-control-fixture-v1.json', 'bytes': 147076, 'sha256': '764ebae62589ba322bfd35d1d79cfcabcfd072683e0e99cd5f370c50c1d98322'}, 'lexer_prephase_portable': {'bytes': 215213, 'path': 'scripts/predecessors/lexer_reservation_source_v1/tests/qualification/unit4_parser_current/portable.py', 'sha256': '86577f7fb47c9514c55adb12900953ffd6d329d2666228aae40ec3d9cd8f1de2'}, 'portable': {'path': 'tests/qualification/unit4_parser_current/portable.py', 'bytes': 237794, 'sha256': '7cb4a8f8c8775d2dd0624fa863fb2dda05410ad42f43a2daddc791a6e1864dfc'}, 'predecessor_view': {'bytes': 241022, 'path': 'tests/qualification/published_formatter_current/predecessor-view-v1.json', 'sha256': 'e262dd17ebae52f7d662a1408420b795607e3ef6ae4612b1fe0a1a33b15081c8'}, 'retained_current_tests': {'bytes': 142428, 'path': 'tests/qualification/published_formatter_current/predecessor-v1/tests/qualification/unit4_parser_current/test_current.py', 'sha256': '0271a71075e3561ff84bbe5820d5560f82c57e4aa82a0d7812411348abb9425b'}, 'retained_lexer_tests': {'bytes': 5317, 'path': 'tests/qualification/published_formatter_current/predecessor-v1/tests/qualification/unit4_parser_current/test_lexer_reservation.py', 'sha256': 'f4fada86d3088cdf6fb8cb7fd54f0fd36cbe54ed71f58536cc0bcffd155056a9'}, 'retained_portable': {'bytes': 244286, 'path': 'tests/qualification/published_formatter_current/predecessor-v1/tests/qualification/unit4_parser_current/portable.py', 'sha256': '3366c49f267ab98bd792a1cd1b01ebbaaca548bc19685d841eb688cf39f99162'}, 'schema': 'oxid-published-parser-control-pins-v1', 'support': {'bytes': 22884, 'path': 'tests/qualification/published_formatter_current/published_parser.py', 'sha256': '415cb1b9391cd330388dd4f80c62f4057209d75ec710348d726c572bc795b424'}}

from contextlib import contextmanager as _control_contextmanager
import tempfile as _control_tempfile
from pathlib import PurePosixPath as _ControlPurePath


def _control_read(key):
    p.require(key in _PARSER_CONTROL_PINS and key != 'schema', 'closed control artifact key')
    row = _PARSER_CONTROL_PINS[key]
    p.verify_map(p.REPOSITORY, [row])
    raw = (p.REPOSITORY / row['path']).read_bytes()
    p.same({'path': row['path'], 'bytes': len(raw), 'sha256': p.sha(raw)}, row,
           'exact second-read parser control artifact')
    return raw


def _control_bindings():
    p.same(p._PUBLISHED_PARSER_SEAL['authority'], _PARSER_CONTROL_PINS['authority'],
           'independently pinned final C parser authority')
    p.same(p.AUTHORITY_SHA, _PARSER_CONTROL_PINS['authority']['sha256'], 'current C authority constant')
    p.same(p._PUBLISHED_PARSER_SEAL['support'], _PARSER_CONTROL_PINS['support'], 'accepted parser support')
    p.same(p._PUBLISHED_PARSER_SEAL['source_seal']['predecessor_view'],
           _PARSER_CONTROL_PINS['predecessor_view'], 'independently pinned M681 descriptor')
    p.same(Path(p.__file__).absolute(), p.REPOSITORY / _PARSER_CONTROL_PINS['portable']['path'],
           'actual current control adapter path')
    # Complete old test/portable texts are authenticated outside every M view.
    for key in ('portable', 'retained_portable', 'retained_current_tests', 'retained_lexer_tests',
                'byte_storage_portable', 'lexer_prephase_portable', 'support', 'derivation',
                'predecessor_view', 'fixture'):
        _control_read(key)


@_control_contextmanager
def _historical_parser(expected_prephase=None):
    _control_bindings()
    support, admitted, api, _ = p._published_parser_context()
    # Source preflight has closed before the only M scope below begins.
    with support._retained_scope(api, p._PUBLISHED_PARSER_SEAL, admitted) as (old, view):
        full = old.authority()
        prior = old.phase_previous(full)
        if expected_prephase is not None:
            support.same(expected_prephase,
                support._transport(prior, p._PUBLISHED_PARSER_SEAL, admitted, 'prephase'),
                'historical test uses the matching complete C prephase')
        yield old, full, prior
    # No old module/helper/callback is stored on a test class or returned.


def _historical_source_projection(a, name, raw):
    p.same(name, 'src/frontend/source.rs', 'closed source-read intermediate control')
    with _historical_parser(a) as (old, full, prior):
        old.same(raw, (old.REPOSITORY / name).read_bytes(), 'unchanged current/M source-read input')
        result = old.restore_enum_source(prior, name, raw)
    return result


def _historical_budget_projection(a, name, raw):
    p.same(name, 'src/frontend/project/budget.rs', 'closed budget intermediate control')
    with _historical_parser(a) as (old, full, prior):
        old.same(raw, (old.REPOSITORY / name).read_bytes(), 'unchanged current/M budget input')
        result = old.restore_hir_import_source(prior, name, raw)
    return result


def _historical_lexer_inverse(a, name, derived, raw):
    p.require(name in ('src/frontend/lexer.rs', 'src/frontend/project/budget.rs'),
              'closed lexer/reserve inverse controls')
    with _historical_parser(a) as (old, full, prior):
        old.same(raw, (old.REPOSITORY / name).read_bytes(), 'unchanged current/M lexer input')
        helper = old.lexer_module(prior['current'])
        hooks = helper.adapter(old.lexer_api(), prior['current'])
        expected = (old.compose_current_lexer(prior, raw) if name == 'src/frontend/lexer.rs' else
                    old.compose_array_instrumentation(prior, name, raw))
        old.same(derived, expected, 'current wrapper equals complete historical composer output')
        seams = hooks.TOKEN_SEAMS if name == 'src/frontend/lexer.rs' else hooks.BUDGET_SEAMS
        result = hooks.inverse_exact(derived, seams)
    return result


def _current_fixture_map():
    value = p.load(_control_read('fixture'))
    p.same(value['schema'], 'oxid-published-parser-control-fixture-v1', 'fixed current control fixture')
    p.same(value['predecessor_view_sha256'],
           p._PUBLISHED_PARSER_SEAL['source_seal']['predecessor_view']['sha256'], 'exact M681 association')
    rows = value['files']
    names = [row['path'] for row in rows]
    p.same(names, sorted(set(names)), 'exact current fixture roster')
    directories = sorted({str(parent) for name in names for parent in _ControlPurePath(name).parents
                          if str(parent) != '.'})
    p.same(value['directories'], directories, 'exact current fixture directories')
    p.same(value['file_count'], len(rows), 'exact current fixture count')
    p.same(value['logical_bytes'], sum(row['bytes'] for row in rows), 'exact fixture logical bytes')
    rounded = sum(((row['bytes'] + 4095) // 4096) * 4096 for row in rows)
    p.same(value['file_rounded_bytes'], rounded, 'exact fixture file allocation')
    p.same(value['allocated_bytes'], rounded + (len(directories) + 1) * 4096, 'exact fixture allocation')
    p.same(value['maximum_lifetime_bytes'], value['allocated_bytes'] + value['other_live_allocated_bytes'],
           'exact independently reviewed full lifetime bound')
    p.require(value['maximum_lifetime_bytes'] <= 128 * 1024 * 1024, 'reviewed source-test lifetime ceiling')
    p.verify_map(p.REPOSITORY, rows)
    return value


def _assert_current_capture_rejects(case, transform, historical_label):
    support, admitted, api, _ = p._published_parser_context()
    changed = copy.deepcopy(admitted)
    transform(changed['current']['files'])
    if historical_label == 'duplicate current member':
        expected = 'duplicate map member or invalid byte count'
    elif historical_label in ('unexpected current transition scope',
                              'current transition deletes historical compiler input'):
        expected = 'exact published-main source membership and one-file delta'
    else:
        p.same(historical_label, 'current transition before/after identities', 'closed capture mutation case')
        before = {row['path']: row for row in admitted['current']['files']}
        names = [row['path'] for row in changed['current']['files'] if row != before[row['path']]]
        p.same(len(names), 1, 'exact one-row current capture mutation')
        expected = ('captured source body identity' if names == ['src/main.rs'] else
                    'exact published-main source membership and one-file delta')
    with patch.object(_control_tempfile, 'TemporaryDirectory',
                      side_effect=AssertionError('new view reached after forged capture')):
        with case.assertRaisesRegex(support.Rejected, expected):
            support._source(api, p._PUBLISHED_PARSER_SEAL, changed)


def _assert_current_artifact_rejects(case, a, field, operation):
    # Added C injected-read coverage, not physical file-drift coverage. The
    # complete original physical controls also run under explicit historical names.
    p.require((field, operation) in {
        ('formatter_transition_patch', 'source'),
        ('combined_transition_patch', 'array'), ('source_binding_runner', 'array')},
        'closed current artifact-corruption controls')
    fixture = _current_fixture_map()
    positive = p.authority()
    p.same(p.phase_previous(positive), a, 'genuine positive C authority/prephase before injection')
    relative = a['current'][field]['path']
    p.same(next(row for row in fixture['files'] if row['path'] == relative), a['current'][field],
           'injected artifact belongs to the independently pinned complete C fixture')
    target = p.REPOSITORY / relative
    original_read_bytes = Path.read_bytes
    raw = original_read_bytes(target)
    p.same({'path': relative, 'bytes': len(raw), 'sha256': p.sha(raw)}, a['current'][field],
           'exact second-read current artifact before injection')
    name = 'src/frontend/source.rs' if operation == 'source' else 'src/frontend/ast.rs'
    source = (p.REPOSITORY / name).read_bytes()
    package = 'tests/fixtures/typed_project_source_binding/'
    p.require(relative.startswith(package), 'fixed current source-package artifact')
    expected = ('file bytes differ: ' + relative if field == 'source_binding_runner' else
                'unapproved complete body: ' + relative[len(package):])
    matching_reads = 0
    def read_bytes(path):
        nonlocal matching_reads
        if path == target:
            matching_reads += 1
            return raw + b'\n'
        return original_read_bytes(path)
    read_patch = patch.object(Path, 'read_bytes', read_bytes)
    read_patch.start()
    try:
        with patch.object(_control_tempfile, 'TemporaryDirectory',
                          side_effect=AssertionError('M view reached after injected current artifact')) as materialization:
            with case.assertRaises(p.Rejected) as caught:
                try:
                    if operation == 'source':
                        p.compose_source_read(a, source)
                    else:
                        p.compose_array_instrumentation(a, name, source)
                except p.Rejected as failure:
                    # unittest clears the caught outer traceback on context exit.
                    frames = set()
                    traceback = (failure.__cause__ or failure).__traceback__
                    while traceback is not None:
                        frames.add(traceback.tb_frame.f_code.co_name)
                        traceback = traceback.tb_next
                    raise
            materialization.assert_not_called()
    finally:
        read_patch.stop()
        case.assertIs(Path.read_bytes, original_read_bytes)
    # A no-match interception, missing helper or unrelated rejection cannot pass.
    case.assertEqual(matching_reads, 1)
    case.assertIs(type(caught.exception), p.Rejected)
    case.assertEqual(str(caught.exception), expected)
    if field == 'source_binding_runner':
        case.assertIsNone(caught.exception.__cause__)
        required_frames = {'_seal', 'verify_map'}
    else:
        boundary = caught.exception.__cause__
        current = p.sys.modules['oxid_published_current']
        case.assertIs(type(boundary), current.adapters.Reject)
        case.assertEqual(str(boundary), expected)
        case.assertIsNone(boundary.__cause__)
        required_frames = {'_current_package', '_verify'}
    case.assertTrue(required_frames <= frames)
    case.assertNotIn('predecessor_view', frames)


def _assert_current_parser_pin_rejects(case):
    original_read_bytes = Path.read_bytes
    target = p.REPOSITORY / _PARSER_CONTROL_PINS['authority']['path']
    original = original_read_bytes(target)
    changed = p.load(original)
    changed['source_delta'] = changed['source_delta'][1:]
    changed_raw = (json.dumps(changed, sort_keys=True, indent=2) + '\n').encode()
    def read_bytes(path):
        return changed_raw if path == target else original_read_bytes(path)
    with patch.object(Path, 'read_bytes', read_bytes), \
         patch.object(_control_tempfile, 'TemporaryDirectory',
                      side_effect=AssertionError('source preflight/view reached before parser pin rejection')):
        with case.assertRaisesRegex(p.Rejected, 'file bytes differ: .*unit4_parser_current/authority.json'):
            p.authority()


_control_bindings()



class CurrentLexer(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.a = p.phase_previous(p.authority())
        cls.inputs = {r['path']: (p.REPOSITORY / r['path']).read_bytes()
                      for r in cls.a['current_source']['files']}

    def test_historical_complete_outer_inverse_and_wrong_stage(self):
        with _historical_parser() as (old, historical_outer, historical_a):
            historical_a = old.phase_previous(old.authority())
            historical_inputs = {r['path']: (old.REPOSITORY / r['path']).read_bytes()
                          for r in historical_a['current_source']['files']}
            historical_helper = old.lexer_module(historical_a['current'])
            historical_hooks = historical_helper.adapter(old.lexer_api(), historical_a['current'])
            prior = old.restore_lexer_source(historical_a['current'], historical_inputs)
            expected = old.read(old.REPOSITORY / historical_a['current']['lexer_reservation']['source_predecessor']['path'])
            self.assertEqual(len(prior), 376)
            self.assertEqual([{'path': n, 'bytes': len(b), 'sha256': old.sha(b)} for n, b in sorted(prior.items())], expected['files'])
            with self.assertRaises(old.Rejected):
                old.restore_lexer_source(historical_a['current'], prior)
            for change in ('missing', 'extra', 'altered'):
                damaged = dict(historical_inputs)
                if change == 'missing': damaged.pop('src/frontend/lexer.rs')
                elif change == 'extra': damaged['src/unapproved.rs'] = b''
                else: damaged['src/frontend/lexer.rs'] += b'\n'
                with self.subTest(change=change), self.assertRaises(old.Rejected):
                    old.restore_lexer_source(historical_a['current'], damaged)

    def test_historical_coherent_manifest_row_rejects_before_preflight_or_materialization(self):
        with _historical_parser() as (old, historical_outer, historical_a):
            historical_a = old.phase_previous(old.authority())
            historical_inputs = {r['path']: (old.REPOSITORY / r['path']).read_bytes()
                          for r in historical_a['current_source']['files']}
            historical_helper = old.lexer_module(historical_a['current'])
            historical_hooks = historical_helper.adapter(old.lexer_api(), historical_a['current'])
            current = copy.deepcopy(historical_a['current_source'])
            name = 'src/frontend/lexer.rs'
            raw = historical_inputs[name] + b'\n'
            next(r for r in current['files'] if r['path'] == name).update(bytes=len(raw), sha256=old.sha(raw))
            # The source API is not even requested for forged incoming metadata.
            with patch.object(old, 'u8_binding_api', side_effect=AssertionError('preflight/materialization reached')):
                with self.assertRaisesRegex(old.Rejected, 'complete admitted lexer source'):
                    old.validate_u8_transition(historical_a['current'], current, historical_a)

    def test_successful_append_hook_and_exact_inverse(self):
        name = 'src/frontend/lexer.rs'
        raw = self.inputs[name]
        derived = p.compose_current_lexer(self.a, raw)
        hook = b'crate::frontend::parser::unit4_observer::lex_token(*tokens.last().unwrap());'
        self.assertEqual(derived.count(hook), 1)
        self.assertIn(b'    tokens.push(token);\n    ' + hook + b'\n    Ok(())', derived)
        self.assertEqual(_historical_lexer_inverse(self.a, name, derived, raw), raw)
        control = next(r for r in self.a['current']['current_control_derived_files'] if r['path'] == name)
        self.assertEqual(control, {'path': name, 'bytes': len(raw), 'sha256': p.sha(raw)})
        with self.assertRaises(p.Rejected):
            p.compose_current_lexer(self.a, derived)

    def test_reserve_hook_and_control_are_preserved(self):
        name = 'src/frontend/project/budget.rs'
        raw = self.inputs[name]
        derived = p.compose_array_instrumentation(self.a, name, raw)
        self.assertEqual(derived.count(b'::reserve(kind, length, element_bytes, success);'), 1)
        self.assertEqual(_historical_lexer_inverse(self.a, name, derived, raw), raw)
        control = next(r for r in self.a['current']['current_control_derived_files'] if r['path'] == name)
        self.assertEqual(control, {'path': name, 'bytes': len(raw), 'sha256': p.sha(raw)})

    def test_historical_stale_and_uninstrumented_current_hook_maps_reject(self):
        with _historical_parser() as (old, historical_outer, historical_a):
            historical_a = old.phase_previous(old.authority())
            historical_inputs = {r['path']: (old.REPOSITORY / r['path']).read_bytes()
                          for r in historical_a['current_source']['files']}
            historical_helper = old.lexer_module(historical_a['current'])
            historical_hooks = historical_helper.adapter(old.lexer_api(), historical_a['current'])
            prior = old.lexer_predecessor_active(historical_a['current'])
            for name in ('src/frontend/lexer.rs', 'src/frontend/project/budget.rs'):
                for source in ('current_base_files', 'current_derived_files'):
                    wrong = copy.deepcopy(historical_a)
                    rows = historical_a['current']['current_base_files'] if source == 'current_base_files' else prior[source]
                    replacement = next(r for r in rows if r['path'] == name)
                    next(r for r in wrong['current']['current_derived_files'] if r['path'] == name).update(replacement)
                    with self.subTest(name=name, source=source), self.assertRaises((old.Rejected, historical_hooks.Reject)):
                        historical_helper.compose(old.lexer_api(), wrong, name, historical_inputs[name])

    def test_historical_division_composer_is_byte_identical(self):
        path = 'tests/qualification/unit4_parser_current/portable.py'
        saved = Path(p.__file__).with_name('byte_storage_portable_v1.py').read_bytes()
        self.assertEqual(p.sha(saved), '2efcbd2f11ee0b9fee041c19b362459b15f7d843b16bc55f8743a938c58210cd')
        old = saved.decode()
        current = Path(p.__file__).read_text()
        def definition(source):
            node = next(n for n in ast.parse(source).body if isinstance(n, ast.FunctionDef) and n.name == 'compose_division_lexer')
            return ast.get_source_segment(source, node)
        self.assertEqual(definition(old), definition(current))

    def test_historical_successful_append_hook_and_exact_inverse(self):
        with _historical_parser() as (old, historical_outer, historical_a):
            historical_a = old.phase_previous(old.authority())
            historical_inputs = {r['path']: (old.REPOSITORY / r['path']).read_bytes()
                          for r in historical_a['current_source']['files']}
            historical_helper = old.lexer_module(historical_a['current'])
            historical_hooks = historical_helper.adapter(old.lexer_api(), historical_a['current'])
            name = 'src/frontend/lexer.rs'
            raw = historical_inputs[name]
            derived = old.compose_current_lexer(historical_a, raw)
            hook = b'crate::frontend::parser::unit4_observer::lex_token(*tokens.last().unwrap());'
            self.assertEqual(derived.count(hook), 1)
            self.assertIn(b'    tokens.push(token);\n    ' + hook + b'\n    Ok(())', derived)
            self.assertEqual(historical_hooks.inverse_exact(derived, historical_hooks.TOKEN_SEAMS), raw)
            control = next(r for r in historical_a['current']['current_control_derived_files'] if r['path'] == name)
            self.assertEqual(control, {'path': name, 'bytes': len(raw), 'sha256': old.sha(raw)})
            with self.assertRaises((old.Rejected, historical_hooks.Reject)):
                old.compose_current_lexer(historical_a, derived)





class PublishedCurrentLexerControls(unittest.TestCase):
    def test_current_provenance_inverse_is_distinct_from_historical_lexer_inverse(self):
        outer = p.authority()
        support, admitted, api, _ = p._published_parser_context()
        prior = support.provenance_previous(api, outer, p._PUBLISHED_PARSER_SEAL, admitted)
        support.same(prior['current_source'], admitted['published_predecessor']['current'], 'exact M source association')
        support.same(outer['current_source'], admitted['current'], 'exact C source association')
        before, after = admitted['published_predecessor']['inputs'], admitted['inputs']
        self.assertEqual(len(before), 376)
        self.assertEqual(set(before), set(after))
        self.assertEqual([name for name in sorted(before) if before[name] != after[name]], ['src/main.rs'])
        self.assertEqual(len(outer['current']['published_main_parser']['source_provenance']['main_delta']), 1)
        self.assertNotEqual(p.current_candidate(outer)['current_source_manifest_sha256'],
                            prior['current']['current_source_manifest']['sha256'])
        with self.assertRaisesRegex(support.Rejected, 'exact full outer inverse input'):
            support.provenance_previous(api, prior, p._PUBLISHED_PARSER_SEAL, admitted)

    def test_coherent_current_lexer_row_rejects_before_new_view(self):
        support, admitted, api, _ = p._published_parser_context()
        changed = copy.deepcopy(admitted)
        name = 'src/frontend/lexer.rs'
        raw = changed['inputs'][name] + b'\n'
        changed['inputs'][name] = raw
        next(row for row in changed['current']['files'] if row['path'] == name).update(bytes=len(raw), sha256=p.sha(raw))
        with patch.object(_control_tempfile, 'TemporaryDirectory', side_effect=AssertionError('view reached')):
            with self.assertRaisesRegex(support.Rejected, 'exact published-main source membership and one-file delta'):
                support._source(api, p._PUBLISHED_PARSER_SEAL, changed)

    def test_current_lexer_and_budget_stale_maps_reject_at_outer_boundary(self):
        current = p.phase_previous(p.authority())
        replacements = {}
        with _historical_parser(current) as (old, full, prior):
            before = old.lexer_predecessor_active(prior['current'])
            for name in ('src/frontend/lexer.rs', 'src/frontend/project/budget.rs'):
                replacements[name] = next(row for row in before['current_derived_files'] if row['path'] == name)
        for name in replacements:
            for source in ('current_base_files', 'historical_derived'):
                wrong = copy.deepcopy(current)
                replacement = (next(row for row in current['current']['current_base_files'] if row['path'] == name)
                               if source == 'current_base_files' else replacements[name])
                next(row for row in wrong['current']['current_derived_files'] if row['path'] == name).update(replacement)
                raw = (p.REPOSITORY / name).read_bytes()
                with self.subTest(name=name, source=source), self.assertRaisesRegex(p.Rejected, 'composer requires exact current prephase stage'):
                    if name == 'src/frontend/lexer.rs': p.compose_current_lexer(wrong, raw)
                    else: p.compose_array_instrumentation(wrong, name, raw)


if __name__ == '__main__':
    unittest.main()
