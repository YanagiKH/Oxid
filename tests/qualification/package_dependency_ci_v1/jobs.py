"""Job source boundaries and independently captured GitHub outcomes.

Boundary documents are inert. A private ProviderOutcomes can be issued only by
capture_ci_outcomes' fixed public GitHub read, after workflow activation review.
No command is wrapped or executed by this module.
"""
import datetime
import hashlib
import http.client
import io
import json
from pathlib import Path
import re
import sys
import socket
import stat
import struct
import threading
import time
import types
import urllib.parse
import uuid
import weakref
import zipfile
import zlib

SOURCE_KEY = '_oxid_package_dependency_ci_source_v1'
SOURCE_SHA256 = 'a202686c177c128628420c4f208aff67cf33830878af2df6132c739ef9cf081d'  # Set only when the canonical source owner is sealed.
BOUNDARY_SCHEMA = 'oxid-package-ci-job-boundary-v1'
OUTCOMES_SCHEMA = 'oxid-package-ci-provider-outcomes-v1'
QUALIFIED_SCHEMA = 'oxid-package-ci-qualified-job-v1'
ARTIFACTS_SCHEMA = 'oxid-package-ci-boundary-artifacts-v1'
MAX_DOCUMENT = 32 * 1024 ** 2
MAX_PAGE = 8 * 1024 ** 2
MAX_RESPONSE = 32 * 1024 ** 2
MAX_PAGES = 20
MAX_JOBS = 2000
HTTP_SECONDS = 60
HTTP_TIMEOUT = 10
MAX_ARTIFACTS = 2000
MAX_ARCHIVE = 32 * 1024 ** 2
MAX_ARCHIVES = 64 * 1024 ** 2
MAX_UNCOMPRESSED = 64 * 1024 ** 2
MAX_COMPRESSION_RATIO = 100
ARCHIVE_SECONDS = 15
_HOST = 'api.github.com'
_BOUNDARIES = weakref.WeakKeyDictionary()
_OUTCOMES = weakref.WeakKeyDictionary()
_BOUNDARY_ARTIFACTS = weakref.WeakKeyDictionary()


class Rejected(ValueError):
    pass


def need(value, message):
    if not value:
        raise Rejected(message)


def canonical(value):
    return json.dumps(value, sort_keys=True, separators=(',', ':'), allow_nan=False).encode()


def decode(raw):
    need(type(raw) is bytes and len(raw) <= MAX_DOCUMENT, 'bounded JSON document required')
    def pairs(items):
        result = {}
        for key, value in items:
            need(key not in result, 'duplicate JSON key')
            result[key] = value
        return result
    try:
        return json.loads(raw.decode('utf-8'), object_pairs_hook=pairs,
                          parse_constant=lambda _: need(False, 'nonfinite JSON'))
    except (UnicodeError, json.JSONDecodeError, RecursionError) as error:
        raise Rejected('invalid JSON document') from error


def clone(value):
    return decode(canonical(value))


def _source():
    owner = globals().get('_SOURCE_OWNER')
    need(type(owner) is types.ModuleType and sys.modules.get(SOURCE_KEY) is owner,
         'jobs requires its authenticated canonical source owner')
    need(type(SOURCE_SHA256) is str and re.fullmatch('[0-9a-f]{64}', SOURCE_SHA256),
         'jobs source owner pin is not sealed')
    owner.owner()
    need(owner.identity(Path(__file__).resolve().with_name('source.py'))['sha256'] == SOURCE_SHA256,
         'jobs source owner body drift')
    need(owner.owns_sibling(sys.modules.get(__name__)), 'unissued jobs sibling')
    return owner


def _ns(value):
    need(type(value) is int and value > 0, 'positive integer capture time required')
    return value


def _window(value):
    need(type(value) is dict and set(value) == {'started_ns', 'finished_ns'}, 'closed capture window')
    start, end = _ns(value['started_ns']), _ns(value['finished_ns'])
    need(start <= end, 'reversed capture window')
    return start, end


def _timestamp(value):
    need(type(value) is str and re.fullmatch(r'\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d{1,6})?(?:Z|[+-]\d{2}:\d{2})', value),
         'timezone-qualified provider timestamp required')
    try:
        result = datetime.datetime.fromisoformat(value[:-1] + '+00:00' if value.endswith('Z') else value)
    except ValueError as error:
        raise Rejected('invalid provider timestamp') from error
    epoch = datetime.datetime(1970, 1, 1, tzinfo=datetime.timezone.utc)
    delta = result.astimezone(datetime.timezone.utc) - epoch
    return ((delta.days * 86400 + delta.seconds) * 1_000_000 + delta.microseconds) * 1000


class JobBoundary:
    __slots__ = ('__weakref__',)
    def __new__(cls, *args, **kwargs):
        raise Rejected('JobBoundary is issued only by begin')
    def document(self):
        _source()
        need(type(self) is JobBoundary and self in _BOUNDARIES, 'unissued job boundary')
        return clone(_BOUNDARIES[self]['document'])


class ProviderOutcomes:
    __slots__ = ('__weakref__',)
    def __new__(cls, *args, **kwargs):
        raise Rejected('ProviderOutcomes requires actual fixed provider capture')
    def document(self):
        _source()
        need(type(self) is ProviderOutcomes and self in _OUTCOMES, 'unissued provider outcomes')
        return clone(_OUTCOMES[self]['document'])


class BoundaryArtifacts:
    __slots__ = ('__weakref__',)
    def __new__(cls, *args, **kwargs):
        raise Rejected('BoundaryArtifacts requires actual fixed artifact capture')
    def document(self):
        _source()
        need(type(self) is BoundaryArtifacts and self in _BOUNDARY_ARTIFACTS,
             'unissued boundary artifact capture')
        # This detached document contains evidence, never a transferable owner.
        return clone(_BOUNDARY_ARTIFACTS[self]['document'])


# These fixed selectors are deliberately explicit. Repository paths, machine
# identity and capture nonces are compared within one job, never across hosts.
_SAME_JOB = ('repository_path', 'checkout_head', 'checkout_tree', 'event_sha',
             'source_checkpoint', 'source_manifest_identity', 'source_files',
             'workflow_identity', 'input_inventory_identity', 'route_registry_identity',
             'package_identity', 'ci', 'job_recipe_id', 'matrix', 'host', 'runner_temp', 'activation', 'status')
_JOIN_KEYS = ('checkout_head', 'checkout_tree', 'event_sha', 'source_checkpoint',
              'source_files', 'activation', 'status')
