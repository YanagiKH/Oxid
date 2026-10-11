"""Pure/mock admission controls. No Git, HTTP, tools, Cargo or CI execution."""
import copy
import importlib.util
import hashlib
import io
import json
from pathlib import Path
import types
import stat
import struct
import zipfile
import unittest
from unittest import mock
import warnings

HERE = Path(__file__).resolve().parent
spec = importlib.util.spec_from_file_location('_pure_package_ci_jobs', HERE / 'jobs.py')
jobs = importlib.util.module_from_spec(spec)
spec.loader.exec_module(jobs)


def identity():
    return {'path': '/fixture/input', 'bytes': 1, 'sha256': 'a' * 64}


def job_source():
    return {'schema': 'synthetic-inert-job-source', 'repository_path': '/fixture',
            'checkout_head': 'a' * 40, 'checkout_tree': 'b' * 40, 'event_sha': 'c' * 40,
            'source_checkpoint': {'head': 'd' * 40}, 'source_manifest_identity': identity(),
            'source_files': {'src/main.rs': identity()}, 'workflow_identity': identity(),
            'input_inventory_identity': identity(), 'route_registry_identity': identity(),
            'package_identity': identity(),
            'ci': {'repository': 'YanagiKH/Oxid', 'run_id': '123', 'run_attempt': '2',
                   'workflow': 'Repository CI', 'event_name': 'push', 'job': 'quality',
                   'workflow_ref': 'YanagiKH/Oxid/.github/workflows/ci.yml@refs/heads/main',
                   'workflow_sha': 'a' * 40}, 'job_recipe_id': 'quality-linux', 'matrix': {},
            'host': 'linux-x86_64', 'capture_started_ns': 1, 'capture_finished_ns': 2,
            'runner_temp': {'path': '/fixture/runner-temp', 'identity': {'dev': 1, 'ino': 2, 'mode': 16832, 'uid': 1, 'gid': 1}},
            'nonce': 'e' * 32, 'activation': {'approved': False}, 'status': 'source-only'}


def registry():
    recipe = {'id': 'quality-linux', 'github_job': 'quality', 'display_name': 'Full repository verification',
              'matrix': {}, 'host': 'linux-x86_64', 'operational': True, 'job_continue_on_error': False,
              'steps': [{'continue_on_error': False}]}
    return {'job_recipes': {'quality-linux': recipe}, 'activation': {'approved': True,
            'workflow_sha256': 'a' * 64, 'provider_jobs': {'quality-linux': mapping()},
            'boundary_transport': transport()}}


def mapping():
    return {'display_name': 'Full repository verification', 'begin_step': 1, 'terminal_step': 3,
            'steps': [{'number': index, 'name': name, 'mandatory': True, 'continue_on_error': False, 'expected_conclusion': 'success'}
                      for index, name in enumerate(('Capture source begin', 'Original Cargo commands', 'Capture source terminal', 'Upload source boundaries'), 1)]}


def join_source():
    value = job_source()
    value.pop('job_recipe_id'); value.pop('matrix')
    value['required_recipes'] = ['quality-linux']
    value['ci']['job'] = 'qualification'
    return value


def provider_job():
    value = {'id': 456, 'name': mapping()['display_name'], 'run_id': 123, 'run_attempt': 2,
             'head_sha': 'a' * 40, 'workflow_name': 'Repository CI', 'status': 'completed', 'conclusion': 'success',
             'started_at': '2026-10-11T01:00:00Z', 'completed_at': '2026-10-11T01:00:40Z', 'steps': []}
    for index, name in enumerate(('Capture source begin', 'Original Cargo commands', 'Capture source terminal', 'Upload source boundaries'), 1):
        start, end = (index - 1) * 10, index * 10
        value['steps'].append({'number': index, 'name': name, 'status': 'completed', 'conclusion': 'success',
                              'started_at': '2026-10-11T01:00:%02dZ' % start,
                              'completed_at': '2026-10-11T01:00:%02dZ' % end})
    return value


def transport():
    return {'reviewed': True, 'archive_hosts': ['archives.example.test'],
            'recipes': {'quality-linux': {'artifact_key': 'quality-linux', 'begin_step': 1,
                                         'terminal_step': 3, 'upload_step': 4}}}


def provider_capture():
    last = jobs._timestamp('2026-10-11T01:01:00Z')
    return {'source': join_source(), 'jobs': {'quality-linux': provider_job()},
            'window': {'started_ns': last, 'finished_ns': last + 1}}


def boundary_bytes():
    first = jobs._timestamp('2026-10-11T01:00:01Z')
    last = jobs._timestamp('2026-10-11T01:00:21Z')
    before = {'schema': jobs.BOUNDARY_SCHEMA, 'kind': 'begin', 'authority': 'inert', 'nonce': 'a' * 32,
              'source': job_source(), 'window': {'started_ns': first, 'finished_ns': first + 1}}
    with mock.patch.object(jobs.time, 'time_ns', return_value=last + 1):
        after = jobs._terminal(before, job_source(), {'success': True}, last)
    # Deliberately noncanonical. Qualification must retain these exact bytes.
    return json.dumps(before, indent=2).encode(), json.dumps(after, indent=2).encode()


def zip_bytes(rows=None, *, compression=zipfile.ZIP_DEFLATED):
    before, after = boundary_bytes()
    rows = [('begin.json', before), ('terminal.json', after)] if rows is None else rows
    output = io.BytesIO()
    with warnings.catch_warnings(), zipfile.ZipFile(output, 'w', compression=compression) as archive:
        warnings.filterwarnings('ignore', message='Duplicate name:', category=UserWarning)
        for name, body in rows:
            entry = zipfile.ZipInfo(name)
            entry.create_system = 3
            entry.external_attr = (stat.S_IFREG | 0o600) << 16
            entry.compress_type = compression
            archive.writestr(entry, body)
    return output.getvalue()


