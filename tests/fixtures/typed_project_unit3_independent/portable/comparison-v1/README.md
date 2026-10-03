# Portable independent comparison entrance

This component runs no compiler, LLVM tool or generated executable. It compares
already captured source, native and mutation evidence through byte-identical
cleared semantic helpers. Its original expectation and helper manifests remain
unchanged. Candidate collectors receive only the separate source-only plan.

## Component and source view

`component-manifest.json` freezes the complete unchanged validation closure:
101 non-source files, 5,873,740 bytes, including seven original manifests. The
files keep their original relative paths under the supplied component root.
The parser is the separately supplied, exact cleared 638fb39b file.

Preparation verifies every component file and the shared physical 445-source
materialization against the original source membership. It copies only the
101 non-source files and parser into a fresh comparator view. Its one declared
`sources` directory alias points exactly to the already materialized physical
source directory. No source file is duplicated, and no path falls back to the
original authoring workspace. The alias exists only in the comparison view;
candidate invokers continue using physical paths.

The original complete manifest validation functions run unchanged on that view,
including the original463-file freeze and the final source/mutation manifests.
Native diagnostic helpers read the same verified source bytes through the one
alias. Imported semantic module location is checked against the view.

## Invocation

Use `PYTHONOPTIMIZE=0 python3 -B portable_compare.py prepare` with explicit
`--component-root`, `--component-manifest`, `--materialization`, `--parser` and
fresh `--output`. It writes `prepared-comparison.json` beneath that output.

Then use `PYTHONOPTIMIZE=0 python3 -B portable_compare.py compare` with
`--prepared`, `--plan`, `--plan-sha256` and fresh `--output`. The exact plan
schema is in PLAN_SCHEMA.md. Comparison writes `comparison.json` and preserves
any mismatch. The plan fixes exact case/profile membership, selected requests,
builds, source paths, receipt paths and output arguments. Full source means304
cases; full native means300 primary invocations. A bounded plan cannot earn full
qualification credit.

Full mutation comparison requires220 internal case/profile pairs and additionally
`--native-evidence` plus `--native-evidence-sha256`, identifying a successful
full300 native comparison made with this exact entrance/view. The entrance
rechecks ten attributed native receipts for the original four driver boundary
requests and their eight profile results, using the original native semantic
comparator and actual source-unavailability/output-protection records. It does
not add any candidate execution. Bounded mutation comparison stays internal
and does not claim these driver attributions.

## Mechanical changes only

- Source: exact physical source roots are passed to unchanged `compare_case`.
- Native: the original `compare_native.main` receives the verified physical
  fixture prefix. Receipt/stream files are copied byte-for-byte into a recorded
  comparison view; an explicit map preserves each original artifact identity.
  Output arguments are admitted from the plan before the helper substitutes
  them into already frozen expected streams.
- Mutation: unchanged `compare_envelope` runs after exact original204/derived123
  compiler-input and omitted81-member proof, controller/parser/request joins,
  and exact overlay-only build-view reconstruction. Only the compiler-overlay
  digest for v2 or parser-supplement overlay digest for v3 is rebound, scoped to
  one call and restored afterward. All raw/event journals remain unchanged.

Missing, unknown, duplicated, mismatched or stale-bound inputs reject before
semantic comparison. Every build binding is used, every row is checked, and
the materialization/view/build identities are rechecked afterward. Python
optimization and bytecode-producing invocation are refused.

The native request gate has49 separate source-only controls. Wrapper controls
cover positive source/native/mutation admission and34 rejection cases. Retained
four-source and six-native actual proofs are reused without compiler reruns.
Actual bounded mutation comparisons require separately authorized collection.
