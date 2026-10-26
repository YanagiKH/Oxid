"""Shared, source-preserving command capture for native data-pipeline checks."""
import hashlib
import json
import os
from pathlib import Path
import resource
import shutil
import subprocess
import time

import linux_native_target

ROOT = Path(__file__).resolve().parents[1]


def sha(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def native_limits():
    resource.setrlimit(resource.RLIMIT_AS, (33554432, 33554432))
    resource.setrlimit(resource.RLIMIT_NOFILE, (32, 32))


class Verification:
    def __init__(self, args, controllers, source_roots, scope):
        if not __debug__:
            raise ValueError('verification requires Python assertions')
        self.host = linux_native_target.required_host()
        self.output = args.output.resolve()
        if self.output.exists() or self.output == ROOT or ROOT in self.output.parents:
            raise ValueError('a new output directory outside the repository is required')
        self.compilers = [p.resolve(strict=True) for p in args.compilers]
        if len(set(self.compilers)) != len(self.compilers) or not all(os.access(p, os.X_OK) for p in self.compilers):
            raise ValueError('distinct executable compilers are required')
        self.llvm = args.llvm_bin.resolve(strict=True)
        self.output.mkdir(parents=True)
        self.sources = {}
        for root in source_roots:
            for path in (ROOT/root).rglob('*'):
                if path.is_file() and path.suffix in {'.ox', '.toml', '.md'} and '.oxid' not in path.parts:
                    self.sources[path.relative_to(ROOT).as_posix()] = sha(path)
        self.report = {'status': 'running', 'target': self.host.triple, 'scope': scope,
                       'controllers': {name: sha(ROOT/'scripts'/name) for name in controllers},
                       'sources': self.sources,
                       'compilers': [{'path': str(p), 'bytes': p.stat().st_size, 'sha256': sha(p)} for p in self.compilers],
                       'commands': [], 'deployments': [], 'cases': [], 'source_packages': []}
        self.save()

    def save(self):
        (self.output/'result.json').write_text(json.dumps(self.report, indent=2, allow_nan=False)+'\n')

    def command(self, name, argv, cwd, status=0, native=False):
        folder = self.output/'commands'/name
        folder.mkdir(parents=True)
        environment = {} if native else dict(os.environ, OXID_LLVM_BIN=str(self.llvm),
                                             OXID_CACHE_DIR=str(self.output/'legacy-cache'))
        environment.pop('OXID_PATH', None)
        started = time.monotonic()
        completed = subprocess.run([str(part) for part in argv], cwd=cwd, env=environment,
                                   stdin=subprocess.DEVNULL, capture_output=True, timeout=300,
                                   preexec_fn=native_limits if native else None)
        for stream in ['stdout', 'stderr']:
            (folder/stream).write_bytes(getattr(completed, stream))
        self.report['commands'].append({'name': name, 'argv': [str(part) for part in argv], 'cwd': str(cwd),
                                        'exit_status': completed.returncode, 'expected_status': status,
                                        'stdout_sha256': sha(folder/'stdout'), 'stderr_sha256': sha(folder/'stderr'),
                                        'seconds': time.monotonic()-started,
                                        'address_space_limit': 33554432 if native else None,
                                        'open_file_limit': 32 if native else None})
        self.save()
        if completed.returncode != status or completed.stderr:
            raise ValueError(name+': unexpected command result')
        return completed

    def snapshot(self, destination):
        for name, digest in self.sources.items():
            target = destination/name
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copy2(ROOT/name, target)
            if sha(target) != digest:
                raise ValueError('source changed during snapshot: '+name)

    def deploy(self, source, name, profile):
        binary = self.output/'deployed'/name/profile/source.name
        binary.parent.mkdir(parents=True)
        shutil.copy2(source, binary)
        body = binary.read_bytes()
        if body[:6] != b'\x7fELF\x02\x01' or int.from_bytes(body[18:20], 'little') != self.host.elf_machine:
            raise ValueError('wrong native target')
        return binary

    def deployed(self, binary, name, profile):
        self.report['deployments'].append({'path': str(binary), 'sha256': sha(binary),
                                           'case': name, 'profile': profile, 'source_removed': True,
                                           'elf_machine': self.host.elf_machine})
        self.save()

    def finish(self):
        for name, digest in self.sources.items():
            if sha(ROOT/name) != digest:
                raise ValueError('source changed during verification: '+name)
        for name, digest in self.report['controllers'].items():
            if sha(ROOT/'scripts'/name) != digest:
                raise ValueError('controller changed during verification: '+name)
        for pin in self.report['compilers']:
            if sha(pin['path']) != pin['sha256']:
                raise ValueError('compiler changed during verification')
        self.report['status'] = 'passed'
        self.save()

    def failed(self, error):
        self.report.update(status='failed', failure=f'{type(error).__name__}: {error}')
        self.save()
