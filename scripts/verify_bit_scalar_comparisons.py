#!/usr/bin/env python3
"""Run the unchanged scalar comparisons oracle with exact bit diagnostics."""
import json
import sys
from integer_bit_native_expectations import descriptor, prepare

if __name__ == "__main__":
    module = prepare('scalar_comparisons')
    if len(sys.argv) < 2:
        raise SystemExit(module.__doc__)
    print(json.dumps(descriptor('scalar_comparisons'), sort_keys=True), flush=True)
    module.verify(sys.argv[1:])