_BODY_KEYS = ('source_manifest_identity', 'workflow_identity', 'input_inventory_identity',
              'route_registry_identity', 'package_identity')
_CI_JOIN_KEYS = ('repository', 'run_id', 'run_attempt', 'event_name', 'workflow',
                 'workflow_ref', 'workflow_sha')


def _job_key(document):
    need(type(document) is dict, 'job source document required')
    need(all(key in document for key in _SAME_JOB), 'incomplete job source identity')
    return {key: document[key] for key in _SAME_JOB}


def _same_job(before, after):
    need(_job_key(before) == _job_key(after), 'job source, workflow, host or recipe changed')


def _body(value):
    need(type(value) is dict and type(value.get('bytes')) is int and value['bytes'] >= 0
         and type(value.get('sha256')) is str and re.fullmatch('[0-9a-f]{64}', value['sha256']),
         'body identity required')
    return {'bytes': value['bytes'], 'sha256': value['sha256']}


def _join_job(join, job, registry):
    _job_key(job)
    need(all(key in join for key in _JOIN_KEYS), 'incomplete expected join identity')
    for key in _JOIN_KEYS:
        need(join[key] == job[key], 'boundary differs from independently expected ' + key)
    for key in _BODY_KEYS:
        need(_body(join[key]) == _body(job[key]), 'boundary body differs from expected ' + key)
    need(type(join.get('ci')) is dict and type(job.get('ci')) is dict, 'CI identity required')
    for key in _CI_JOIN_KEYS:
        need(key in join['ci'] and key in job['ci'] and join['ci'][key] == job['ci'][key],
             'boundary differs from independently expected CI ' + key)
    required = join.get('required_recipes')
    need(type(required) is list and len(set(required)) == len(required), 'fixed required job recipes required')
    recipe_id = job['job_recipe_id']
    need(type(recipe_id) is str and recipe_id in required and recipe_id in registry['job_recipes'],
         'boundary job or matrix is not independently required')
    recipe = registry['job_recipes'][recipe_id]
    need(job['ci']['job'] == recipe['github_job'] and job['matrix'] == recipe['matrix']
         and job['host'] == recipe['host'], 'boundary fixed job/matrix/host mismatch')
    return recipe_id


def begin(context):
    source = _source()
    document = source.verify_job(context)
    _job_key(document)
    started = time.time_ns()
    record = {'schema': BOUNDARY_SCHEMA, 'kind': 'begin', 'authority': 'inert',
              'nonce': uuid.uuid4().hex, 'source': clone(document),
              'window': {'started_ns': started, 'finished_ns': time.time_ns()}}
    handle = object.__new__(JobBoundary)
    _BOUNDARIES[handle] = {'context': context, 'document': record, 'finished': False}
    return handle


def _begin_data(value):
    need(type(value) is dict and set(value) == {'schema', 'kind', 'authority', 'nonce', 'source', 'window'},
         'closed inert begin boundary required')
    need(value['schema'] == BOUNDARY_SCHEMA and value['kind'] == 'begin' and value['authority'] == 'inert',
         'begin boundary schema or authority')
    need(type(value['nonce']) is str and re.fullmatch('[0-9a-f]{32}', value['nonce']), 'begin boundary nonce')
    _job_key(value['source']); _window(value['window'])
    return clone(value)


def _status(value):
    # The caller may preserve failure observations, but none are provider proof.
    if value is None:
        return None
    need(type(value) is dict, 'captured status is inert object data only')
    raw = canonical(value)
    need(len(raw) <= 64 * 1024, 'bounded inert status')
    return decode(raw)


def _terminal(before, after, status, started, error=None):
    first = _begin_data(before)
    need(_window(first['window'])[1] <= started, 'terminal predates begin boundary')
    if error is None:
        _same_job(first['source'], after)
    return {'schema': BOUNDARY_SCHEMA, 'kind': 'terminal', 'authority': 'inert',
            'begin_sha256': hashlib.sha256(canonical(first)).hexdigest(),
            'begin_nonce': first['nonce'], 'source': clone(after) if after is not None else None,
            'source_valid': error is None, 'source_error': error,
            'captured_status': _status(status),
            'qualification': 'unqualified-boundary-only',
            'window': {'started_ns': started, 'finished_ns': time.time_ns()}}


def finish(boundary, captured_status=None):
    source = _source()
    need(type(boundary) is JobBoundary and boundary in _BOUNDARIES, 'original in-process JobBoundary required')
    owned = _BOUNDARIES[boundary]
    need(not owned['finished'], 'job boundary already finished')
    status = _status(captured_status)
    owned['finished'] = True
    started = time.time_ns()
    after = None
    try:
        after = source.verify_job(owned['context'])
        _same_job(owned['document']['source'], after)
    except (ValueError, OSError) as error:
        return _terminal(owned['document'], after, status, started,
                         'source readmission failed: ' + str(error))
    return _terminal(owned['document'], after, status, started)


def capture_terminal(fresh_job_context, inert_begin_data, captured_status=None):
    source = _source()
    before = _begin_data(inert_begin_data)
    started = time.time_ns()
    try:
        after = source.verify_job(fresh_job_context)
    except (ValueError, OSError) as error:
        return _terminal(before, None, captured_status, started, 'fresh source readmission failed: ' + str(error))
    _same_job(before['source'], after)
    need(_window(before['window'])[1] <= _ns(after['capture_started_ns'])
         <= _ns(after['capture_finished_ns']) <= started,
         'later-step terminal requires a newly completed source capture after begin')
    # No JobBoundary or original owner is restored from the transported data.
    return _terminal(before, after, captured_status, started)


def _end_data(value):
    keys = {'schema', 'kind', 'authority', 'begin_sha256', 'begin_nonce', 'source',
            'source_valid', 'source_error', 'captured_status', 'qualification', 'window'}
    need(type(value) is dict and set(value) == keys, 'closed inert terminal boundary required')
    need(value['schema'] == BOUNDARY_SCHEMA and value['kind'] == 'terminal' and value['authority'] == 'inert'
         and value['qualification'] == 'unqualified-boundary-only', 'terminal boundary schema or authority')
    need(type(value['source_valid']) is bool, 'terminal validity must be boolean')
    need(type(value['begin_sha256']) is str and re.fullmatch('[0-9a-f]{64}', value['begin_sha256']), 'begin digest')
    need(type(value['begin_nonce']) is str and re.fullmatch('[0-9a-f]{32}', value['begin_nonce']), 'begin nonce')
    need(value['source_error'] is None if value['source_valid'] else type(value['source_error']) is str,
         'terminal failure preservation')
    _window(value['window']); _status(value['captured_status'])
    if value['source_valid']:
        _job_key(value['source'])
    return clone(value)


