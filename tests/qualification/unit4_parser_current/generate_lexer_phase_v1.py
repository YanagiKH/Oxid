#!/usr/bin/env python3
"""Inactive source-only outer phase authority serializer for independent review.

The pure serializer is uninvoked at this checkpoint. Main is explicitly held;
there is no activation switch, arbitrary output root, or source-tree generator.
"""
from __future__ import annotations

import argparse
import copy
import hashlib
import json
from pathlib import Path, PurePosixPath
import re
import types

W = Path('/workspace/scratch/8266abf56995')
E = W / 'lexer-reservation-evidence/publication-ci-required'
CANDIDATES = W / 'source-v2-final-candidates-attempt1'
STAGED_INPUT = CANDIDATES / 'stage'
MEASUREMENT_ROOT = W / 'phase-layout-discovery-c2-6fde787-attempt1'
MEASUREMENT_REVIEW = E / 'phase-layout-measurement-results-v1/review-package.json'
MEASUREMENT_REVIEW_SHA = 'e4e44a3caf36b3008a4ddccd195b930245e95b03ad5f0c15f8079971d278c93b'
DIRECTORY = 'tests/qualification/unit4_parser_current/'
PREDECESSOR = DIRECTORY + 'lexer-reservation-authority-v2.json'
COMPOSITION = DIRECTORY + 'lexer-phase-composition-v1.json'
AUTHORITY = DIRECTORY + 'lexer-phase-authority-v1.json'
C2 = 'b3abc9f0dda99d6d8fe65d9c3a9ed31dedcbd489'
T2 = '7f5c9aa08c569c4d0b5a27391d8fc68337075d36'
C2_MANIFEST = {'path': 'tests/fixtures/typed_project_source_binding/current-source.json', 'bytes': 74328,
               'sha256': '9432c61fc4f63b760e5f55599aedb24067a206e40e8b44b0911392c00cda7261'}
FROZEN_U8 = {'path': DIRECTORY + 'u8_policy_controls.py', 'bytes': 7600,
             'sha256': '5228fc42aedb19708e727c51266acd8760440acb973784463f02fbf70be03d35'}
SOURCE_PINS = {
    DIRECTORY + 'lexer_phase.py': (54473, '1067d223defa8de2a6b0faa71910d9acc97d635491022216e5d59fd48c62ed0e'),
    DIRECTORY + 'lexer_phase_controls.py': (33650, 'cf8667d190c9b50c2f69b623783a33c8c931f795e359a43980ef2486ad44d8c9'),
    DIRECTORY + 'test_lexer_phase.py': (51636, '709b828b5e6752ee6c4f4450e5a422cf6a5a2b0d45a5ae8941915fdc7709ccc5'),
    DIRECTORY + 'u8_policy_controls.py': (9018, '90807a872ef4b042abfc6f29d785a903c54fa00ae7dc364d26fe07dc6baa7b95'),
}
MAP_FILES = ('candidate-source-manifest.json', 'base-files.candidate.json',
             'prephase-observed-files.candidate.json', 'prephase-control-files.candidate.json',
             'phase-observed-files.candidate.json', 'phase-control-files.candidate.json',
             'complete-inverse-proof.json', 'derivation-inputs.json')
ROLE_PROFILES = (('debug', 'observed'), ('debug', 'not_observed'),
                 ('release', 'observed'), ('release', 'not_observed'))
LAYOUT_FILES = ('layout-debug-observed/result.json', 'layout-debug-control/result.json',
                'layout-release-observed/result.json', 'layout-release-control/result.json')
MEASUREMENT_FILES = MAP_FILES + LAYOUT_FILES
OPERATIONS = (('src/frontend/project/budget.rs', 'reserve_dispatch_v1'),
              ('src/frontend/lexer.rs', 'token_forward_v1'),
              ('src/frontend/parser/unit4_observer.rs', 'observer_phase_v1'))