def provider_artifact(raw=None):
    raw = zip_bytes() if raw is None else raw
    return {'id': 789, 'name': 'package-ci-boundary-123-2-quality-linux',
            'size_in_bytes': len(raw), 'digest': 'sha256:' + hashlib.sha256(raw).hexdigest(),
            'expired': False, 'created_at': '2026-10-11T01:00:31Z',
            'updated_at': '2026-10-11T01:00:39Z', 'expires_at': '2099-10-11T01:00:39Z',
            'workflow_run': {'id': 123, 'head_sha': 'a' * 40}}


def conditional_provider(host):
    recipe_id = 'unit4-portable-hosts:' + host
    actual = json.loads((HERE / 'routes.json').read_text())
    original = copy.deepcopy(actual['job_recipes'][recipe_id]); original['operational'] = True
    expected = join_source(); expected['required_recipes'] = [recipe_id]
    fixed = registry(); fixed['job_recipes'] = {recipe_id: original}
    provider = mapping(); provider['display_name'] = original['display_name']
    windows = host == 'windows-x86_64'
    branches = []
    for number, original_number, applies in ((3, 8, not windows), (4, 9, windows)):
        old = next(step for step in original['steps'] if step['number'] == original_number)
        branches.append({'number': number, 'name': old['name'], 'original_step': original_number,
                         'mandatory': False, 'continue_on_error': False,
                         'expected_conclusion': 'success' if applies else 'skipped'})
    provider['steps'][2]['number'] = 5; provider['steps'][3]['number'] = 6
    provider['steps'][2:2] = branches; provider['terminal_step'] = 5
    fixed['activation']['provider_jobs'] = {recipe_id: provider}
    fixed['activation']['boundary_transport']['recipes'] = {
        recipe_id: {'artifact_key': recipe_id.replace(':', '--').replace('_', '-'),
                    'begin_step': 1, 'terminal_step': 5, 'upload_step': 6}}
    job = provider_job(); job['name'] = provider['display_name']; job['completed_at'] = '2026-10-11T01:01:00Z'
    job['steps'] = []
    for index, step in enumerate(provider['steps']):
        skipped = step['expected_conclusion'] == 'skipped'
        start = '2026-10-11T01:00:%02dZ' % (index * 10)
        end = '2026-10-11T01:00:%02dZ' % ((index + 1) * 10) if index < 5 else job['completed_at']
        job['steps'].append({'number': step['number'], 'name': step['name'], 'status': 'completed',
                             'conclusion': step['expected_conclusion'],
                             'started_at': None if skipped else start, 'completed_at': None if skipped else end})
    return expected, fixed, provider, job


class PureSource:
    def __init__(self, document=None):
        self.value = document or job_source()
    def verify_job(self, context):
        return copy.deepcopy(self.value)
    def verify_join(self, context):
        return join_source()
    def registry(self, context):
        return registry()
    def require_operational(self, context, recipe=None):
        raise jobs.Rejected('operational capture remains held in pure tests')