def read_boundary(path, expected_join):
    source = _source()
    join = source.verify_join(expected_join)
    value = decode(source.read(Path(path), cap=MAX_DOCUMENT))
    kind = value.get('kind') if type(value) is dict else None
    value = _begin_data(value) if kind == 'begin' else _end_data(value)
    need(value.get('source_valid', True), 'failed terminal source boundary')
    _join_job(join, value['source'], source.registry(expected_join))
    need(source.verify_join(expected_join) == join, 'expected join changed during boundary read')
    return value


def _expected_conclusion(step):
    need(type(step.get('mandatory')) is bool, 'fixed provider step obligation')
    expected = step.get('expected_conclusion')
    need(expected in ('success', 'skipped') and (not step['mandatory'] or expected == 'success'),
         'mandatory provider steps require success; optional conclusion must be fixed')
    return expected


def _conditional_conclusion(recipe, step):
    # No expression evaluator or caller-selected optionality. Only the two
    # retained opposite-platform workflow branches have a current skip policy.
    original_number = step.get('original_step')
    need(type(original_number) is int and original_number > 0, 'conditional provider step needs fixed original step')
    originals = [original for original in recipe['steps'] if original.get('number') == original_number]
    need(len(originals) == 1 and originals[0].get('name') == step['name'], 'conditional original step association')
    condition = originals[0].get('condition')
    need(condition in ("runner.os == 'Windows'", "runner.os != 'Windows'"),
         'conditional skip lacks reviewed opposite-platform condition')
    host = recipe.get('host')
    matrices = {
        'windows-x86_64': {'artifact': 'windows-x86_64', 'name': 'Windows x86_64', 'os': 'windows-latest'},
        'macos-x86_64': {'artifact': 'macos-x86_64', 'name': 'macOS x86_64', 'os': 'macos-15-intel'},
        'macos-aarch64': {'artifact': 'macos-arm64', 'name': 'macOS arm64', 'os': 'macos-14'},
    }
    need(host in matrices and recipe.get('id') == 'unit4-portable-hosts:' + host
         and recipe.get('github_job') == 'unit4-portable-hosts' and recipe.get('matrix') == matrices[host],
         'conditional skip requires the exact reviewed portable-host matrix')
    applies = (host == 'windows-x86_64') == (condition == "runner.os == 'Windows'")
    return 'success' if applies else 'skipped'


def _activation(join, registry):
    activation = registry.get('activation')
    need(type(activation) is dict and activation.get('approved') is True,
         'operational CI qualification awaits exact workflow/topology review')
    need(activation.get('workflow_sha256') == join['workflow_identity']['sha256'], 'activation workflow body mismatch')
    required = join.get('required_recipes')
    need(type(required) is list and required and len(required) == len(set(required)), 'closed required job recipe set')
    mappings = activation.get('provider_jobs')
    need(type(mappings) is dict and set(mappings) == set(required), 'complete reviewed provider topology required')
    recipes, names = {}, []
    for recipe_id in required:
        recipe = registry['job_recipes'][recipe_id]
        mapping = mappings[recipe_id]
        need(recipe.get('operational') is True, 'job recipe is operationally blocked')
        need(recipe.get('job_continue_on_error') is False, 'job continue-on-error lacks authenticated raw outcome')
        need(all(step.get('continue_on_error') is False for step in recipe['steps']),
             'job recipe continue-on-error lacks authenticated raw outcome')
        need(type(mapping) is dict and mapping.get('display_name') == recipe['display_name'],
             'reviewed exact provider display/matrix name required')
        name = mapping['display_name']
        need(type(name) is str and name, 'provider display name required')
        names.append(name)
        steps = mapping.get('steps')
        need(type(steps) is list and steps, 'reviewed complete ordered provider steps required')
        need(all(type(step) is dict for step in steps), 'provider step object required')
        numbers = [step.get('number') for step in steps]
        need(all(type(number) is int and number > 0 for number in numbers)
             and numbers == sorted(set(numbers)), 'exact strictly increasing provider step numbers')
        conditional_originals = []
        for step in steps:
            need(type(step.get('name')) is str and step['name'] and type(step.get('mandatory')) is bool,
                 'provider step name and obligation')
            need(step.get('continue_on_error') is False, 'step continue-on-error lacks authenticated raw outcome')
            expected = _expected_conclusion(step)
            if not step['mandatory']:
                need(expected == _conditional_conclusion(recipe, step), 'provider conclusion differs from fixed condition/matrix policy')
                conditional_originals.append(step['original_step'])
        need(len(conditional_originals) == len(set(conditional_originals)), 'ambiguous conditional original-step mapping')
        need(type(mapping.get('begin_step')) is int and type(mapping.get('terminal_step')) is int
             and mapping['begin_step'] in numbers and mapping['terminal_step'] in numbers
             and mapping['begin_step'] < mapping['terminal_step'], 'reviewed boundary step positions')
        boundary_steps = {step['number']: step for step in steps}
        need(all(boundary_steps[mapping[name]]['mandatory'] is True for name in ('begin_step', 'terminal_step')),
             'begin and terminal provider steps must be mandatory successes')
        recipes[recipe_id] = mapping
    need(len(names) == len(set(names)), 'provider names must map uniquely to job/matrix recipes')
    return recipes


def _endpoint(join):
    ci = join['ci']
    repository = ci.get('repository')
    need(repository == 'YanagiKH/Oxid', 'fixed GitHub repository required')
    run_id, attempt = ci.get('run_id'), ci.get('run_attempt')
    need(type(run_id) is str and re.fullmatch('[1-9][0-9]*', run_id), 'numeric run ID')
    need(type(attempt) is str and re.fullmatch('[1-9][0-9]*', attempt), 'numeric run attempt')
    return '/repos/YanagiKH/Oxid/actions/runs/' + run_id + '/attempts/' + attempt + '/jobs'


def _page_url(endpoint, page):
    return 'https://' + _HOST + endpoint + '?per_page=100&page=' + str(page)


