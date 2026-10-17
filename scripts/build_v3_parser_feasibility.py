#!/usr/bin/env python3
"""Isolated V3 parser closure materializer. Does not select a public provider."""
import argparse
import json
from pathlib import Path
import re
import build_wide_span_parser as wide
ROOT = Path(__file__).resolve().parents[1]
PARSER = 'fixtures/typed-frontend-v3/parser'
SHARED = 'fixtures/typed-frontend-v3/shared'

def materialize(output, root=ROOT):
    root = Path(root)
    originals = wide.sources(root)
    overrides = {name: f'{PARSER}/{name}.ox' for name in wide.MEMBERS}
    overrides.update({name: f'{SHARED}/{name}.ox' for name in ('source_bytes', 'lexer_core', 'keywords')})
    expected = {name + '.ox' for name in wide.MEMBERS}
    if {p.name for p in (root / PARSER).iterdir()} != expected:
        raise ValueError('unexpected V3 parser overlay membership')
    selected, pending = {}, ['parser_main']
    while pending:
        name = pending.pop()
        if name in selected: continue
        if name in overrides:
            path = root / overrides[name]
            data = wide.regular(root, overrides[name])
        elif name in originals:
            path, data = originals[name]
        else: raise ValueError('unexpected module ' + name)
        selected[name] = path, data
        pending.extend(re.findall(r'\bmod\s+(\w+)\s*;', data.decode()))
    if not set(overrides) <= set(selected): raise ValueError('unused overlay')
    output = Path(output)
    output.mkdir(parents=True, exist_ok=False)
    for name, (_, data) in selected.items(): (output / (name + '.ox')).write_bytes(data)
    receipt = {name: {'sha256': wide.digest(data), 'origin': path.relative_to(root).as_posix()}
               for name, (path, data) in sorted(selected.items())}
    (output/'materialization.json').write_text(json.dumps(receipt, indent=2)+'\n')
    return output/'parser_main.ox'

if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=Path, required=True)
    print(materialize(parser.parse_args().output))
