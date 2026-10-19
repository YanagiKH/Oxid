# I32Vec pure contracts

This ordinary-source typed-preview project depends on `packages/i32_vec` via
a relative path and performs no I/O. Five `test_` functions return zero for
success; `main` runs all five and returns the fixed Result value 42.

Coverage:

- Empty pop/get, negative and extreme signed indexes, upper bounds, invalid
  set preserving contents and length.
- Set encoding i32::MIN while preserving adjacent elements and exact bytes.
- Empty and reserved-empty imports, owned record/ByteBuf helper returns,
  consuming export, and retained capacity.
- Invalid initialized lengths 1, 2, 3, and 5 returning the original bytes and
  capacity through the owned enum arm.
- Sixteen signed boundaries, fixed little-endian bytes, push/get/export/import
  round trips, reverse pop order, lengths after each pop, empty pop, and valid
  imports whose capacity need not be divisible by four.

The minimum value is written `-2147483647 - 1`; this avoids depending on parsing
a positive literal outside i32. Expected bytes are independent fixed constants,
not derived by the package under test. This suite has no allocation-failure
injection and does not prove allocation identity merely by comparing capacity.

Run from the repository root using the existing CLI:

```sh
oxid check --manifest-path examples/projects/i32_vec_contracts/oxid.toml
oxid test --manifest-path examples/projects/i32_vec_contracts/oxid.toml
oxid run --manifest-path examples/projects/i32_vec_contracts/oxid.toml --engine interpreter --entry-mode result
oxid test --manifest-path examples/projects/i32_vec_contracts/oxid.toml --engine native
oxid run --manifest-path examples/projects/i32_vec_contracts/oxid.toml --engine native --entry-mode result
```

## Focused execution status

CHECK passed with 34 functions; the dependency package CHECK passed with 16.
Interpreter TEST passed all five tests and Result returned 42. With LLVM
19.1.7, native BUILD succeeded and the standalone executable returned exact
`42\n`, exit 0 and empty stderr under the process driver with cwd `/` and an
empty environment. Native TEST also passed all five tests.

These focused package-stage results do not establish a full repository CI
pass, completed R2, recoverable allocation-failure injection, capacity/fuel
qualification, terminal-failure behavior, or other outstanding qualifications.
The source and fixed expectations are unchanged from that validated stage.