def _links(header, endpoint, page):
    if not header:
        return False
    need(type(header) is str and len(header) < 8192, 'bounded pagination links')
    relations = {}
    for item in header.split(','):
        match = re.fullmatch(r'\s*<([^>]+)>;\s*rel="(next|prev|first|last)"\s*', item)
        need(match is not None, 'unsupported pagination link')
        url, relation = match.groups()
        need(relation not in relations, 'duplicate pagination relation')
        parsed = urllib.parse.urlsplit(url)
        need(parsed.scheme == 'https' and parsed.netloc == _HOST and parsed.path == endpoint
             and not parsed.fragment, 'foreign pagination endpoint')
        query = urllib.parse.parse_qs(parsed.query, keep_blank_values=True, strict_parsing=True)
        need(set(query) == {'per_page', 'page'} and query['per_page'] == ['100']
             and len(query['page']) == 1 and re.fullmatch('[1-9][0-9]*', query['page'][0]), 'pagination query changed')
        target = int(query['page'][0])
        need(target <= MAX_PAGES, 'pagination exceeds page bound')
        relations[relation] = target
    if 'next' in relations:
        need(relations['next'] == page + 1, 'pagination skipped or repeated a page')
    if 'prev' in relations:
        need(relations['prev'] == page - 1, 'pagination previous mismatch')
    if 'first' in relations:
        need(relations['first'] == 1, 'pagination first mismatch')
    if 'last' in relations:
        need(relations['last'] >= relations.get('next', page) and ('next' in relations or relations['last'] == page), 'pagination incomplete')
    return 'next' in relations


def _http_page(endpoint, page, deadline):
    remaining = deadline - time.monotonic()
    need(remaining > 0, 'provider capture deadline')
    # HTTPSConnection ignores proxy/auth/netrc environment, and never follows
    # redirects. No user credentials or tokens are read, stored, or transmitted.
    connection = http.client.HTTPSConnection(_HOST, timeout=min(HTTP_TIMEOUT, remaining))
    def expire():
        current = connection.sock
        if current is not None:
            try:
                current.shutdown(socket.SHUT_RDWR)
            except OSError:
                pass
        connection.close()
    watchdog = threading.Timer(remaining, expire)
    watchdog.daemon = True
    watchdog.start()
    try:
        path = endpoint + '?per_page=100&page=' + str(page)
        connection.request('GET', path, headers={'Accept': 'application/vnd.github+json',
                           'X-GitHub-Api-Version': '2022-11-28',
                           'User-Agent': 'oxid-package-ci-source-admission-v1'})
        response = connection.getresponse()
        need(response.status == 200, 'public GitHub run-attempt jobs read failed or redirected')
        need(response.getheader('Content-Encoding', 'identity') == 'identity', 'encoded provider body refused')
        length = response.getheader('Content-Length')
        need(length is None or re.fullmatch('[0-9]+', length) and int(length) <= MAX_PAGE, 'provider page byte bound')
        parts, size = [], 0
        while True:
            need(time.monotonic() < deadline, 'provider capture deadline')
            if connection.sock is not None:
                connection.sock.settimeout(min(HTTP_TIMEOUT, max(0.001, deadline - time.monotonic())))
            block = response.read1(min(64 * 1024, MAX_PAGE + 1 - size))
            if not block:
                break
            parts.append(block); size += len(block)
            need(size <= MAX_PAGE, 'provider page byte bound')
        need(length is None or size == int(length), 'truncated provider body')
        return b''.join(parts), response.getheader('Link')
    finally:
        watchdog.cancel()
        connection.close()


def _provider_identity(job, join):
    need(type(job) is dict, 'provider job object')
    need(type(job.get('id')) is int and job['id'] > 0, 'numeric provider job ID')
    need(type(job.get('name')) is str and job['name'], 'provider job display name required')
    need(type(job.get('run_id')) is int and str(job['run_id']) == join['ci']['run_id'], 'provider run mismatch')
    # Documented job objects may omit run_attempt. Attempt authority is the
    # actual fixed /runs/{id}/attempts/{attempt}/jobs request, never this body.
    if 'run_attempt' in job:
        need(type(job['run_attempt']) is int and str(job['run_attempt']) == join['ci']['run_attempt'], 'provider attempt mismatch')
    need(job.get('head_sha') == join['checkout_head'], 'provider checkout head mismatch')
    need(job.get('workflow_name') == join['ci']['workflow'], 'provider workflow mismatch')


def _provider_job(job, recipe, join):
    _provider_identity(job, join)
    need(job['name'] == recipe['display_name'], 'provider display/matrix name mismatch')
    need(job.get('status') == 'completed' and job.get('conclusion') == 'success', 'provider job did not complete successfully')
    start, end = _timestamp(job.get('started_at')), _timestamp(job.get('completed_at'))
    need(start <= end, 'provider job time order')
    steps = job.get('steps')
    need(type(steps) is list and len(steps) == len(recipe['steps']), 'missing or unknown provider steps')
    previous = start
    for actual, expected in zip(steps, recipe['steps']):
        need(type(actual) is dict and type(actual.get('number')) is int
             and actual['number'] == expected['number'] and actual.get('name') == expected['name'],
             'provider ordered step recipe mismatch')
        expected_conclusion = _expected_conclusion(expected)
        need(actual.get('status') == 'completed' and actual.get('conclusion') == expected_conclusion,
             'provider step did not match its fixed reviewed conclusion')
        if expected_conclusion == 'skipped':
            need('started_at' in actual and 'completed_at' in actual
                 and actual['started_at'] is None and actual['completed_at'] is None,
                 'skipped provider steps require explicit null timestamps')
            continue  # A nonapplicable branch creates no execution window.
        first, last = _timestamp(actual.get('started_at')), _timestamp(actual.get('completed_at'))
        need(previous <= first <= last <= end, 'provider step time order')
        previous = last
    return clone(job)


