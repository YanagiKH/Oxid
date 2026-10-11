"""Dormant facade and retained-consumer inventory controls."""
import json,sys,types,unittest
from pathlib import Path
from unittest.mock import patch
sys.dont_write_bytecode=True
HERE=Path(__file__).resolve().parent

def module(name):
 p=HERE/(name+'.py');m=types.ModuleType('ci_test_'+name);m.__file__=str(p)
 exec(compile(p.read_bytes(),str(p),'exec'),m.__dict__);return m

class FacadeTests(unittest.TestCase):
 def test_all_original_consumers_classified(self):
  d=json.loads((HERE/'consumer-dispositions.json').read_text());rows=d['entries']
  self.assertEqual(len(rows),173);self.assertEqual(len({r['id'] for r in rows}),173)
  self.assertTrue(all(r['original']['id']==r['id'] for r in rows))
  self.assertEqual(d['unresolved_mandatory_ids'],[r['id'] for r in rows if r['mandatory'] and r['disposition']=='blocked'])
  self.assertFalse(d['activation_allowed'])
  self.assertTrue(all(r['execution_status']=='not-run-by-this-source-only-CI-candidate' for r in rows))
 def test_unsealed_helper_rejects(self):
  m=module('facade');m.COMMON_SHA256=None
  with self.assertRaisesRegex(ValueError,'unsealed'):m.common_api()
 def test_unknown_route_no_fallback(self):
  m=module('facade');q=types.SimpleNamespace(load_sibling=lambda _:self.fail('unexpected dispatch'))
  with patch.object(m,'common_api',return_value=q),patch.object(m,'dispositions',return_value={}):
   with self.assertRaisesRegex(ValueError,'no historical'):m.dispatch('old-ci',{})
 def test_host_route_is_exact(self):
  m=module('facade');seen=[];request={'x':1}
  q=types.SimpleNamespace(load_sibling=lambda name:types.SimpleNamespace(produce_host=lambda r:seen.append((name,r)) or 'result'))
  with patch.object(m,'common_api',return_value=q),patch.object(m,'dispositions',return_value={}):self.assertEqual(m.dispatch('ci-host',request),'result')
  self.assertEqual(seen,[('gate',request)])
 def test_join_constructs_own_expected_context(self):
  m=module('facade');seen=[];ctx=object();req={'repository':'r','expected_head':'h','event_sha':'e','capsules':['c']}
  def need(ok,message):
   if not ok:raise ValueError(message)
  q=types.SimpleNamespace(need=need,expected_join=lambda *a,**kw:seen.append((a,kw)) or ctx,load_sibling=lambda name:types.SimpleNamespace(join=lambda c,p:seen.append((name,c,p)) or 'joined'))
  with patch.object(m,'common_api',return_value=q),patch.object(m,'dispositions',return_value={}):
   self.assertEqual(m.dispatch('ci-join',req),'joined')
   with self.assertRaises(ValueError):m.dispatch('ci-join',dict(req,expected_context={}))
  self.assertEqual(seen,[(('r',),{'expected_head':'h','event_sha':'e'}),('join',ctx,['c'])])
 def test_fixed_inputs_have_closed_unique_paths(self):
  d=json.loads((HERE/'inputs.json').read_text());names=[r['path'] for r in d['files']]
  self.assertEqual(names,sorted(set(names)));self.assertEqual(len(names),616)
  self.assertEqual(len(d['closed_roots']),25)
  self.assertFalse(any('package_dependency_unit4_ci_v1/' in n for n in names))
 def test_old_workflow_not_in_candidate(self):
  self.assertFalse((HERE/'.github').exists())



# Gate/staging writer supplied exact source-only route controls.
import ast,copy,hashlib
def _definitions(path, name):
    """Load only the dormant candidate's definitions, without registering owners."""
    module = types.ModuleType(name)
    module.__file__ = str(path)
    raw = path.read_bytes()
    syntax = ast.parse(raw, filename=str(path))
    exec(compile(syntax, str(path), 'exec'), module.__dict__)
    return module


