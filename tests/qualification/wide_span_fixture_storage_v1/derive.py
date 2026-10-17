"""One fixed historical fixture-data test relocation, no source-roster change."""
import hashlib
PREVIOUS = 'tests/qualification/stdlib_text_case_v1/predecessor.py'
ORIGINAL = (8161, 'c29caaaaf4dd7f96d809a2cdc9e4d19ee9378aa4d58161b79a9c9b9387d3892c')
CONSTANTS = "\n# One fixed test-body relocation; previous text/strings rules remain intact.\n_FIXTURE_TEST = 'scripts/test_verify_repo_fixture_data.py'\n_FIXTURE_ARCHIVE = 'tests/qualification/wide_span_fixture_storage_v1/archive/scripts/test_verify_repo_fixture_data.py.data'\n_FIXTURE_ROW = {'bytes': 56742, 'git_blob': '74e71a1a8e252bdfa426e214a0e2606c1445287d', 'mode': '100644', 'path': 'scripts/test_verify_repo_fixture_data.py', 'read_from': 'scripts/test_verify_repo_fixture_data.py', 'sha256': '623a6ac490d97c3e45132630f5a0f334d81e13493f8384d27bedf2956b3872c6'}\n\ndef _fixture_archive(repo, row):\n    g.need(g.encoded(row) == g.encoded(_FIXTURE_ROW), 'fixed historical fixture-data test row')\n    body = g.regular(repo, _FIXTURE_ARCHIVE, limit=_FIXTURE_ROW['bytes'])\n    g.pin(body, (_FIXTURE_ROW['bytes'], _FIXTURE_ROW['sha256']), 'fixed historical fixture-data test bytes')\n    g.need(g.gitid('blob', body) == _FIXTURE_ROW['git_blob'], 'fixed historical fixture-data test blob')\n    return body\n\n"

def exact(text,before,after):
    if text.count(before)!=1:raise ValueError('exact fixture storage anchor')
    return text.replace(before,after)

def derive(original):
    if type(original) is not bytes or (len(original),hashlib.sha256(original).hexdigest())!=ORIGINAL:
        raise ValueError('unreviewed fixture storage predecessor')
    text=original.decode()
    text=exact(text,'\ndef capture(repo, restored, descriptor):\n',CONSTANTS+'\ndef capture(repo, restored, descriptor):\n')
    text=exact(text,"    g.need(_TEXT not in restored, 'text cannot be restored source')\n",
        "    g.need(_TEXT not in restored, 'text cannot be restored source')\n"
        "    g.need(_FIXTURE_TEST not in restored, 'fixture-data test cannot be restored source')\n")
    text=exact(text,'        elif name in restored:\n',
        '        elif name == _FIXTURE_TEST:\n'
        '            body = _fixture_archive(repo, row)\n'
        '        elif name in restored:\n')
    return text.encode()
