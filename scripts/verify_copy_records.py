#!/usr/bin/env python3
"""Verify Copy record examples with reference and source-free native execution."""
import argparse
import json
from pathlib import Path
import sys

from verify_float_vector import main_for_package
from verify_typed_vectors import macos_native_limits, native_identity

ROOT = Path(__file__).resolve().parents[1]
EXAMPLE = Path('examples/copy-records')
FIXTURES = Path('tests/fixtures/copy_records')
SOURCES = (EXAMPLE / 'oxid.toml', EXAMPLE / 'src/main.ox',
           EXAMPLE / 'tests/copies.ox', FIXTURES / 'wide_unicode.ox',
           FIXTURES / 'all_scalars.ox')


def main():
    cases = [('example', (ROOT / SOURCES[1]).read_text(), b'42\n'),
             ('independent-copies', (ROOT / SOURCES[2]).read_text(), b'0\n'),
             ('wide-unicode', (ROOT / SOURCES[3]).read_text(), b'42\n'),
             ('all-scalars', (ROOT / SOURCES[4]).read_text(), b'42\n')]
    main_for_package(
        ROOT, EXAMPLE, SOURCES, 'copy-records-example', 'copies', cases,
        {'scalar_types': 17, 'copy_cases': 4}, __file__,
        description=__doc__, finalize=False,
        native_guard=macos_native_limits if sys.platform == 'darwin' else None,
        warm_cache_status='unavailable' if sys.platform == 'darwin' else 'hit',
        # Finite input-independent programs may fold all fuel checks. Retain
        # every actual identical pair rather than requiring different bytes.
        policy_equivalent_cases=tuple(name for name, _, _ in cases))
    parser = argparse.ArgumentParser(add_help=False)
    parser.add_argument('--output', type=Path, required=True)
    args, _ = parser.parse_known_args()
    path = args.output.resolve() / 'result.json'
    receipt = json.loads(path.read_text())
    try:
        for artifact in receipt['native_elfs']:
            artifact.update(native_identity(artifact['path']))
        receipt['status'] = 'passed'
    except BaseException as error:
        receipt.update(status='failed', failure=f'{type(error).__name__}: {error}')
        raise
    finally:
        path.write_text(json.dumps(receipt, indent=2) + '\n')
    print(json.dumps({key: value for key, value in receipt.items()
                      if key not in {'sources', 'native_elfs'}}, sort_keys=True))


if __name__ == '__main__':
    main()
