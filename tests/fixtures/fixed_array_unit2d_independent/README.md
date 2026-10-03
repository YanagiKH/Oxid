# Fixed-array Unit2D independent regression inputs

This is a source-only durability checkpoint of the reviewer-owned inputs used to
qualify the private native consumer. The Rust files are not registered Cargo
modules yet. Automatic repository replay integration is pending; their presence
does not claim CI has executed them or that public source arrays are enabled.

The exact passed source is recorded in qualification.json. The combined Rust
artifact and every component are preserved byte-for-byte. The temporary build
content-marker function is deliberately excluded; its historical executable and
receipts remain separate. No production source changes are part of this package.

## Contents and independence

- sources/reviewer-array-native-v1.rs combines the frozen raw adapters, scalar
  sequence expectations, hand-counted fuel schedules, LLVM cases, allocation/work
  controls, actual-store readback observer and physical manifest executor
- sources/reviewer-array-native-binding-v1.json binds all component inputs; only
  duplicate top-level imports were removed when combining them
- The inherited raw fixture functions retain their Unit2C source provenance in
  sources/inherited-fixture-binding-v1.json. They import no production cost/layout
  helper for expected values
- expectations/expected-v1.json predates candidate observation and contains the
  independent small-index, coordinate, physical-layout and resource expectations
- The checkpoint1 controls preserve the 719-point Unicode oracle and the exact
  partial-allocation regression that failed on the first diagnostic checkpoint
- expectations/old-ir-manifest.json and old-ir-inventory.tsv bind a separately
  built PR28 baseline: 90 inputs, 59 distinct modules
- tools/llvm_structure.py independently reads emitted CFGs, bounds dominance and
  pointer-phi predecessor edges. tools/storage contains the closed physical
  storage observer and its standard-library-only text tests

The physical observer checks every intermediate i32 byte, bool i1 cell, zero/unit
byte and initialized guard. It never asserts that LLVM i1 padding is canonical.
Eleven narrowly targeted empty-i32 mutants change only one full-width zero write
or matching load/store pair to one byte. The original modules and operation
bodies are retained and reconstructed exactly outside those explicit mutants.

## Manual replay in an isolated source copy

Use Linux x86_64, Rust 1.99.0, LLVM 19.1.7, jobs 2 and CARGO_INCREMENTAL=0. Do not
reuse an unverified old Cargo test binary. A clean external target and an added
source-content marker are required for fresh independent evidence. Normal
repository source-array and verifier production gates remain closed.

For historical reproduction, materialize a fresh source archive or isolated
copy of the public commit 8251d2a96dd78e26ed63bd25ec9353b111cc28b5. Its tree is
ef54c68a3688e855325f5879e55c124fdf6e91d6, exactly equal to the originally tested
local head 3882ccd174e361ef1a1dcfa030e12740cb139d93. Keep this packaged input
directory available separately while preparing that historical source copy.
Do not switch an existing user worktree merely to reproduce this evidence.

In the isolated copy, copy sources/reviewer-array-native-v1.rs to
src/frontend/oir/owned/unit2d_independent.rs and append a cfg(test) module entry
to native_tests.rs using #[path = "unit2d_independent.rs"]. The module is a child
of native::tests and reuses its existing Scratch/LLVM/source-free ELF harness.
Append the checkpoint1 controls and old exporter to native_tests.rs for the
historical controls. These append operations are test-only and should not be
performed in a user worktree. Permanent registration is a later reviewed change.

Create fresh evidence directories and set OXID_OWNED_NATIVE_EVIDENCE,
OXID_UNIT2D_EXEC_EVIDENCE and OXID_UNIT2D_OLD_IR_OUTPUT. Build the test binary,
verify its exact inventory and marker, then run the independent_unit2d ordinary
filter. Run each ignored native family explicitly with one test thread. Compare
the exporter files/TSV against the frozen expectation hashes. The exporter is an
explicit evidence action; it is not suitable as an unconfigured ordinary Cargo
test until repository integration supplies its output directory.

The transfer family emits 18 chain/selfchain modules. From tools/storage run:

    python3 -B -m unittest -v test_observe_storage.py
    python3 -B prepare_checkpoint2.py --input CHAIN_MODULES --output NEW_BUNDLE --source-commit EXACT_HEAD
    python3 -B verify_checkpoint2.py NEW_BUNDLE

Set OXID_UNIT2D_EXTERNAL_OBSERVERS to NEW_BUNDLE. The unchanged Rust executor
reads manifest.tsv; use an identical-byte copy or relative symlink from the
generated harness.tsv. Execute independent_unit2d_external_storage_observer_llvm
with --ignored. Every observed module, ELF, command, stdout/stderr and status
must be retained, including the 11 expected status-1 mutant executions.

The optional native_tool_logger.py requires OXID_UNIT2D_TRUSTED_TOOLS pointing to
a JSON map of llvm-as/opt/clang/ld.lld to verified official tool paths, and
OXID_UNIT2D_TOOL_RECEIPTS for logs. Invoke it through symlinks named for each tool.
It preserves argv/output/exit status and records every requested LLVM command.

## Status and limits

The passed source inputs produced 331 ELF artifacts and 1,205 source-free runs;
see qualification.json for disjoint counts. Those are historical fresh evidence,
not a claim that this newly packaged directory has been automatically replayed.
Full release/integration review and final Unit2D acceptance remain separate.
Any mechanical formatting, harness registration or portability adjustment must
be reviewer-owned, versioned, and replayed before replacing these frozen bytes.
