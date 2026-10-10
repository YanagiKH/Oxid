#!/usr/bin/env python3
"""One approved hosted job: scoped userns profile, capability, then candidate-only generation."""
import argparse
import io
import tarfile
import urllib.parse
import hashlib
import json
import os
from pathlib import Path
import re
import resource
import selectors
import secrets
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
PROBE_PATH = 'tests/qualification/package_dependency_current/generate_hosted_once.py'
WORKFLOW_PATH = '.github/workflows/package-source-generation-once.yml'
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
ROSTER = ('setup.json', 'invocation.json', 'source-before.json', 'source-after.json',
    'guard-result.json', 'profile-cleanup.json', 'always-cleanup.json',
    'capability-stdout.log', 'capability-stderr.log', 'generation-stdout.log', 'generation-stderr.log',
    'capability.json', 'generation-entry.json', 'package-dependency-source-v1.json',
    'package-dependency-authority-v1.json', 'package-dependency-transition-v1.patch',
    'source-generation-receipt.json')
STOP_SIGNAL = None


def stop(signum, frame):
    global STOP_SIGNAL
    STOP_SIGNAL = signum


class GuardError(ValueError):
    """Only fixed literal guard reasons; other exception messages are never published."""


def need(ok, message):
    if not ok:
        raise GuardError(message)


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
    security = sandbox_entry(args)
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
        ('git_common_dir', args.git_common_dir), ('probe_code', Path(__file__).absolute()),
        ('sandbox_binary', args.bwrap))}
    need(not os.statvfs(args.output).f_flag & os.ST_RDONLY, 'candidate mount not writable')
    need(not list(args.output.iterdir()), 'candidate parent not empty')
    save(args.output, 'probe.json', {'schema': 'oxid-hosted-capability-probe-v1',
        'network_namespace_distinct': True, 'interfaces': interfaces,
        'ipv4_routes': 0, 'ipv6_routes_loopback_or_unreachable_only': True,
        'readonly_mounts': mounts, 'exclusive_candidate_write': True,
        'generator_invoked': False, 'entry_security': security})
    if args.generate:
        need(digest((args.repository / RECIPE_PATH).read_bytes()) == RECIPE_SHA, 'recipe changed at generation entry')
        # Fixed reviewed generator is executed only after all repeated sandbox assertions pass.
        os.execve('/usr/bin/python3', ['/usr/bin/python3', '-B', str(args.repository / RECIPE_PATH),
            '--repository', str(args.repository), '--candidate-output-dir', str(args.output / 'products'),
            '--source-head', C, '--source-tree', T, '--adapter-head', A,
            '--source-review-reference', 'https://github.com/YanagiKH/Oxid/commit/' + C,
            '--adapter-review-reference', 'https://github.com/YanagiKH/Oxid/commit/' + A], ENV)


def closed(root):
    need([p.name for p in root.iterdir()] == ['probe.json'], 'wrong probe output roster')
    st = (root / 'probe.json').lstat()
    need(stat.S_ISREG(st.st_mode) and not st.st_mode & 0o111, 'invalid probe output')
    need(st.st_size <= LIMIT and candidate_size(root) <= OUTPUT_LIMIT, 'probe output size')
    value = json.loads((root / 'probe.json').read_bytes())
    need(value.get('generator_invoked') is False, 'invalid probe receipt')


# Official package is downloaded, never installed. No maintainer code runs.
DEB_VERSION = '0.9.0-1ubuntu0.3'
DEB_NAME = 'bubblewrap_' + DEB_VERSION + '_amd64.deb'
DEB_BYTES = 50436
DEB_SHA = '2461f1beee9cb04c8942739fe1a2b37e7b7c2a3d518f0779dc75f9245baa3094'
BWRAP_BYTES = 72160
BWRAP_SHA = 'e318903862396f96de3df57264e0158682b952fd3fb53ac23d876413e7b30f71'


def setup_command(argv, cwd, started):
    """Bounded plain-user setup child; never execute downloaded package code."""
    need(STOP_SIGNAL is None, 'stopped during setup')
    need(time.monotonic() - started <= 120, 'setup wall limit')
    selector = selectors.DefaultSelector()
    streams = {'stdout': bytearray(), 'stderr': bytearray()}
    cleanup_failed = False
    proc = subprocess.Popen(argv, cwd=cwd, env=ENV, stdout=subprocess.PIPE,
        stderr=subprocess.PIPE, start_new_session=True, preexec_fn=limits)
    try:
        for label, pipe in (('stdout', proc.stdout), ('stderr', proc.stderr)):
            os.set_blocking(pipe.fileno(), False)
            selector.register(pipe, selectors.EVENT_READ, label)
        while selector.get_map() or proc.poll() is None:
            need(STOP_SIGNAL is None, 'stopped during setup command')
            need(time.monotonic() - started <= 120, 'setup command wall limit')
            need(free(cwd) >= DISK_FLOOR, 'setup disk floor')
            for key, _ in selector.select(0.05):
                data = os.read(key.fd, 65536)
                if not data:
                    selector.unregister(key.fileobj)
                    continue
                stream = streams[key.data]
                need(len(stream) + len(data) <= LIMIT, 'setup command stream limit')
                stream.extend(data)
        need(proc.wait(timeout=1) == 0, 'setup command returned nonzero')
        return bytes(streams['stdout'])
    finally:
        try:
            os.killpg(proc.pid, signal.SIGKILL)
        except ProcessLookupError:
            pass
        except BaseException:
            cleanup_failed = True
        try:
            proc.wait(timeout=5)
        except BaseException:
            cleanup_failed = True
        empty = False
        deadline = time.monotonic() + 5
        while time.monotonic() < deadline:
            try:
                os.killpg(proc.pid, 0)
            except ProcessLookupError:
                empty = True
                break
            except BaseException:
                cleanup_failed = True
                break
            time.sleep(0.05)
        try:
            selector.close()
            proc.stdout.close()
            proc.stderr.close()
        except BaseException:
            cleanup_failed = True
        need(empty and not cleanup_failed, 'setup owned-group cleanup failed')
        need(STOP_SIGNAL is None, 'stop during setup cleanup')
        need(time.monotonic() - started <= 120, 'terminal setup wall limit')


def deb822(raw):
    paragraphs = []
    for paragraph in re.split(r'\n\s*\n', raw.strip()):
        fields = {}
        previous = None
        for line in paragraph.splitlines():
            if not line.strip() or line.startswith('#'):
                continue
            if line.startswith((' ', '\t')):
                need(previous is not None, 'invalid metadata continuation')
                fields[previous] += ' ' + line.strip()
                continue
            need(':' in line, 'invalid metadata field')
            key, value = line.split(':', 1)
            need(key not in fields, 'duplicate metadata field')
            fields[key] = value.strip()
            previous = key
        if fields:
            paragraphs.append(fields)
    return paragraphs


MIRROR_URI = 'mirror+file:/etc/apt/apt-mirrors.txt'
MIRROR_PATH = '/etc/apt/apt-mirrors.txt'
MIRROR_SHA = 'e0d6b0af979e4662d16357a27ae456cc6f21e26da031540f4df8706ebed1d583'
MIRROR_ROWS = (
    ('http://azure.archive.ubuntu.com/ubuntu/', 1),
    ('https://archive.ubuntu.com/ubuntu/', 2),
    ('https://security.ubuntu.com/ubuntu/', 3),
)


