"""One fixed text relocation derived from the immutable PR65 helper."""
import hashlib
ROOT = 'tests/qualification/stdlib_text_case_v1/'
PREVIOUS = 'tests/qualification/stdlib_trim_storage_v1/predecessor.py'
ORIGINAL = (7223, '7afae06f0687a4834f23c8545fabd1cc61906e6fb99846a545310db6698c4979')
CONSTANTS = "\n# Fixed text relocation; the earlier strings relocation remains unchanged.\n_TEXT = 'stdlib/text.ox'\n_TEXT_ARCHIVE = 'tests/qualification/stdlib_text_case_v1/archive/stdlib/text.ox.data'\n_TEXT_ROW = {'bytes': 192, 'git_blob': 'c8bfe5052483e748422a5ef7b401b41da053743d', 'mode': '100644', 'path': 'stdlib/text.ox', 'read_from': 'stdlib/text.ox', 'sha256': 'db27438c3f467d3185805be4593700bc76f70fe124b174ce1b392fa0e4a99f5b'}\n\ndef _text_archive(repo, row):\n    g.need(g.encoded(row) == g.encoded(_TEXT_ROW), 'fixed historical text row')\n    body = g.regular(repo, _TEXT_ARCHIVE, limit=_TEXT_ROW['bytes'])\n    g.pin(body, (_TEXT_ROW['bytes'], _TEXT_ROW['sha256']), 'fixed historical text bytes')\n    g.need(g.gitid('blob', body) == _TEXT_ROW['git_blob'], 'fixed historical text blob')\n    return body\n\n"

def exact(text, before, after):
    if text.count(before) != 1:
        raise ValueError('missing/duplicate text derivation anchor')
    return text.replace(before, after)

def derive(original):
    if type(original) is not bytes or (len(original), hashlib.sha256(original).hexdigest()) != ORIGINAL:
        raise ValueError('unreviewed text storage predecessor helper')
    text = original.decode('utf-8')
    text = exact(text, '\ndef capture(repo, restored, descriptor):\n',
                 CONSTANTS + '\ndef capture(repo, restored, descriptor):\n')
    text = exact(text, "    g.need(_STRINGS not in restored, 'strings cannot be restored source')\n",
                 "    g.need(_STRINGS not in restored, 'strings cannot be restored source')\n"
                 "    g.need(_TEXT not in restored, 'text cannot be restored source')\n")
    text = exact(text, '        elif name in restored:\n',
                 '        elif name == _TEXT:\n'
                 '            body = _text_archive(repo, row)\n'
                 '        elif name in restored:\n')
    return text.encode('utf-8')