def capture_ci_outcomes(expected_join):
    source = _source()
    join = source.verify_join(expected_join)
    source.require_operational(expected_join)
    recipes = _activation(join, source.registry(expected_join))
    endpoint = _endpoint(join)
    deadline = time.monotonic() + HTTP_SECONDS
    started = time.time_ns()
    raw_pages, jobs, total, size = [], [], None, 0
    for page in range(1, MAX_PAGES + 1):
        raw, link = _http_page(endpoint, page, deadline)
        size += len(raw)
        need(size <= MAX_RESPONSE, 'complete provider response byte bound')
        value = decode(raw)
        need(type(value) is dict and type(value.get('total_count')) is int
             and 0 < value['total_count'] <= MAX_JOBS and type(value.get('jobs')) is list, 'provider page schema')
        if total is None:
            total = value['total_count']
        need(total == value['total_count'], 'provider pagination total changed')
        need(0 < len(value['jobs']) <= 100, 'empty or oversized provider page')
        jobs.extend(value['jobs'])
        need(len(jobs) <= total, 'provider duplicate or excess page entries')
        raw_pages.append({'url': _page_url(endpoint, page), 'body': raw.decode('utf-8'),
                          'sha256': hashlib.sha256(raw).hexdigest(), 'link': link})
        has_next = _links(link, endpoint, page)
        if not has_next:
            need(len(jobs) == total, 'missing provider pagination pages')
            break
        need(len(jobs) < total and len(value['jobs']) == 100, 'inconsistent provider pagination')
    else:
        raise Rejected('provider pagination exhausted bound')
    need(time.monotonic() < deadline, 'provider capture deadline')
    identifiers = [job.get('id') for job in jobs if type(job) is dict]
    need(len(identifiers) == len(jobs) and all(type(item) is int and item > 0 for item in identifiers)
         and len(set(identifiers)) == len(jobs), 'duplicate or invalid provider numeric job IDs')
    for job in jobs:
        _provider_identity(job, join)
    admitted = {}
    for recipe_id, recipe in recipes.items():
        selected = [job for job in jobs if job.get('name') == recipe['display_name']]
        need(len(selected) == 1, 'missing or ambiguous reviewed provider display/matrix name')
        admitted[recipe_id] = _provider_job(selected[0], recipe, join)
    need(source.verify_join(expected_join) == join, 'expected source join drift during provider capture')
    document = {'schema': OUTCOMES_SCHEMA, 'source': clone(join), 'jobs': admitted,
                'pages': raw_pages, 'window': {'started_ns': started, 'finished_ns': time.time_ns()}}
    result = object.__new__(ProviderOutcomes)
    _OUTCOMES[result] = {'context': expected_join, 'document': document}
    return result


def _outcome_document(expected_join, outcomes):
    source = _source()
    need(type(outcomes) is ProviderOutcomes and outcomes in _OUTCOMES, 'original captured provider outcomes required')
    owned = _OUTCOMES[outcomes]
    need(owned['context'] is expected_join, 'provider outcomes belong to another independent join')
    join = source.verify_join(expected_join)
    need(join == owned['document']['source'], 'provider expected join drift')
    source.require_operational(expected_join)
    _activation(join, source.registry(expected_join))
    return join, clone(owned['document'])


def _boundary_pair(join, captured, registry, begin_bytes, terminal_bytes):
    need(type(begin_bytes) is bytes and type(terminal_bytes) is bytes,
         'original downloaded boundary bytes required')
    begin_data, end_data = decode(begin_bytes), decode(terminal_bytes)
    before, after = _begin_data(begin_data), _end_data(end_data)
    recipe_id = _join_job(join, before['source'], registry)
    need(after['source_valid'], 'terminal source readmission failed')
    _same_job(before['source'], after['source'])
    need(after['begin_nonce'] == before['nonce'] and after['begin_sha256'] == hashlib.sha256(canonical(before)).hexdigest(),
         'terminal is not bound to this exact begin boundary')
    bstart, bend = _window(before['window']); estart, eend = _window(after['window'])
    need(bend <= estart, 'boundary capture windows overlap or reverse')
    recipe = _activation(join, registry)[recipe_id]
    job = _provider_job(captured['jobs'][recipe_id], recipe, join)
    # API timestamps have second precision; boundaries must fit those measured
    # provider step windows, without using caller-supplied status as authority.
    numbered = {step['number']: step for step in job['steps']}
    begin_step, end_step = numbered[recipe['begin_step']], numbered[recipe['terminal_step']]
    need(_timestamp(begin_step['started_at']) <= bstart <= bend < _timestamp(begin_step['completed_at']) + 1_000_000_000,
         'begin capture outside independently reported boundary step')
    need(_timestamp(end_step['started_at']) <= estart <= eend < _timestamp(end_step['completed_at']) + 1_000_000_000,
         'terminal capture outside independently reported boundary step')
    need(_timestamp(job['completed_at']) <= captured['window']['finished_ns'], 'outcomes captured before job completion')
    return recipe_id, before, after, job


def _transport_activation(join, registry):
    providers = _activation(join, registry)
    transport = registry['activation'].get('boundary_transport')
    need(type(transport) is dict and set(transport) == {'reviewed', 'archive_hosts', 'recipes'}
         and transport['reviewed'] is True, 'boundary artifact transport awaits separate review')
    hosts = transport['archive_hosts']
    need(type(hosts) is list and hosts and len(hosts) <= 16
         and all(type(host) is str and len(host) <= 253 and re.fullmatch(
             r'(?:[a-z0-9](?:[a-z0-9-]{0,61}[a-z0-9])?\.)+[a-z]{2,63}', host) for host in hosts)
         and len(hosts) == len(set(hosts)), 'nonempty exact reviewed archive hosts required')
    recipes = transport['recipes']
    need(type(recipes) is dict and set(recipes) == set(providers), 'complete boundary transport recipes required')
    keys = []
    for recipe_id, provider in providers.items():
        entry = recipes[recipe_id]
        need(type(entry) is dict and set(entry) == {'artifact_key', 'begin_step', 'terminal_step', 'upload_step'},
             'closed boundary transport recipe')
        key = entry['artifact_key']
        need(type(recipe_id) is str and type(key) is str and key == recipe_id.replace(':', '--').replace('_', '-')
             and re.fullmatch(r'[a-z0-9][a-z0-9-]{0,99}', key), 'fixed lowercase ASCII artifact key required')
        keys.append(key)
        steps = {step['number']: step for step in provider['steps']}
        numbers = [entry[name] for name in ('begin_step', 'terminal_step', 'upload_step')]
        need(all(type(number) is int and number in steps for number in numbers)
             and numbers[0] == provider['begin_step'] and numbers[1] == provider['terminal_step']
             and numbers[0] < numbers[1] < numbers[2], 'reviewed generating/terminal/upload step order')
        need(all(steps[number]['mandatory'] is True for number in numbers), 'boundary transport steps are mandatory')
    need(len(keys) == len(set(keys)), 'unique boundary artifact keys required')
    return clone(transport)


def _artifact_endpoint(join):
    _endpoint(join)  # Reuse exact repository, numeric run and attempt admission.
    return '/repos/YanagiKH/Oxid/actions/runs/' + join['ci']['run_id'] + '/artifacts'


def _artifact_name(join, recipe):
    name = 'package-ci-boundary-' + join['ci']['run_id'] + '-' + join['ci']['run_attempt'] + '-' + recipe['artifact_key']
    need(len(name) <= 240, 'bounded attempt-specific artifact name')
    return name