def focused_controls(package=HERE):
    package = Path(package).resolve()
    gate = _definitions(package / 'gate.py', '_pure_gate_controls')
    stage = _definitions(package / 'staging.py', '_pure_staging_controls')
    passed = []

    def check(name, predicate):
        if not predicate:
            raise AssertionError(name)
        passed.append(name)

    def rejects(name, action):
        try:
            action()
        except (ValueError, TypeError):
            passed.append(name)
        else:
            raise AssertionError('negative control accepted: ' + name)

    # Test the explicit unsealed-loader branch even after the integrator seals
    # the candidate. This changes only an unregistered test module's globals.
    literal = gate.COMMON_SHA256
    gate.COMMON_SHA256 = None
    try:
        rejects('gate_rejects_unsealed_common', gate._common)
    finally:
        gate.COMMON_SHA256 = literal
    rejects('staging_rejects_unissued_sibling_owner', stage._common)
    rejects('staging_constructor_rejects_direct_issue', stage.StagedRuntime)
    rejects('staging_rejects_dictionary_capability', lambda: stage._entry({}))
    rejects('staging_rejects_unregistered_allocated_object',
            lambda: stage._entry(object.__new__(stage.StagedRuntime)))

    dependencies = {'dpkg_query': {'selected': '/usr/bin/dpkg-query'}}
    before, after = stage._queries(dependencies), stage._queries(dependencies, after=True)
    queries = before + after
    check('fourteen_unique_dpkg_queries',
          len(queries) == 14 and len({name for name, _ in queries}) == 14)
    check('queries_use_exact_installed_tool_and_closed_verbs',
          all(argv[0] == '/usr/bin/dpkg-query' and argv[1] in ('--show', '--listfiles', '--search')
              for _, argv in queries))

    operations = ([{'name': name} for name, _ in before] +
                  [{'name': 'stage-installed-llvm'}] + [{'name': name} for name, _ in after])
    files = {'runtime/llvm-runtime-stage.json', *('runtime/libraries/' + name for name in stage.FILES)}
    directories = {'commands', 'home', 'tmp', 'runtime', 'runtime/libraries'}
    for operation in operations:
        prefix = 'commands/' + operation['name']
        directories.add(prefix)
        files.update(prefix + '/' + name for name in ('stdout', 'stderr', 'process.json'))
    manifest = ([{'path': name, 'kind': 'file'} for name in sorted(files)] +
                [{'path': name, 'kind': 'directory'} for name in sorted(directories)] +
                [{'path': 'runtime/libraries/' + stage.ALIAS, 'kind': 'symlink'}])
    stage._closed_output(manifest, operations)
    passed.append('exact_success_output_catalogue_accepts_pure_fixture')
    rejects('output_catalogue_rejects_extra_member', lambda: stage._closed_output(
        manifest + [{'path': 'home/unregistered', 'kind': 'file'}], operations))
    rejects('output_catalogue_rejects_missing_member', lambda: stage._closed_output(manifest[1:], operations))
    rejects('output_catalogue_rejects_duplicate_member', lambda: stage._closed_output(manifest + manifest[:1], operations))

    environment = stage._environment(Path('/tmp/pure-stage-fixture'))
    check('staging_environment_is_closed', set(environment) == {
        'PATH', 'HOME', 'TMPDIR', 'LANG', 'LC_ALL', 'PYTHONDONTWRITEBYTECODE',
        'PYTHONNOUSERSITE', 'PYTHONOPTIMIZE', 'DPKG_COLORS'})
    check('staging_path_cannot_inherit_ambient_tools', environment['PATH'] == '/usr/bin')

    helper = {'path': '/pure-fixture/stager.py', 'bytes': 1, 'sha256': '0' * 64}
    entries = [{'path': name, 'kind': 'file', 'soname': name, **row} for name, row in stage.FILES.items()]
    entries.append({'path': stage.ALIAS, 'kind': 'symlink', 'link': stage.ALIAS_TARGET, 'package': 'libllvm19'})
    content = {'package_version': stage.PACKAGE_VERSION, 'architecture': 'amd64',
               'entries': sorted(entries, key=lambda row: row['path'])}
    receipt = {
        'schema': 'oxid-unit3-portable-v1-llvm-runtime-stage', 'status': 'verified',
        'assertion_mode': {'__debug__': True, 'optimize': 0, 'PYTHONOPTIMIZE': '0', 'python': sys.version},
        'helper': helper, 'source_directory': str(stage.SOURCE_ROOT),
        'library_directory': '/stage/runtime/libraries', 'packages': stage._expected_packages(),
        'unique_target_bytes': sum(row['bytes'] for row in stage.FILES.values()),
        'dependency_scope': 'Pinned LLVM override libraries only; ordinary host loader/libc, C++ and other system dependencies remain required.',
        'content': content,
        'content_sha256': hashlib.sha256(json.dumps(content, sort_keys=True, separators=(',', ':')).encode()).hexdigest(),
    }
    stage._receipt(None, receipt, {'stager': helper}, Path('/stage'))
    passed.append('pure_original_receipt_semantics_accept_baseline')
    for field, value, label in (
        ('status', 'restored', 'status'),
        ('schema', 'oxid-restored-llvm-runtime-v1', 'restored_schema'),
        ('library_directory', '/old/libraries', 'old_directory'),
        ('unique_target_bytes', 0, 'byte_count'),
        ('packages', [], 'package_ownership'),
    ):
        altered = copy.deepcopy(receipt)
        altered[field] = value
        rejects('original_receipt_rejects_' + label,
                lambda altered=altered: stage._receipt(None, altered, {'stager': helper}, Path('/stage')))
    check('pure_controls_issue_no_staging_capability', not stage._ISSUED and not stage._USED_CONTEXTS)

    for seconds in (30, 180):
        policy = stage._process_policy(seconds)
        check('exact_process_policy_' + str(seconds),
              policy['wall_seconds'] == seconds and policy['cleanup_grace_seconds'] == 10 and
              policy['linux_processes'] == 8 and policy['linux_rss_bytes'] == 2 * 1024 ** 3)
    for seconds in (0, 1, 600, 36000):
        rejects('process_policy_rejects_' + str(seconds), lambda seconds=seconds: stage._process_policy(seconds))
    if len(passed) != 26:
        raise AssertionError('focused control count changed: ' + str(len(passed)))
    return passed


