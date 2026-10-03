#!/usr/bin/env python3
"""Restore the one relocated documentation link in frozen native auxiliaries."""
import argparse
import pathlib
import shutil
import sys

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parents[1]))
from common import *

ORIGINAL_MANIFEST = '5ada020fb9d4a34b6fd37307396fec1558b608800895c12508f66aa681addaaf'
OVERLAY = 'f61460acd26bd9f8bf783b02c49315d0a950507834de945fe1ae07fd25f4c401'
README = 'tests/fixtures/owned_source/README.md'
ORIGINAL_README = {'bytes': 17787, 'sha256': 'b43352a4036cda792bf2b674cc7c3a71862a31230702b5d43f520b0ba17dec41'}
TRANSPORT_README = {'bytes': 17805, 'sha256': '3154a9a8861178156eac1bf888367c3a7fc03e89588059e3858713f315da30bc'}
ORIGINAL_LINK = b'](../../../fixtures/owned_source/batch.ox)'
TRANSPORT_LINK = b'](../../../../../../../../../fixtures/owned_source/batch.ox)'


def materialize(auxiliary_root, original_manifest, output):
    require(identity(original_manifest)['sha256'] == ORIGINAL_MANIFEST,
            'original auxiliary manifest selection differs')
    original = read_json(original_manifest)
    require(original['schema'] == 'unit3-native-auxiliary-v1' and
            original['overlay_manifest_sha256'] == OVERLAY,
            'original auxiliary contract differs')
    rows = original['files']
    require(len(rows) == 81 and len({row['path'] for row in rows}) == 81,
            'original auxiliary membership differs')
    original_readme = [row for row in rows if row['path'] == README]
    require(original_readme == [{'path': README, **ORIGINAL_README}],
            'original README identity differs')
    transported = [dict(row, **TRANSPORT_README) if row['path'] == README
                   else row for row in rows]
    root = pathlib.Path(auxiliary_root).resolve()
    require(not any(path.is_symlink() for path in root.rglob('*')),
            'auxiliary aliases are not admitted')
    check_manifest(root, transported, exact=True)
    readme = (root / README).read_bytes()
    require(readme.count(TRANSPORT_LINK) == 1, 'transport link occurrence differs')
    restored = readme.replace(TRANSPORT_LINK, ORIGINAL_LINK, 1)
    require({'bytes': len(restored), 'sha256': hashlib.sha256(restored).hexdigest()}
            == ORIGINAL_README, 'restored README identity differs')
    out = fresh(output)
    inputs = out / 'inputs'
    inputs.mkdir()
    for row in rows:
        destination = inputs / relative(row['path'])
        destination.parent.mkdir(parents=True, exist_ok=True)
        if row['path'] == README:
            destination.write_bytes(restored)
        else:
            shutil.copyfile(root / row['path'], destination)
    check_manifest(root, transported, exact=True)
    check_manifest(inputs, rows, exact=True)
    receipt = {
        'schema': SCHEMA + '-auxiliary-transport',
        'status': 'verified-81-original-auxiliary-inputs',
        'assertion_mode': assertion_mode(),
        'controller': bound_file(__file__),
        'original_manifest': bound_file(original_manifest),
        'transport_root': str(root),
        'transport_inputs': files_manifest(root),
        'input_root': str(inputs.resolve()),
        'inputs': files_manifest(inputs),
        'restored_path': README,
        'transport_readme': TRANSPORT_README,
        'original_readme': ORIGINAL_README,
        'restored_link_occurrences': 1,
        'compiler_executions': 0,
    }
    write_json(out / 'auxiliary-transport.json', receipt)
    return receipt


def main():
    parser = argparse.ArgumentParser()
    for name in ('auxiliary-root', 'original-manifest', 'output'):
        parser.add_argument('--' + name, required=True)
    args = parser.parse_args()
    receipt = materialize(args.auxiliary_root, args.original_manifest, args.output)
    print(json.dumps({'status': receipt['status'], 'input_root': receipt['input_root']}))


if __name__ == '__main__':
    main()