CHANGED_FIELDS = ('current_derived_files', 'current_control_derived_files', 'u8_policy_controls')
PHASE_KEYS = ('version', 'predecessor', 'helper', 'source_controls', 'correspondence', 'controls')
OUTPUT_NAMES = ('lexer-phase-composition-v1.json', 'lexer-phase-authority-v1.json')


class Rejected(ValueError):
    pass


def need(condition, message):
    if not condition:
        raise Rejected(message)


def same(actual, expected, message):
    need(type(actual) is type(expected), message)
    if isinstance(expected, dict):
        need(actual.keys() == expected.keys(), message)
        for key in expected:
            same(actual[key], expected[key], message)
    elif isinstance(expected, (list, tuple)):
        need(len(actual) == len(expected), message)
        for left, right in zip(actual, expected):
            same(left, right, message)
    else:
        need(actual == expected, message)


def closed(value, keys, label):
    need(type(value) is dict and set(value) == set(keys), label + ': missing or extra input')


def sha(raw):
    need(type(raw) is bytes, 'exact byte input required')
    return hashlib.sha256(raw).hexdigest()


def decode(raw):
    def pairs(items):
        result = {}
        for key, value in items:
            need(key not in result, 'duplicate JSON key: ' + key)
            result[key] = value
        return result
    need(type(raw) is bytes, 'JSON input must be exact bytes')
    return json.loads(raw, object_pairs_hook=pairs,
                      parse_constant=lambda _: (_ for _ in ()).throw(Rejected('nonfinite JSON value')))


def serial(value):
    return (json.dumps(value, sort_keys=True, indent=2, allow_nan=False) + '\n').encode('utf-8')


def relative(name):
    need(type(name) is str and name and '\\' not in name, 'canonical relative input path')
    parts = name.split('/')
    need(all(part not in ('', '.', '..') for part in parts) and
         not PurePosixPath(name).is_absolute() and str(PurePosixPath(name)) == name,
         'unsafe relative input path')
    return name


def identity(name, raw):
    return {'path': relative(name), 'bytes': len(raw), 'sha256': sha(raw)}


def admit_identity(record, *, path=None):
    closed(record, ('path', 'bytes', 'sha256'), 'identity')
    relative(record['path'])
    need(type(record['bytes']) is int and record['bytes'] >= 0, 'identity byte count')
    need(type(record['sha256']) is str and re.fullmatch('[0-9a-f]{64}', record['sha256']), 'identity SHA256')
    if path is not None:
        same(record['path'], path, 'exact independently reviewed identity path')
    return record


def validate_sources(source_files):
    closed(source_files, SOURCE_PINS, 'four phase source inputs')
    for name, raw in source_files.items():
        same((len(raw), sha(raw)), SOURCE_PINS[name], 'changed reviewed source input: ' + name)
    rows = {name: identity(name, raw) for name, raw in source_files.items()}
    modules = {}
    for filename in ('lexer_phase.py', 'lexer_phase_controls.py'):
        name = DIRECTORY + filename
        module = types.ModuleType('outer_' + filename[:-3])
        module.__file__ = str(STAGED_INPUT / name)
        exec(compile(source_files[name], module.__file__, 'exec'), module.__dict__)
        modules[filename] = module
    return rows, modules['lexer_phase.py'], modules['lexer_phase_controls.py']


def map_rows(raw, count):
    value = decode(raw)
    need(type(value) is list and len(value) == count, 'complete independent map count')
    for row in value:
        admit_identity(row)
    need(len({row['path'] for row in value}) == count, 'duplicate complete map member')
    return value