def bounded_public_read(path, limit):
    safe(path)
    descriptor = os.open(path, os.O_RDONLY | os.O_NOFOLLOW | os.O_NONBLOCK)
    with os.fdopen(descriptor, 'rb') as stream:
        state = os.fstat(stream.fileno())
        need(stat.S_ISREG(state.st_mode), 'nonregular public metadata')
        need(state.st_uid == 0, 'public distribution metadata must be root-owned')
        need(state.st_size <= limit, 'public metadata too large')
        raw = stream.read(limit + 1)
    need(len(raw) <= limit, 'public metadata read exceeds bound')
    return raw


def validate_direct_archive_uri(uri):
    # Preserve the existing direct HTTP/HTTPS admission predicate unchanged.
    parsed = urllib.parse.urlsplit(uri)
    need(parsed.scheme in ('http', 'https') and parsed.hostname is not None and
         (parsed.hostname in ('archive.ubuntu.com', 'security.ubuntu.com') or
          parsed.hostname.endswith('.archive.ubuntu.com')) and
         parsed.path.rstrip('/') == '/ubuntu' and not parsed.username and
         not parsed.password and parsed.port is None and not parsed.query and not parsed.fragment,
         'source is not an official Ubuntu archive')


def validate_mirror_list(raw):
    need(isinstance(raw, bytes) and len(raw) <= 16384, 'mirror data missing or too large')
    lines = raw.decode('ascii').splitlines()
    need(len(lines) == 3, 'mirror roster must contain exactly three rows')
    rows = []
    for line in lines:
        parts = line.split('\t')
        need(len(parts) == 2, 'mirror row must have URI and one priority')
        uri, option = parts
        validate_direct_archive_uri(uri)
        need(re.fullmatch(r'priority:[1-3]', option) is not None, 'unexpected mirror option')
        rows.append((uri, int(option[-1])))
    need(tuple(rows) == MIRROR_ROWS, 'mirror endpoints or priorities changed')
    need(digest(raw) == MIRROR_SHA, 'mirror bytes differ from observed official image')
    return {'path': MIRROR_PATH, 'sha256': digest(raw), 'endpoint_count': len(rows),
            'exact_observed_roster': True}


def validate_source_document(raw, mirror_raw=None):
    need(isinstance(raw, bytes) and len(raw) <= 65536, 'source metadata too large')
    entries = deb822(raw.decode('utf-8'))
    need(1 <= len(entries) <= 16, 'source stanza count rejected')
    mirror = None
    for entry in entries:
        need(set(entry) <= {'Types', 'URIs', 'Suites', 'Components', 'Signed-By'},
             'unexpected source configuration field')
        need(set(entry.get('Types', '').split()) <= {'deb', 'deb-src'} and
             'deb' in entry.get('Types', '').split(), 'binary source required')
        need(entry.get('Signed-By') == '/usr/share/keyrings/ubuntu-archive-keyring.gpg',
             'unexpected archive trust keyring')
        need(set(entry.get('Suites', '').split()) <= {'noble', 'noble-updates', 'noble-security', 'noble-backports'}
             and entry.get('Suites'), 'unexpected Ubuntu suite')
        need(set(entry.get('Components', '').split()) <= {'main', 'universe', 'restricted', 'multiverse'}
             and 'main' in entry.get('Components', '').split(), 'unexpected Ubuntu components')
        uris = entry.get('URIs', '').split()
        need(1 <= len(uris) <= 32, 'source URI count rejected')
        for uri in uris:
            if uri == MIRROR_URI:
                mirror = validate_mirror_list(mirror_raw)
            else:
                # No other wrapping/local transport or alternate mirror-list path.
                validate_direct_archive_uri(uri)
    return {'source_file_sha256': digest(raw),
            'source_origin': 'Ubuntu noble official archives; exact observed mirror transport when present',
            'mirror_list': mirror}


def official_sources():
    # Read only fixed root-owned distribution paths; no URI selects a file to read.
    raw = bounded_public_read(Path('/etc/apt/sources.list.d/ubuntu.sources'), 65536)
    entries = deb822(raw.decode('utf-8'))
    use_mirror = any(MIRROR_URI in entry.get('URIs', '').split() for entry in entries)
    mirror_raw = bounded_public_read(Path(MIRROR_PATH), 16384) if use_mirror else None
    result = validate_source_document(raw, mirror_raw)
    keyring = bounded_public_read(Path('/usr/share/keyrings/ubuntu-archive-keyring.gpg'), 1024 * 1024)
    result['keyring_sha256'] = digest(keyring)
    return result

def select_binary(raw, data):
    """Read bounded ar/tar bytes; never extract a tree or execute maintainer scripts."""
    need(len(raw) == DEB_BYTES and digest(raw) == DEB_SHA, 'official package hash or size mismatch')
    need(raw.startswith(b'!<arch>\n'), 'invalid deb container')
    offset, members = 8, {}
    while offset < len(raw):
        header = raw[offset:offset + 60]
        need(len(header) == 60 and header[58:] == b'`\n', 'invalid ar header')
        name = header[:16].decode().strip().rstrip('/')
        length = int(header[48:58])
        need(name not in members and 0 <= length <= DEB_BYTES, 'invalid ar member')
        offset += 60
        body = raw[offset:offset + length]
        need(len(body) == length, 'short ar member')
        members[name] = body
        offset += length + (length % 2)
    need(offset == len(raw) and set(members) == {'debian-binary', 'control.tar.zst', 'data.tar.zst'}
         and members['debian-binary'] == b'2.0\n', 'unexpected deb member roster')
    need(len(data) <= 2 * 1024**2, 'excessive decoded package data archive')
    with tarfile.open(fileobj=io.BytesIO(data), mode='r:') as archive:
        entries = archive.getmembers()
        need(len(entries) <= 64, 'excessive archive membership')
        targets = [e for e in entries if e.name in ('./usr/bin/bwrap', 'usr/bin/bwrap')]
        need(len(targets) == 1, 'binary archive member not unique')
        entry = targets[0]
        need(entry.isfile() and not entry.issym() and not entry.islnk() and
             entry.uid == 0 and entry.gid == 0 and entry.mode == 0o755 and
             entry.size == BWRAP_BYTES and not entry.pax_headers,
             'binary archive member identity rejected')
        binary = archive.extractfile(entry).read(BWRAP_BYTES + 1)
    need(len(binary) == BWRAP_BYTES and digest(binary) == BWRAP_SHA, 'binary identity mismatch')
    return binary