class JobsControls(unittest.TestCase):
    def test_unsealed_or_detached_owner_rejected(self):
        with self.assertRaises(jobs.Rejected):
            jobs.begin(object())
        with self.assertRaises(jobs.Rejected):
            jobs.capture_ci_outcomes(object())

    def test_public_constructors_and_unissued_objects_rejected(self):
        for cls in (jobs.JobBoundary, jobs.ProviderOutcomes, jobs.BoundaryArtifacts):
            with self.subTest(cls=cls.__name__), self.assertRaises(jobs.Rejected):
                cls({'success': True})
            forged = object.__new__(cls)
            with mock.patch.object(jobs, '_source', return_value=PureSource()), self.assertRaises(jobs.Rejected):
                forged.document()

    def test_boundaries_are_inert_even_with_success_dictionary(self):
        with mock.patch.object(jobs, '_source', return_value=PureSource()):
            boundary = jobs.begin(object())
            before = boundary.document()
            before['source']['checkout_head'] = 'f' * 40
            self.assertEqual(boundary.document()['source']['checkout_head'], 'a' * 40)
            report = jobs.finish(boundary, {'success': True, 'conclusion': 'success'})
            self.assertEqual(report['authority'], 'inert')
            self.assertEqual(report['qualification'], 'unqualified-boundary-only')
            self.assertNotIn('qualified', report)
            with self.assertRaises(jobs.Rejected):
                jobs.finish(boundary)
            with self.assertRaises(jobs.Rejected):
                jobs.finish(boundary.document())

    def test_finish_preserves_actual_source_failure(self):
        source = PureSource()
        with mock.patch.object(jobs, '_source', return_value=source):
            boundary = jobs.begin(object())
            source.value['checkout_head'] = 'f' * 40
            report = jobs.finish(boundary, {'success': True})
            self.assertFalse(report['source_valid'])
            self.assertIn('source readmission failed', report['source_error'])
            self.assertEqual(report['qualification'], 'unqualified-boundary-only')

    def test_separate_step_requires_fresh_source_and_never_restores_owner(self):
        source = PureSource()
        with mock.patch.object(jobs, '_source', return_value=source):
            boundary = jobs.begin(object()); inert = boundary.document()
            count = len(jobs._BOUNDARIES)
            with self.assertRaises(jobs.Rejected):
                jobs.capture_terminal(object(), inert)
            source.value['capture_started_ns'] = inert['window']['finished_ns'] + 1
            source.value['capture_finished_ns'] = source.value['capture_started_ns'] + 1
            source.value['nonce'] = 'f' * 32
            terminal = jobs.capture_terminal(object(), inert, {'outcome': 'failure'})
            self.assertEqual(len(jobs._BOUNDARIES), count)
            self.assertEqual(terminal['captured_status'], {'outcome': 'failure'})
            self.assertEqual(terminal['authority'], 'inert')
            with self.assertRaises(jobs.Rejected):
                jobs.finish(inert)

    def test_separate_step_preserves_fresh_readmission_failure(self):
        source = PureSource()
        with mock.patch.object(jobs, '_source', return_value=source):
            before = jobs.begin(object()).document()
            with mock.patch.object(source, 'verify_job', side_effect=ValueError('current body changed')):
                report = jobs.capture_terminal(object(), before, {'success': True})
            self.assertFalse(report['source_valid'])
            self.assertIsNone(report['source'])
            self.assertIn('current body changed', report['source_error'])

    def test_wrong_job_matrix_host_or_source_rejected(self):
        before = job_source()
        for key, changed in [('checkout_head', 'f' * 40), ('job_recipe_id', 'other'), ('matrix', {'os': 'Windows'}),
                             ('host', 'windows-x86_64'), ('runner_temp', {}), ('source_files', {}), ('workflow_identity', {})]:
            after = copy.deepcopy(before); after[key] = changed
            with self.subTest(key=key), self.assertRaises(jobs.Rejected):
                jobs._same_job(before, after)

    def test_independent_join_does_not_learn_submitted_identity(self):
        expected, job = join_source(), job_source()
        self.assertEqual(jobs._join_job(expected, job, registry()), 'quality-linux')
        for key in ('run_id', 'run_attempt', 'workflow_sha', 'event_name'):
            changed = copy.deepcopy(job); changed['ci'][key] = 'stale'
            with self.subTest(key=key), self.assertRaises(jobs.Rejected):
                jobs._join_job(expected, changed, registry())
        changed = copy.deepcopy(job); changed['job_recipe_id'] = 'invented'
        with self.assertRaises(jobs.Rejected):
            jobs._join_job(expected, changed, registry())

    def test_bounded_strict_json_and_windows(self):
        for raw in (b'{"a":1,"a":2}', b'{"a":NaN}', b'broken', '{"a":1}'.encode('utf-16')):
            with self.subTest(raw=raw), self.assertRaises(jobs.Rejected):
                jobs.decode(raw)
        for window in ({'started_ns': True, 'finished_ns': 2}, {'started_ns': 3, 'finished_ns': 2},
                       {'started_ns': 1, 'finished_ns': 2, 'extra': 0}):
            with self.assertRaises(jobs.Rejected):
                jobs._window(window)

    def test_provider_endpoint_fixed_repository_and_attempt(self):
        self.assertEqual(jobs._endpoint(join_source()), '/repos/YanagiKH/Oxid/actions/runs/123/attempts/2/jobs')
        for key, value in [('repository', 'other/repo'), ('run_id', '1/evil'), ('run_attempt', '0')]:
            join = join_source(); join['ci'][key] = value
            with self.assertRaises(jobs.Rejected):
                jobs._endpoint(join)

    def test_complete_same_endpoint_pagination_only(self):
        endpoint = jobs._endpoint(join_source())
        url = jobs._page_url(endpoint, 2)
        self.assertTrue(jobs._links('<' + url + '>; rel="next"', endpoint, 1))
        self.assertFalse(jobs._links(None, endpoint, 1))
        for bad in (url.replace('api.github.com', 'evil.invalid'), url.replace('/attempts/2/', '/attempts/1/'),
                    url.replace('page=2', 'page=3'), url + '&token=secret', url.replace('https:', 'http:'),
                    url.replace('api.github.com', 'api.github.com:443')):
            with self.subTest(url=bad), self.assertRaises(jobs.Rejected):
                jobs._links('<' + bad + '>; rel="next"', endpoint, 1)
        with self.assertRaises(jobs.Rejected):
            jobs._links('<' + url + '>; rel="next", <' + url + '>; rel="next"', endpoint, 1)

    def test_provider_pure_validation_is_only_detached_data(self):
        raw = provider_job()
        result = jobs._provider_job(raw, mapping(), join_source())
        self.assertEqual(result, raw)
        self.assertIsNot(result, raw)
        self.assertNotIsInstance(result, jobs.ProviderOutcomes)

    def test_provider_fail_skip_cancel_wrong_attempt_and_unknown_steps_reject(self):
        variants = []
        for conclusion in ('failure', 'skipped', 'cancelled', 'neutral', 'timed_out', None):
            value = provider_job(); value['conclusion'] = conclusion; variants.append(value)
            value = provider_job(); value['steps'][1]['conclusion'] = conclusion; variants.append(value)
        for key, replacement in [('id', True), ('run_attempt', 1), ('head_sha', 'f' * 40), ('workflow_name', 'Other')]:
            value = provider_job(); value[key] = replacement; variants.append(value)
        value = provider_job(); value['steps'].reverse(); variants.append(value)
        value = provider_job(); value['steps'][1]['name'] = 'Different Cargo test'; variants.append(value)
        value = provider_job(); value['steps'].pop(); variants.append(value)
        value = provider_job(); value['steps'][0]['completed_at'] = '2026-10-11T01:00:25Z'; variants.append(value)
        for value in variants:
            with self.subTest(value=value), self.assertRaises(jobs.Rejected):
                jobs._provider_job(value, mapping(), join_source())

    def test_documented_job_schema_omitted_attempt_gaps_and_offsets(self):
        # Inert adapted schema control from GitHub's workflow-jobs reference:
        # run_attempt is absent; post-job step numbers need not be contiguous;
        # step times may have explicit UTC offsets.
        value, recipe = provider_job(), mapping()
        value.pop('run_attempt')
        for actual, expected, number in zip(value['steps'], recipe['steps'], (1, 8, 17, 20)):
            actual['number'] = expected['number'] = number
            actual['started_at'] = actual['started_at'].replace('01:00:', '02:00:').replace('Z', '+01:00')
            actual['completed_at'] = actual['completed_at'].replace('01:00:', '02:00:').replace('Z', '+01:00')
        recipe['terminal_step'] = 17
        fixed = registry(); fixed['activation']['provider_jobs']['quality-linux'] = recipe
        self.assertEqual(jobs._activation(join_source(), fixed)['quality-linux']['terminal_step'], 17)
        detached = jobs._provider_job(value, recipe, join_source())
        self.assertNotIn('run_attempt', detached)
        self.assertNotIsInstance(detached, jobs.ProviderOutcomes)
        self.assertEqual(jobs._timestamp('2026-10-11T02:00:00.001+01:00'),
                         jobs._timestamp('2026-10-11T01:00:00.001Z'))

    def test_structural_replay_cannot_qualify_unauthenticated_boundary_bytes(self):
        before, after = boundary_bytes()
        pair = jobs._boundary_pair(join_source(), provider_capture(), registry(), before, after)
        self.assertEqual(pair[0], 'quality-linux')
        self.assertIs(type(pair[1]), dict)
        self.assertNotIsInstance(pair, jobs.BoundaryArtifacts)
        for begin_value, end_value in ((json.loads(before), json.loads(after)), (before, after)):
            with mock.patch.object(jobs, '_source', return_value=PureSource()), self.assertRaises(jobs.Rejected):
                jobs.qualify_job(object(), begin_value, end_value, {'jobs': 'invented'}, {'artifacts': 'invented'})
        changed = json.loads(after); changed['begin_sha256'] = 'f' * 64
        with self.assertRaises(jobs.Rejected):
            jobs._boundary_pair(join_source(), provider_capture(), registry(), before, jobs.canonical(changed))

    def test_missing_duplicate_or_ambiguous_provider_pages_fail_without_issuance(self):
        source = PureSource()
        page_variants = [
            {'total_count': 2, 'jobs': [provider_job()]},
            {'total_count': 2, 'jobs': [provider_job(), provider_job()]},
            {'total_count': 0, 'jobs': []},
        ]
        other = provider_job(); other['id'] = 999
        page_variants.append({'total_count': 2, 'jobs': [provider_job(), other]})
        count = len(jobs._OUTCOMES)
        for page in page_variants:
            with mock.patch.object(jobs, '_source', return_value=source), \
                 mock.patch.object(source, 'require_operational', return_value=None), \
                 mock.patch.object(jobs, '_http_page', return_value=(jobs.canonical(page), None)) as capture:
                with self.assertRaises(jobs.Rejected):
                    jobs.capture_ci_outcomes(object())
                capture.assert_called_once()
                self.assertEqual(len(jobs._OUTCOMES), count)

    def test_pending_activation_and_continue_on_error_reject(self):
        expected = join_source()
        self.assertEqual(set(jobs._activation(expected, registry())), {'quality-linux'})
        for mutate in (lambda x: x['activation'].update(approved=False),
                       lambda x: x['activation'].update(provider_jobs={}),
                       lambda x: x['job_recipes']['quality-linux']['steps'][0].update(continue_on_error=True),
                       lambda x: x['job_recipes']['quality-linux'].update(job_continue_on_error=True),
                       lambda x: x['activation']['provider_jobs']['quality-linux']['steps'][1].update(continue_on_error=True)):
            value = registry(); mutate(value)
            with self.assertRaises(jobs.Rejected):
                jobs._activation(expected, value)

    def test_operational_gate_precedes_network(self):
        with mock.patch.object(jobs, '_source', return_value=PureSource()), mock.patch.object(jobs, '_http_page') as network:
            with self.assertRaises(jobs.Rejected):
                jobs.capture_ci_outcomes(object())
            network.assert_not_called()

    def test_redirects_fail_without_second_request_and_connection_closes(self):
        connection = mock.Mock()
        connection.getresponse.return_value.status = 302
        with mock.patch.object(jobs.http.client, 'HTTPSConnection', return_value=connection):
            with self.assertRaises(jobs.Rejected):
                jobs._http_page(jobs._endpoint(join_source()), 1, jobs.time.monotonic() + 5)
        connection.request.assert_called_once()
        connection.close.assert_called_once()
        arguments = connection.request.call_args
        self.assertNotIn('Authorization', arguments.kwargs['headers'])

    def test_invented_provider_dictionary_cannot_qualify(self):
        with mock.patch.object(jobs, '_source', return_value=PureSource()), self.assertRaises(jobs.Rejected):
            jobs.qualify_job(object(), {}, {}, {'success': True}, {'success': True})


