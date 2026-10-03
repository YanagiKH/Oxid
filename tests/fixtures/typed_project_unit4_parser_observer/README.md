# Additive Unit4 parser observation adapter

This package owns observations and transport, not the independent expected corpus or its predicates. The candidate checkout is fixed at d9e6b9bf172abd5e15da7212c9e6224e29ccc768. `prepare.py` rejects dirty tracked inputs against their Git objects and derives a new test-only source view; its source manifest and instrumentation diff retain every change. No production file in the actual checkout is edited.

1. `prepare.py --repo CHECKOUT --output NEW_VIEW` derives an instrumented test view. Add `--control` for a parser without the instrumentation hooks.
2. Source the approved toolchains/env.sh, then `build.py --overlay NEW_VIEW/overlay-manifest.json --profile debug --output NEW_BUILD` (or release). The heavy build slot must be coordinated externally. It uses two jobs and disables incremental compilation.
3. After independent instrumentation review and durable authority checkpoint, `run.py --build-receipt NEW_BUILD/build-receipt.json --contract-dir FROZEN_CONTRACT --checkpoint DURABLE_MANIFEST --output NEW_RESULTS` collects the exact 248-case roster. Optional repeated `--case ID` performs an explicitly partial smoke collection, never full qualification.
4. `passivity.py --instrumented BUILD_RECEIPT --control CONTROL_BUILD_RECEIPT --checkpoint DURABLE_MANIFEST --output NEW_RESULTS` compares six ordinary mode/source pairs through actual binaries.
5. An independently maintained comparator evaluates both profiles and every conjunctive expectation. A build or collection success is never a qualification pass.

The observer uses original lex/token/node limits, real node/allocator gates, and the declared direct path-segment seam. Hooks record actual sticky recognition, ordered node/consume/recovery/append/reserve events, field-prefix scans and work deltas. Full AST Debug is mechanically decoded without source interpretation; the actual Program provenance is exposed read-only. Each raw envelope includes the exact executed source bytes as UTF8 for independent driver hashing. Every complete observation is tied to source, mode, profile, platform, binary and collector identity. Missing signals or bounded-collector overflow fail closed.

`python3 -m unittest discover -s PACKAGE -p 'test_*.py' -v` tests the transport with synthetic protocol data only: missing/extra/duplicate/zero/unexecuted rows, source and generation mismatch, duplicate JSON keys and lossy Debug decoding. This does not run the compiler or validate a candidate case.

See API.md for the complete data boundary. Prior compile/collection failures must be retained in distinct versioned output directories. Never edit frozen expectation files to accommodate observed behavior.