def prepare_official_binary(root):
    started = time.monotonic()
    record = {'schema': 'oxid-official-userspace-bwrap-setup-v1', 'status': 'failed',
              'package': 'bubblewrap', 'version': DEB_VERSION, 'architecture': 'amd64',
              'deb_sha256': DEB_SHA, 'binary_sha256': BWRAP_SHA,
              'package_installed': False, 'maintainer_scripts_executed': False,
              'system_policy_written': False, 'generator_invoked': False}
    try:
        need(os.geteuid() != 0, 'privileged package setup refused')
        need(free(root) >= DISK_FLOOR, 'setup disk floor')
        record.update(official_sources())
        # Read global policy only. The separately reviewed temporary profile must load before any sandbox.
        aa = Path('/sys/module/apparmor/parameters/enabled')
        restriction = Path('/proc/sys/kernel/apparmor_restrict_unprivileged_userns')
        userns = Path('/proc/sys/kernel/unprivileged_userns_clone')
        maxns = Path('/proc/sys/user/max_user_namespaces')
        policy = {'apparmor_enabled': aa.read_text().strip() if aa.is_file() else 'unknown',
                  'apparmor_restrict_unprivileged_userns': restriction.read_text().strip() if restriction.is_file() else 'unknown',
                  'unprivileged_userns_clone': userns.read_text().strip() if userns.is_file() else 'unknown',
                  'max_user_namespaces': maxns.read_text().strip() if maxns.is_file() else 'unknown'}
        record['policy_readback'] = policy
        need(policy['apparmor_enabled'] == 'Y' and policy['apparmor_restrict_unprivileged_userns'] == '1',
             'global AppArmor restriction must remain enabled')
        need(policy['unprivileged_userns_clone'] == '1' and policy['max_user_namespaces'].isdigit()
             and int(policy['max_user_namespaces']) > 0, 'user namespaces unavailable or unknown')
        tools_root = root / 'tools'
        tools_root.mkdir()
        download = tools_root / 'download'
        download.mkdir()
        installed = {}
        for package, minimum in (('libc6:amd64', '2.38'), ('libcap2:amd64', '1:2.10'), ('libselinux1:amd64', '3.1~')):
            value = setup_command(['/usr/bin/dpkg-query', '-W', '-f=${Status}\t${Version}\t${Architecture}\n', package], download, started).decode().strip().split('\t')
            need(len(value) == 3 and value[0] == 'install ok installed' and value[2] == 'amd64',
                 'required installed dependency missing')
            setup_command(['/usr/bin/dpkg', '--compare-versions', value[1], 'ge', minimum], download, started)
            installed[package] = value[1]
        for library in ('/lib64/ld-linux-x86-64.so.2', '/usr/lib/x86_64-linux-gnu/libc.so.6',
                        '/usr/lib/x86_64-linux-gnu/libcap.so.2', '/usr/lib/x86_64-linux-gnu/libselinux.so.1'):
            path = Path(library).resolve(strict=True)
            need(path.is_file() and stat.S_ISREG(path.stat().st_mode), 'required loader or library missing')
        record['installed_dependencies'] = installed
        options = ['-o', 'Dir::Etc::sourcelist=/etc/apt/sources.list.d/ubuntu.sources',
                   '-o', 'Dir::Etc::sourceparts=-', '-o', 'Dir::Cache::pkgcache=', '-o', 'Dir::Cache::srcpkgcache=',
                   '-o', 'APT::Get::AllowUnauthenticated=false', '-o', 'Acquire::AllowInsecureRepositories=false',
                   '-o', 'Acquire::AllowDowngradeToInsecureRepositories=false', '-o', 'Acquire::Check-Valid-Until=true',
                   '-o', 'Acquire::Retries=0', '-o', 'Acquire::http::Timeout=20', '-o', 'Acquire::https::Timeout=20']
        metadata = setup_command(['/usr/bin/apt-cache', *options, 'show', 'bubblewrap:amd64=' + DEB_VERSION], download, started)
        paragraphs = deb822(metadata.decode())
        need(paragraphs and all(p.get('Package') == 'bubblewrap' and p.get('Version') == DEB_VERSION and
             p.get('Architecture') == 'amd64' and p.get('SHA256') == DEB_SHA and
             p.get('Size') == str(DEB_BYTES) and p.get('Filename') == 'pool/main/b/bubblewrap/' + DEB_NAME
             for p in paragraphs), 'APT package metadata mismatch')
        record['apt_metadata_sha256'] = digest(metadata)
        setup_command(['/usr/bin/apt-get', *options, 'download', 'bubblewrap:amd64=' + DEB_VERSION], download, started)
        need(sorted(p.name for p in download.iterdir()) == [DEB_NAME], 'unexpected download roster')
        package = download / DEB_NAME
        safe(package)
        st = package.lstat()
        need(stat.S_ISREG(st.st_mode) and st.st_nlink == 1 and st.st_size == DEB_BYTES,
             'unsafe downloaded package')
        raw = package.read_bytes()
        need(digest(raw) == DEB_SHA, 'package hash before decoding')
        data = setup_command(['/usr/bin/dpkg-deb', '--fsys-tarfile', str(package)], download, started)
        need(package.read_bytes() == raw, 'package changed during decoding')
        binary = select_binary(raw, data)
        target = tools_root / 'bwrap'
        with target.open('xb') as stream:
            stream.write(binary)
        target.chmod(0o755)
        need(not os.listxattr(target), 'unexpected binary extended attributes')
        need(target.lstat().st_nlink == 1 and stat.S_IMODE(target.stat().st_mode) == 0o755,
             'unsafe extracted executable mode')
        need(digest(target.read_bytes()) == BWRAP_SHA, 'extracted executable changed')
        need(STOP_SIGNAL is None and time.monotonic() - started <= 120 and free(root) >= DISK_FLOOR,
             'terminal setup bounds')
        record['status'] = 'official-binary-prepared-not-executed'
        record['binary_bytes'] = BWRAP_BYTES
        return target
    except BaseException as error:
        record['failure'] = str(error) if type(error) is GuardError else type(error).__name__
        raise
    finally:
        record['elapsed_seconds'] = time.monotonic() - started
        save(root, 'setup.json', record)


# Profile files and operation state stay local to this ephemeral job, never uploaded.
PROFILE_BRANCH = 'KH/package-source-generation-once-v2'
PROFILE_MODES = ('unconfined',)
GENERATOR_NAMES = ('package-dependency-source-v1.json',
    'package-dependency-authority-v1.json', 'package-dependency-transition-v1.patch',
    'source-generation-receipt.json')
PARSER = '/usr/sbin/apparmor_parser'
ABI_PATH = '/etc/apparmor.d/abi/4.0'


def read_small(path, limit=65536):
    # Procfs files report zero sizes; reads still have a strict byte cap.
    with path.open('rb') as stream:
        value = stream.read(limit + 1)
    need(len(value) <= limit, 'metadata size limit')
    return value


def policy_readback():
    paths = {'apparmor_enabled': '/sys/module/apparmor/parameters/enabled',
        'restrict_unprivileged_userns': '/proc/sys/kernel/apparmor_restrict_unprivileged_userns',
        'unprivileged_userns_clone': '/proc/sys/kernel/unprivileged_userns_clone',
        'max_user_namespaces': '/proc/sys/user/max_user_namespaces'}
    value = {key: read_small(Path(path), 64).decode().strip() for key, path in paths.items()}
    need(value['apparmor_enabled'] == 'Y' and value['restrict_unprivileged_userns'] == '1'
         and value['unprivileged_userns_clone'] == '1' and value['max_user_namespaces'].isdigit()
         and int(value['max_user_namespaces']) > 0, 'required unchanged global policy unavailable')
    return value


def profile_identity(root, nonce):
    need(re.fullmatch('[0-9a-f]{32}', nonce) is not None, 'invalid profile nonce')
    # No escaping or glob expansion is accepted in a policy attachment path.
    path = str(root / 'tools' / 'bwrap')
    need(re.fullmatch('/[A-Za-z0-9_./-]+', path) is not None and '..' not in Path(path).parts,
         'unsafe literal profile attachment')
    name = 'oxid-package-source-once-' + nonce
    body = ('abi "/etc/apparmor.d/abi/4.0",\nprofile ' + name + ' "' + path + '" flags=(unconfined) {\n  userns,\n}\n').encode()
    return name, body