class BoundaryArtifactControls(unittest.TestCase):
    def deadline(self):
        return jobs.time.monotonic() + 5

    def test_dormant_registry_and_opaque_owners_gate_all_io(self):
        actual = json.loads((HERE / 'routes.json').read_text())
        self.assertIs(actual['activation']['approved'], False)
        with mock.patch.object(jobs, '_http_page') as listing, mock.patch.object(jobs, '_http_artifact') as download:
            with self.assertRaises(jobs.Rejected):
                jobs._collect_boundary_artifacts(join_source(), provider_capture(), actual, self.deadline(), 1)
            for owner in ({'success': True}, object.__new__(jobs.ProviderOutcomes)):
                with mock.patch.object(jobs, '_source', return_value=PureSource()), self.assertRaises(jobs.Rejected):
                    jobs.capture_boundary_artifacts(object(), owner)
            listing.assert_not_called(); download.assert_not_called()

    def test_transport_activation_exact_closed_steps_and_hosts(self):
        self.assertEqual(jobs._transport_activation(join_source(), registry()), transport())
        mutations = [lambda value: value.update(reviewed=False), lambda value: value.update(archive_hosts=[]),
                     lambda value: value.update(extra=True), lambda value: value.update(recipes={}),
                     lambda value: value['recipes']['quality-linux'].update(extra=1),
                     lambda value: value['recipes']['quality-linux'].update(artifact_key='different'),
                     lambda value: value['recipes']['quality-linux'].update(begin_step=True),
                     lambda value: value['recipes']['quality-linux'].update(terminal_step=4),
                     lambda value: value['recipes']['quality-linux'].update(upload_step=3),
                     lambda value: value['recipes']['quality-linux'].update(upload_step=99)]
        for host in ('*.example.test', 'localhost', '127.0.0.1', 'example.test:443',
                     'archive.example.test/path', 'user@example.test', 'Upper.example.test'):
            mutations.append(lambda value, host=host: value.update(archive_hosts=[host]))
        mutations.append(lambda value: value.update(archive_hosts=['archive.example.test'] * 2))
        for mutate in mutations:
            value = registry(); mutate(value['activation']['boundary_transport'])
            with self.subTest(registry=value), self.assertRaises(jobs.Rejected):
                jobs._transport_activation(join_source(), value)
        value = registry(); value['activation']['provider_jobs']['quality-linux']['steps'][3]['mandatory'] = False
        with self.assertRaises(jobs.Rejected):
            jobs._transport_activation(join_source(), value)

    def test_artifact_key_collision_and_non_ascii_rejected(self):
        for ids in (('a:b', 'a--b'), ('a_b', 'a-b'), ('unicode-é', 'normal'), ('Uppercase', 'normal'), ('with.dot', 'normal')):
            expected = join_source(); expected['required_recipes'] = list(ids)
            fixed = registry(); fixed['job_recipes'] = {}; fixed['activation']['provider_jobs'] = {}
            fixed['activation']['boundary_transport']['recipes'] = {}
            for index, recipe_id in enumerate(ids):
                recipe = registry()['job_recipes']['quality-linux']; recipe['id'] = recipe_id
                recipe['display_name'] = 'Unique display ' + str(index)
                provider = mapping(); provider['display_name'] = recipe['display_name']
                fixed['job_recipes'][recipe_id] = recipe
                fixed['activation']['provider_jobs'][recipe_id] = provider
                row = transport()['recipes']['quality-linux']; row['artifact_key'] = recipe_id.replace(':', '--').replace('_', '-')
                fixed['activation']['boundary_transport']['recipes'][recipe_id] = row
            with self.assertRaises(jobs.Rejected):
                jobs._transport_activation(expected, fixed)

    def test_run_scoped_artifact_endpoint_and_attempt_specific_name(self):
        self.assertEqual(jobs._artifact_endpoint(join_source()), '/repos/YanagiKH/Oxid/actions/runs/123/artifacts')
        self.assertEqual(jobs._artifact_name(join_source(), transport()['recipes']['quality-linux']),
                         'package-ci-boundary-123-2-quality-linux')
        expected = join_source(); expected['ci']['run_attempt'] = '3'
        self.assertNotEqual(jobs._artifact_name(expected, transport()['recipes']['quality-linux']), provider_artifact()['name'])

    def test_listing_retains_all_original_pages_without_issuing_owner(self):
        items = [dict(provider_artifact(), id=1000 + number, name='other-' + str(number)) for number in range(101)]
        endpoint = jobs._artifact_endpoint(join_source())
        link = '<' + jobs._page_url(endpoint, 2) + '>; rel="next"'
        raw = [json.dumps({'total_count': 101, 'artifacts': chunk}, indent=1).encode()
               for chunk in (items[:100], items[100:])]
        count = len(jobs._BOUNDARY_ARTIFACTS)
        with mock.patch.object(jobs, '_http_page', side_effect=[(raw[0], link), (raw[1], None)]) as capture:
            listed, pages = jobs._artifact_listing(join_source(), self.deadline())
        self.assertEqual(listed, items)
        self.assertEqual([page['body'].encode() for page in pages], raw)
        self.assertEqual(pages[0]['link'], link)
        self.assertEqual([call.args[1] for call in capture.call_args_list], [1, 2])
        self.assertEqual(len(jobs._BOUNDARY_ARTIFACTS), count)

    def test_incomplete_duplicate_or_changed_listing_rejected(self):
        one = provider_artifact()
        duplicate_name = dict(one, id=999)
        duplicate_id = dict(one, name='other')
        bad_id = dict(one, id=True)
        variants = [{'total_count': 2, 'artifacts': [one]}, {'total_count': 0, 'artifacts': []},
                    {'total_count': 2, 'artifacts': [one, duplicate_name]},
                    {'total_count': 2, 'artifacts': [one, duplicate_id]},
                    {'total_count': 1, 'artifacts': [bad_id]}, {'total_count': True, 'artifacts': [one]}]
        for value in variants:
            with mock.patch.object(jobs, '_http_page', return_value=(jobs.canonical(value), None)), self.assertRaises(jobs.Rejected):
                jobs._artifact_listing(join_source(), self.deadline())
        endpoint = jobs._artifact_endpoint(join_source())
        link = '<' + jobs._page_url(endpoint, 2) + '>; rel="next"'
        rows = [dict(one, id=1000 + i, name='other-' + str(i)) for i in range(100)]
        for second in ({'total_count': 102, 'artifacts': [one]}, {'total_count': 101, 'artifacts': []}):
            with mock.patch.object(jobs, '_http_page', side_effect=[(jobs.canonical({'total_count': 101, 'artifacts': rows}), link),
                                                                  (jobs.canonical(second), None)]), self.assertRaises(jobs.Rejected):
                jobs._artifact_listing(join_source(), self.deadline())

    def test_listing_pagination_cannot_switch_to_attempt_jobs_or_repository(self):
        endpoint = jobs._artifact_endpoint(join_source())
        for url in (jobs._page_url(jobs._endpoint(join_source()), 2),
                    jobs._page_url(endpoint.replace('/123/', '/124/'), 2),
                    jobs._page_url(endpoint.replace('YanagiKH/Oxid', 'Other/Repo'), 2)):
            with self.assertRaises(jobs.Rejected):
                jobs._links('<' + url + '>; rel="next"', endpoint, 1)

    def test_artifact_identity_digest_expiry_and_step_windows(self):
        now = jobs._timestamp('2026-10-11T01:01:01Z')
        recipe = transport()['recipes']['quality-linux']
        result = jobs._artifact_record(provider_artifact(), join_source(), recipe, provider_job(), now)
        self.assertEqual(result['id'], 789)
        variants = [('id', True), ('name', 'package-ci-boundary-123-1-quality-linux'), ('expired', True),
                    ('size_in_bytes', 0), ('size_in_bytes', jobs.MAX_ARCHIVE + 1), ('digest', 'sha256:bad'),
                    ('digest', None), ('workflow_run', {'id': 124, 'head_sha': 'a' * 40}),
                    ('workflow_run', {'id': 123, 'head_sha': 'f' * 40}),
                    ('created_at', '2026-10-11T01:00:29Z'), ('updated_at', '2026-10-11T01:00:41Z'),
                    ('expires_at', '2026-10-11T01:01:00Z')]
        for key, replacement in variants:
            value = provider_artifact(); value[key] = replacement
            with self.subTest(field=key), self.assertRaises(jobs.Rejected):
                jobs._artifact_record(value, join_source(), recipe, provider_job(), now)

    def test_redirect_exact_https_host_port_and_one_hop_only(self):
        hosts = ['archives.example.test']
        self.assertEqual(jobs._archive_target('https://archives.example.test:443/object?signature=ephemeral', hosts),
                         ('archives.example.test', '/object?signature=ephemeral'))
        for url in ('http://archives.example.test/object', 'https://evil.example.test/object',
                    'https://archives.example.test.evil.test/object', 'https://user@archives.example.test/object',
                    'https://archives.example.test:444/object', 'https://archives.example.test/object#fragment',
                    'https://archives.example.test./object', 'https://archives.example.test/object\n',
                    'https://archives.example.test:invalid/object', 'https://archives.example.test',
                    'https://ARCHIVES.example.test/object'):
            with self.subTest(url=url), self.assertRaises(jobs.Rejected):
                jobs._archive_target(url, hosts)
        with mock.patch.object(jobs, '_http_archive_response', side_effect=[(b'', 'https://archives.example.test/object'),
                                                                         (b'zip', 'https://archives.example.test/again')]) as io_call:
            with self.assertRaises(jobs.Rejected):
                jobs._http_artifact(789, hosts, self.deadline())
            self.assertEqual(io_call.call_count, 2)

    def test_fixed_download_ignores_caller_url_and_does_not_retain_signed_url(self):
        location = 'https://archives.example.test/object?signature=ephemeral'
        with mock.patch.object(jobs, '_http_archive_response', side_effect=[(b'', location), (b'zip', None)]) as io_call:
            raw, route = jobs._http_artifact(789, ['archives.example.test'], self.deadline())
        self.assertEqual(raw, b'zip')
        self.assertEqual(io_call.call_args_list[0].args[:2],
                         ('api.github.com', '/repos/YanagiKH/Oxid/actions/artifacts/789/zip'))
        self.assertEqual(io_call.call_args_list[1].args[:2], ('archives.example.test', '/object?signature=ephemeral'))
        self.assertEqual(route['location_sha256'], hashlib.sha256(location.encode()).hexdigest())
        self.assertNotIn('ephemeral', json.dumps(route))

    def test_archive_http_status_and_size_fail_closed_without_credentials(self):
        for redirect, status in ((True, 200), (True, 401), (True, 403), (False, 302), (False, 307)):
            connection = mock.Mock(); connection.getresponse.return_value.status = status
            with mock.patch.object(jobs.http.client, 'HTTPSConnection', return_value=connection), self.assertRaises(jobs.Rejected):
                jobs._http_archive_response('archives.example.test', '/object', self.deadline(), redirect=redirect)
            self.assertEqual(connection.request.call_count, 1)
            self.assertEqual(connection.close.call_count, 1)
            self.assertNotIn('Authorization', connection.request.call_args.kwargs['headers'])
            self.assertNotIn('Cookie', connection.request.call_args.kwargs['headers'])
        for headers, chunks in (({'Content-Length': '5'}, [b'abc', b'']),
                                ({'Content-Length': str(jobs.MAX_ARCHIVE + 1)}, []),
                                ({'Content-Encoding': 'gzip'}, [])):
            connection = mock.Mock(); response = connection.getresponse.return_value; response.status = 200
            response.getheader.side_effect = lambda key, default=None: headers.get(key, default)
            response.read1.side_effect = chunks
            with mock.patch.object(jobs.http.client, 'HTTPSConnection', return_value=connection), self.assertRaises(jobs.Rejected):
                jobs._http_archive_response('archives.example.test', '/object', self.deadline(), redirect=False)

    def test_two_regular_zip_members_are_exact_original_bytes(self):
        before, after = boundary_bytes()
        for compression in (zipfile.ZIP_STORED, zipfile.ZIP_DEFLATED):
            result = jobs._zip_members(zip_bytes(compression=compression), self.deadline())
            self.assertEqual(result, {'begin.json': before, 'terminal.json': after})
            self.assertNotEqual(result['begin.json'], jobs.canonical(json.loads(before)))

    def test_zip_streaming_descriptors_supported_without_weakening_roster(self):
        class Nonseekable(io.BytesIO):
            def seekable(self):
                return False
            def seek(self, *args):
                raise io.UnsupportedOperation('no seeking')
        output = Nonseekable()
        with zipfile.ZipFile(output, 'w', compression=zipfile.ZIP_DEFLATED) as archive:
            for name, raw in zip(('begin.json', 'terminal.json'), boundary_bytes()):
                archive.writestr(name, raw)
        self.assertEqual(jobs._zip_members(output.getvalue(), self.deadline())['begin.json'], boundary_bytes()[0])

    def test_zip_unsafe_names_extra_members_directories_and_duplicates_rejected(self):
        for name in ('../begin.json', '/begin.json', './begin.json', 'begin.json/', 'BEGIN.JSON',
                     'dir/begin.json', 'begin.json\\evil', 'begin.json\x00alias'):
            raw = zip_bytes([(name, b'{}'), ('terminal.json', b'{}')])
            # ZipInfo strips NUL before writing; mutate the physical name to
            # retain a hostile alias instead of assuming writestr preserves it.
            if '\x00' in name:
                raw = zip_bytes([('begin.jsonX', b'{}'), ('terminal.json', b'{}')]).replace(b'begin.jsonX', b'begin.json\x00')
            with self.subTest(name=name), self.assertRaises(jobs.Rejected):
                jobs._zip_members(raw, self.deadline())
        for rows in ([('begin.json', b'{}')], [('begin.json', b'{}'), ('terminal.json', b'{}'), ('extra', b'{}')],
                     [('begin.json', b'{}'), ('begin.json', b'{}')]):
            with self.assertRaises(jobs.Rejected):
                jobs._zip_members(zip_bytes(rows), self.deadline())

    def test_zip_special_modes_encryption_and_unsupported_methods_rejected(self):
        original = zip_bytes()
        central = original.index(b'PK\x01\x02')
        for mode in (stat.S_IFLNK, stat.S_IFIFO, stat.S_IFSOCK, stat.S_IFCHR, stat.S_IFBLK, stat.S_IFDIR):
            raw = bytearray(original); struct.pack_into('<I', raw, central + 38, (mode | 0o600) << 16)
            with self.subTest(mode=mode), self.assertRaises(jobs.Rejected):
                jobs._zip_members(bytes(raw), self.deadline())
        for bit in (1, 0x40, 0x2000):
            raw = bytearray(original); struct.pack_into('<H', raw, 6, bit); struct.pack_into('<H', raw, central + 8, bit)
            with self.assertRaises(jobs.Rejected):
                jobs._zip_members(bytes(raw), self.deadline())
        with self.assertRaises(jobs.Rejected):
            jobs._zip_members(zip_bytes(compression=zipfile.ZIP_BZIP2), self.deadline())

    def test_zip_closed_physical_layout_crc_and_limits(self):
        raw = zip_bytes()
        corrupt = bytearray(raw); corrupt[42] ^= 0xff
        for value in (b'', b'not zip', raw + b'trailing', b'preamble' + raw, raw[:-1], bytes(corrupt)):
            with self.assertRaises(jobs.Rejected):
                jobs._zip_members(value, self.deadline())
        with self.assertRaises(jobs.Rejected):
            jobs._zip_members(zip_bytes([('begin.json', b'0' * 100000), ('terminal.json', b'{}')]), self.deadline())
        for cap, value in (('MAX_ARCHIVE', 1), ('MAX_DOCUMENT', 1), ('MAX_UNCOMPRESSED', 1), ('MAX_COMPRESSION_RATIO', 1)):
            with mock.patch.object(jobs, cap, value), self.assertRaises(jobs.Rejected):
                jobs._zip_members(raw, self.deadline())
        with self.assertRaises(jobs.Rejected):
            jobs._zip_members(raw, jobs.time.monotonic() - 1)

    def test_complete_mock_collector_retains_bytes_but_cannot_issue_capability(self):
        raw = zip_bytes(); artifact = provider_artifact(raw)
        page = json.dumps({'total_count': 1, 'artifacts': [artifact]}, indent=2).encode()
        count = len(jobs._BOUNDARY_ARTIFACTS)
        with mock.patch.object(jobs, '_http_page', return_value=(page, None)), \
             mock.patch.object(jobs, '_http_artifact', return_value=(raw, {'synthetic': True})):
            document, members, archives = jobs._collect_boundary_artifacts(
                join_source(), provider_capture(), registry(), self.deadline(), jobs.time.time_ns())
        self.assertEqual(document['pages'][0]['body'].encode(), page)
        self.assertEqual(archives, {'quality-linux': raw})
        self.assertEqual(members['quality-linux']['begin.json'], boundary_bytes()[0])
        row = document['artifacts']['quality-linux']
        self.assertEqual(set(row['steps']), {'begin', 'terminal', 'upload'})
        self.assertEqual(row['members']['begin.json']['sha256'], hashlib.sha256(boundary_bytes()[0]).hexdigest())
        self.assertEqual(len(jobs._BOUNDARY_ARTIFACTS), count)
        with mock.patch.object(jobs, '_source', return_value=PureSource()), self.assertRaises(jobs.Rejected):
            jobs.qualify_job(object(), *boundary_bytes(), provider_capture(), document)

    def test_wrong_archive_digest_size_and_stale_attempt_fail_before_issuance(self):
        raw = zip_bytes()
        for key, value in (('digest', 'sha256:' + 'f' * 64), ('size_in_bytes', len(raw) + 1),
                           ('name', 'package-ci-boundary-123-1-quality-linux')):
            artifact = provider_artifact(raw); artifact[key] = value
            with mock.patch.object(jobs, '_http_page', return_value=(jobs.canonical({'total_count': 1, 'artifacts': [artifact]}), None)), \
                 mock.patch.object(jobs, '_http_artifact', return_value=(raw, {'synthetic': True})), self.assertRaises(jobs.Rejected):
                jobs._collect_boundary_artifacts(join_source(), provider_capture(), registry(), self.deadline(), 1)
        self.assertEqual(len(jobs._BOUNDARY_ARTIFACTS), 0)

    def test_downloaded_json_failure_preservation_and_source_step_binding(self):
        before, after = boundary_bytes()
        variants = []
        bad = json.loads(before); bad['source']['ci']['run_attempt'] = '1'; variants.append((jobs.canonical(bad), after))
        bad = json.loads(before); bad['window'] = {'started_ns': 1, 'finished_ns': 2}; variants.append((jobs.canonical(bad), after))
        bad = json.loads(after); bad['source_valid'] = False; bad['source_error'] = 'changed'; variants.append((before, jobs.canonical(bad)))
        bad = json.loads(after); bad['source']['matrix'] = {'os': 'other'}; variants.append((before, jobs.canonical(bad)))
        variants.extend(((b'{"duplicate":1,"duplicate":2}', after), (before, b'{"bad":NaN}')))
        for first, last in variants:
            raw = zip_bytes([('begin.json', first), ('terminal.json', last)])
            artifact = provider_artifact(raw)
            with mock.patch.object(jobs, '_http_page', return_value=(jobs.canonical({'total_count': 1, 'artifacts': [artifact]}), None)), \
                 mock.patch.object(jobs, '_http_artifact', return_value=(raw, {'synthetic': True})), self.assertRaises(jobs.Rejected):
                jobs._collect_boundary_artifacts(join_source(), provider_capture(), registry(), self.deadline(), 1)


