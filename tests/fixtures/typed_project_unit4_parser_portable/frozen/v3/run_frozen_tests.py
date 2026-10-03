#!/usr/bin/env python3
"""Run unchanged v8b controls against explicit base and packaged amendment files."""
import argparse
from pathlib import Path
import sys
import unittest

import portable as p


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--contract-dir', type=Path, required=True)
    args = parser.parse_args()
    a = p.authority()
    sys.path.insert(0, str(p.HERE / 'frozen/comparator'))
    import test_mutations
    test_mutations.CONTRACT = args.contract_dir
    # Only relocatable test data resolution changes in this runner. The exact
    # reviewed test/comparator bytes are verified above and remain unchanged.
    test_mutations.MutationControls.amendment_authority = lambda self: p.effective_authority(a)
    result = unittest.TextTestRunner(verbosity=2).run(unittest.defaultTestLoader.loadTestsFromModule(test_mutations))
    return 0 if result.wasSuccessful() else 1


if __name__ == '__main__':
    raise SystemExit(main())
