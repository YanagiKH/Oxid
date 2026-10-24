#!/usr/bin/env python3
"""Run the unchanged boolean logic oracle with exact bit diagnostics."""
import json
from integer_bit_native_expectations import descriptor, prepare

if __name__ == "__main__":
    print(json.dumps(descriptor('boolean_logic'), sort_keys=True), flush=True)
    prepare('boolean_logic').main()