PROFILE_READ_ARGV = ('/usr/bin/sudo', '-n', '/usr/bin/dd',
    'if=/sys/kernel/security/apparmor/profiles', 'bs=1048577', 'count=1',
    'iflag=fullblock,nofollow,nonblock', 'status=none')
PROFILE_READ_PHASES = ('before-add', 'after-add', 'before-capability', 'before-generation',
                       'cleanup-before-remove', 'cleanup-after-remove')


def diagnostic(error, state):
    # Never stringify OSError/UnicodeError: those can contain paths or raw input bytes.
    return {'operation': state.get('diagnostic_operation', 'unspecified'),
        'error_class': type(error).__name__,
        'errno': error.errno if isinstance(error, OSError) else None,
        'reason': str(error) if type(error) is GuardError else type(error).__name__}


def diagnostic_step(root, state, operation):
    allowed = {'profile.same-uid-scan', 'profile.policy-readback', 'profile.parser-identity',
        'profile.abi-identity', 'profile.tempfile-write', 'profile.add', 'profile.post-add-identity',
        'cleanup.owned-groups', 'cleanup.same-uid-scan', 'cleanup.privileged-terminal',
        'cleanup.remove', 'cleanup.policy-readback'}
    allowed.update('profile.inventory.' + phase for phase in PROFILE_READ_PHASES)
    need(operation in allowed, 'invalid diagnostic operation')
    state['diagnostic_operation'] = operation
    profile_state_write(root, state)


def parse_profile_inventory(raw, name):
    need(isinstance(raw, bytes) and len(raw) <= 1024**2, 'profile inventory byte limit')
    need(re.fullmatch('oxid-package-source-once-[0-9a-f]{32}', name) is not None,
         'invalid exact profile name')
    need(not raw or raw.endswith(b'\n'), 'incomplete profile inventory record')
    try:
        text = raw.decode('utf-8')
    except UnicodeError:
        raise ValueError('profile inventory encoding invalid') from None
    # This is the only value returned from the privileged inventory: our exact label family.
    return [line for line in text.splitlines() if line == name or line.startswith(name + ' ')
            or line.startswith(name + '//')]


def readers_terminal(state):
    return all(r['terminal'] and r['group_absent'] and isinstance(r['pgid'], int)
               and group_absent(r['pgid']) for r in state['profile_read_operations'])


def read_profile_inventory(root, state, name, phase):
    need(phase in PROFILE_READ_PHASES, 'invalid profile read phase')
    diagnostic_step(root, state, 'profile.inventory.' + phase)
    need(readers_terminal(state), 'previous privileged reader terminal state unverified')
    reader = Path('/usr/bin/dd')
    safe(reader)
    st = reader.lstat()
    need(stat.S_ISREG(st.st_mode) and st.st_uid == 0 and not st.st_mode & 0o022
         and os.access(reader, os.X_OK), 'fixed system reader unavailable or writable')
    reader_sha = digest(reader.read_bytes())
    if state['reader_sha256'] is None:
        state['reader_sha256'] = reader_sha
    need(state['reader_sha256'] == reader_sha, 'fixed system reader changed')
    selector = selectors.DefaultSelector()
    streams = {'stdout': bytearray(), 'stderr': bytearray()}
    record = {'kind': 'fixed-kernel-profile-inventory-read', 'phase': phase,
        'pgid': None, 'terminal': False, 'returncode': None, 'group_absent': False,
        'data_complete': False, 'own_profile_matches': None}
    state['profile_read_operations'].append(record)
    profile_state_write(root, state)  # Durable pending intent BEFORE privileged launch.
    started = time.monotonic()
    proc = subprocess.Popen(PROFILE_READ_ARGV, env=ENV, stdout=subprocess.PIPE,
        stderr=subprocess.PIPE, start_new_session=True, preexec_fn=limits)
    try:
        record['pgid'] = proc.pid
        profile_state_write(root, state)
        for label, pipe in (('stdout', proc.stdout), ('stderr', proc.stderr)):
            os.set_blocking(pipe.fileno(), False)
            selector.register(pipe, selectors.EVENT_READ, label)
        while selector.get_map() or proc.poll() is None:
            # Cleanup reads do not inherit the failed operation's deadline/stop flag.
            need(time.monotonic() - started <= 15, 'privileged reader terminal state unverified')
            for key, _ in selector.select(0.05):
                data = os.read(key.fd, 65536)
                if not data:
                    selector.unregister(key.fileobj)
                    continue
                cap = 1024**2 + 1 if key.data == 'stdout' else 8192
                need(len(streams[key.data]) + len(data) <= cap, 'privileged reader stream bound')
                streams[key.data].extend(data)
        record['returncode'] = proc.wait(timeout=1)
        record['terminal'] = True
        record['group_absent'] = group_absent(proc.pid)
        need(record['returncode'] == 0, 'fixed profile-state read returned nonzero')
        need(record['group_absent'], 'privileged reader group remains')
        matches = parse_profile_inventory(bytes(streams['stdout']), name)
        record['data_complete'] = True
        record['own_profile_matches'] = {'absent': not matches, 'matching_label_count': len(matches),
            'exact_expected_label': matches == [name + ' (unconfined)']}
        return matches
    finally:
        # Root read child is observed, never killed through extra sudo authority.
        record['returncode'] = proc.poll()
        record['terminal'] = record['returncode'] is not None
        record['group_absent'] = group_absent(proc.pid)
        record['elapsed_seconds'] = time.monotonic() - started
        record['stream_bytes'] = {key: len(value) for key, value in streams.items()}
        record['stream_sha256'] = {key: digest(value) for key, value in streams.items()}
        selector.close()
        for pipe in (proc.stdout, proc.stderr):
            pipe.close()
        profile_state_write(root, state)
        # Raw stdout/stderr remain only in these bounded transient memory buffers.


def matching_profiles(name, root, state, phase):
    return read_profile_inventory(root, state, name, phase)


def profile_present(name, root, state, phase):
    need(matching_profiles(name, root, state, phase) == [name + ' (unconfined)'],
         'exact profile name or mode mismatch')



def group_absent(pgid):
    try:
        os.killpg(pgid, 0)
        return False
    except ProcessLookupError:
        return True
    except PermissionError:
        return False


def same_uid_labelled(name):
    matches = []
    for entry in Path('/proc').iterdir():
        if not entry.name.isdigit():
            continue
        try:
            # Only same-host-UID tasks can be descendants under the checked NNP/zero-cap contract.
            if entry.stat().st_uid != os.getuid():
                continue
            label = read_small(entry / 'attr/current', 4096).decode().strip()
            if label == name + ' (unconfined)' or label.startswith(name + '//'):
                matches.append(int(entry.name))
        except FileNotFoundError:
            continue  # Task exited while sampled.
        except (PermissionError, OSError, UnicodeError):
            raise ValueError('potentially owned process label unreadable')
    return sorted(matches)


def profile_state_write(root, value):
    # Atomic progress journal; only this private local file contains literal runner paths.
    temporary = root / ('profile-state-' + secrets.token_hex(16) + '.next')
    with temporary.open('x') as stream:
        json.dump(value, stream, sort_keys=True)
        stream.write('\n')
    os.replace(temporary, root / 'profile-state.json')


