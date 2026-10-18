"""NEW bounded read-use adapter. No source authority, builds, or global patching.

Caller authenticates this module, the native core, and immutable authority inputs.
Native core is retained separately for initial/final checkout and layout scans.
All content-consuming validation must receive this private checked facade.
"""
from dataclasses import dataclass
import hashlib
import io
import os
from pathlib import Path as NativePath, PurePath
import re
import stat
from types import MappingProxyType, ModuleType


class ReadBoundaryError(ValueError):
    pass


def need(ok, message):
    if not ok:
        raise ReadBoundaryError(message)


def absolute(value):
    path = PurePath(str(value))
    need(path.is_absolute() and '..' not in path.parts, 'exact absolute authority path required')
    return str(path)


def joined(root, value):
    need(type(value) is str and value and not PurePath(value).is_absolute()
         and '\\' not in value and all(p not in ('', '.', '..') for p in value.split('/')),
         'canonical authority-relative path required')
    return absolute(PurePath(root) / value)


@dataclass(frozen=True)
class Descriptor:
    path: str
    authority: str
    kind: str
    mode: object
    size: object = None
    sha256: object = None
    target: object = None


def file_descriptor(path, row, authority):
    need(type(row) is dict, 'file descriptor must be an object')
    size, digest, mode = row.get('bytes'), row.get('sha256'), row.get('mode')
    need(type(size) is int and size >= 0, 'invalid descriptor byte count')
    need(type(digest) is str and re.fullmatch('[0-9a-f]{64}', digest), 'invalid descriptor digest')
    need((type(mode) is int and 0 <= mode <= 0o7777)
         or (authority == 'source' and type(mode) is str and mode in ('100644', '100755')),
         'invalid descriptor mode')
    return Descriptor(path, authority, 'file', mode, size, digest)


@dataclass(frozen=True, init=False)
class SourceAuthority:
    root: str
    descriptors: tuple
    directories: tuple

    def __init__(self, root, entries, *, directories=(), module_files=()):
        root = absolute(root)
        descriptors = []
        for row in tuple(entries):
            need(type(row) is dict, 'source descriptor must be an object')
            descriptors.append(file_descriptor(joined(root, row.get('path')), dict(row), 'source'))
        for row in tuple(module_files):
            need(type(row) is dict, 'module descriptor must be an object')
            descriptors.append(file_descriptor(absolute(row.get('path')), dict(row), 'source'))
        paths = [d.path for d in descriptors]
        need(len(paths) == len(set(paths)), 'duplicate source/module descriptor')
        allowed = tuple(absolute(p) for p in tuple(directories))
        need(len(allowed) == len(set(allowed)), 'duplicate source directory authority')
        for directory in allowed:
            need(directory == root or PurePath(root) in PurePath(directory).parents,
                 'source directory enumeration outside authenticated checkout root')
            need(any(PurePath(directory) in PurePath(p).parents for p in paths),
                 'source directory has no authenticated descendants')
        object.__setattr__(self, 'root', root)
        object.__setattr__(self, 'descriptors', tuple(descriptors))
        object.__setattr__(self, 'directories', allowed)


@dataclass(frozen=True, init=False)
class EvidenceAuthority:
    root: str
    descriptors: tuple

    def __init__(self, root, entries):
        root = absolute(root)
        descriptors = []
        for row in tuple(entries):
            need(type(row) is dict, 'evidence descriptor must be an object')
            row = dict(row)
            path, kind = joined(root, row.get('path')), row.get('kind')
            if kind == 'file':
                descriptor = file_descriptor(path, row, 'evidence')
            elif kind == 'directory':
                mode = row.get('mode')
                need(type(mode) is int and 0 <= mode <= 0o7777, 'invalid evidence directory mode')
                descriptor = Descriptor(path, 'evidence', kind, mode)
            elif kind == 'symlink':
                target = absolute(row.get('target'))
                descriptor = Descriptor(path, 'evidence', kind, None, target=target)
            else:
                raise ReadBoundaryError('unsupported evidence descriptor kind')
            descriptors.append(descriptor)
        need(len(descriptors) == len({d.path for d in descriptors}), 'duplicate evidence descriptor')
        object.__setattr__(self, 'root', root)
        object.__setattr__(self, 'descriptors', tuple(descriptors))


