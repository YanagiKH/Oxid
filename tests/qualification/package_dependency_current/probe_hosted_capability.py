#!/usr/bin/env python3
"""Capability-only hosted probe. Never import, invoke, or activate a generator."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import resource
import selectors
import signal
import socket
import stat
import subprocess
import sys
import time

LOCAL_REVIEW_C = '9429f94fb97b391546e5ee17327d228a13aa5f3a'
LOCAL_REVIEW_A = '24a7cbe7369124523b5319c4960c7e71017fedc2'
BASE = '0fef9bf4c664c52f441e1c8b842edfcb57fa15d0'
C = 'ce522e393646ce6478848cee31f307cb4b6ad5e8'
T = '8c3f4837e1fc89a6272ebee3552ce617ce584131'
A = '06fbffae6fa74ec08f5c3fcc761b15b393bb1b8e'
AT = 'a15ab7812a67d4f639f6c3427beeff216b0aa11a'
RECIPE_PATH = 'tests/qualification/package_dependency_current/generate_source_v1.py'
RECIPE_SHA = '5d724e0c588901fce69c59cd923ae9b83f6cfecb37865d7d9256dec156062b24'
PROBE_PATH = 'tests/qualification/package_dependency_current/probe_hosted_capability.py'
WORKFLOW_PATH = '.github/workflows/package-source-capability.yml'
PUBLIC_PATHS = {PROBE_PATH, WORKFLOW_PATH}
LIMIT = 8 * 1024**2
OUTPUT_LIMIT = 32 * 1024**2
DISK_FLOOR = 16 * 1024**3
WALL = 300
ENV = {'PATH': '/usr/bin:/bin', 'HOME': '/nonexistent/oxid-capability-home',
       'LANG': 'C.UTF-8', 'LC_ALL': 'C.UTF-8', 'PYTHONDONTWRITEBYTECODE': '1',
       'GIT_CONFIG_NOSYSTEM': '1', 'GIT_CONFIG_GLOBAL': '/dev/null',
       'GIT_NO_REPLACE_OBJECTS': '1', 'GIT_NO_LAZY_FETCH': '1',
       'GIT_OPTIONAL_LOCKS': '0', 'GIT_TERMINAL_PROMPT': '0'}
ROSTER = ('capability.json', 'invocation.json', 'source-before.json',
          'source-after.json', 'owned-process.json', 'guard-result.json',
          'stdout.log', 'stderr.log', 'probe.json')
STOP_SIGNAL = None


def stop(signum, frame):
    global STOP_SIGNAL
    STOP_SIGNAL = signum


def need(ok, message):
    if not ok:
        raise ValueError(message)


def digest(raw):
    return hashlib.sha256(raw).hexdigest()


def save(root, name, value):
    with (root / name).open('x') as stream:
        json.dump(value, stream, sort_keys=True, indent=2)
        stream.write('\n')


def safe(path):
    need(path.is_absolute() and path == path.resolve(strict=False), 'noncanonical path')
    need(not any(p.is_symlink() for p in (path, *path.parents)), 'symlink path')


def git(repo, *args):
    return subprocess.check_output(['/usr/bin/git', '-c', 'core.hooksPath=/dev/null',
        '-c', 'core.fsmonitor=false', '-C', str(repo), *args], env=ENV,
        timeout=20, stderr=subprocess.DEVNULL)


def free(root):
    value = os.statvfs(root)
    return value.f_bavail * value.f_frsize


def snapshot(repo):
    need(git(repo, 'rev-parse', 'HEAD').decode().strip() == A, 'changed adapter head')
    need(git(repo, 'rev-parse', 'HEAD^{tree}').decode().strip() == AT, 'changed adapter tree')
    need(not git(repo, 'status', '--porcelain', '--untracked-files=all'), 'dirty input checkout')
    rows = []
    for item in git(repo, 'ls-tree', '-r', '-z', A).split(b'\0'):
        if not item:
            continue
        meta, name = item.split(b'\t', 1)
        mode, kind, blob = meta.decode().split()
        name = name.decode()
        p = repo / name
        need(mode in ('100644', '100755') and kind == 'blob', 'nonregular committed member')
        safe(p)
        st = p.stat()
        need(stat.S_ISREG(st.st_mode) and stat.S_IMODE(st.st_mode) == int(mode, 8) & 0o777,
             'changed source mode')
        raw = p.read_bytes()
        actual = hashlib.sha1(b'blob ' + str(len(raw)).encode() + b'\0' + raw).hexdigest()
        need(actual == blob, 'changed committed body')
        rows.append({'path': name, 'mode': mode, 'git_blob': blob,
                     'bytes': len(raw), 'sha256': digest(raw)})
    need(len(rows) == 2672, 'wrong complete input member count')
    expected_files = {r['path'] for r in rows}
    expected_dirs = {p.as_posix() for n in expected_files for p in Path(n).parents
                     if p.as_posix() != '.'}
    actual_files, actual_dirs = set(), set()
    marker = repo / '.git'
    need(marker.is_file() and not marker.is_symlink(), 'unexpected worktree Git marker')
    for parent, directories, files in os.walk(repo, followlinks=False):
        for name in directories + files:
            p = Path(parent) / name
            relative = p.relative_to(repo).as_posix()
            if relative == '.git':
                continue
            need(not p.is_symlink(), 'symlink in complete input closure')
            if name in directories:
                actual_dirs.add(relative)
            else:
                need(stat.S_ISREG(p.stat().st_mode), 'nonregular input closure member')
                actual_files.add(relative)
    need(actual_files == expected_files and actual_dirs == expected_dirs,
         'extra or missing source closure member')
    need(digest((repo / RECIPE_PATH).read_bytes()) == RECIPE_SHA, 'wrong recipe bytes')
    return rows


def limits():
    resource.setrlimit(resource.RLIMIT_AS, (2 * 1024**3, 2 * 1024**3))
    resource.setrlimit(resource.RLIMIT_FSIZE, (LIMIT, LIMIT))
    resource.setrlimit(resource.RLIMIT_CORE, (0, 0))


def candidate_size(root):
    total = 0
    for path in root.rglob('*'):
        need(not path.is_symlink(), 'candidate symlink')
        mode = path.lstat().st_mode
        if stat.S_ISREG(mode):
            total += path.stat().st_size
        else:
            need(stat.S_ISDIR(mode), 'nonregular candidate member')
    return total


def readonly(path):
    # statvfs reports the effective mount covering this exact canonical path.
    safe(path)
    need(bool(os.statvfs(path).f_flag & os.ST_RDONLY), 'required mount is not read-only')
    return True


def inside(args):
    need(os.geteuid() != 0, 'privileged probe refused')
    own_net = os.readlink('/proc/self/ns/net')
    need(own_net != args.host_net, 'network namespace was not changed')
    interfaces = sorted(name for _, name in socket.if_nameindex())
    need(interfaces == ['lo'], 'non-loopback interface')
    ipv4 = Path('/proc/net/route').read_text().splitlines()[1:]
    need(not any(row.strip() for row in ipv4), 'IPv4 route exists')
    ipv6 = Path('/proc/net/ipv6_route').read_text().splitlines()
    need(all(row.split()[-1] == 'lo' for row in ipv6 if row.strip()), 'external IPv6 route')
    # IPv6 entries must be loopback (::1) or kernel's unreachable zero routes.
    for row in ipv6:
        fields = row.split()
        if fields:
            need(fields[0] in ('0' * 32, '0' * 31 + '1') and
                 fields[4] == '0' * 32, 'non-loopback IPv6 destination or gateway')
    mounts = {label: readonly(path) for label, path in (
        ('source', args.repository), ('git_dir', args.git_dir),
        ('git_common_dir', args.git_common_dir), ('probe_code', Path(__file__).absolute()))}
    need(not os.statvfs(args.output).f_flag & os.ST_RDONLY, 'candidate mount not writable')
    need(not list(args.output.iterdir()), 'candidate parent not empty')
    save(args.output, 'probe.json', {'schema': 'oxid-hosted-capability-probe-v1',
        'network_namespace_distinct': True, 'interfaces': interfaces,
        'ipv4_routes': 0, 'ipv6_routes_loopback_or_unreachable_only': True,
        'readonly_mounts': mounts, 'exclusive_candidate_write': True,
        'generator_invoked': False})


def closed(root):
    need([p.name for p in root.iterdir()] == ['probe.json'], 'wrong probe output roster')
    st = (root / 'probe.json').lstat()
    need(stat.S_ISREG(st.st_mode) and not st.st_mode & 0o111, 'invalid probe output')
    need(st.st_size <= LIMIT and candidate_size(root) <= OUTPUT_LIMIT, 'probe output size')
    value = json.loads((root / 'probe.json').read_bytes())
    need(value.get('generator_invoked') is False, 'invalid probe receipt')


def host(args):
    for signum in (signal.SIGINT, signal.SIGTERM):
        signal.signal(signum, stop)
    safe(args.repository)
    safe(args.output)
    need(not args.output.exists() and args.output.parent.is_dir(), 'fresh attempt required')
    for path in (args.repository, args.output):
        need(not any(path == base or base in path.parents for base in
                     (Path('/tmp'), Path('/proc'), Path('/dev'))), 'path hidden by sandbox mount')
    args.output.mkdir()
    source = args.output / 'source-view'
    writable = args.output / 'candidates'
    writable.mkdir()
    start = time.monotonic()
    proc, before, selector = None, None, None
    logs = {}
    status, failure = 'failed', None
    counts = {'stdout': 0, 'stderr': 0}
    group_empty = False
    # Failure strings are fixed diagnoses, never raw exceptions with host paths.
    stage = 'preflight'
    try:
        need(STOP_SIGNAL is None, 'stopped before launch')
        need(os.geteuid() != 0, 'privileged host refused')
        need(re.fullmatch('[0-9a-f]{40}', args.expected_head) is not None, 'invalid expected head')
        need(git(args.repository, 'rev-parse', 'HEAD').decode().strip() == args.expected_head,
             'wrong workflow head')
        parents = git(args.repository, 'rev-list', '--parents', '-n', '1', 'HEAD').decode().split()
        need(parents == [args.expected_head, A], 'workflow commit must have sole parent A')
        changes = git(args.repository, 'diff', '--name-status', '--no-renames', '-z', A, 'HEAD').split(b'\0')
        expected_changes = [part for name in sorted(PUBLIC_PATHS) for part in (b'A', name.encode())] + [b'']
        need(changes == expected_changes, 'workflow commit must contain exactly two additions')
        for name in sorted(PUBLIC_PATHS):
            need(not git(args.repository, 'ls-tree', '-z', A, '--', name), 'public tooling already exists in A')
            raw = (args.repository / name).read_bytes()
            oid = hashlib.sha1(b'blob ' + str(len(raw)).encode() + b'\0' + raw).hexdigest()
            expected = ('100644 blob ' + oid + '\t' + name + '\0').encode()
            need(git(args.repository, 'ls-tree', '-z', 'HEAD', '--', name) == expected,
                 'public tooling must be exact regular 100644 blobs')
            need(git(args.repository, 'show', 'HEAD:' + name) == raw, 'changed public tooling bytes')
        need(git(args.repository, 'rev-list', '--parents', '-n', '1', C).decode().split() == [C, BASE],
             'remote source parent mismatch')
        need(git(args.repository, 'rev-list', '--parents', '-n', '1', A).decode().split() == [A, C],
             'remote adapter parent mismatch')
        need(git(args.repository, 'rev-parse', C + '^{tree}').decode().strip() == T, 'source tree mismatch')
        need(git(args.repository, 'rev-parse', A + '^{tree}').decode().strip() == AT, 'adapter tree mismatch')
        bwrap = Path('/usr/bin/bwrap')
        available = bwrap.is_file() and os.access(bwrap, os.X_OK)
        save(args.output, 'capability.json', {'schema': 'oxid-hosted-capability-preflight-v1',
            'workflow_head': args.expected_head, 'source_head': C, 'source_tree': T,
            'adapter_head': A, 'adapter_tree': AT, 'recipe_sha256': RECIPE_SHA,
            'original_local_review_source_head': LOCAL_REVIEW_C,
            'original_local_review_adapter_head': LOCAL_REVIEW_A,
            'remote_transport_preserves_trees_not_commit_identity': True,
            'free_bytes': free(args.output), 'required_free_bytes': DISK_FLOOR,
            'bwrap_available': available,
            'bwrap_sha256': digest(bwrap.read_bytes()) if available else None,
            'git_sha256': digest(Path('/usr/bin/git').read_bytes()),
            'python_sha256': digest(Path('/usr/bin/python3').read_bytes()),
            'kernel': os.uname().release,
            'python_version': list(sys.version_info[:3]),
            'probe_sha256': digest(Path(__file__).read_bytes()),
            'image_os': os.environ.get('ImageOS', 'unknown'),
            'image_version': os.environ.get('ImageVersion', 'unknown'),
            'generator_invoked': False})
        need(available, 'bwrap unavailable; no installation or fallback')
        need(free(args.output) >= DISK_FLOOR, 'disk floor before launch')
        stage = 'prepare-source-view'
        git(args.repository, 'worktree', 'add', '--detach', str(source), A)
        before = snapshot(source)
        save(args.output, 'source-before.json', before)
        stores = []
        for option in ('--git-dir', '--git-common-dir'):
            p = Path(git(source, 'rev-parse', '--path-format=absolute', option).decode().strip())
            safe(p)
            need(p.is_dir(), 'missing canonical Git store')
            stores.append(p)
        need(STOP_SIGNAL is None, 'stopped before sandbox launch')
        need(time.monotonic() - start <= WALL, 'wall limit before sandbox launch')
        need(free(args.output) >= DISK_FLOOR, 'disk floor before sandbox launch')
        host_net = os.readlink('/proc/self/ns/net')
        argv = ['/usr/bin/bwrap', '--die-with-parent', '--unshare-net', '--ro-bind', '/', '/',
                '--bind', str(writable), str(writable), '--tmpfs', '/tmp', '--proc', '/proc',
                '--dev', '/dev', '--', '/usr/bin/python3', '-B', str(Path(__file__).absolute()),
                '--inside', '--repository', str(source), '--output', str(writable),
                '--git-dir', str(stores[0]), '--git-common-dir', str(stores[1]), '--host-net', host_net]
        save(args.output, 'invocation.json', {'schema': 'oxid-hosted-capability-invocation-v1',
            'sandbox_flags': ['--die-with-parent', '--unshare-net', '--ro-bind / /',
                              '--bind CANDIDATES CANDIDATES', '--tmpfs /tmp', '--proc /proc', '--dev /dev'],
            'command': 'python3 -B PUBLIC_PROBE --inside', 'environment': ENV,
            'wall_seconds': WALL, 'wall_scope': 'monitored-and-terminally-checked-not-hard-total-deadline',
            'rlimit_scope': 'sandbox-and-descendants-only', 'address_space_bytes': 2 * 1024**3,
            'file_and_stream_limit_bytes': LIMIT, 'aggregate_candidate_limit_bytes': OUTPUT_LIMIT,
            'disk_floor_bytes': DISK_FLOOR, 'generator_invoked': False})
        stage = 'sandbox-probe'
        for name in counts:
            logs[name] = (args.output / (name + '.log')).open('xb')
        proc = subprocess.Popen(argv, cwd=source, env=ENV, stdout=subprocess.PIPE,
            stderr=subprocess.PIPE, start_new_session=True, preexec_fn=limits)
        save(args.output, 'owned-process.json', {'pid': proc.pid, 'pgid': proc.pid})
        selector = selectors.DefaultSelector()
        for name, pipe in (('stdout', proc.stdout), ('stderr', proc.stderr)):
            os.set_blocking(pipe.fileno(), False)
            selector.register(pipe, selectors.EVENT_READ, name)
        while selector.get_map() or proc.poll() is None:
            need(STOP_SIGNAL is None, 'stop signal received')
            need(time.monotonic() - start <= WALL, 'wall-time limit')
            need(free(args.output) >= DISK_FLOOR, 'disk floor during probe')
            need(candidate_size(writable) <= OUTPUT_LIMIT, 'aggregate candidate-size limit')
            for key, _ in selector.select(0.05):
                chunk = os.read(key.fd, 65536)
                if not chunk:
                    selector.unregister(key.fileobj)
                    continue
                name = key.data
                logs[name].write(chunk[:LIMIT - counts[name]])
                counts[name] += len(chunk)
                need(counts[name] <= LIMIT, 'stream limit')
        need(proc.wait(timeout=1) == 0, 'sandbox or probe returned nonzero')
        closed(writable)
        status = 'capability-demonstrated-generator-not-run'
    except BaseException as error:
        failure = str(error) if isinstance(error, ValueError) else type(error).__name__
    finally:
        if proc is not None:
            try:
                os.killpg(proc.pid, signal.SIGKILL)
            except ProcessLookupError:
                pass
            except BaseException:
                status, failure = 'failed', (failure or '') + '; group cleanup failed'
            try:
                proc.wait(timeout=5)
            except BaseException:
                status, failure = 'failed', (failure or '') + '; cleanup wait failed'
            deadline = time.monotonic() + 5
            while time.monotonic() < deadline:
                try:
                    os.killpg(proc.pid, 0)
                except ProcessLookupError:
                    group_empty = True
                    break
                except BaseException:
                    failure = (failure or '') + '; group verification failed'
                    break
                time.sleep(0.05)
        else:
            group_empty = True
        for stream in logs.values():
            try:
                stream.close()
            except BaseException:
                status, failure = 'failed', (failure or '') + '; stream close failed'
        if selector is not None:
            try:
                selector.close()
            except BaseException:
                status, failure = 'failed', (failure or '') + '; selector close failed'
        if before is not None:
            try:
                after = snapshot(source)
                save(args.output, 'source-after.json', after)
                need(after == before, 'source before/after mismatch')
            except BaseException:
                status, failure = 'failed', (failure or '') + '; source recheck failed'
        try:
            need(group_empty, 'owned group not empty')
            need(time.monotonic() - start <= WALL, 'terminal wall limit')
            need(free(args.output) >= DISK_FLOOR, 'terminal disk floor')
            need(STOP_SIGNAL is None, 'stop signal received')
            if status == 'capability-demonstrated-generator-not-run':
                closed(writable)
        except BaseException as error:
            status = 'failed'
            failure = (failure or '') + '; ' + (str(error) if isinstance(error, ValueError) else type(error).__name__)
        save(args.output, 'guard-result.json', {'status': status, 'failure': failure, 'stage': stage,
            'elapsed_seconds': time.monotonic() - start, 'stream_observed_bytes': counts,
            'returncode': proc.returncode if proc else None, 'owned_group_empty': group_empty,
            'stop_signal': STOP_SIGNAL, 'generator_invoked': False, 'generation_authorized': False,
            'historical_preflight': 'not-run', 'execution_qualified': False})
    return 0 if status == 'capability-demonstrated-generator-not-run' else 1


def stage_upload(root, output):
    safe(root)
    safe(output)
    need(not output.exists() and output.parent.is_dir(), 'fresh upload directory required')
    output.mkdir()
    entries = []
    for name in ROSTER:
        source = root / ('candidates/probe.json' if name == 'probe.json' else name)
        if not source.exists():
            entries.append({'name': name, 'status': 'not-produced'})
            continue
        safe(source)
        st = source.lstat()
        need(stat.S_ISREG(st.st_mode) and st.st_nlink == 1 and st.st_size <= LIMIT,
             'unsafe evidence member')
        raw = source.read_bytes()
        # Logs contain only probe/bwrap diagnostics. Replace actual run paths before upload.
        if name.endswith('.log'):
            text = raw.decode('utf-8', errors='replace')
            for p in (str(root), os.environ.get('GITHUB_WORKSPACE', ''), os.environ.get('RUNNER_TEMP', '')):
                if p:
                    text = text.replace(p, '<HOSTED_PATH>')
            raw = text.encode()
        with (output / name).open('xb') as stream:
            stream.write(raw)
        entries.append({'name': name, 'status': 'preserved', 'bytes': len(raw), 'sha256': digest(raw)})
    save(output, 'upload-manifest.json', {'schema': 'oxid-capability-upload-v1', 'files': entries,
        'generator_invoked': False, 'missing_evidence_is_not_success': True})
    need((output / 'guard-result.json').is_file(), 'mandatory guard result missing')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--repository', type=Path)
    parser.add_argument('--output', type=Path)
    parser.add_argument('--expected-head')
    parser.add_argument('--inside', action='store_true')
    parser.add_argument('--git-dir', type=Path)
    parser.add_argument('--git-common-dir', type=Path)
    parser.add_argument('--host-net')
    parser.add_argument('--stage', type=Path)
    parser.add_argument('--upload', type=Path)
    args = parser.parse_args()
    if args.stage:
        need(args.upload is not None, 'upload path required')
        stage_upload(args.stage, args.upload)
        return 0
    need(args.repository is not None and args.output is not None, 'source and output required')
    if args.inside:
        need(args.git_dir is not None and args.git_common_dir is not None and args.host_net,
             'namespace and stores required')
        inside(args)
        return 0
    need(args.expected_head is not None, 'expected head required')
    return host(args)


if __name__ == '__main__':
    try:
        sys.exit(main())
    except Exception as error:
        # Suppress traceback/local filesystem paths from public logs.
        print('Capability operation failed: ' + type(error).__name__, file=sys.stderr)
        sys.exit(1)
