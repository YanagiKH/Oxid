"""Fixed alias/Unicode source generation, with genuine Git-protocol evidence.

Pure generation accepts bytes, never repository paths, callbacks or authority
from the environment. This module is not execution qualification. Its retained
inverse implementation is authenticated before compilation. Historical v1
artifacts are inputs, never regenerated or relabeled.
"""
import copy
import hashlib
import json
import os
from pathlib import Path, PurePosixPath
import stat
import types

ROOT = 'tests/qualification/package_json_current_v1/'
PACKAGE = 'tests/fixtures/typed_project_source_binding/'
PATHS = ('src/main.rs', 'src/runtime/data.rs')
MAIN = {'head': '40e5aae618d7ce1f5efe7096ee634f19ab61063c',
        'tree': '164484b7167650279350bcbba37cdf7bcca2f2b7'}
CHECKPOINT = {'head': '858c70d862587b6a8b5d59a70afe4b8334b1f150',
              'tree': 'c44b7dfb2938dc510016ff4939104a2236ba2e9c',
              'parent': MAIN['head']}
FORMATTTER_HEAD = 'fbff527c895e6cd4ec9350bd8b5d460bc20bb80d'
PREDECESSOR_SHA = 'd83edc200b9b900668ccc7ee9679a7972e1d497d10acf5f82ccd602680dd04e9'
PREDECESSOR_NAME = 'published-formatter-source-v1.json'
CURRENT_NAME = 'package-json-source-v1.json'
AUTHORITY_NAME = 'package-json-authority-v1.json'
PROOF_NAME = 'package-json-source-proof-v1.json'
PATCH_NAME = 'package-json-transition-v1.patch'
PATCH_PIN = (9044, '3017dfb1ae239afb0c0341eff004fd9447c637438370c27ccb1eba9f18fcd052')
RUNNER_PIN = (216279, '584e8ee937cb2d3ef62d59d3eb69a79bfb6afce23df86de3df305f81afae7c76')
READBACKS = {
    'accepted-main': (690555, '9e5786a7dc78835c91db6b38fc9dc6f2048e91048332ab72a2a2cc99fc81320a'),
    'source-checkpoint': (689823, '253a049c416f88c3b2fe04e4c502db9e4514f413fb36d75e5f4101e5afcca64d'),
}
BEFORE_BLOBS = {'src/main.rs': '476fe16b8506299475c5acbacf1b6976cc08324b',
                'src/runtime/data.rs': 'ac04561087bf689b5ae89d6d97a657852ea2bd24'}
AFTER_BLOBS = {'src/main.rs': '05f15750502ad48c204bdaff05aa34c6bfad1a96',
               'src/runtime/data.rs': '7b172a481b22484de0bfd063dc258da4e1d2f86b'}
CHANGED_FILES = ('docs/PACKAGES.md', 'src/main.rs', 'src/runtime/data.rs',
                 'tests/package_manifest_validation.rs')
ACCOUNTING_PATHS = ('src/frontend/declaration_index/resource.rs',
                    'src/frontend/declaration_index/sealed.rs',
                    'src/frontend/declaration_index/u8_reservation.rs')


def need(value, message):
    if not value:
        raise ValueError(message)


def sha(raw):
    return hashlib.sha256(raw).hexdigest()


def encoded(value):
    return (json.dumps(value, sort_keys=True, indent=2) + '\n').encode()


def identity(name, raw):
    return {'path': name, 'bytes': len(raw), 'sha256': sha(raw)}


def gitid(kind, raw):
    return hashlib.sha1(kind.encode() + b' ' + str(len(raw)).encode() + b'\0' + raw).hexdigest()


def pin(raw, expected, message):
    need(type(raw) is bytes and (len(raw), sha(raw)) == expected, message)


def relative(name):
    need(type(name) is str and name and '\\' not in name
         and not name.startswith('/') and str(PurePosixPath(name)) == name
         and all(p not in ('', '.', '..') for p in name.split('/')), 'noncanonical path')
    return name


def rows_map(rows):
    need(type(rows) is list and rows, 'empty source roster')
    names = [relative(r['path']) for r in rows]
    need(names == sorted(set(names)), 'duplicate or unordered source roster')
    return {r['path']: r for r in rows}


