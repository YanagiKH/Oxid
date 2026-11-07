#!/usr/bin/env python3
"""Verify Copy record vectors with reference and source-free native execution."""
import argparse
import json
from pathlib import Path
import sys

from verify_float_vector import main_for_package
from verify_typed_vectors import macos_native_limits, native_identity

ROOT = Path(__file__).resolve().parents[1]
EXAMPLE = Path('examples/copy-record-vectors')
SOURCES = (EXAMPLE / 'oxid.toml', EXAMPLE / 'src/main.ox',
           EXAMPLE / 'tests/records.ox')


def main():
    # The first program adds the original x=20 snapshot to the changed x=22.
    # The second returns zero only after its independent-copy checks succeed.
    cases = [('example', (ROOT / SOURCES[1]).read_text(), b'42\n'),
             ('independent-copies', (ROOT / SOURCES[2]).read_text(), b'0\n')]
    main_for_package(
        ROOT, EXAMPLE, SOURCES, 'copy-record-vectors-example', 'records', cases,
        {'copy_record_vector_cases': 2}, __file__,
        description=__doc__, finalize=False,
        native_guard=macos_native_limits if sys.platform == 'darwin' else None,
        warm_cache_status='unavailable' if sys.platform == 'darwin' else 'hit',
        # These two finite programs can fold all bounded fuel checks.
        policy_equivalent_cases=('example', 'independent-copies'))
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