def parser_call(root, state, operation):
    need(operation in ('add', 'remove'), 'unsupported profile operation')
    if operation == 'add':
        need(STOP_SIGNAL is None, 'stopped before profile add')
    name, body = profile_identity(root, state['nonce'])
    profile = root / 'temporary-userns.profile'
    safe(profile)
    need(profile.read_bytes() == body, 'profile body changed')
    need(state['profile_sha256'] == digest(body), 'profile identity mismatch')
    need(digest(bounded_public_read(Path(ABI_PATH), 1024**2)) == state['abi_sha256'],
         'existing ABI input changed')
    selector = selectors.DefaultSelector()
    streams = {'stdout': bytearray(), 'stderr': bytearray()}
    # No shell, no includes, no inherited parser config, no cache, no worker children.
    argv = ['/usr/bin/sudo', '-n', PARSER, '--config-file=/dev/null',
            '--' + operation, '--skip-cache', '--jobs=0', '--Werror', str(profile)]
    started = time.monotonic()
    record = {'operation': operation, 'pgid': None, 'terminal': False,
              'returncode': None, 'group_absent': False}
    state['parser_operations'].append(record)
    profile_state_write(root, state)  # Durable nonterminal launch intent closes add/remove race.
    if operation == 'add':
        need(STOP_SIGNAL is None, 'stopped before privileged add launch')
    proc = subprocess.Popen(argv, env=ENV, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                            start_new_session=True, preexec_fn=limits)
    try:
        record['pgid'] = proc.pid
        profile_state_write(root, state)  # Journal before checking any child result.
        for label, pipe in (('stdout', proc.stdout), ('stderr', proc.stderr)):
            os.set_blocking(pipe.fileno(), False)
            selector.register(pipe, selectors.EVENT_READ, label)
        while selector.get_map() or proc.poll() is None:
            # Cleanup deliberately ignores the setup deadline and STOP_SIGNAL.
            need(time.monotonic() - started <= 30, 'privileged parser terminal state unverified')
            for key, _ in selector.select(0.05):
                data = os.read(key.fd, 65536)
                if not data:
                    selector.unregister(key.fileobj)
                    continue
                need(len(streams[key.data]) + len(data) <= LIMIT, 'parser stream limit')
                streams[key.data].extend(data)
        record['returncode'] = proc.wait(timeout=1)
        record['terminal'] = True
        record['group_absent'] = group_absent(proc.pid)
        need(record['group_absent'], 'privileged parser group remains')
        need(record['returncode'] == 0, 'profile parser returned nonzero')
    finally:
        # Do not attempt unprivileged SIGKILL of a root parser, or add a sudo kill route.
        # Unknown terminal state is preserved and prohibits generation/success/removal races.
        record['returncode'] = proc.poll()
        record['terminal'] = record['returncode'] is not None
        record['group_absent'] = group_absent(proc.pid)
        record['elapsed_seconds'] = time.monotonic() - started
        record['stream_bytes'] = {key: len(value) for key, value in streams.items()}
        record['stream_sha256'] = {key: digest(value) for key, value in streams.items()}
        selector.close()
        for pipe in (proc.stdout, proc.stderr):
            pipe.close()
        profile_state_write(root, state)


def profile_add(root, state):
    name, body = profile_identity(root, state['nonce'])
    need(not matching_profiles(name, root, state, 'before-add'), 'profile name already exists')
    diagnostic_step(root, state, 'profile.same-uid-scan')
    need(not same_uid_labelled(name), 'profile label already in use')
    diagnostic_step(root, state, 'profile.policy-readback')
    state['policy_before'] = policy_readback()
    diagnostic_step(root, state, 'profile.parser-identity')
    safe(Path(PARSER))
    need(Path(PARSER).is_file() and os.access(PARSER, os.X_OK), 'existing profile parser unavailable')
    state['parser_sha256'] = digest(Path(PARSER).read_bytes())
    diagnostic_step(root, state, 'profile.abi-identity')
    state['abi_sha256'] = digest(bounded_public_read(Path(ABI_PATH), 1024**2))
    diagnostic_step(root, state, 'profile.tempfile-write')
    with (root / 'temporary-userns.profile').open('xb') as stream:
        stream.write(body)
    state['profile_sha256'] = digest(body)
    need(STOP_SIGNAL is None, 'stopped before committing profile add')
    state['add_attempted'] = True  # Errors/timeouts may still have loaded the policy.
    profile_state_write(root, state)
    diagnostic_step(root, state, 'profile.add')
    parser_call(root, state, 'add')
    profile_present(name, root, state, 'after-add')
    diagnostic_step(root, state, 'profile.post-add-identity')
    need((root / 'temporary-userns.profile').read_bytes() == body, 'profile body changed during add')
    need(digest(bounded_public_read(Path(ABI_PATH), 1024**2)) == state['abi_sha256'],
         'ABI input changed during add')
    need(policy_readback() == state['policy_before'], 'global policy changed during add')
    state['add_verified'] = True
    profile_state_write(root, state)


def verify_binary(path):
    safe(path)
    st = path.lstat()
    need(stat.S_ISREG(st.st_mode) and st.st_nlink == 1 and stat.S_IMODE(st.st_mode) == 0o755
         and st.st_size == BWRAP_BYTES and not os.listxattr(path), 'binary type or privilege metadata changed')
    need(digest(path.read_bytes()) == BWRAP_SHA, 'binary bytes changed')


def verify_entry(root, name):
    path = root / 'probe.json'
    safe(path)
    st = path.lstat()
    need(stat.S_ISREG(st.st_mode) and st.st_nlink == 1 and st.st_size <= LIMIT, 'unsafe entry receipt')
    value = json.loads(path.read_bytes())
    need(value['entry_security'] == {'profile_name': name, 'profile_mode': 'unconfined',
         'same_host_uid': True, 'no_new_privileges': True, 'effective_permitted_ambient_caps': 0},
         'entry privilege or profile proof mismatch')
    need(value['network_namespace_distinct'] is True and value['interfaces'] == ['lo']
         and value['ipv4_routes'] == 0 and value['ipv6_routes_loopback_or_unreachable_only'] is True
         and all(value['readonly_mounts'].get(key) is True for key in
                 ('source', 'git_dir', 'git_common_dir', 'probe_code', 'sandbox_binary')),
         'entry namespace or readonly proof mismatch')


def sandbox_entry(args):
    need(os.geteuid() == args.host_uid and os.getuid() == args.host_uid and args.host_uid != 0,
         'sandbox changed host user identity')
    status = dict(line.split(':', 1) for line in read_small(Path('/proc/self/status')).decode().splitlines()
                  if ':' in line)
    need(status['NoNewPrivs'].strip() == '1', 'sandbox no-new-privileges not established')
    need(all(int(status[key].strip(), 16) == 0 for key in ('CapEff', 'CapPrm', 'CapAmb')),
         'sandbox retains privilege capability')
    label = read_small(Path('/proc/self/attr/current'), 4096).decode().strip()
    need(label == args.profile_name + ' (unconfined)', 'inherited profile label mismatch')
    policy_readback()
    return {'profile_name': args.profile_name, 'profile_mode': 'unconfined',
            'same_host_uid': True, 'no_new_privileges': True, 'effective_permitted_ambient_caps': 0}