def verify_git(record, expected, ref, count):
    """Reconstruct every Git tree node and the exact fetched commit body."""
    need(record['schema'] == 'oxid-git-protocol-readback-v1'
         and record['repository'] == 'https://github.com/YanagiKH/Oxid.git'
         and record['ref'] == ref and record['commit'] == expected['head']
         and record['tree'] == expected['tree'], 'Git checkpoint association')
    need(expected['head'] + '\t' + ref + '\n' in record['remote_refs'], 'remote ref readback')
    raw = record['raw_commit_utf8'].encode()
    need(gitid('commit', raw) == expected['head'], 'exact Git commit body')
    headers = raw.split(b'\n\n', 1)[0].splitlines()
    need(headers[0] == b'tree ' + expected['tree'].encode(), 'commit tree association')
    if 'parent' in expected:
        need([h for h in headers if h.startswith(b'parent ')]
             == [b'parent ' + expected['parent'].encode()], 'exact source commit parent')
    rows = record['entries']
    need(type(rows) is list and len(rows) == count, 'complete Git tree inventory')
    bypath, children = {}, {'': []}
    for row in rows:
        need(set(row) == {'path', 'mode', 'type', 'sha'}, 'Git row fields')
        name = relative(row['path'])
        need(name not in bypath, 'duplicate Git path')
        need((row['type'], row['mode']) in {('tree', '040000'), ('blob', '100644'),
                                          ('blob', '100755')}, 'Git mode/type')
        need(type(row['sha']) is str and len(row['sha']) == 40
             and all(c in '0123456789abcdef' for c in row['sha']), 'Git object ID')
        bypath[name] = row
        parent = name.rpartition('/')[0]
        children.setdefault(parent, []).append(row)
        if row['type'] == 'tree':
            children.setdefault(name, [])
    need(set(children) - {''} == {p for p, r in bypath.items() if r['type'] == 'tree'},
         'complete tree ancestry')
    for parent, members in children.items():
        ordered = sorted(members, key=lambda r: r['path'].rsplit('/', 1)[-1].encode()
                         + (b'/' if r['type'] == 'tree' else b''))
        body = b''.join(r['mode'].lstrip('0').encode() + b' '
                        + r['path'].rsplit('/', 1)[-1].encode() + b'\0'
                        + bytes.fromhex(r['sha']) for r in ordered)
        need(gitid('tree', body) == (expected['tree'] if not parent else bypath[parent]['sha']),
             'complete Git tree-node mismatch: ' + parent)
    return {p: r for p, r in sorted(bypath.items()) if r['type'] == 'blob'}


def readbacks(bodies):
    need(type(bodies) is dict and set(bodies) == set(READBACKS), 'exact Git readback set')
    for name, expected in READBACKS.items():
        pin(bodies[name], expected, 'changed actual Git readback: ' + name)
    before = verify_git(json.loads(bodies['accepted-main']), MAIN, 'refs/heads/main', 3417)
    after = verify_git(json.loads(bodies['source-checkpoint']), CHECKPOINT,
                       'refs/heads/KH/alias-unicode-source-v1', 3418)
    changes = [n for n in sorted(before.keys() | after.keys()) if before.get(n) != after.get(n)]
    need(changes == list(CHANGED_FILES), 'complete four-file publication scope')
    return before, after


def inverse(inputs, patch, runner):
    pin(runner, RUNNER_PIN, 'changed retained inverse implementation')
    module = types.ModuleType('package_json_retained_inverse')
    # The original module only declares its API on import; main is not executed.
    module.__file__ = '/retained/typed_project_source_binding_byte_storage_v1/run.py'
    exec(compile(runner, module.__file__, 'exec'), module.__dict__)
    return module.apply_inverse_patch(inputs, patch, PATCH_PIN[1], PATCH_PIN[0], PATHS)


