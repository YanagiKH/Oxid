"""Exact, source-only fallible-lexer lifecycle successor.

No historical parser input is opened. The retained lifecycle insertions are
transported byte-for-byte; only lexer hook locations change. Static proofs do
not establish runtime event cardinality or semantic qualification.
"""
import hashlib
import json
import re
import stat
import types
from pathlib import Path, PurePosixPath

HERE = Path(__file__).resolve().parent
REPOSITORY = HERE.parents[2]
ADAPTER_SHA = '0520fbf2c651f9b22897a19937c54749b0153ee234bae424dc4132d48e198e2a'
PREDECESSOR_PATCH_SHA = 'c64b43cd0b630fcae46ac4cf73812ad0eb614696474e3af5d75d79b32f5047cf'
LEXER = 'src/frontend/lexer.rs'


class Rejected(ValueError):
    pass


def need(ok, message):
    if not ok:
        raise Rejected(message)


def sha(raw):
    return hashlib.sha256(raw).hexdigest()


def row(name, raw):
    return {'path': name, 'bytes': len(raw), 'sha256': sha(raw)}


def rows(inputs, components=False):
    key = (lambda name: PurePosixPath(name).parts) if components else None
    return [row(name, inputs[name]) for name in sorted(inputs, key=key)]


def adapter():
    path = REPOSITORY / 'tests/qualification/lexer_reservation_current/adapters.py'
    raw = path.read_bytes()
    need(sha(raw) == ADAPTER_SHA, 'unapproved lexer observer adapter')
    module = types.ModuleType('current_lexer_observer_adapter')
    exec(compile(raw, str(path), 'exec'), module.__dict__)
    return module


def sections(patch):
    """Parse only complete unified text patches, with a strict section roster."""
    lines = patch.splitlines(keepends=True)
    result = []
    i = 0
    while i < len(lines):
        need(lines[i].startswith(b'--- '), 'unexpected lifecycle patch section')
        before = lines[i][4:].decode().rstrip('\n')
        i += 1
        need(i < len(lines) and lines[i].startswith(b'+++ b/'), 'missing lifecycle target')
        name = lines[i][6:].decode().rstrip('\n')
        need(before in ('/dev/null', 'a/' + name), 'lifecycle path mismatch')
        need(str(PurePosixPath(name)) == name and not name.startswith('/')
             and '..' not in PurePosixPath(name).parts, 'unsafe lifecycle path')
        i += 1
        hunks = []
        while i < len(lines) and not lines[i].startswith(b'--- '):
            match = re.fullmatch(rb'@@ -(\d+)(?:,(\d+))? \+(\d+)(?:,(\d+))? @@\n', lines[i])
            need(match is not None, 'invalid lifecycle hunk header')
            starts = [int(match[1]), int(match[3])]
            counts = [int(match[2] or 1), int(match[4] or 1)]
            i += 1
            body = []
            while i < len(lines) and not lines[i].startswith((b'@@ ', b'--- ')):
                need(lines[i][:1] in (b' ', b'+', b'-') and lines[i].endswith(b'\n'),
                     'invalid lifecycle hunk line')
                body.append(lines[i]); i += 1
            old = b''.join(line[1:] for line in body if line[:1] != b'+')
            new = b''.join(line[1:] for line in body if line[:1] != b'-')
            need(len(old.splitlines()) == counts[0] and len(new.splitlines()) == counts[1],
                 'lifecycle hunk length differs')
            hunks.append((starts, counts, old, new, body))
        need(hunks, 'empty lifecycle section')
        result.append((name, before == '/dev/null', hunks))
    names = [name for name, _, _ in result]
    need(len(names) == len(set(names)), 'duplicate lifecycle patch section')
    return result


def inserted_roster(patch):
    return [{'path': name, 'inserted': b''.join(line[1:] for h in hunks for line in h[4]
             if line.startswith(b'+')).decode()}
            for name, _, hunks in sections(patch)]


def exact_patch(inputs, patch, reverse=False):
    """Apply exact offsets and full contexts; no fuzz, offset search, or mutation."""
    output = dict(inputs)
    for name, added, hunks in sections(patch):
        need((name in inputs) == (reverse or not added), 'lifecycle member presence differs')
        old_lines = inputs.get(name, b'').splitlines(keepends=True)
        result = []
        cursor = 0
        for starts, counts, old, new, _ in hunks:
            direction = int(reverse)
            offset = starts[direction] - (counts[direction] != 0)
            before, after = (new, old) if reverse else (old, new)
            need(offset >= cursor and offset <= len(old_lines), 'stale or reordered lifecycle hunk')
            need(b''.join(old_lines[offset:offset + counts[direction]]) == before,
                 'lifecycle hunk exact context differs')
            result.extend(old_lines[cursor:offset]); result.append(after)
            cursor = offset + counts[direction]
        result.extend(old_lines[cursor:])
        derived = b''.join(result)
        if reverse and added:
            need(derived == b'', 'added lifecycle body did not invert to empty')
            del output[name]
        else:
            output[name] = derived
    return output