def admit_measurement(measurement_review_raw, measurement_files, controls):
    """Validate existing approved measurement artifacts; create no new maps."""
    same(sha(measurement_review_raw), MEASUREMENT_REVIEW_SHA, 'independently approved measured-values package')
    review = decode(measurement_review_raw)
    same(review['schema'], 'oxid-phase-candidate-layout-review-v1', 'measurement package schema')
    same(review['compiler_head'], C2, 'measurement exact compiler checkpoint')
    same(review['phase_checkpoint'], '6fde7876d8d9237e2f238348f078a905b6097a84', 'measurement exact phase checkpoint')
    same(review['candidate_root'], str(MEASUREMENT_ROOT), 'measurement exact candidate root')
    same(review['four_layouts_equal'], True, 'all four actual layouts were equal')
    same(review['authority_activated'], False, 'measurement never activated authority')
    same(review['qualification_performed'], False, 'measurement is not execution qualification')
    closed(measurement_files, MEASUREMENT_FILES, 'closed measured source/layout inputs')
    rows = {row['path']: row for row in review['files']}
    need(len(rows) == len(review['files']), 'duplicate measured artifact identity')
    for name, raw in measurement_files.items():
        path = str(MEASUREMENT_ROOT / name)
        need(path in rows, 'measurement input absent from approved package')
        same({'path': path, 'bytes': len(raw), 'sha256': sha(raw)}, rows[path], 'changed measured artifact ' + name)
    maps = {name: map_rows(measurement_files[name], 539 if name == 'base-files.candidate.json' else 542)
            for name in MAP_FILES if name.endswith('-files.candidate.json')}
    candidate = decode(measurement_files['candidate-source-manifest.json'])
    closed(candidate, ('schema', 'historical_commit', 'reviewed_source_head', 'source_only_tree',
                       'current_source_manifest_sha256', 'files'), 'exact candidate metadata')
    same(candidate['schema'], 'oxid-unit4-current-candidate-source-manifest-v1', 'unchanged candidate metadata schema')
    same(candidate['reviewed_source_head'], C2, 'candidate compiler checkpoint')
    same(candidate['source_only_tree'], T2, 'candidate compiler tree')
    same(candidate['current_source_manifest_sha256'], C2_MANIFEST['sha256'], 'candidate exact source-v2 metadata')
    same(candidate['files'], maps['base-files.candidate.json'], 'candidate complete base metadata')
    candidate_row = identity('candidate-source-manifest.json', measurement_files['candidate-source-manifest.json'])
    for name, values in maps.items():
        if name != 'base-files.candidate.json':
            same(next(row for row in values if row['path'] == candidate_row['path']), candidate_row,
                 'same complete candidate metadata in every role map')
    expected_roles = [{'profile': profile, 'role': role} for profile, role in ROLE_PROFILES]
    same([{key: row[key] for key in ('profile', 'role')} for row in review['builds']], expected_roles,
         'exact four measured build roles')
    same(len(review['completion']['results']), 4, 'exact four measured results')
    actual = review['actual_layout']
    closed(actual, controls.LAYOUT_KEYS, 'closed approved actual 25-field layout')
    approved = {key: actual[key] for key in controls.LAYOUT_KEYS}
    controls.validate_layout(approved)
    same(approved['stdout_lock_bytes'], 8, 'reviewed actual G')
    same(approved['frame_bytes'], 40, 'reviewed actual frame')
    same(approved['named_total_bytes'], 2536, 'reviewed logical named union')
    layouts = {}
    for (profile, role), name, result_identity in zip(ROLE_PROFILES, LAYOUT_FILES, review['completion']['results']):
        same(result_identity['path'], str(MEASUREMENT_ROOT / name), 'prescribed actual measured result path')
        raw = measurement_files[name]
        same({'path': result_identity['path'], 'bytes': len(raw), 'sha256': sha(raw)}, result_identity,
             'measured result binds approved completion')
        result = decode(raw)
        for key, value in (('schema', 'oxid-unit4-layout-discovery-result-v1'), ('status', 'measured'),
                           ('boundary', 'layout'), ('exit_code', 0), ('marker_count', 1),
                           ('failure', None), ('execution_qualified', False)):
            same(result[key], value, 'actual measurement result ' + key)
        closed(result['layout'], controls.LAYOUT_KEYS, 'closed actual measured layout')
        same(result['layout'], actual, 'all four actual measured values equal approved values')
        layouts[profile + '-' + role] = copy.deepcopy(approved)
    return maps, candidate_row, decode(measurement_files['complete-inverse-proof.json']), layouts