class ConditionalProviderControls(unittest.TestCase):
    def test_opposite_platform_branches_are_fixed_by_original_step_and_matrix(self):
        for host in ('windows-x86_64', 'macos-x86_64', 'macos-aarch64'):
            expected, fixed, provider, job = conditional_provider(host)
            recipe_id = expected['required_recipes'][0]
            admitted = jobs._activation(expected, fixed)
            self.assertEqual(admitted[recipe_id], provider)
            self.assertEqual(jobs._provider_job(job, admitted[recipe_id], expected), job)
            self.assertEqual([step['conclusion'] for step in job['steps'][2:4]],
                             ['skipped', 'success'] if host == 'windows-x86_64' else ['success', 'skipped'])
            self.assertEqual(jobs._transport_activation(expected, fixed), fixed['activation']['boundary_transport'])
            self.assertEqual(len(jobs._OUTCOMES), 0)
            self.assertEqual(len(jobs._BOUNDARY_ARTIFACTS), 0)

    def test_wrong_platform_branch_cannot_run_or_skip_by_caller_choice(self):
        for host in ('windows-x86_64', 'macos-x86_64'):
            expected, fixed, provider, job = conditional_provider(host)
            for index in (2, 3):
                wrong = copy.deepcopy(job)
                wrong['steps'][index]['conclusion'] = 'success' if job['steps'][index]['conclusion'] == 'skipped' else 'skipped'
                with self.assertRaises(jobs.Rejected):
                    jobs._provider_job(wrong, provider, expected)
                wrong_policy = copy.deepcopy(fixed)
                recipe_id = expected['required_recipes'][0]
                policy = wrong_policy['activation']['provider_jobs'][recipe_id]['steps'][index]
                policy['expected_conclusion'] = wrong['steps'][index]['conclusion']
                with self.assertRaises(jobs.Rejected):
                    jobs._activation(expected, wrong_policy)

    def test_mandatory_skip_and_unknown_expected_conclusions_reject(self):
        expected, fixed, provider, job = conditional_provider('windows-x86_64')
        recipe_id = expected['required_recipes'][0]
        for index in (0, 1, 4, 5):
            wrong = copy.deepcopy(job)
            wrong['steps'][index].update(conclusion='skipped', started_at=None, completed_at=None)
            with self.assertRaises(jobs.Rejected):
                jobs._provider_job(wrong, provider, expected)
            bad = copy.deepcopy(fixed)
            bad['activation']['provider_jobs'][recipe_id]['steps'][index]['expected_conclusion'] = 'skipped'
            with self.assertRaises(jobs.Rejected):
                jobs._activation(expected, bad)
        for conclusion in (None, 'failure', 'cancelled', 'neutral', 'any'):
            bad = copy.deepcopy(fixed)
            bad['activation']['provider_jobs'][recipe_id]['steps'][2]['expected_conclusion'] = conclusion
            with self.assertRaises(jobs.Rejected):
                jobs._activation(expected, bad)

    def test_nonmandatory_flag_alone_does_not_authorize_any_skip(self):
        expected, fixed, provider, job = conditional_provider('windows-x86_64')
        recipe_id = expected['required_recipes'][0]
        for mutate in (lambda step: step.pop('original_step'), lambda step: step.update(original_step=True),
                       lambda step: step.update(original_step=1), lambda step: step.update(name='invented name')):
            bad = copy.deepcopy(fixed); mutate(bad['activation']['provider_jobs'][recipe_id]['steps'][2])
            with self.assertRaises(jobs.Rejected):
                jobs._activation(expected, bad)
        for condition in (None, 'always()', "runner.os == 'Linux'", '${{ arbitrary.expression }}'):
            bad = copy.deepcopy(fixed)
            bad['job_recipes'][recipe_id]['steps'][7]['condition'] = condition
            with self.assertRaises(jobs.Rejected):
                jobs._activation(expected, bad)
        bad = copy.deepcopy(fixed); bad['job_recipes'][recipe_id]['matrix']['os'] = 'macos-14'
        with self.assertRaises(jobs.Rejected):
            jobs._activation(expected, bad)
        bad = copy.deepcopy(fixed)
        steps = bad['activation']['provider_jobs'][recipe_id]['steps']
        steps[3].update(original_step=8, name=steps[2]['name'], expected_conclusion='skipped')
        with self.assertRaises(jobs.Rejected):
            jobs._activation(expected, bad)

    def test_skipped_timestamp_null_policy_creates_no_execution_window(self):
        expected, fixed, provider, job = conditional_provider('windows-x86_64')
        for field in ('started_at', 'completed_at'):
            bad = copy.deepcopy(job); bad['steps'][2].pop(field)
            with self.assertRaises(jobs.Rejected):
                jobs._provider_job(bad, provider, expected)
            bad = copy.deepcopy(job); bad['steps'][2][field] = '2026-10-11T01:00:25Z'
            with self.assertRaises(jobs.Rejected):
                jobs._provider_job(bad, provider, expected)
        # The timed Windows step still has to follow the last timed success;
        # the intervening null-timestamp skip cannot reset that ordering.
        bad = copy.deepcopy(job); bad['steps'][3]['started_at'] = '2026-10-11T01:00:19Z'
        with self.assertRaises(jobs.Rejected):
            jobs._provider_job(bad, provider, expected)


if __name__ == '__main__':
    unittest.main()
