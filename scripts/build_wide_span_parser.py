#!/usr/bin/env python3
"""Materialize the isolated wide-span scalar parser; no provider activation."""
import argparse
import hashlib
import json
from pathlib import Path
import re

ROOT = Path(__file__).resolve().parents[1]
OVERLAY_PATH = 'fixtures/typed-parser-wide-span'
OVERLAY = ROOT / OVERLAY_PATH
MEMBERS = tuple('parser_main parser_state parser_signature parser_atom parser_call '
                'parser_expression parser_statement parser_control parser_driver parser_output'.split())
OVERLAY_FILES = tuple(OVERLAY_PATH + '/' + name + '.ox' for name in (*MEMBERS, 'control_main'))
BASELINE_MANIFEST = 'fixtures/typed-frontend-v2/sources.json'
BASELINE_SHA256 = '08629aee51a25e05fd7bf4b121ff8a7d882683687425f4e5469f8b508618d608'


def digest(data):
    return hashlib.sha256(data).hexdigest()


def regular(root, name):
    path = Path(root)
    for part in Path(name).parts:
        path /= part
        if path.is_symlink():
            raise ValueError('aliased parser source: ' + name)
    if not path.is_file():
        raise ValueError('missing parser source: ' + name)
    return path.read_bytes()


def sources(root=ROOT):
    """Admit a closed original manifest and exact visible overlay membership."""
    root = Path(root)
    raw = regular(root, BASELINE_MANIFEST)
    baseline = json.loads(raw)
    if (not isinstance(baseline, dict) or set(baseline) != {'schema_version', 'sources'}
            or type(baseline['schema_version']) is not int or baseline['schema_version'] != 2
            or not isinstance(baseline['sources'], dict) or digest(raw) != BASELINE_SHA256):
        raise ValueError('unreviewed baseline manifest')
    originals = {}
    for name, record in baseline['sources'].items():
        if (not re.fullmatch('[a-z_]+', name) or not isinstance(record, dict)
                or set(record) != {'path', 'sha256'}
                or record['path'] not in (f'fixtures/typed-lexer-samples/{name}.ox',
                                          f'fixtures/typed-frontend-v2/{name}.ox')):
            raise ValueError('invalid baseline source record')
        data = regular(root, record['path'])
        if digest(data) != record['sha256']:
            raise ValueError('baseline source identity changed: ' + name)
        originals[name] = data
    overlay = root / OVERLAY_PATH
    expected = {name + '.ox' for name in (*MEMBERS, 'control_main')} | {'README.md'}
    if not overlay.is_dir() or overlay.is_symlink() or {p.name for p in overlay.iterdir()} != expected:
        raise ValueError('unexpected overlay membership')
    overlays = {name: regular(root, OVERLAY_PATH + '/' + name + '.ox')
                for name in (*MEMBERS, 'control_main')}
    regular(root, OVERLAY_PATH + '/README.md')
    selected, pending = {}, ['parser_main', 'control_main']
    while pending:
        name = pending.pop()
        if name in selected:
            continue
        if name not in originals and name != 'control_main':
            raise ValueError('unexpected module: ' + name)
        data = overlays[name] if name in overlays else originals[name]
        origin = OVERLAY_PATH + '/' + name + '.ox' if name in overlays else baseline['sources'][name]['path']
        selected[name] = (root / origin, data)
        pending.extend(re.findall(r'\bmod\s+(\w+)\s*;', data.decode()))
    if not set(overlays) <= selected.keys():
        raise ValueError('unused overlay member')
    return selected


def materialize(output, root=ROOT):
    selected = sources(root)
    output = Path(output)
    output.mkdir(parents=True, exist_ok=False)
    for name, (_, data) in selected.items():
        (output / (name + '.ox')).write_bytes(data)
    receipt = {name: dict(sha256=digest(data), origin=path.relative_to(root).as_posix())
               for name, (path, data) in sorted(selected.items())}
    (output / 'materialization.json').write_text(json.dumps(receipt, indent=2) + '\n')
    return output / 'parser_main.ox'


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', required=True, type=Path)
    print(materialize(parser.parse_args().output))