def signal_owned(record):
    pgid = record.get('pgid')
    need(isinstance(pgid, int) and pgid > 1, 'owned process group identity missing')
    if group_absent(pgid):
        record['group_absent'] = True
        return
    need(not record.get('group_absent'), 'previously empty process group identity reused')
    leader = Path('/proc') / str(pgid)
    try:
        value = read_small(leader / 'stat').decode().rsplit(')', 1)[1].split()
        need(value[19] == record.get('leader_start_ticks') and leader.stat().st_uid == os.getuid(),
             'owned process leader identity reused')
    except FileNotFoundError:
        # Leader exited. Only exact same-UID, inherited-profile group members are owned.
        members = []
        for entry in Path('/proc').iterdir():
            if not entry.name.isdigit():
                continue
            try:
                value = read_small(entry / 'stat').decode().rsplit(')', 1)[1].split()
                if int(value[2]) != pgid:
                    continue
                need(entry.stat().st_uid == os.getuid(), 'process group UID mismatch')
                label = read_small(entry / 'attr/current', 4096).decode().strip()
                need(label == record['profile_name'] + ' (unconfined)', 'orphan group label mismatch')
                members.append(int(entry.name))
            except FileNotFoundError:
                continue
        need(members or group_absent(pgid), 'remaining group ownership ambiguous')
    try:
        os.killpg(pgid, signal.SIGKILL)
    except ProcessLookupError:
        record['group_absent'] = True


def kill_owned(record):
    signal_owned(record)
    deadline = time.monotonic() + 5
    while not group_absent(record['pgid']) and time.monotonic() < deadline:
        time.sleep(0.05)
    record['group_absent'] = group_absent(record['pgid'])
    need(record['group_absent'], 'owned sandbox group remains')


def bounded_sandbox(root, state, argv, source, writable, phase, started):
    need(phase in ('capability', 'generation'), 'invalid sandbox phase')
    selector = selectors.DefaultSelector()
    counts = {'stdout': 0, 'stderr': 0}
    logs = {}
    proc = None
    record = {'phase': phase, 'pgid': None, 'returncode': None, 'group_absent': False,
              'profile_name': profile_identity(root, state['nonce'])[0]}
    state['sandbox_operations'].append(record)
    profile_state_write(root, state)  # A missing post-launch PID is uncertainty, never an empty group.
    try:
        for label in counts:
            logs[label] = (root / (phase + '-' + label + '.log')).open('xb')
        proc = subprocess.Popen(argv, cwd=source, env=ENV, stdout=subprocess.PIPE,
            stderr=subprocess.PIPE, start_new_session=True, preexec_fn=limits)
        record['pgid'] = proc.pid
        record['leader_start_ticks'] = read_small(Path('/proc') / str(proc.pid) / 'stat').decode().rsplit(')', 1)[1].split()[19]
        profile_state_write(root, state)
        for label, pipe in (('stdout', proc.stdout), ('stderr', proc.stderr)):
            os.set_blocking(pipe.fileno(), False)
            selector.register(pipe, selectors.EVENT_READ, label)
        while selector.get_map() or proc.poll() is None:
            need(STOP_SIGNAL is None, 'stop signal received')
            need(time.monotonic() - started <= WALL, 'wall-time limit')
            need(free(root) >= DISK_FLOOR, 'disk floor during sandbox')
            need(candidate_size(writable) <= OUTPUT_LIMIT, 'aggregate candidate-size limit')
            for key, _ in selector.select(0.05):
                data = os.read(key.fd, 65536)
                if not data:
                    selector.unregister(key.fileobj)
                    continue
                label = key.data
                logs[label].write(data[:LIMIT - counts[label]])
                counts[label] += len(data)
                need(counts[label] <= LIMIT, 'sandbox stream limit')
        record['returncode'] = proc.wait(timeout=1)
        need(record['returncode'] == 0, 'sandbox command returned nonzero')
    finally:
        errors = []
        if proc is not None:
            try:
                signal_owned(record)
            except ProcessLookupError:
                pass
            except BaseException:
                errors.append('sandbox kill failed')
            try:
                proc.wait(timeout=5)
            except BaseException:
                errors.append('sandbox terminal wait failed')
            record['returncode'] = proc.poll()
            try:
                kill_owned(record)
            except BaseException:
                errors.append('sandbox group cleanup unverified')
        try:
            selector.close()
        except BaseException:
            errors.append('selector close failed')
        for stream in logs.values():
            try:
                stream.close()
            except BaseException:
                errors.append('stream close failed')
        if proc is not None:
            for pipe in (proc.stdout, proc.stderr):
                try:
                    pipe.close()
                except BaseException:
                    errors.append('pipe close failed')
        record['stream_bytes'] = counts
        record['cleanup_errors'] = errors
        profile_state_write(root, state)
        need(not errors and record['returncode'] is not None and record['group_absent'],
             'sandbox terminal cleanup failed')
    need(STOP_SIGNAL is None and time.monotonic() - started <= WALL, 'terminal sandbox stop or time limit')
    need(free(root) >= DISK_FLOOR and candidate_size(writable) <= OUTPUT_LIMIT, 'terminal sandbox bounds')


def verify_generation(root):
    need(sorted(p.name for p in root.iterdir()) == sorted(('probe.json', 'products')),
         'generation parent roster mismatch')
    products = root / 'products'
    safe(products)
    need(products.is_dir() and sorted(p.name for p in products.iterdir()) == sorted(GENERATOR_NAMES),
         'closed candidate product roster mismatch')
    rows = []
    for name in GENERATOR_NAMES:
        path = products / name
        safe(path)
        st = path.lstat()
        need(stat.S_ISREG(st.st_mode) and st.st_nlink == 1 and not st.st_mode & 0o111
             and st.st_size <= LIMIT, 'unsafe candidate output')
        raw = path.read_bytes()
        rows.append({'path': name, 'bytes': len(raw), 'sha256': digest(raw)})
    receipt = json.loads((products / GENERATOR_NAMES[3]).read_bytes())
    need(receipt['status'] == 'candidate-only-awaiting-independent-review'
         and receipt['compiler_head'] == C and receipt['compiler_tree'] == T and receipt['adapter_head'] == A
         and receipt['recipe']['sha256'] == RECIPE_SHA and receipt['outputs'] == rows[:3]
         and receipt['activated'] is False and receipt['execution_qualified'] is False
         and receipt['compiler_invocations'] == 0 and receipt['source_materializations'] == 0,
         'candidate receipt mismatch')
    need(rows[2]['bytes'] == 752 and rows[2]['sha256'] ==
         '3b123cfa29136244e62038e885761e68858d2b9b472bc14231d2e7a60a13586d',
         'candidate transition patch changed')
    need(candidate_size(root) <= OUTPUT_LIMIT, 'candidate aggregate limit')
    return rows