class ViewPath:
    """Finite lexical/readonly interface; no inherited native I/O methods."""
    __slots__ = ('_view', '_pure')

    def __init__(self, view, pure):
        object.__setattr__(self, '_view', view)
        object.__setattr__(self, '_pure', pure)

    def __setattr__(self, name, value):
        raise TypeError('immutable view path')

    def __getattr__(self, name):
        raise PermissionError('unsupported view path operation: ' + name)

    def __fspath__(self):
        raise PermissionError('native filesystem escape from view path')

    def __str__(self):
        return str(self._pure)

    def __repr__(self):
        return 'ViewPath(' + repr(str(self)) + ')'

    def __hash__(self):
        return hash(self._pure)

    def __eq__(self, other):
        if isinstance(other, ViewPath):
            return self._pure == other._pure
        if isinstance(other, PurePath):
            return self._pure == other
        return NotImplemented

    def __truediv__(self, other):
        return self._view.path(self._pure / self._view._lexical(other))

    def __rtruediv__(self, other):
        return self._view.path(self._view._lexical(other) / self._pure)

    @property
    def name(self): return self._pure.name
    @property
    def parts(self): return self._pure.parts
    @property
    def parent(self): return self._view.path(self._pure.parent)
    @property
    def parents(self): return tuple(self._view.path(p) for p in self._pure.parents)

    def is_absolute(self): return self._pure.is_absolute()
    def as_posix(self): return self._pure.as_posix()
    def relative_to(self, other): return self._view.path(self._pure.relative_to(self._view._lexical(other)))
    def read_bytes(self): return self._view.regular(self)

    def read_text(self, encoding=None, errors=None):
        raw = self._view.regular(self)
        # Match pathlib's locale/UTF-8-mode default and universal newlines using
        # the same checked bytes; never reopen the underlying pathname.
        with io.TextIOWrapper(io.BytesIO(raw), encoding=io.text_encoding(encoding),
                              errors=errors, newline=None) as stream:
            return stream.read()

    def stat(self): return self._view._stat(self, allow_alias=False)
    def exists(self): return self._view._predicate(self, 'exists')
    def is_file(self): return self._view._predicate(self, 'file')
    def is_dir(self): return self._view._predicate(self, 'directory')
    def is_symlink(self): return self._view._predicate(self, 'symlink')
    def readlink(self): return self._view._readlink(self)
    def iterdir(self): return iter(self._view._iterdir(self))
    def rglob(self, pattern): return iter(self._view._rglob(self, pattern))

    def resolve(self, strict=False):
        need(type(strict) is bool and self.is_absolute(), 'relative resolution is not admitted')
        self._view._stat(self, allow_alias=False)
        return self


def _fixed_children(descriptor_names, metadata_names, enumeration):
    # Pure index over caller-admitted names only. No filesystem observations.
    children = {name: set() for name in enumeration}
    for name in set(descriptor_names) | set(metadata_names):
        path = PurePath(name)
        parent = str(path.parent)
        if parent in children and name != parent:
            children[parent].add(path.name)
    return MappingProxyType({name: frozenset(values) for name, values in children.items()})