def catalogue_controls(package=HERE):
    package = Path(package).resolve()
    tree = ast.parse((package / 'gate.py').read_bytes(), filename=str(package / 'gate.py'))
    static = {node.args[0].value for node in ast.walk(tree)
              if isinstance(node, ast.Call) and isinstance(node.func, ast.Name) and node.func.id == 'step'
              and node.args and isinstance(node.args[0], ast.Constant)}
    join = _definitions(package / 'join.py', '_pure_gate_catalogue_controls')
    dynamic = {'build-public-ordinary', 'build-public-lifecycle',
               'build-parser-observed-debug', 'build-parser-observed-release',
               'build-parser-control-debug', 'build-parser-control-release'}
    expected_counts = {'linux-public': 20, 'linux-parser': 17, 'macos-x86_64-public': 13,
                       'macos-arm64-public': 13, 'windows-x86_64-public': 13}
    passed = []
    if set(join.DOMAINS) != set(expected_counts):
        raise AssertionError('fixed five-domain catalogue changed')
    for domain in join.DOMAINS:
        expected = join.stage_catalogue(domain)
        if not (len(expected) == len(set(expected)) == expected_counts[domain] and set(expected) - dynamic <= static):
            raise AssertionError('gate catalogue mismatch: ' + domain)
        passed.append({'domain': domain, 'stages': len(expected)})
    return passed


def deadline_controls(package=HERE):
    """Pure cancellation guards; fake timers never start threads or send signals."""
    package = Path(package).resolve()
    gate = _definitions(package / 'gate.py', '_pure_gate_deadline_controls')
    passed = []
    owner = gate._Cancellation()
    owner.finalizing = True
    owner.preservation_expired = True
    owner(gate.signal.SIGINT, None)
    if owner.error is not None:
        raise AssertionError('expired reservation replaced original error outside guarded action')
    passed.append('expiry_outside_guarded_action_does_not_interrupt_bookkeeping')
    owner.preservation_active = True
    try:
        owner(gate.signal.SIGINT, None)
    except gate.PreservationDeadline:
        passed.append('expiry_interrupts_guarded_preservation_action')
    else:
        raise AssertionError('expired guarded action was not interrupted')

    owner = gate._Cancellation()
    original = gate.Cancelled('original producer cancellation')
    owner.error = original
    owner.finalizing = owner.preservation_active = True
    try:
        owner(gate.signal.SIGTERM, None)
    except gate.Cancelled as error:
        if error is not original or owner.error is not original:
            raise AssertionError('original cancellation was replaced')
        passed.append('guarded_late_signal_retains_first_cancellation')
    else:
        raise AssertionError('guarded signal was swallowed')
    owner.sealed = True
    owner(gate.signal.SIGTERM, None)
    if owner.error is not original:
        raise AssertionError('sealed cancellation changed terminal original error')
    passed.append('terminal_serialization_is_not_reinterrupted')

    owner = gate._Cancellation()
    timers, interrupts = [], []
    class FakeTimer:
        def __init__(self, seconds, callback):
            self.seconds, self.callback = seconds, callback
            self.daemon = self.started = self.cancelled = False
            timers.append(self)
        def start(self):
            self.started = True
        def cancel(self):
            self.cancelled = True
    # Only these unregistered module globals are replaced. No live registry,
    # thread, subprocess, signal delivery, or source file is touched.
    gate.threading = types.SimpleNamespace(Timer=FakeTimer)
    gate._thread = types.SimpleNamespace(interrupt_main=lambda: interrupts.append(True))
    owner.finish_work()
    until = owner.start_preservation(12)
    if not (len(timers) == 1 and timers[0].seconds == 12 and timers[0].daemon and
            timers[0].started and until > gate.time.monotonic()):
        raise AssertionError('reservation timer/deadline association differs')
    timers[0].callback()
    if not (owner.preservation_expired and interrupts == [True]):
        raise AssertionError('reservation expiry did not request guarded interruption')
    passed.append('trusted_reservation_timer_sets_deadline_and_expiry')
    owner.close_preservation()
    timers[0].callback()
    if not (owner.sealed and timers[0].cancelled and interrupts == [True]):
        raise AssertionError('closed reservation timer still interrupts')
    passed.append('closed_reservation_timer_cannot_interrupt_terminal_write')
    return passed


class GateStagingTests(unittest.TestCase):
 def test_fixed_gate_staging_controls(self):
  self.assertGreaterEqual(len(focused_controls(HERE)),26)
 def test_deadline_reservations_and_cancellation(self):
  self.assertEqual(len(deadline_controls(HERE)),6)
 def test_five_complete_stage_catalogues(self):
  self.assertEqual(len(catalogue_controls(HERE)),5)

if __name__=="__main__":unittest.main()