def compose(inputs, retained_patch):
    """Use exact unique retained contexts to transport unchanged nonlexer hooks."""
    need(sha(retained_patch) == PREDECESSOR_PATCH_SHA, 'retained lifecycle patch changed')
    output = dict(inputs)
    for name, added, hunks in sections(retained_patch):
        if name == LEXER:
            continue
        if added:
            need(name not in output and len(hunks) == 1 and hunks[0][2] == b'',
                 'retained observer addition drift')
            output[name] = hunks[0][3]
            continue
        raw = output[name]
        for _, _, before, after, body in hunks:
            need(not any(line.startswith(b'-') for line in body), 'nonadditive retained lifecycle hook')
            need(raw.count(before) == 1 and after not in raw, 'retained lifecycle semantic context drift')
            raw = raw.replace(before, after, 1)
        output[name] = raw
    api = adapter()
    raw = inputs[LEXER]
    derived = api.replace_exact(raw, api.LIFECYCLE_SEAMS)
    output[LEXER], receipt = api.instrument_lexer(raw, row(LEXER, raw), row(LEXER, derived), 'lifecycle')
    return output, receipt


def read_inputs(root, records):
    root = Path(root)
    result = {}
    for expected in records:
        name = expected['path']
        path = root / name
        need(not any(p.is_symlink() for p in (path, *path.parents)), 'symlink lifecycle source')
        need(path.is_file() and not stat.S_IMODE(path.stat().st_mode) & 0o111,
             'nonregular or executable lifecycle source')
        raw = path.read_bytes()
        need(row(name, raw) == expected, 'unapproved lifecycle base: ' + name)
        need(name not in result, 'duplicate lifecycle base map')
        result[name] = raw
    files, dirs = set(), set()
    for base in ('src', 'native'):
        for path in (root / base).rglob('*'):
            need(not path.is_symlink(), 'symlink in lifecycle compiler closure')
            name = path.relative_to(root).as_posix()
            if path.is_dir():
                dirs.add(name)
            else:
                need(path.is_file(), 'special lifecycle compiler member')
                files.add(name)
    expected_files = {name for name in result if name.startswith(('src/', 'native/'))}
    expected_dirs = {str(p) for name in expected_files for p in PurePosixPath(name).parents
                     if str(p) not in ('.', 'src', 'native')}
    need(files == expected_files and dirs == expected_dirs, 'complete lifecycle compiler membership differs')
    return result


def admit(root, patch):
    """Check every base/observer identity and exact inverse before materialization."""
    import authority
    need(sha(Path(__file__).read_bytes()) == authority.LEXER_LIFECYCLE_HELPER_SHA,
         'unapproved lifecycle helper bytes')
    raw = (HERE / 'lexer-reservation-lifecycle-v2.json').read_bytes()
    need(sha(raw) == authority.LEXER_LIFECYCLE_AUTHORITY_SHA, 'unapproved lifecycle map authority')
    proof = json.loads(raw)
    need(sha(patch) == authority.LIFECYCLE_PATCH_SHA == proof['patch']['sha256']
         and len(patch) == proof['patch']['bytes'], 'unapproved exact lexer lifecycle patch')
    retained = (HERE / 'observer-u8-v1.patch').read_bytes()
    need(sha(retained) == PREDECESSOR_PATCH_SHA, 'changed immutable lifecycle predecessor')
    prior_authority = (HERE / 'byte_storage_source_authority.py').read_bytes()
    need(sha(prior_authority) == proof['predecessor_authority']['sha256'], 'changed public authority predecessor')
    source = read_inputs(root, proof['base_files'])
    derived, receipt = compose(source, retained)
    need(rows(derived, components=True) == proof['observer_files'], 'stale complete lifecycle observer map')
    need(receipt == proof['lexer_hook_correspondence'], 'stale lifecycle hook correspondence')
    need(exact_patch(source, patch) == derived, 'lifecycle forward patch differs from composition')
    need(exact_patch(derived, patch, reverse=True) == source, 'lifecycle inverse differs from complete source')
    roster = inserted_roster(patch)
    prior_roster = inserted_roster(retained)
    need(roster == proof['inserted_roster'], 'stale lifecycle inserted-event roster')
    need({r['path']: r['inserted'] for r in roster if r['path'] != LEXER}
         == {r['path']: r['inserted'] for r in prior_roster if r['path'] != LEXER},
         'nonlexer lifecycle instrumentation changed')
    return derived, proof