def project_previous(active, predecessor):
    """Exact full-object inverse, without widening any prior omit allowlist."""
    need(type(predecessor) is dict and 'lexer_phase' not in predecessor, 'exact pre-phase predecessor object')
    same(set(active), set(predecessor) | {'lexer_phase'}, 'phase outer complete key set')
    closed(active['lexer_phase'], PHASE_KEYS, 'phase layer section')
    same(active['lexer_phase']['version'], 'oxid-unit4-lexer-phase-layer-v1', 'phase layer version')
    closed(active['lexer_phase']['controls'], ('driver', 'approved_layouts'), 'phase controls section')
    restored = copy.deepcopy(active)
    del restored['lexer_phase']
    for field in CHANGED_FIELDS:
        need(field in predecessor and field in restored, 'required exact predecessor field')
        restored[field] = copy.deepcopy(predecessor[field])
    same(restored, predecessor, 'phase inverse changes unrelated predecessor bytes/values')
    return restored


def validate_predecessor(predecessor_raw, predecessor_identity, maps, candidate_row):
    admit_identity(predecessor_identity, path=PREDECESSOR)
    same(identity(PREDECESSOR, predecessor_raw), predecessor_identity, 'independently reviewed pre-phase bytes')
    predecessor = decode(predecessor_raw)
    same(serial(predecessor), predecessor_raw, 'exact retained pretty-JSON predecessor serialization')
    same(predecessor['schema'], 'oxid-unit4-current-parser-authority-v1', 'pre-phase schema')
    need('lexer_phase' not in predecessor, 'already-applied/wrong-stage phase predecessor')
    same(predecessor['reviewed_source_head'], C2, 'exact pre-phase compiler checkpoint')
    same(predecessor['source_only_tree'], T2, 'exact pre-phase compiler tree')
    same(predecessor['current_source_manifest'], C2_MANIFEST, 'unchanged final source-v2 manifest bytes')
    same(predecessor['current_base_files'], maps['base-files.candidate.json'], 'complete 539-member pre-phase base map')
    same(predecessor['current_derived_files'], maps['prephase-observed-files.candidate.json'], 'complete 542-member observed predecessor map')
    same(predecessor['current_control_derived_files'], maps['prephase-control-files.candidate.json'], 'complete 542-member control predecessor map')
    same(predecessor['current_candidate_source_manifest_sha256'], candidate_row['sha256'], 'candidate metadata must remain identical')
    same(predecessor['u8_policy_controls'], FROZEN_U8, 'exact retained pre-phase u8 helper')
    return predecessor


def validate_body_inputs(phase_bodies):
    closed(phase_bodies, ('observed', 'not_observed'), 'phase body roles')
    for role in ('observed', 'not_observed'):
        operations = OPERATIONS if role == 'observed' else OPERATIONS[-1:]
        closed(phase_bodies[role], (name for name, _ in operations), 'exact changed phase body scope')
        need(all(type(raw) is bytes for raw in phase_bodies[role].values()), 'phase body exact bytes')
    return phase_bodies


