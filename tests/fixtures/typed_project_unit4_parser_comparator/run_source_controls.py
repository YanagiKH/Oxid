#!/usr/bin/env python3
"""Run unchanged frozen test bodies with explicit relocated fixture paths."""
import argparse
import hashlib
from pathlib import Path
import sys
import unittest

from verify_package import HERE, verify


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--contract-dir", required=True, type=Path)
    parser.add_argument("--amendment-dir", required=True, type=Path)
    parser.add_argument("--amendment-review", type=Path, default=HERE / "reviews/source-amendment-review.json")
    args = parser.parse_args()
    verify()
    sys.dont_write_bytecode = True
    sys.path.insert(0, str(HERE / "frozen/v8b"))
    import test_mutations
    test_mutations.CONTRACT = args.contract_dir.resolve()
    paths = {"checkpoint": args.amendment_dir / "AMENDMENT-CHECKPOINT-v1.json",
             "descriptor": args.amendment_dir / "artifacts/effective-contract.json",
             "amendment": args.amendment_dir / "artifacts/amendment.json",
             "review": args.amendment_review}

    def authority_paths(self):
        return {key: {"path": str(path.resolve()), "bytes": path.stat().st_size,
                      "sha256": hashlib.sha256(path.read_bytes()).hexdigest()} for key, path in paths.items()}

    # Change only test fixture locations. Production pins and all test bodies,
    # assertions, expected values and comparator functions remain unchanged.
    test_mutations.MutationControls.amendment_authority = authority_paths
    result = unittest.TextTestRunner(verbosity=2).run(unittest.defaultTestLoader.loadTestsFromModule(test_mutations))
    return 0 if result.wasSuccessful() else 1


if __name__ == "__main__":
    raise SystemExit(main())