def _archive_target(location, hosts):
    need(type(location) is str and 0 < len(location) <= 8192
         and all(33 <= ord(char) <= 126 for char in location), 'bounded ASCII artifact redirect URL')
    try:
        parsed = urllib.parse.urlsplit(location)
        port = parsed.port
    except ValueError as error:
        raise Rejected('invalid artifact redirect URL') from error
    need(parsed.scheme == 'https' and parsed.hostname in hosts and port in (None, 443)
         and parsed.username is None and parsed.password is None and not parsed.fragment
         and parsed.netloc in (parsed.hostname, parsed.hostname + ':443')
         and parsed.path.startswith('/'), 'artifact redirect outside exact HTTPS archive hosts')
    return parsed.hostname, parsed.path + ('?' + parsed.query if parsed.query else '')


def _http_archive_response(host, path, deadline, *, redirect):
    # Called only for the fixed API artifact ID path, then for its single
    # separately allowlisted HTTPS Location. No auth/netrc/proxy is consulted.
    remaining = deadline - time.monotonic()
    need(remaining > 0, 'artifact capture deadline')
    connection = http.client.HTTPSConnection(host, port=443, timeout=min(HTTP_TIMEOUT, remaining))
    def expire():
        current = connection.sock
        if current is not None:
            try:
                current.shutdown(socket.SHUT_RDWR)
            except OSError:
                pass
        connection.close()
    watchdog = threading.Timer(remaining, expire)
    watchdog.daemon = True
    watchdog.start()
    try:
        headers = {'User-Agent': 'oxid-package-ci-source-admission-v1'}
        if redirect:
            headers.update({'Accept': 'application/vnd.github+json', 'X-GitHub-Api-Version': '2022-11-28'})
        connection.request('GET', path, headers=headers)
        response = connection.getresponse()
        need(response.status == (302 if redirect else 200),
             'artifact API must issue one redirect; archive response must not redirect')
        need(response.getheader('Content-Encoding', 'identity') == 'identity', 'encoded archive response refused')
        cap = 8192 if redirect else MAX_ARCHIVE
        length = response.getheader('Content-Length')
        need(length is None or re.fullmatch('[0-9]+', length) and int(length) <= cap, 'artifact response byte bound')
        parts, size = [], 0
        while True:
            need(time.monotonic() < deadline, 'artifact capture deadline')
            if connection.sock is not None:
                connection.sock.settimeout(min(HTTP_TIMEOUT, max(0.001, deadline - time.monotonic())))
            block = response.read1(min(64 * 1024, cap + 1 - size))
            if not block:
                break
            parts.append(block); size += len(block)
            need(size <= cap, 'artifact response byte bound')
        need(length is None or size == int(length), 'truncated artifact response')
        return b''.join(parts), response.getheader('Location')
    finally:
        watchdog.cancel()
        connection.close()


def _http_artifact(artifact_id, hosts, deadline):
    need(type(artifact_id) is int and artifact_id > 0, 'numeric artifact download ID')
    need(hosts, 'archive routing remains held')
    endpoint = '/repos/YanagiKH/Oxid/actions/artifacts/' + str(artifact_id) + '/zip'
    _, location = _http_archive_response(_HOST, endpoint, deadline, redirect=True)
    host, path = _archive_target(location, hosts)
    raw, secondary = _http_archive_response(host, path, deadline, redirect=False)
    need(secondary is None, 'secondary archive redirect refused')
    return raw, {'api_url': 'https://' + _HOST + endpoint, 'api_status': 302,
                 'archive_host': host, 'archive_status': 200,
                 'location_sha256': hashlib.sha256(location.encode('ascii')).hexdigest()}


def _artifact_listing(join, deadline):
    endpoint = _artifact_endpoint(join)
    pages, artifacts, total, size = [], [], None, 0
    for page in range(1, MAX_PAGES + 1):
        raw, link = _http_page(endpoint, page, deadline)
        size += len(raw)
        need(size <= MAX_RESPONSE, 'complete artifact listing byte bound')
        value = decode(raw)
        need(type(value) is dict and type(value.get('total_count')) is int
             and 0 < value['total_count'] <= MAX_ARTIFACTS and type(value.get('artifacts')) is list,
             'artifact listing schema')
        if total is None:
            total = value['total_count']
        need(total == value['total_count'], 'artifact pagination total changed')
        need(0 < len(value['artifacts']) <= 100, 'empty or oversized artifact page')
        artifacts.extend(value['artifacts'])
        need(len(artifacts) <= total, 'artifact excess page entries')
        pages.append({'url': _page_url(endpoint, page), 'body': raw.decode('utf-8'),
                      'sha256': hashlib.sha256(raw).hexdigest(), 'link': link})
        if not _links(link, endpoint, page):
            need(len(artifacts) == total, 'missing artifact listing pages')
            break
        need(len(artifacts) < total and len(value['artifacts']) == 100, 'inconsistent artifact pagination')
    else:
        raise Rejected('artifact pagination exhausted bound')
    need(time.monotonic() < deadline, 'artifact listing deadline')
    identifiers, names = [], []
    for artifact in artifacts:
        need(type(artifact) is dict and type(artifact.get('id')) is int and artifact['id'] > 0
             and type(artifact.get('name')) is str and 0 < len(artifact['name']) <= 256,
             'numeric artifact ID and bounded name required')
        identifiers.append(artifact['id']); names.append(artifact['name'])
    need(len(set(identifiers)) == len(artifacts) and len(set(names)) == len(artifacts),
         'duplicate artifact IDs or names')
    return artifacts, pages


def _artifact_record(artifact, join, recipe, job, captured_ns):
    need(type(artifact) is dict and type(artifact.get('id')) is int and artifact['id'] > 0,
         'numeric provider artifact ID')
    need(artifact.get('name') == _artifact_name(join, recipe), 'artifact run/attempt/recipe name mismatch')
    need(artifact.get('expired') is False, 'expired artifact refused')
    need(type(artifact.get('size_in_bytes')) is int and 0 < artifact['size_in_bytes'] <= MAX_ARCHIVE,
         'provider archive size bound')
    need(type(artifact.get('digest')) is str and re.fullmatch(r'sha256:[0-9a-f]{64}', artifact['digest']),
         'provider SHA256 archive digest required')
    workflow = artifact.get('workflow_run')
    need(type(workflow) is dict and type(workflow.get('id')) is int
         and str(workflow['id']) == join['ci']['run_id'] and workflow.get('head_sha') == join['checkout_head'],
         'provider artifact workflow run/head mismatch')
    created, updated, expires = (_timestamp(artifact.get(key)) for key in ('created_at', 'updated_at', 'expires_at'))
    steps = {step['number']: step for step in job['steps']}
    upload = steps[recipe['upload_step']]
    first, last = _timestamp(upload['started_at']), _timestamp(upload['completed_at'])
    need(first <= created <= updated < last + 1_000_000_000 and updated <= captured_ns < expires,
         'artifact outside reviewed upload window or expired at capture')
    need(_timestamp(job['completed_at']) <= captured_ns, 'artifact listing predates completed provider job')
    return clone(artifact)