def serialize(predecessor_raw, predecessor_identity, measurement_review_raw,
              measurement_files, source_files, phase_bodies):
    """Uninvoked pure bytes-in/bytes-out candidate serializer; never an approval.

    predecessor_identity is supplied only after the separate intermediate output
    review. It is not an arbitrary digest override in the held command wrapper.
    """
    validate_body_inputs(phase_bodies)
    source_rows, phase, controls = validate_sources(source_files)
    maps, candidate_row, inverse_proof, layouts = admit_measurement(measurement_review_raw, measurement_files, controls)
    predecessor = validate_predecessor(predecessor_raw, predecessor_identity, maps, candidate_row)
    closed(phase_bodies, ('observed', 'not_observed'), 'phase body roles')
    same(inverse_proof['counts'], [539, 542, 542], 'independently reviewed whole-map counts')
    same(inverse_proof['execution_qualified'], False, 'source inverse proof is not execution qualification')
    closed(inverse_proof['roles'], ('observed', 'not_observed'), 'complete inverse proof roles')
    derived_maps, roles = {}, []
    for role, field, prior_name, after_name in (
        ('observed', 'current_derived_files', 'prephase-observed-files.candidate.json', 'phase-observed-files.candidate.json'),
        ('not_observed', 'current_control_derived_files', 'prephase-control-files.candidate.json', 'phase-control-files.candidate.json')):
        operations = OPERATIONS if role == 'observed' else OPERATIONS[-1:]
        closed(phase_bodies[role], (name for name, _ in operations), 'exact changed phase body scope')
        prior = predecessor[field]
        derived = {row['path']: copy.deepcopy(row) for row in prior}
        final = {row['path']: row for row in maps[after_name]}
        members = []
        for name, operation in operations:
            actual_after = phase_bodies[role][name]
            same(identity(name, actual_after), final[name], 'actual measured body at exact complete-map identity')
            if name == 'src/frontend/project/budget.rs':
                before = phase.invert_budget(actual_after)
                after = phase.compose_budget(before)
            elif name == 'src/frontend/lexer.rs':
                before = phase.invert_lexer(actual_after)
                after = phase.compose_lexer(before)
            else:
                before = phase.invert_observer(actual_after, role)
                after = phase.compose_observer(before, role)
            before_row, after_row = identity(name, before), identity(name, after)
            same(before_row, derived[name], 'full exact retained predecessor body recovered')
            same(after, actual_after, 'exact unchanged phase body re-composition')
            derived[name] = after_row
            members.append({'path': name, 'before': before_row, 'after': after_row, 'operation': operation})
        ordered = [derived[name] for name in sorted(derived, key=lambda name: PurePosixPath(name).parts)]
        same(ordered, maps[after_name], 'complete final role map; no metadata normalization or additional delta')
        inverse_map = {row['path']: copy.deepcopy(row) for row in ordered}
        for member in members:
            inverse_map[member['path']] = member['before']
        restored = [inverse_map[name] for name in sorted(inverse_map, key=lambda name: PurePosixPath(name).parts)]
        same(restored, prior, 'entire final map inverse recovers complete pre-phase map')
        prior_sha, after_sha = sha(phase.process_projection(prior)), sha(phase.process_projection(ordered))
        proof = inverse_proof['roles'][role]
        same(proof['complete_inverse_body_equality'], True, 'independently reviewed exact inverse bodies')
        same(proof['prephase_map_sha256'], prior_sha, 'independent complete predecessor map projection')
        same(proof['complete_inverse_map_sha256'], prior_sha, 'independent inverse complete map projection')
        same(proof['phase_map_sha256'], after_sha, 'independent complete phase map projection')
        same(proof['members'], [{key: row[key] for key in ('path', 'before', 'after')} for row in members],
             'exact reviewed three observed/one control phase members')
        derived_maps[field] = ordered
        roles.append({'role': role, 'predecessor_map_sha256': prior_sha,
                      'derived_map_sha256': after_sha, 'members': members})
    correspondence = {'schema': 'oxid-unit4-lexer-phase-composition-v1',
                      'predecessor_authority': copy.deepcopy(predecessor_identity),
                      'adapter': source_rows[DIRECTORY + 'lexer_phase.py'], 'roles': roles}
    correspondence_raw = serial(correspondence)
    active = copy.deepcopy(predecessor)
    active.update(derived_maps)
    active['u8_policy_controls'] = source_rows[DIRECTORY + 'u8_policy_controls.py']
    active['lexer_phase'] = {'version': 'oxid-unit4-lexer-phase-layer-v1',
        'predecessor': copy.deepcopy(predecessor_identity), 'helper': source_rows[DIRECTORY + 'lexer_phase.py'],
        'source_controls': source_rows[DIRECTORY + 'test_lexer_phase.py'],
        'correspondence': identity(COMPOSITION, correspondence_raw),
        'controls': {'driver': source_rows[DIRECTORY + 'lexer_phase_controls.py'], 'approved_layouts': layouts}}
    same(project_previous(active, predecessor), predecessor, 'whole-object outer predecessor inverse')
    outputs = {OUTPUT_NAMES[0]: correspondence_raw, OUTPUT_NAMES[1]: serial(active)}
    same(tuple(outputs), OUTPUT_NAMES, 'exact two immutable output artifacts')
    return outputs