def cleanup_profile(root, state):
    result = {'schema': 'oxid-once-profile-cleanup-v2', 'status': 'unverified',
        'scope': 'owned-sandbox-groups-and-same-host-uid-profile-labelled-tasks',
        'global_process_absence_claimed': False, 'profile_absent': False,
        'profile_add_attempted': state['add_attempted'], 'profile_add_verified': state['add_verified'],
        'profile_removal_attempted': any(r['operation'] == 'remove' for r in state['parser_operations']),
        'no_policy_change_attempted': not state['add_attempted'] and not state['parser_operations'],
        'profile_sha256': state['profile_sha256'], 'abi_sha256': state['abi_sha256'],
        'parser_sha256': state['parser_sha256'],
        'reader_sha256': state['reader_sha256'],
        'profile_read_operations': state['profile_read_operations'],
        'failure_detail': None, 'remaining_owned_groups': None, 'remaining_same_uid_profile_pids': None,
        'parser_operations': state['parser_operations'], 'failure': None}
    try:
        diagnostic_step(root, state, 'cleanup.owned-groups')
        need(all(r.get('pgid') is not None for r in state['sandbox_operations']),
             'sandbox launch identity unverified')
        for record in state['sandbox_operations']:
            kill_owned(record)
        result['remaining_owned_groups'] = [r['pgid'] for r in state['sandbox_operations']
                                            if r['pgid'] and not group_absent(r['pgid'])]
        need(not result['remaining_owned_groups'], 'owned groups remain before profile removal')
        name, body = profile_identity(root, state['nonce'])
        diagnostic_step(root, state, 'cleanup.same-uid-scan')
        remaining = same_uid_labelled(name)
        result['remaining_same_uid_profile_pids'] = remaining
        need(not remaining, 'same-UID profile-labelled processes remain')
        # Do not race an uncertain privileged --add or a previous --remove.
        diagnostic_step(root, state, 'cleanup.privileged-terminal')
        need(readers_terminal(state), 'privileged reader terminal state unverified')
        need(all(r['terminal'] and r['group_absent'] and group_absent(r['pgid'])
                 for r in state['parser_operations']), 'privileged parser terminal state unverified')
        matches = matching_profiles(name, root, state, 'cleanup-before-remove')
        if state['add_attempted'] and matches:
            need(matches == [name + ' (unconfined)'], 'exact profile name or mode mismatch')
            need(not any(r['operation'] == 'remove' for r in state['parser_operations']),
                 'no automatic repeat profile removal')
            need(digest(Path(PARSER).read_bytes()) == state['parser_sha256'], 'profile parser changed')
            diagnostic_step(root, state, 'cleanup.remove')
            parser_call(root, state, 'remove')
        need(not matching_profiles(name, root, state, 'cleanup-after-remove'), 'profile remains after cleanup')
        result['profile_absent'] = True
        diagnostic_step(root, state, 'cleanup.policy-readback')
        result['policy_after'] = policy_readback()
        need(result['policy_after'] == state['policy_before'], 'global policy changed')
        result['status'] = 'verified'
    except BaseException as error:
        result['failure_detail'] = diagnostic(error, state)
        result['failure'] = result['failure_detail']['reason']
    finally:
        result['parser_operations'] = state['parser_operations']
        result['profile_read_operations'] = state['profile_read_operations']
        result['reader_sha256'] = state['reader_sha256']
        result['profile_removal_attempted'] = any(r['operation'] == 'remove' for r in state['parser_operations'])
        profile_state_write(root, state)
    return result


def cleanup_again(root):
    # Independent always-step. Never loads a profile or launches package/sandbox/generator work.
    safe(root)
    need(root.is_dir(), 'attempt directory missing; cleanup unverified')
    state_path = root / 'profile-state.json'
    safe(state_path)
    state = json.loads(read_small(state_path, 65536))
    need(state['schema'] == 'oxid-once-private-state-v2' and state['host_uid'] == os.getuid(),
         'wrong cleanup ownership')
    need(state['root'] == str(root) and state['head'] == os.environ.get('GITHUB_SHA'),
         'wrong cleanup attempt identity')
    name, body = profile_identity(root, state['nonce'])
    need(state['profile_sha256'] in (None, digest(body)), 'wrong cleanup profile identity')
    result = cleanup_profile(root, state)
    save(root, 'always-cleanup.json', result)
    return 0 if result['status'] == 'verified' else 1