class FreshReadView:
    """Fresh content checks against independently supplied immutable descriptors."""
    def __init__(self, core, *, source_authority, evidence_authority, known_absent=frozenset()):
        need(type(source_authority) is SourceAuthority and type(evidence_authority) is EvidenceAuthority,
             'explicit frozen source and evidence authorities required')
        # Capture native primitives before any facade exists. No facade exports
        # these bound functions, and no native module/global is modified.
        self._native_read = core.regular
        self._loads = core.loads
        self._stamp = core.stamp
        self._ancestor = core.ancestor
        self._windows = core.windows_host
        self._root = evidence_authority.root
        source_root, evidence_root = PurePath(source_authority.root), PurePath(self._root)
        need(source_root != evidence_root and source_root not in evidence_root.parents
             and evidence_root not in source_root.parents, 'source/evidence authority roots overlap')
        descriptors = {}
        for descriptor in source_authority.descriptors + evidence_authority.descriptors:
            need(descriptor.path not in descriptors, 'source/evidence authority overlap')
            need(descriptor.authority != 'source' or evidence_root not in PurePath(descriptor.path).parents,
                 'source/module descriptor inside process evidence')
            descriptors[descriptor.path] = descriptor
        # Metadata-only ancestors do not acquire body-read or recursive access.
        metadata = {str(p) for name in descriptors for p in PurePath(name).parents}
        metadata.add(source_authority.root)
        metadata.add(self._root)
        metadata.update(str(p) for root in (source_root, evidence_root) for p in root.parents)
        enumeration = set(source_authority.directories) | {self._root}
        enumeration.update(d.path for d in evidence_authority.descriptors if d.kind == 'directory')
        need(not (set(descriptors) & metadata - {p for p, d in descriptors.items() if d.kind == 'directory'}),
             'non-directory authority is also an ancestor')
        absent = frozenset(absolute(p) for p in tuple(known_absent))
        for name in absent:
            need(name not in descriptors and name not in metadata and PurePath(self._root) in PurePath(name).parents
                 and str(PurePath(name).parent) in metadata | enumeration,
                 'invalid or unknown-parent negative descriptor')
        self._descriptors = MappingProxyType(dict(descriptors))
        self._metadata_only = frozenset(metadata - set(descriptors))
        self._enumeration = frozenset(enumeration)
        self._absent = absent
        self._children = _fixed_children(descriptors, metadata, enumeration)
        # Snapshot only metadata of admitted entries and their ancestors. No
        # current directory scan or body read can enlarge the descriptor set.
        snapshots, parents = {}, {}
        for name in sorted(metadata | set(descriptors)):
            path = NativePath(name)
            info = path.lstat()
            descriptor = descriptors.get(name)
            if descriptor is None or descriptor.kind == 'directory':
                need(stat.S_ISDIR(info.st_mode) and not self._alias(info), 'aliased/non-directory descriptor ancestor')
                if descriptor is not None:
                    self._mode(descriptor, info)
                parents[name] = self._directory_identity(info)
            else:
                self._kind(descriptor, info)
                self._mode(descriptor, info)
                if descriptor.kind == 'symlink':
                    need(str(path.readlink()) == descriptor.target and descriptor.target in metadata | set(descriptors),
                         'unapproved deliberate link target')
                snapshots[name] = self._stamp(info)
        self._snapshots = MappingProxyType(snapshots)
        self._parents = MappingProxyType(parents)
        for name in absent:
            self._check_absent(name)
        for name in enumeration:
            self._iterdir(self.path(name))

    @staticmethod
    def _alias(info):
        return stat.S_ISLNK(info.st_mode) or bool(getattr(info, 'st_file_attributes', 0) & 1024)

    def _directory_identity(self, info):
        """Validate and identify this exact observation, with no second lstat."""
        need(stat.S_ISDIR(info.st_mode) and not self._alias(info), 'aliased/non-directory observation')
        birth = None
        if self._windows():
            need(hasattr(info, 'st_birthtime_ns'), 'missing native Windows ancestor birthtime')
            birth = info.st_birthtime_ns
        return (info.st_dev, info.st_ino, info.st_mode, birth, getattr(info, 'st_file_attributes', 0))

    def _kind(self, descriptor, info):
        if descriptor.kind == 'symlink':
            need(stat.S_ISLNK(info.st_mode), 'deliberate link changed kind')
        else:
            need(not self._alias(info) and stat.S_ISREG(info.st_mode), 'regular descriptor became alias/nonregular')
            need(info.st_size == descriptor.size, 'sealed descriptor byte count changed')

    def _mode(self, descriptor, info):
        if type(descriptor.mode) is str:
            if not self._windows():
                need(('100755' if info.st_mode & 0o111 else '100644') == descriptor.mode, 'source index mode changed')
        elif descriptor.mode is not None:
            need(stat.S_IMODE(info.st_mode) == descriptor.mode, 'sealed native mode changed')

    def _lexical(self, value):
        if isinstance(value, ViewPath):
            need(value._view is self, 'foreign view path')
            return value._pure
        need(isinstance(value, (str, PurePath)), 'unsupported path input')
        return PurePath(value)

    def path(self, *parts):
        need(bool(parts), 'explicit path required; no ambient cwd')
        return ViewPath(self, PurePath(*(self._lexical(p) for p in parts)))

    def _key(self, path):
        return absolute(self._lexical(path))

    def _check_parents(self, name):
        for parent in PurePath(name).parents:
            key = str(parent)
            need(key in self._parents and self._ancestor(NativePath(key)) == self._parents[key],
                 'admitted ancestor changed')

    def _metadata_observation(self, name, observe):
        # This private helper does not admit paths or refresh any anchor.
        primary = None
        try:
            self._check_parents(name)
            return observe()
        except BaseException as error:
            primary = error
            raise
        finally:
            try:
                self._check_parents(name)
            except BaseException as boundary:
                if primary is not None:
                    raise BaseExceptionGroup('metadata observation and ancestor recheck failed',
                                             [primary, boundary]) from None
                raise

    def _check_absent(self, name):
        need(name in self._absent, 'unknown absence query')
        def observe():
            try:
                NativePath(name).lstat()
            except FileNotFoundError:
                return
            raise ReadBoundaryError('known-absent path appeared')
        return self._metadata_observation(name, observe)

    def _stat(self, path, *, allow_alias=True):
        name = self._key(path)
        need(name in self._descriptors or name in self._metadata_only, 'unknown path metadata query')
        def observe():
            native = NativePath(name)
            info = native.lstat()
            descriptor = self._descriptors.get(name)
            if name in self._parents:
                need(self._directory_identity(info) == self._parents[name], 'admitted directory changed')
            else:
                self._kind(descriptor, info)
                need(self._stamp(info) == self._snapshots[name], 'admitted file/link metadata changed')
            if descriptor is not None:
                self._mode(descriptor, info)
                if descriptor.kind == 'symlink':
                    need(allow_alias and str(native.readlink()) == descriptor.target, 'alias read or changed link target')
            return info
        return self._metadata_observation(name, observe)

    def regular(self, path):
        name = self._key(path)
        descriptor = self._descriptors.get(name)
        need(descriptor is not None and descriptor.kind == 'file', 'unknown or nonregular body read')
        self._stat(self.path(name), allow_alias=False)
        raw = self._native_read(NativePath(name))
        need(type(raw) is bytes and len(raw) == descriptor.size
             and hashlib.sha256(raw).hexdigest() == descriptor.sha256,
             'freshly consumed bytes differ from sealed descriptor')
        self._stat(self.path(name), allow_alias=False)
        return raw

    def load(self, path): return self._loads(self.regular(path))

    def _predicate(self, path, kind):
        name = self._key(path)
        if name in self._absent:
            self._check_absent(name)
            return False
        info = self._stat(path)
        if kind == 'exists': return True
        if kind == 'symlink': return stat.S_ISLNK(info.st_mode)
        if stat.S_ISLNK(info.st_mode):
            # Only approved real links reach here; their target metadata is
            # independently admitted. This does not permit reading via the link.
            info = self._stat(self.path(self._descriptors[name].target), allow_alias=False)
        return stat.S_ISREG(info.st_mode) if kind == 'file' else stat.S_ISDIR(info.st_mode)

    def _readlink(self, path):
        name = self._key(path)
        self._stat(path)
        descriptor = self._descriptors.get(name)
        need(descriptor is not None and descriptor.kind == 'symlink', 'readlink on non-admitted link')
        return self.path(descriptor.target)

    def _iterdir(self, path):
        name = self._key(path)
        need(name in self._enumeration, 'directory enumeration not authorized')
        self._stat(path, allow_alias=False)
        names = tuple(p.name for p in NativePath(name).iterdir())
        need(len(names) == len(set(names)) and frozenset(names) == self._children[name],
             'sealed directory membership changed')
        children = tuple(self.path(name) / child for child in names)
        for child in children:
            self._stat(child)
        self._stat(path, allow_alias=False)
        return children

    def _rglob(self, path, pattern):
        need(pattern == '*.ox', 'unsupported recursive pattern')
        result, pending = [], [path]
        while pending:
            folder = pending.pop()
            for child in self._iterdir(folder):
                name = self._key(child)
                descriptor = self._descriptors.get(name)
                if child.is_symlink():
                    continue
                if name in self._enumeration:
                    pending.append(child)
                elif child.is_dir():
                    raise ReadBoundaryError('recursive metadata-only directory access')
                elif descriptor is not None and descriptor.kind == 'file' and child.name.endswith('.ox'):
                    result.append(child)
        return result

    def recheck_layout(self):
        for name in self._enumeration:
            self._iterdir(self.path(name))
        for name in self._descriptors:
            self._stat(self.path(name))
        for name in self._absent:
            self._check_absent(name)

    def pathlib_facade(self):
        module = ModuleType('pathlib')
        module.Path = self.path
        return module

    def core_facade(self):
        module = ModuleType('run_hir_producer_v3_ci')
        module.regular, module.load, module.loads = self.regular, self.load, self._loads
        module.sha = lambda raw: hashlib.sha256(raw).hexdigest()
        module.need = need
        return module