def generate(previous_raw, inputs, patch, evidence, runner):
    pin(previous_raw, (74567, PREDECESSOR_SHA), 'changed accepted formatter manifest')
    pin(patch, PATCH_PIN, 'changed exact two-source inverse patch')
    previous = json.loads(previous_raw)
    need(previous['reviewed_source_head'] == FORMATTTER_HEAD, 'historical checkpoint relabel')
    old = rows_map(previous['files'])
    need(len(old) == 376 and type(inputs) is dict and list(inputs) == list(old)
         and all(type(b) is bytes for b in inputs.values()), 'complete ordered 376 source bodies')
    before_tree, after_tree = readbacks(evidence)
    rows = [identity(n, b) for n, b in inputs.items()]
    need([r['path'] for r in rows if r != old[r['path']]] == list(PATHS), 'exact two-source change')
    for name, raw in inputs.items():
        need(after_tree[name]['mode'] == '100644' and gitid('blob', raw) == after_tree[name]['sha'],
             'source differs from actual published blob: ' + name)
        if name in PATHS:
            need(gitid('blob', raw) == AFTER_BLOBS[name], 'unapproved source body')
    restored, touched = inverse(inputs, patch, runner)
    need(touched == list(PATHS) and sorted(restored) == list(old), 'exact inverse source scope')
    for name, raw in restored.items():
        need(identity(name, raw) == old[name] and before_tree[name]['mode'] == '100644'
             and gitid('blob', raw) == before_tree[name]['sha'], 'inverse before identity: ' + name)
        if name in PATHS:
            need(gitid('blob', raw) == BEFORE_BLOBS[name], 'unapproved inverse body')
        else:
            need(raw == inputs[name], 'changed one of the other 374 bodies')
    need('src/runtime/data.rs' not in ACCOUNTING_PATHS, 'data is an ordinary source')
    need(all(inputs[n] == restored[n] for n in ACCOUNTING_PATHS), 'accounting source changed')
    current = copy.deepcopy(previous)
    current.update(files=rows, purpose='Alias and Unicode JSON source-only successor',
                   reviewed_source_head=CHECKPOINT['head'], source_only_tree=CHECKPOINT['tree'])
    # Earlier published_main_* fields remain at their historical stage. This
    # generation has its own explicit association and complete predecessor.
    provenance = {'schema': 'oxid-package-json-source-generation-v1',
                  'accepted_main': MAIN, 'source_checkpoint': CHECKPOINT,
                  'predecessor': identity(PREDECESSOR_NAME, previous_raw),
                  'source_deltas': [{'path': n, 'before': old[n], 'after': identity(n, inputs[n]),
                                     'before_git_blob': BEFORE_BLOBS[n],
                                     'after_git_blob': AFTER_BLOBS[n]} for n in PATHS],
                  'source_only': True, 'execution_qualified': False}
    current['package_json_source'] = copy.deepcopy(provenance)
    current_raw = encoded(current)
    proof = {**provenance, 'complete_changed_files': list(CHANGED_FILES),
             'readbacks': [identity(ROOT + 'source-proof/' + n + '.json', evidence[n])
                           for n in sorted(evidence)],
             'current_source_members': len(rows), 'unchanged_source_members': len(rows) - len(PATHS),
             'transition_paths': list(PATHS), 'transition': identity(PATCH_NAME, patch),
             'current_source': identity(CURRENT_NAME, current_raw),
             'accounting_paths': list(ACCOUNTING_PATHS)}
    authority = {**proof, 'schema': 'oxid-package-json-source-authority-v1',
                 'before_inputs': [dict(old[n], git_blob=before_tree[n]['sha']) for n in old],
                 'after_inputs': [dict(r, git_blob=after_tree[r['path']]['sha']) for r in rows],
                 'proof': identity(PROOF_NAME, encoded(proof))}
    return {CURRENT_NAME: current_raw, AUTHORITY_NAME: encoded(authority),
            PROOF_NAME: encoded(proof), PATCH_NAME: patch}, restored


def regular(root, name, limit=2 * 1024 * 1024):
    """Bounded binary read; no symlinks, executable files, aliases or read races."""
    path = Path(root) / relative(name)
    ancestors = []
    for parent in reversed(path.parents):
        info = parent.lstat()
        need(stat.S_ISDIR(info.st_mode) and not getattr(info, 'st_file_attributes', 0) & 0x400,
             'aliased input ancestor: ' + name)
        ancestors.append((parent, (info.st_dev, info.st_ino, info.st_mode)))
    before = path.lstat()
    need(stat.S_ISREG(before.st_mode) and before.st_nlink == 1
         and not before.st_mode & 0o111 and 0 <= before.st_size <= limit
         and not getattr(before, 'st_file_attributes', 0) & 0x400,
         'unsafe input mode/size: ' + name)
    fields = ('st_dev', 'st_ino', 'st_mode', 'st_nlink', 'st_size', 'st_mtime_ns', 'st_ctime_ns')
    stamp = lambda s: tuple(getattr(s, f) for f in fields)
    fd = os.open(path, os.O_RDONLY | getattr(os, 'O_NOFOLLOW', 0)
                 | getattr(os, 'O_NONBLOCK', 0) | getattr(os, 'O_BINARY', 0))
    try:
        opened = os.fstat(fd)
        left, right = stamp(before), stamp(opened)
        if os.name == 'nt' and hasattr(before, 'st_birthtime_ns') and hasattr(opened, 'st_birthtime_ns'):
            need(left[:-1] == right[:-1] and before.st_birthtime_ns == opened.st_birthtime_ns,
                 'input changed before read')
        else:
            need(left == right, 'input changed before read')
        chunks, count = [], 0
        while True:
            chunk = os.read(fd, min(65536, limit + 1 - count))
            if not chunk:
                break
            chunks.append(chunk)
            count += len(chunk)
            need(count <= limit, 'input grew beyond bound')
        need(stamp(os.fstat(fd)) == right and count == opened.st_size, 'input changed during read')
    finally:
        os.close(fd)
    need(stamp(path.lstat()) == stamp(before), 'input name changed during read')
    for parent, expected in ancestors:
        info = parent.lstat()
        need((info.st_dev, info.st_ino, info.st_mode) == expected
             and not getattr(info, 'st_file_attributes', 0) & 0x400,
             'input ancestor changed during read')
    return b''.join(chunks)


def verify_materialized(source, inputs, predecessor_inputs):
    """Additional check after the retained Unit2 verifier, never its replacement."""
    for name in PATHS:
        raw = regular(source, name)
        need(raw == inputs[name] and raw != predecessor_inputs[name]
             and gitid('blob', raw) == AFTER_BLOBS[name], 'wrong materialized current source: ' + name)