def host(args):
    for signum in (signal.SIGINT, signal.SIGTERM):
        signal.signal(signum, stop)
    safe(args.repository)
    safe(args.output)
    need(os.geteuid() == os.getuid() and os.getuid() != 0, 'ordinary host UID required')
    need(os.environ.get('GITHUB_REPOSITORY') == 'YanagiKH/Oxid' and
         os.environ.get('GITHUB_REF') == 'refs/heads/' + PROFILE_BRANCH and
         os.environ.get('GITHUB_RUN_ATTEMPT') == '1', 'wrong one-shot repository/ref/attempt')
    event = json.loads(read_small(Path(os.environ['GITHUB_EVENT_PATH']), 1024**2))
    need(os.environ.get('GITHUB_EVENT_NAME') == 'push' and event.get('created') is True
         and event.get('before') == '0' * 40 and event.get('after') == args.expected_head
         and event.get('ref') == 'refs/heads/' + PROFILE_BRANCH and not event.get('deleted')
         and event.get('repository', {}).get('full_name') == 'YanagiKH/Oxid',
         'only first named branch creation is authorized')
    need(re.fullmatch('[0-9]+', os.environ.get('GITHUB_RUN_ID', '')) is not None, 'missing run identity')
    expected_root = Path(os.environ['RUNNER_TEMP']) / ('oxid-package-source-once-' + os.environ['GITHUB_RUN_ID'] + '-1')
    need(args.output == expected_root, 'wrong exclusive attempt root')
    need(not args.output.exists() and args.output.parent.is_dir(), 'fresh attempt required')
    for path in (args.repository, args.output):
        need(not any(path == base or base in path.parents for base in
                     (Path('/tmp'), Path('/proc'), Path('/dev'))), 'path hidden by sandbox mount')
    args.output.mkdir()
    source = args.output / 'source-view'
    start = time.monotonic()
    before, bwrap = None, None
    status, failure, stage = 'failed', None, 'preflight'
    failure_detail = None
    generation_attempted, products = False, None
    state = {'schema': 'oxid-once-private-state-v2', 'root': str(args.output),
        'head': args.expected_head, 'host_uid': os.getuid(), 'nonce': secrets.token_hex(16),
        'profile_sha256': None, 'parser_sha256': None, 'abi_sha256': None, 'add_attempted': False,
        'add_verified': False, 'policy_before': None, 'parser_operations': [], 'sandbox_operations': [],
        'profile_read_operations': [], 'reader_sha256': None, 'diagnostic_operation': 'preflight'}
    profile_state_write(args.output, state)
    cleanup = None
    try:
        need(STOP_SIGNAL is None, 'stopped before launch')
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
        state['policy_before'] = policy_readback()
        stage = 'official-userspace-binary-setup'
        bwrap = prepare_official_binary(args.output)
        need(bwrap.is_file() and os.access(bwrap, os.X_OK), 'prepared bwrap unavailable')
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
        name, body = profile_identity(args.output, state['nonce'])
        save(args.output, 'invocation.json', {'schema': 'oxid-hosted-once-invocation-v1',
            'workflow_head': args.expected_head, 'source_head': C, 'source_tree': T,
            'adapter_head': A, 'adapter_tree': AT, 'recipe_sha256': RECIPE_SHA,
            'supervisor_sha256': digest(Path(__file__).read_bytes()),
            'binary_sha256': BWRAP_SHA, 'profile_name': name, 'profile_sha256': digest(body),
            'profile_attachment_role': 'ATTEMPT/tools/bwrap', 'profile_attachment_is_hash_bound': False,
            'profile_privilege_scope': 'exact-path-and-inheriting-descendants-single-ephemeral-job',
            'sandbox_flags': ['--die-with-parent', '--unshare-net', '--ro-bind / /',
                             '--bind PHASE_OUTPUT PHASE_OUTPUT', '--tmpfs /tmp', '--proc /proc', '--dev /dev'],
            'wall_seconds': WALL, 'wall_scope': 'setup-plus-two-phases-monitored-terminally-checked',
            'cleanup_separate_budget_seconds_per_parser_operation': 30,
            'address_space_bytes': 2 * 1024**3, 'rlimit_scope': 'sandbox-and-setup-child-processes',
            'stream_and_file_limit_bytes': LIMIT, 'candidate_aggregate_limit_bytes': OUTPUT_LIMIT,
            'disk_floor_bytes': DISK_FLOOR, 'candidate_only': True,
            'execution_qualified': False, 'source_review_reference': 'https://github.com/YanagiKH/Oxid/commit/' + C,
            'adapter_review_reference': 'https://github.com/YanagiKH/Oxid/commit/' + A})
        need(STOP_SIGNAL is None and time.monotonic() - start <= WALL, 'stopped or timed out before profile add')
        stage = 'temporary-profile-add'
        profile_add(args.output, state)
        for phase in ('capability', 'generation'):
            stage = phase
            need(STOP_SIGNAL is None and time.monotonic() - start <= WALL, 'stopped or timed out before phase')
            need(free(args.output) >= DISK_FLOOR, 'disk floor before phase')
            need(snapshot(source) == before, 'source changed before phase')
            verify_binary(bwrap)
            profile_present(name, args.output, state, 'before-' + phase)
            need(policy_readback() == state['policy_before'], 'global policy changed before phase')
            writable = args.output / (phase + '-output')
            writable.mkdir()
            argv = [str(bwrap), '--die-with-parent', '--unshare-net', '--ro-bind', '/', '/',
                '--bind', str(writable), str(writable), '--tmpfs', '/tmp', '--proc', '/proc',
                '--dev', '/dev', '--', '/usr/bin/python3', '-B', str(Path(__file__).absolute()),
                '--inside', '--repository', str(source), '--output', str(writable),
                '--git-dir', str(stores[0]), '--git-common-dir', str(stores[1]),
                '--host-net', os.readlink('/proc/self/ns/net'), '--bwrap', str(bwrap),
                '--profile-name', name, '--host-uid', str(os.getuid())]
            if phase == 'generation':
                # A completed capability child and empty owned group are mandatory prerequisites.
                closed(args.output / 'capability-output')
                need(state['sandbox_operations'][0]['returncode'] == 0
                     and state['sandbox_operations'][0]['group_absent'], 'capability not terminally clear')
                argv.append('--generate')
                generation_attempted = True
            bounded_sandbox(args.output, state, argv, source, writable, phase, start)
            verify_entry(writable, name)
            if phase == 'capability':
                closed(writable)
            else:
                products = verify_generation(writable)
            need(snapshot(source) == before, 'source changed after phase')
        status = 'candidate-only-awaiting-independent-review'
    except BaseException as error:
        failure_detail = diagnostic(error, state)
        failure = failure_detail['reason']
    finally:
        # Teardown is independent of STOP_SIGNAL, setup deadline and generation success.
        cleanup = cleanup_profile(args.output, state)
        save(args.output, 'profile-cleanup.json', cleanup)
        if cleanup['status'] != 'verified':
            status, failure = 'failed', (failure or '') + '; profile cleanup unverified'
        if before is not None:
            try:
                after = snapshot(source)
                save(args.output, 'source-after.json', after)
                need(after == before, 'source before/after mismatch')
            except BaseException:
                status, failure = 'failed', (failure or '') + '; source recheck failed'
        try:
            need(STOP_SIGNAL is None, 'stop signal received')
            need(time.monotonic() - start <= WALL + 60, 'terminal operation and cleanup budget')
            need(free(args.output) >= DISK_FLOOR, 'terminal disk floor')
            if bwrap is not None:
                verify_binary(bwrap)
            if status == 'candidate-only-awaiting-independent-review':
                closed(args.output / 'capability-output')
                need(verify_generation(args.output / 'generation-output') == products, 'terminal output mismatch')
        except BaseException as error:
            status, failure = 'failed', (failure or '') + '; ' + (str(error) if type(error) is GuardError else type(error).__name__)
        save(args.output, 'guard-result.json', {'schema': 'oxid-hosted-once-guard-v2',
            'status': status, 'failure': failure, 'failure_detail': failure_detail, 'stage': stage,
            'elapsed_seconds': time.monotonic() - start, 'stop_signal': STOP_SIGNAL,
            'sandbox_operations': state['sandbox_operations'],
            'generation_invocation_attempted': generation_attempted,
            'candidate_products_verified': products is not None,
            'profile_cleanup_verified': cleanup['status'] == 'verified',
            'activated': False, 'execution_qualified': False, 'historical_preflight': 'not-run'})
    return 0 if status == 'candidate-only-awaiting-independent-review' else 1

def stage_upload(root, output):
    safe(root)
    safe(output)
    need(not output.exists() and output.parent.is_dir(), 'fresh upload directory required')
    output.mkdir()
    entries = []
    for name in ROSTER:
        relative = {'capability.json': 'capability-output/probe.json',
                    'generation-entry.json': 'generation-output/probe.json'}
        source = root / (('generation-output/products/' + name) if name in GENERATOR_NAMES
                         else relative.get(name, name))
        if not source.exists():
            entries.append({'name': name, 'status': 'not-produced'})
            continue
        safe(source)
        st = source.lstat()
        need(stat.S_ISREG(st.st_mode) and st.st_nlink == 1 and st.st_size <= LIMIT,
             'unsafe evidence member')
        raw = source.read_bytes()
        # Logs contain only reviewed supervisor/bwrap/generator diagnostics. Replace actual run paths before upload.
        if name.endswith('.log'):
            text = raw.decode('utf-8', errors='replace')
            for p in (str(root), os.environ.get('GITHUB_WORKSPACE', ''), os.environ.get('RUNNER_TEMP', '')):
                if p:
                    text = text.replace(p, '<HOSTED_PATH>')
            raw = text.encode()
        with (output / name).open('xb') as stream:
            stream.write(raw)
        entries.append({'name': name, 'status': 'preserved', 'bytes': len(raw), 'sha256': digest(raw)})
    save(output, 'upload-manifest.json', {'schema': 'oxid-hosted-once-upload-v1', 'files': entries,
        'candidate_only': True, 'missing_evidence_is_not_success': True})
    need((output / 'guard-result.json').is_file(), 'mandatory guard result missing')


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--repository', type=Path)
    parser.add_argument('--output', type=Path)
    parser.add_argument('--expected-head')
    parser.add_argument('--inside', action='store_true')
    parser.add_argument('--generate', action='store_true')
    parser.add_argument('--profile-name')
    parser.add_argument('--host-uid', type=int)
    parser.add_argument('--cleanup', type=Path)
    parser.add_argument('--git-dir', type=Path)
    parser.add_argument('--git-common-dir', type=Path)
    parser.add_argument('--host-net')
    parser.add_argument('--bwrap', type=Path)
    parser.add_argument('--stage', type=Path)
    parser.add_argument('--upload', type=Path)
    args = parser.parse_args()
    if args.cleanup:
        return cleanup_again(args.cleanup)
    if args.stage:
        need(args.upload is not None, 'upload path required')
        stage_upload(args.stage, args.upload)
        return 0
    need(args.repository is not None and args.output is not None, 'source and output required')
    if args.inside:
        need(args.git_dir is not None and args.git_common_dir is not None and args.host_net and args.bwrap is not None
             and args.profile_name is not None and args.host_uid is not None,
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
        print('One-shot generation operation failed: ' + type(error).__name__, file=sys.stderr)
        sys.exit(1)
