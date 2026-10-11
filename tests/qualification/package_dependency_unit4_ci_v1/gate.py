"""Current-source CI domain producer; importing or planning has no effects.

This dormant sibling never calls the local-session producer or historical CI
admission. Only a committed, reviewed activation policy can start this route.
"""
import argparse
import _thread
import hashlib
import json
import os
from pathlib import Path
import re
import signal
import stat
import sys
import threading
import time
import types

COMMON_SHA256 = '017a0b034ac120b246d2f9b88600cc3515534fe97c3e033b73a9f363e7b76e3d'
COMMON_KEY = '_oxid_package_dependency_unit4_ci_common_v1'
HERE = Path(__file__).resolve().parent
_COMMON = None
REQUEST_KEYS = {'schema', 'repository', 'expected_head', 'event_sha', 'domain',
                'fresh_output', 'original_repo', 'historical_repo', 'public', 'parser'}
PUBLIC_KEYS = {'contract_root', 'path_map', 'amendment_root', 'llvm_root', 'existing_user'}


class Rejected(ValueError):
    pass


class Cancelled(BaseException):
    pass


class PreservationDeadline(TimeoutError):
    pass


def need(value, message):
    if not value:
        raise Rejected(message)


def _common():
    global _COMMON
    path = HERE / 'common.py'
    need(type(COMMON_SHA256) is str and re.fullmatch('[0-9a-f]{64}', COMMON_SHA256),
         'CI gate awaits reviewed common body seal')
    need(path.resolve(strict=True) == path and not any(p.is_symlink() for p in (path, *path.parents)),
         'canonical common owner path required')
    info = path.stat()
    need(stat.S_ISREG(info.st_mode) and info.st_size <= 2 * 1024 ** 2, 'bounded common body')
    raw = path.read_bytes()
    need(len(raw) == info.st_size and hashlib.sha256(raw).hexdigest() == COMMON_SHA256,
         'CI common body changed')
    module = globals().get('_COMMON_OWNER') or _COMMON
    if module is not None:
        need(sys.modules.get(COMMON_KEY) is module, 'common owner replacement')
        if globals().get('_COMMON_OWNER') is not None:
            module.owns_sibling(sys.modules.get(__name__))
    else:
        need(COMMON_KEY not in sys.modules, 'preoccupied common module key')
        module = types.ModuleType(COMMON_KEY)
        module.__file__ = str(path)
        module.__source_sha256__ = COMMON_SHA256
        sys.modules[COMMON_KEY] = module
        try:
            exec(compile(raw, str(path), 'exec'), module.__dict__)
        except BaseException:
            if sys.modules.get(COMMON_KEY) is module:
                del sys.modules[COMMON_KEY]
            raise
    need(type(module) is types.ModuleType and module.__file__ == str(path) and
         getattr(module, '__source_sha256__', None) == COMMON_SHA256,
         'foreign common module owner')
    module.owner()
    need(_COMMON is None or _COMMON is module, 'common owner replacement')
    _COMMON = module
    return module


def _path(value, *, exists=True, directory=False):
    need(type(value) is str and value and '\x00' not in value, 'explicit path required')
    path = Path(value)
    need(path.is_absolute() and path.resolve(strict=exists) == path and
         not any(p.is_symlink() for p in (path, *path.parents)), 'canonical path required')
    if exists:
        need(path.is_dir() if directory else path.exists(), 'missing required input')
    return path


