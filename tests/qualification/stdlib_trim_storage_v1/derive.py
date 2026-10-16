"""Finite stdlib-trim-storage-v1 helper derivation; no activation or source reseal.

The accepted helper, descriptor and strings body are immutable inputs. This
producer emits only the named predecessor helper. It never discovers outputs,
changes historical mappings, or invokes the older activation scripts.
"""
import hashlib
from pathlib import Path
import sys

sys.dont_write_bytecode = True
ROOT = 'tests/qualification/stdlib_trim_storage_v1/'
PREVIOUS = 'tests/qualification/package_json_current_v1/predecessor.py'
ORIGINAL = (6089, '5de6efa9af91ae2aec153a5496454de0ca6d0fa785c657ba3d12f999564b8eef')

CONSTANTS = '''
# Fixed storage relocation, independent of any caller-supplied seal or mapping.
_STRINGS = 'stdlib/strings.ox'
_ARCHIVE = 'tests/qualification/stdlib_trim_storage_v1/archive/stdlib/strings.ox'
_VIEW_PIN = (1023905, 'ddcf5e3c09cda020aca4f1b639d2a66d7357f843f130054800d20228606ad76a')
_STRINGS_ROW = {
    'path': 'stdlib/strings.ox', 'read_from': 'stdlib/strings.ox',
    'bytes': 2345, 'mode': '100644',
    'sha256': '6ecb6b4c96acd0e788bf49db9a480c930d4235e317af8ffc596bbe8868fb65fd',
    'git_blob': '9f81e678fa63bd44b3387db2e1f06e2279b2d72f',
}


def _strings_archive(repo, row):
    g.need(g.encoded(row) == g.encoded(_STRINGS_ROW), 'fixed historical strings row')
    body = g.regular(repo, _ARCHIVE, limit=2345)
    g.pin(body, (2345, _STRINGS_ROW['sha256']), 'fixed historical strings bytes')
    g.need(g.gitid('blob', body) == _STRINGS_ROW['git_blob'], 'fixed historical strings blob')
    return body

'''


def exact(raw, before, after):
    if raw.count(before) != 1:
        raise ValueError('missing/duplicate trim storage derivation anchor')
    return raw.replace(before, after)


def derive(original):
    if type(original) is not bytes or (len(original), hashlib.sha256(original).hexdigest()) != ORIGINAL:
        raise ValueError('unreviewed trim storage predecessor helper')
    text = original.decode('utf-8')
    text = exact(text, '\ndef capture(repo, restored, descriptor):\n',
                 CONSTANTS + '\ndef capture(repo, restored, descriptor):\n')
    text = exact(text, '    raw = g.regular(repo, VIEW_NAME)\n',
                 '    raw = g.regular(repo, VIEW_NAME)\n'
                 "    g.pin(raw, _VIEW_PIN, 'immutable accepted-view descriptor')\n"
                 "    g.need(_STRINGS not in restored, 'strings cannot be restored source')\n")
    text = exact(text, '        if name in restored:\n',
                 '        if name == _STRINGS:\n'
                 '            body = _strings_archive(repo, row)\n'
                 '        elif name in restored:\n')
    return text.encode('utf-8')


if __name__ == '__main__':
    repo = Path(__file__).absolute().parents[3]
    if sys.argv[1:] != ['--check']:
        raise SystemExit('usage: derive.py --check (read-only)')
    expected = derive((repo / PREVIOUS).read_bytes())
    if (repo / ROOT / 'predecessor.py').read_bytes() != expected:
        raise SystemExit('trim storage helper differs from fixed derivation')
    print('PASS: fixed trim storage helper derivation')
