"""Fixed inactive four-path storage derivation; never changes historical inventories."""
import hashlib
PREVIOUS = 'tests/qualification/wide_span_fixture_storage_v1/predecessor.py'
ORIGINAL = (9322, '937a5ca1ca376bc3a92294edfd81314ce959db0d3a6e88235bc428cd7cc3b687')
CONSTANTS = '\n# Four fixed historical storage routes; no current source authority.\n_FEATURE_ROWS = {\'docs/API.md\': {\'bytes\': 2358, \'git_blob\': \'1924416108c91d9d5d0bb312c2da13812d2d9dc9\', \'mode\': \'100644\', \'path\': \'docs/API.md\', \'read_from\': \'docs/API.md\', \'sha256\': \'a022ce558db0032ae34716a0dfa18d52b30fe1e99dc664b50ac030e5f72315d8\'}, \'docs/MODULES.md\': {\'bytes\': 3249, \'git_blob\': \'8d943b4dc2d7349bb64c4088d94011969003abf0\', \'mode\': \'100644\', \'path\': \'docs/MODULES.md\', \'read_from\': \'docs/MODULES.md\', \'sha256\': \'2787a21a75d110724bb53c32693c46e3efc72e08ee0d7fbf8b20044020392da7\'}, \'stdlib/numbers.ox\': {\'bytes\': 315, \'git_blob\': \'5388b1f00daa76e5ad60c1fefa3f4a24aabbf5f8\', \'mode\': \'100644\', \'path\': \'stdlib/numbers.ox\', \'read_from\': \'stdlib/numbers.ox\', \'sha256\': \'a8c0d74e2e8b7e7964122ce8c9ce8806334dda6b8c55cb55c75a5a5695b14b3d\'}, \'tests/legacy_modules.rs\': {\'bytes\': 7782, \'git_blob\': \'82abceb3cae2609bb8b102f975a8b3a689debc53\', \'mode\': \'100644\', \'path\': \'tests/legacy_modules.rs\', \'read_from\': \'tests/legacy_modules.rs\', \'sha256\': \'53cb238bba1626bd45868ea1e640cb7a3214c5474a744b5cec60a3cb1f9349f4\'}}\n_FEATURE_ARCHIVE = \'tests/qualification/stdlib_feature_storage_v1/archive/\'\n\ndef _feature_archive(repo, row):\n    name = row.get("path")\n    g.need(name in _FEATURE_ROWS and g.encoded(row) == g.encoded(_FEATURE_ROWS[name]),\n           "fixed historical feature row")\n    expected = _FEATURE_ROWS[name]\n    body = g.regular(repo, _FEATURE_ARCHIVE + name + ".data", limit=expected["bytes"])\n    g.pin(body, (expected["bytes"], expected["sha256"]), "fixed historical feature bytes")\n    g.need(g.gitid("blob", body) == expected["git_blob"], "fixed historical feature blob")\n    return body\n\n'

def exact(text, before, after):
    if text.count(before) != 1:
        raise ValueError('exact feature storage anchor')
    return text.replace(before, after)

def derive(original):
    if type(original) is not bytes or (len(original), hashlib.sha256(original).hexdigest()) != ORIGINAL:
        raise ValueError('unreviewed feature storage predecessor')
    text = original.decode()
    text = exact(text, '\ndef capture(repo, restored, descriptor):\n',
                 CONSTANTS + '\ndef capture(repo, restored, descriptor):\n')
    text = exact(text, "    raw = g.regular(repo, VIEW_NAME)\n",
                 "    g.need(not set(restored) & set(_FEATURE_ROWS), 'feature path cannot be restored source')\n"
                 "    raw = g.regular(repo, VIEW_NAME)\n")
    text = exact(text, '        elif name in restored:\n',
                 '        elif name in _FEATURE_ROWS:\n'
                 '            body = _feature_archive(repo, row)\n'
                 '        elif name in restored:\n')
    return text.encode()