def read_regular(path):
    need(path.is_file() and not any(p.is_symlink() for p in (path, *path.parents)), 'unsafe regular input path')
    return path.read_bytes()


def parse_invocation(argv):
    """Closed generation-only argv; this is not a runtime authority override."""
    parser = argparse.ArgumentParser(description=__doc__)
    for name in ('prephase-authority', 'candidate-evidence', 'measurement-review', 'staged-input-root', 'output-dir'):
        parser.add_argument('--' + name, required=True, type=Path)
    parser.add_argument('--prephase-authority-sha256', required=True)
    args = parser.parse_args(argv)
    need(re.fullmatch('[0-9a-f]{64}', args.prephase_authority_sha256) is not None,
         'required independently reviewed generation input SHA256')
    for actual, expected in ((args.prephase_authority, CANDIDATES / 'parser-prephase/lexer-reservation-authority-v2.json'),
                             (args.candidate_evidence, MEASUREMENT_ROOT), (args.measurement_review, MEASUREMENT_REVIEW),
                             (args.staged_input_root, STAGED_INPUT), (args.output_dir, CANDIDATES / 'phase')):
        same(actual, expected, 'exact reviewed staged invocation path')
    return args


def admit_predecessor_digest(predecessor_raw, reviewed_digest):
    need(type(reviewed_digest) is str and re.fullmatch('[0-9a-f]{64}', reviewed_digest),
         'required independently reviewed generation input SHA256')
    same(sha(predecessor_raw), reviewed_digest, 'independently reviewed full intermediate pre-phase bytes/hash')
    return predecessor_raw


def planned_main(argv=None):
    """Held command recipe; exact generation invocation needs independent review."""
    args = parse_invocation(argv)
    for path in (args.prephase_authority, args.candidate_evidence, args.measurement_review,
                 args.staged_input_root, args.output_dir):
        need(not any(p.is_symlink() for p in (path, *path.parents)), 'symlink staged path')
    predecessor_raw = admit_predecessor_digest(read_regular(args.prephase_authority), args.prephase_authority_sha256)
    sources = {name: read_regular(args.staged_input_root / name) for name in SOURCE_PINS}
    measurement = {name: read_regular(args.candidate_evidence / name) for name in MEASUREMENT_FILES}
    bodies = {role: {name: read_regular(args.candidate_evidence / folder / name)
                     for name, _ in (OPERATIONS if role == 'observed' else OPERATIONS[-1:])}
              for role, folder in (('observed', 'source'), ('not_observed', 'control-source'))}
    outputs = serialize(predecessor_raw, identity(PREDECESSOR, predecessor_raw),
                        read_regular(args.measurement_review), measurement, sources, bodies)
    need(not args.output_dir.exists() and args.output_dir.parent.is_dir(), 'fresh separate phase output directory')
    args.output_dir.mkdir()
    for name in OUTPUT_NAMES:
        with (args.output_dir / name).open('xb') as sink:
            need(sink.write(outputs[name]) == len(outputs[name]), 'complete immutable candidate write')
    same(sorted(path.name for path in args.output_dir.iterdir()), sorted(OUTPUT_NAMES), 'exact output directory membership')


if __name__ == '__main__':
    planned_main()