def _zip_layout(raw, archive, members):
    # In addition to zipfile's CRC/local-name checks, require a closed physical
    # archive: no preamble, hidden local entries, comments, ZIP64 or trailing data.
    need(len(raw) >= 22 and raw[-22:-18] == b'PK\x05\x06', 'closed ZIP end record required')
    _, disk, directory_disk, on_disk, count, size, offset, comment = struct.unpack('<4s4H2IH', raw[-22:])
    need((disk, directory_disk, on_disk, count, comment) == (0, 0, 2, 2, 0)
         and offset == archive.start_dir and offset + size == len(raw) - 22, 'closed single-volume two-member ZIP')
    cursor = 0
    for member in sorted(members, key=lambda value: value.header_offset):
        need(member.header_offset == cursor and cursor + 30 <= offset, 'extra or overlapping ZIP local entry')
        header = struct.unpack('<4s5H3I2H', raw[cursor:cursor + 30])
        signature, version, flags, method, _, _, crc, compressed, uncompressed, name_size, extra_size = header
        need(signature == b'PK\x03\x04' and version <= 20 and flags == member.flag_bits
             and method == member.compress_type and extra_size == 0, 'unsupported ZIP local metadata')
        start = cursor + 30 + name_size
        need(raw[cursor + 30:start] == member.filename.encode('ascii'), 'ZIP local member name mismatch')
        cursor = start + member.compress_size
        need(cursor <= offset, 'ZIP member outside local region')
        if flags & 8:
            need((crc, compressed, uncompressed) in ((0, 0, 0), (member.CRC, member.compress_size, member.file_size)),
                 'ZIP streaming local sizes disagree')
            if raw[cursor:cursor + 4] == b'PK\x07\x08':
                cursor += 4
            need(cursor + 12 <= offset and struct.unpack('<3I', raw[cursor:cursor + 12])
                 == (member.CRC, member.compress_size, member.file_size), 'ZIP data descriptor mismatch')
            cursor += 12
        else:
            need((crc, compressed, uncompressed) == (member.CRC, member.compress_size, member.file_size),
                 'ZIP local sizes disagree')
    need(cursor == offset, 'unlisted ZIP local bytes')
    for member in members:
        need(cursor + 46 <= len(raw) - 22, 'truncated ZIP central entry')
        header = struct.unpack('<4s6H3I5H2I', raw[cursor:cursor + 46])
        need(header[0] == b'PK\x01\x02' and header[11] == header[12] == header[13] == 0,
             'ZIP central extras/comments/other-volume entry')
        name_size = header[10]
        need(raw[cursor + 46:cursor + 46 + name_size] == member.filename.encode('ascii'),
             'ZIP central member name mismatch')
        cursor += 46 + name_size
    need(cursor == len(raw) - 22, 'unlisted ZIP central bytes')


def _zip_members(raw, deadline):
    need(type(raw) is bytes and 0 < len(raw) <= MAX_ARCHIVE, 'bounded original ZIP bytes required')
    deadline = min(deadline, time.monotonic() + ARCHIVE_SECONDS)
    need(time.monotonic() < deadline, 'ZIP inspection deadline')
    try:
        # Bound central parsing before ZipFile allocates any member objects.
        need(len(raw) >= 22 and raw[-22:-18] == b'PK\x05\x06', 'closed ZIP end record required')
        _, disk, directory_disk, on_disk, count, size, offset, comment = struct.unpack('<4s4H2IH', raw[-22:])
        need((disk, directory_disk, on_disk, count, comment) == (0, 0, 2, 2, 0)
             and size == 2 * 46 + len('begin.json') + len('terminal.json')
             and offset + size == len(raw) - 22, 'bounded two-member ZIP central roster')
        with zipfile.ZipFile(io.BytesIO(raw), 'r') as archive:
            members = archive.infolist()
            need(len(members) == 2 and {member.filename for member in members} == {'begin.json', 'terminal.json'},
                 'exact two boundary ZIP members required')
            need(not archive.comment, 'ZIP comments refused')
            total = 0
            for member in members:
                need(member.orig_filename == member.filename and not member.is_dir() and not member.extra
                     and not member.comment and member.create_system in (0, 3)
                     and stat.S_IFMT(member.external_attr >> 16) in (0, stat.S_IFREG)
                     and not (member.external_attr & 0x58), 'regular boundary ZIP files only')
                need(member.flag_bits & ~0x808 == 0 and member.compress_type in (zipfile.ZIP_STORED, zipfile.ZIP_DEFLATED),
                     'encrypted or unsupported ZIP member')
                need(0 < member.file_size <= MAX_DOCUMENT and 0 < member.compress_size <= MAX_ARCHIVE
                     and member.file_size <= member.compress_size * MAX_COMPRESSION_RATIO, 'ZIP member size or ratio bound')
                total += member.file_size
            need(total <= MAX_UNCOMPRESSED, 'ZIP uncompressed total bound')
            _zip_layout(raw, archive, members)
            result = {}
            for member in members:
                chunks, size = [], 0
                with archive.open(member, 'r') as stream:
                    while True:
                        need(time.monotonic() < deadline, 'ZIP decompression deadline')
                        block = stream.read(min(64 * 1024, member.file_size + 1 - size))
                        if not block:
                            break
                        size += len(block)
                        need(size <= member.file_size, 'ZIP member exceeded declared size')
                        chunks.append(block)
                need(size == member.file_size, 'truncated ZIP member')
                result[member.filename] = b''.join(chunks)
            need(time.monotonic() < deadline, 'ZIP decompression deadline')
            return result
    except (zipfile.BadZipFile, NotImplementedError, RuntimeError, EOFError, struct.error,
            OSError, UnicodeError, zlib.error) as error:
        raise Rejected('invalid or unsupported boundary ZIP') from error