def plan_request(request):
    """Validate the closed request. This never captures a machine or creates output."""
    common = _common()
    need(type(request) is dict and set(request) == REQUEST_KEYS and
         request['schema'] == 'oxid-package-unit4-ci-request-v1', 'closed CI request required')
    policy = common.host_policy(request['domain'])
    repository = _path(request['repository'], directory=True)
    for key in ('expected_head', 'event_sha'):
        need(type(request[key]) is str and re.fullmatch('[0-9a-f]{40}', request[key]),
             'explicit checkout/event commit identity')
    output = _path(request['fresh_output'], exists=False)
    need(not os.path.lexists(output) and output.parent.is_dir(), 'fresh outer CI root required')
    need(repository != output and repository not in output.parents and output not in repository.parents,
         'CI output must be external to checkout')
    if policy['public_required']:
        public = request['public']
        need(type(public) is dict and set(public) == PUBLIC_KEYS, 'closed public inputs required')
        for key in ('contract_root', 'amendment_root'):
            _path(public[key], directory=True)
        need(stat.S_ISREG(_path(public['path_map']).lstat().st_mode), 'regular public path map required')
        if policy['staging_required']:
            _path(public['llvm_root'], directory=True)
            need(type(public['existing_user']) is str and
                 re.fullmatch('[a-z_][a-z0-9_-]{0,31}', public['existing_user']),
                 'explicit existing nonroot account required')
        else:
            need(public['llvm_root'] is None and public['existing_user'] is None,
                 'portable public domain requires original no-native traps')
    else:
        need(request['public'] is None, 'public inputs forbidden in parser domain')
    if policy['parser_required']:
        need(type(request['parser']) is dict and set(request['parser']) == {'contract_root'},
             'closed parser inputs required')
        _path(request['parser']['contract_root'], directory=True)
        repos = [repository]
        for key in ('original_repo', 'historical_repo'):
            retained = _path(request[key], directory=True)
            git = retained / '.git'
            need(git.is_dir() and not git.is_symlink() and git.resolve() == git,
                 'genuine retained repository with physical .git required; worktrees are forbidden')
            repos.append(retained)
        need(len(set(repos)) == 3 and all(a not in b.parents for a in repos for b in repos if a != b),
             'three nonoverlapping genuine repositories required')
    else:
        need(request['parser'] is None and request['original_repo'] is None and
             request['historical_repo'] is None, 'parser repositories forbidden outside parser domain')
    return {'schema': 'oxid-package-unit4-ci-plan-v1', 'policy': policy,
            'repository': str(repository), 'outer_root': str(output),
            'producer_root': str(output / 'producer'), 'checkout_head': request['expected_head'],
            'event_sha': request['event_sha'], 'status': 'NotReady', 'execution_qualified': False,
            'ci_runtime_proof': False, 'order': 'capture-prepare-machine-all-builds-stage-runtime-readmit-seal-export'}


class _Cancellation:
    """One producer owns cancellation, including a reviewed cross-platform deadline."""
    def __init__(self):
        self.error = None
        self.finalizing = False
        self.sealed = False
        self.api = None
        self.invocation = None
        self.timer = None
        self.deadline = None
        self.preservation_timer = None
        self.preservation_active = False
        self.preservation_expired = False
        self.lock = threading.Lock()

    def poison(self):
        if self.api is not None and self.invocation is not None:
            # Preserve the first error even when the original producer is already poisoned.
            try:
                self.api['preparation'].cancel_invocation(self.invocation)
            except BaseException:
                pass

    def check(self):
        if self.error is not None:
            raise self.error
        if self.deadline is not None and time.monotonic() >= self.deadline:
            self.error = Cancelled('reviewed CI work deadline exhausted')
            self.poison()
            raise self.error

    def __call__(self, signum, frame):
        if self.sealed:
            return
        if self.preservation_expired:
            if self.preservation_active:
                raise PreservationDeadline('reviewed CI preservation reservation exhausted')
            return
        if self.error is None:
            self.error = Cancelled('CI domain cancelled by ' + signal.Signals(signum).name)
        self.poison()
        if not self.finalizing:
            self.finalizing = True
            raise self.error
        if self.preservation_active:
            raise self.error

    def start(self, seconds):
        self.deadline = time.monotonic() + seconds
        def expire():
            with self.lock:
                if self.finalizing or self.sealed:
                    return
                self.error = self.error or Cancelled('reviewed CI work deadline exhausted')
                _thread.interrupt_main()
        self.timer = threading.Timer(seconds, expire)
        self.timer.daemon = True
        self.timer.start()

    def finish_work(self):
        # Set this before acquiring the timer lock so a simultaneous expiry
        # cannot interrupt the first preservation bookkeeping operation.
        self.finalizing = True
        with self.lock:
            if self.timer is not None:
                self.timer.cancel()

    def start_preservation(self, seconds):
        until = time.monotonic() + seconds
        def expire():
            with self.lock:
                if self.sealed:
                    return
                self.preservation_expired = True
                _thread.interrupt_main()
        self.preservation_timer = threading.Timer(seconds, expire)
        self.preservation_timer.daemon = True
        self.preservation_timer.start()
        return until

    def close_preservation(self):
        # The remaining action is one bounded terminal journal write. All
        # substantive verification and archiving has already passed its cap.
        with self.lock:
            self.sealed = True
            self.preservation_active = False
            if self.preservation_timer is not None:
                self.preservation_timer.cancel()


def _error(error):
    return {'type': type(error).__name__, 'message': str(error)[:4096]}


def _write_result(common, output, report):
    path = output / 'ci-host-result.json'
    raw = common.canonical(report)
    need(len(raw) <= 8 * 1024 ** 2, 'bounded CI terminal journal required')
    # The terminal journal is outside the immutable shared producer namespace.
    common.write_new(path, report)
    return str(path)