def _collect_boundary_artifacts(join, captured, registry, deadline, started_ns):
    # This private parser/collector returns inert data. Only the public fixed
    # producer below may issue an owner, after original-source readmission.
    transport = _transport_activation(join, registry)
    listed, pages = _artifact_listing(join, deadline)
    listing_ns = time.time_ns()
    records, originals, archives, total, total_documents = {}, {}, {}, 0, 0
    for recipe_id in join['required_recipes']:
        need(time.monotonic() < deadline, 'boundary artifact capture deadline')
        recipe = transport['recipes'][recipe_id]
        provider_recipe = registry['activation']['provider_jobs'][recipe_id]
        job = _provider_job(captured['jobs'][recipe_id], provider_recipe, join)
        selected = [item for item in listed if item['name'] == _artifact_name(join, recipe)]
        need(len(selected) == 1, 'missing or ambiguous same-attempt boundary artifact')
        artifact = _artifact_record(selected[0], join, recipe, job, listing_ns)
        raw, route = _http_artifact(artifact['id'], transport['archive_hosts'], deadline)
        total += len(raw)
        need(total <= MAX_ARCHIVES, 'complete boundary archive byte bound')
        digest = hashlib.sha256(raw).hexdigest()
        need(len(raw) == artifact['size_in_bytes'] and 'sha256:' + digest == artifact['digest'],
             'downloaded ZIP differs from provider size/digest')
        members = _zip_members(raw, deadline)
        total_documents += sum(len(body) for body in members.values())
        need(total_documents <= MAX_UNCOMPRESSED, 'complete boundary document byte bound')
        actual_recipe, before, after, _ = _boundary_pair(join, captured, registry, members['begin.json'], members['terminal.json'])
        need(actual_recipe == recipe_id, 'downloaded boundary belongs to another job/matrix')
        need(_window(after['window'])[1] < _timestamp(artifact['created_at']) + 1_000_000_000,
             'artifact creation precedes terminal capture')
        records[recipe_id] = {'provider_artifact': artifact, 'download': route,
                              'archive_sha256': digest, 'archive_bytes': len(raw), 'job_id': job['id'],
                              'steps': {kind: clone(next(step for step in job['steps'] if step['number'] == recipe[kind + '_step']))
                                        for kind in ('begin', 'terminal', 'upload')},
                              'members': {name: {'body': body.decode('utf-8'), 'bytes': len(body),
                                                 'sha256': hashlib.sha256(body).hexdigest()} for name, body in members.items()}}
        originals[recipe_id], archives[recipe_id] = members, raw
    finished_ns = time.time_ns()
    need(time.monotonic() < deadline, 'boundary artifact capture deadline')
    need(all(_timestamp(row['provider_artifact']['expires_at']) > finished_ns for row in records.values()),
         'artifact expired during complete capture')
    document = {'schema': ARTIFACTS_SCHEMA, 'source': clone(join), 'artifacts': records, 'pages': pages,
                'provider_sha256': hashlib.sha256(canonical(captured)).hexdigest(),
                'window': {'started_ns': started_ns, 'finished_ns': finished_ns}}
    # Keep public detached evidence finite, including retained original pages.
    need(len(canonical(document)) <= MAX_DOCUMENT, 'complete boundary evidence document byte bound')
    return document, originals, archives


def capture_boundary_artifacts(expected_join, outcomes):
    source = _source()
    join, captured = _outcome_document(expected_join, outcomes)
    registry = source.registry(expected_join)
    _transport_activation(join, registry)  # Closed, dormant route gates all I/O.
    started_ns = time.time_ns()
    need(_window(captured['window'])[1] <= started_ns, 'artifact capture predates original provider capture')
    document, members, archives = _collect_boundary_artifacts(
        join, captured, registry, time.monotonic() + HTTP_SECONDS, started_ns)
    repeated_join, repeated_outcomes = _outcome_document(expected_join, outcomes)
    need(repeated_join == join and repeated_outcomes == captured and source.registry(expected_join) == registry,
         'original source/provider/route drift during boundary capture')
    result = object.__new__(BoundaryArtifacts)
    _BOUNDARY_ARTIFACTS[result] = {'context': expected_join, 'outcomes': outcomes, 'document': document,
                                 'members': members, 'archives': archives}
    return result


def qualify_job(expected_join, begin_bytes, terminal_bytes, outcomes, artifacts):
    source = _source()
    join, captured = _outcome_document(expected_join, outcomes)
    need(type(artifacts) is BoundaryArtifacts and artifacts in _BOUNDARY_ARTIFACTS,
         'original captured boundary artifacts required')
    owned = _BOUNDARY_ARTIFACTS[artifacts]
    need(owned['context'] is expected_join and owned['outcomes'] is outcomes,
         'boundary artifacts belong to another original join/provider capture')
    registry = source.registry(expected_join)
    _transport_activation(join, registry)
    artifact_document = owned['document']
    need(artifact_document['source'] == join
         and artifact_document['provider_sha256'] == hashlib.sha256(canonical(captured)).hexdigest(),
         'boundary artifact source/provider drift')
    recipe_id, before, after, job = _boundary_pair(join, captured, registry, begin_bytes, terminal_bytes)
    originals = owned['members'][recipe_id]
    need(begin_bytes == originals['begin.json'] and terminal_bytes == originals['terminal.json'],
         'qualification requires exact originally downloaded boundary bytes')
    record = artifact_document['artifacts'][recipe_id]
    need(hashlib.sha256(begin_bytes).hexdigest() == record['members']['begin.json']['sha256']
         and hashlib.sha256(terminal_bytes).hexdigest() == record['members']['terminal.json']['sha256']
         and hashlib.sha256(owned['archives'][recipe_id]).hexdigest() == record['archive_sha256'],
         'retained original boundary/archive body drift')
    _artifact_record(record['provider_artifact'], join, registry['activation']['boundary_transport']['recipes'][recipe_id],
                     job, time.time_ns())
    repeated_join, repeated_outcomes = _outcome_document(expected_join, outcomes)
    need(repeated_join == join and repeated_outcomes == captured, 'qualification original owner drift')
    return {'schema': QUALIFIED_SCHEMA, 'authority': 'provider-bound-source-qualification', 'qualified': True,
            'status': 'Qualified', 'recipe_id': recipe_id, 'job_id': job['id'], 'source': clone(before['source']),
            'begin_sha256': hashlib.sha256(begin_bytes).hexdigest(),
            'terminal_sha256': hashlib.sha256(terminal_bytes).hexdigest(),
            'artifact_id': record['provider_artifact']['id'], 'archive_sha256': record['archive_sha256'],
            'provider_sha256': hashlib.sha256(canonical(captured)).hexdigest(),
            'scope': 'provider-bound-source-boundaries-and-job-steps; no individual-command-supervision'}