def _preflight(request, policy):
    need(sys.dont_write_bytecode and not sys.flags.optimize and __debug__,
         'normal bytecode-disabled Python required')
    need(threading.current_thread() is threading.main_thread(), 'CI producer requires signal-owning main thread')
    need(Path(sys.executable).is_absolute() and Path(sys.executable).resolve() == Path(sys.executable),
         'launch the canonical interpreter directly')
    if policy['root_required']:
        need(sys.platform.startswith('linux') and hasattr(os, 'geteuid') and os.geteuid() == 0,
             'actual root Linux domain required')
    if policy['hosted_required']:
        import pwd
        import shutil
        account = pwd.getpwnam(request['public']['existing_user'])
        need(account.pw_uid > 0 and account.pw_gid > 0, 'existing nonroot account required')
        need(shutil.which('setpriv') is not None, 'existing setpriv required; no provisioning fallback')


def produce_host(request):
    """Produce one entire fixed domain, retaining live objects through its terminal seal."""
    common = _common()
    plan = plan_request(request)
    policy = plan['policy']
    _preflight(request, policy)
    ci_context = common.admit_ci(request['repository'], expected_head=request['expected_head'],
        event_sha=request['event_sha'], domain=request['domain'], fresh_output=request['fresh_output'])
    activation = common.require_activation(ci_context)
    need(activation['approved'] is True and type(activation['work_seconds']) is int and
         type(activation['failure_preservation_seconds']) is int and
         0 < activation['work_seconds'] and 0 < activation['failure_preservation_seconds'] and
         activation['work_seconds'] + activation['failure_preservation_seconds'] <=
         activation['job_limit_seconds'] <= 21600, 'reviewed whole-domain timing policy required')
    api = common.apis(ci_context)
    staging_api = common.load_sibling('staging')
    evidence = common.load_sibling('evidence')
    before = common.verify_ci(ci_context)
    output, root = Path(before['outer_root']), Path(before['producer_root'])
    need(output == Path(plan['outer_root']) and root == Path(plan['producer_root']) and
         not os.path.lexists(output), 'original fresh CI output association required')
    cancellation = _Cancellation()
    previous = {}
    report = {'schema': 'oxid-package-unit4-ci-host-result-v1', 'ci': before, 'plan': plan,
              'request_sha256': common.sha(common.canonical(request)), 'started_ns': time.time_ns(),
              'stages': [], 'status': 'NotReady', 'domain_complete': False,
              'global_host_qualification': False, 'failure': None, 'preservation': {}, 'capsule': None}
    source = invocation = machine = public_result = parser_handle = staged = None
    public_builds, parser_builds = {}, {}
    public_joined = False
    first_error = None
    created = False
    seal = None

    def step(name, action):
        cancellation.check()
        if invocation is not None:
            api['preparation'].check_deadline(invocation)
        row = {'name': name, 'started_ns': time.time_ns(), 'completed': False}
        report['stages'].append(row)
        try:
            result = action()
            cancellation.check()
            row.update(completed=True, completed_ns=time.time_ns())
            return result
        except BaseException as error:
            row.update(completed_ns=time.time_ns(), failure=_error(error))
            raise

    try:
        for sig in (signal.SIGINT, signal.SIGTERM):
            previous[sig] = signal.getsignal(sig)
            signal.signal(sig, cancellation)
        cancellation.start(activation['work_seconds'])
        output.mkdir(mode=0o755, exist_ok=False)
        created = True
        captured = step('capture-current-source', lambda: api['context'].capture_source(
            request['repository'], request['expected_head'], request['event_sha'], scope=policy['scope']))
        invocation = api['context'].begin_invocation(captured, root)
        common.bind_invocation(ci_context, invocation)
        cancellation.api, cancellation.invocation = api, invocation
        report['invocation'] = invocation.document()
        source = step('prepare-source', lambda: api['preparation'].produce(invocation, policy['scope'],
            original_repo=request['original_repo'], historical_repo=request['historical_repo']))
        report['source_context'] = source.document()
        machine = step('capture-machine', lambda: api['machine'].capture_machine(invocation))
        report['machine_sha256'] = common.sha(common.canonical(machine.document()))
        if policy['public_required']:
            for role in ('ordinary', 'lifecycle'):
                public_builds[role] = step('build-public-' + role, lambda role=role: api['build'].build(
                    source, role, root / 'targets' / ('public-' + role), root / 'public-build' / role, machine))
        if policy['parser_required']:
            parser = api['parser']
            authority = step('admit-parser-source', lambda: parser.authority(source))
            layout = step('prepare-parser-layout', lambda: parser.prepare(authority))
            for control in (False, True):
                for profile in ('debug', 'release'):
                    parser_builds[profile, control] = step(
                        'build-parser-' + ('control-' if control else 'observed-') + profile,
                        lambda profile=profile, control=control: parser.build(
                            layout['path'], profile, authority, machine, control=control))
            need(set(parser_builds) == {(p, c) for p in ('debug', 'release') for c in (False, True)},
                 'all four original parser builds required before runtime')
        if policy['staging_required']:
            staged = step('stage-installed-dpkg-runtime', lambda: staging_api.stage_runtime(
                ci_context, invocation, machine, output / 'staging'))
            step('verify-fresh-staging', lambda: staging_api.verify_staging(staged, ci_context, invocation))
        if policy['public_required']:
            public, runtime = request['public'], api['public_runtime']
            stage = staged.document() if staged is not None else None
            tools = step('capture-public-runtime-tools', lambda: api['machine'].capture_runtime_tools(
                machine, llvm_root=public['llvm_root'],
                library_root=stage['library_root'] if stage else None,
                runtime_receipt=stage['runtime_receipt']['path'] if stage else None))
            admission = step('admit-public-runtime', lambda: runtime.admit_runtime(
                public_builds['ordinary'], public_builds['lifecycle'], contract_root=public['contract_root'],
                path_map=public['path_map'], amendment_root=public['amendment_root'], runtime_tools=tools))
            public_result = step('collect-complete-public', lambda: runtime.collect(admission))
            step('verify-public-collection', lambda: runtime.verify_collection(public_result))
            need(public_result.document()['mandatory_hosted_join_required'] is policy['hosted_required'],
                 'actual root capability branch differs from fixed domain policy')
            if policy['hosted_required']:
                transport = step('prepare-existing-nonroot-transport', lambda: api['hosted'].prepare_transport(
                    admission, existing_user=public['existing_user']))
                shard = step('execute-twelve-hosted-replacements', lambda: api['hosted'].execute_transport(transport))
                public_result = step('join-twelve-hosted-replacements', lambda: runtime.join_hosted(public_result, shard))
                public_joined = True
                step('verify-public-hosted-join', lambda: runtime.verify_join(public_result))
            report['public'] = public_result.document()
        if policy['parser_required']:
            inputs = step('capture-parser-runtime-inputs', lambda: api['machine'].capture_parser_runtime(machine, invocation))
            parser_handle = step('admit-complete-parser-runtime', lambda: parser.create_runtime(
                authority, list(parser_builds.values()), request['parser']['contract_root'], inputs))
            step('complete-seven-parser-stages', lambda: parser.complete_runtime(parser_handle))
            report['parser'] = step('verify-complete-parser-runtime', lambda: parser.verify_runtime(parser_handle))
        if public_result is not None:
            step('terminal-public-readmission', lambda: api['public_runtime'].verify_join(public_result)
                 if public_joined else api['public_runtime'].verify_collection(public_result))
        if parser_handle is not None:
            step('terminal-parser-readmission', lambda: api['parser'].verify_runtime(parser_handle))
        if staged is not None:
            step('terminal-staging-readmission', lambda: staging_api.verify_staging(staged, ci_context, invocation))
        step('terminal-source-readmission', lambda: api['context'].verify_context(source))
        step('terminal-machine-readmission', lambda: api['machine'].verify_machine(machine, invocation))
        step('terminal-ci-readmission', lambda: common.verify_ci(ci_context))
        common.write_new(output / 'ci-host-stages.json', {
            'schema': 'oxid-package-unit4-ci-stages-v1', 'ci': ci_context.document(),
            'invocation': invocation.document(), 'stages': report['stages'],
            'started_ns': report['started_ns'], 'completed_ns': time.time_ns()})
        seal = step('seal-live-domain', lambda: evidence.seal_live_host(ci_context, source_context=source,
            machine=machine, public_result=public_result, parser_handle=parser_handle, staging=staged))
    except BaseException as error:
        first_error = error
        report['failure'] = _error(error)
        cancellation.poison()
    finally:
        cancellation.finish_work()
        try:
            preservation_until = cancellation.start_preservation(activation['failure_preservation_seconds'])
        except BaseException as error:
            # Failure to create the bounded guard cannot replace an earlier
            # producer error or silently permit unbounded preservation work.
            preservation_until = time.monotonic()
            report['preservation']['deadline_guard'] = {'status': 'failed', 'failure': _error(error)}
            if first_error is None:
                first_error = error
                report['failure'] = _error(error)

        def preservation_tick():
            if cancellation.preservation_expired or time.monotonic() >= preservation_until:
                raise PreservationDeadline('reviewed CI preservation reservation exhausted')

        def preserve_action(action):
            # A timer can interrupt opaque frozen verifiers; explicit checks
            # guard every boundary. Archive code receives this same absolute
            # deadline and also checks within every bounded read/write loop.
            cancellation.preservation_active = True
            try:
                preservation_tick()
                value = action()
                preservation_tick()
                return value
            finally:
                cancellation.preservation_active = False

        if cancellation.error is not None and first_error is None:
            first_error = cancellation.error
            report['failure'] = _error(first_error)
        checks = [('ci', lambda: common.verify_ci(ci_context))]
        if source is not None:
            checks.append(('source', lambda: api['context'].verify_context(source)))
        if machine is not None:
            checks.append(('machine', lambda: api['machine'].verify_machine(machine, invocation)))
        if staged is not None:
            checks.append(('staging', lambda: staging_api.verify_staging(staged, ci_context, invocation)))
        for name, check in checks:
            try:
                preserve_action(check)
                if cancellation.error is not None:
                    raise cancellation.error
                report['preservation'][name] = {'status': 'verified'}
            except BaseException as error:
                report['preservation'][name] = {'status': 'failed', 'failure': _error(error)}
                if first_error is None:
                    first_error = error
                    report['failure'] = _error(error)
        # Never export a success capsule after any failed preservation boundary.
        if first_error is None:
            try:
                need(seal is not None and time.monotonic() < preservation_until,
                     'sealed domain and remaining evidence reservation required')
                if cancellation.error is not None:
                    raise cancellation.error
                report['capsule'] = str(preserve_action(lambda: evidence.export_capsule(
                    seal, output / 'capsule', deadline=preservation_until)))
                if cancellation.error is not None:
                    raise cancellation.error
                report['domain_complete'] = True
            except BaseException as error:
                first_error = error
                report['failure'] = _error(error)
        # Close cancellation only after it has had a final chance to poison success.
        if cancellation.error is not None and first_error is None:
            first_error = cancellation.error
            report['failure'] = _error(first_error)
            report['domain_complete'] = False
        if first_error is None:
            try:
                preservation_tick()
            except BaseException as error:
                first_error = error
                report['failure'] = _error(error)
                report['domain_complete'] = False
        if first_error is not None:
            report['domain_complete'] = False
            if created:
                try:
                    retained = preserve_action(lambda: evidence.export_failure(
                        ci_context, output / 'failure-evidence',
                        original_error=_error(first_error), deadline=preservation_until))
                    need(type(retained) is dict and retained.get('status') in ('preserved', 'failed'),
                         'explicit bounded raw failure preservation outcome required')
                    report['preservation']['raw_evidence'] = retained
                except BaseException as preservation_error:
                    report['preservation']['raw_evidence'] = {
                        'status': 'failed', 'failure': _error(preservation_error),
                        'original_failure_retained': True}
            else:
                report['preservation']['raw_evidence'] = {
                    'status': 'failed', 'reason': 'No owned outer output root was created',
                    'original_failure_retained': True}
        cancellation.close_preservation()
        report['completed_ns'] = time.time_ns()
        if created:
            try:
                _write_result(common, output, report)
            except BaseException as error:
                report['domain_complete'] = False
                report['terminal_capture_error'] = _error(error)
                if first_error is None:
                    first_error = error
                    report['failure'] = _error(error)
                report['preservation'].setdefault('raw_evidence', {
                    'status': 'failed', 'reason': 'Terminal journal storage failed after the bounded preservation phase',
                    'failure': _error(error), 'original_failure_retained': True})
                # An earlier raw error stays primary even if evidence storage failed.
                print(json.dumps({'failure': report['failure'],
                                  'terminal_capture_error': report['terminal_capture_error'],
                                  'raw_evidence_preservation': report['preservation']['raw_evidence']}, sort_keys=True),
                      file=sys.stderr)
        cancellation.sealed = True
        for sig, handler in previous.items():
            signal.signal(sig, handler)
    if first_error is not None:
        raise first_error
    return report


def main(argv=None):
    parser = argparse.ArgumentParser()
    parser.add_argument('command', choices=('plan', 'host'))
    parser.add_argument('--request', required=True)
    args = parser.parse_args(argv)
    common = _common()
    request = common.decode(common.read(_path(args.request)))
    result = plan_request(request) if args.command == 'plan' else produce_host(request)
    print(json.dumps(result, sort_keys=True))


if __name__ == '__main__':
    main()
