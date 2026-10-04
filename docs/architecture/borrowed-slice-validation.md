# Borrowed scalar slices: local validation

The implementation follows [RFC 0019](../../rfcs/0019-borrowed-scalar-slices.md).
It is an experimental `typed-preview` capability, not a completed milestone,
stable ABI, general slice implementation, or cross-platform certification.

## Source and scope

The functional checkpoint is commit
`5e4fadbd227330c374ae4801f2af60db0f4d4ce6`, tree
`dc81a9dd379e0d5f331177260d6b4d0dac95f4dc`, based on main
`c5798a232ebdacaf720d580007ee8d760957a081`. Later documentation, explicit CI
registration, and the checked-in sample do not change its compiler bodies.

Local execution used Linux x86_64 with LLVM/Clang/LLD 19.1.7 at O0. Both debug
and release Rust compiler builds were exercised. Hosted CI and the immutable
source-binding successor are separate requirements; older array, record, and
project qualification reports are evidence only for their recorded inputs.

## Observable results

- One shared `sum(xs: &[i32])` body accepts backing arrays of lengths 2, 3,
  and 0 and returns the independently specified combined result 312
- Exclusive mutation and explicit shared/exclusive reborrowing produce 515,
  with access restored to the caller's original owners after normal returns
- The same behavior works across declared modules and in source-free native
  executables with a cleared environment
- Bool/unit elements, empty arrays, signed negative and upper-bound indexes,
  mixed fixed/slice views, parent suspension, moved owners, argument ordering,
  RHS snapshots, and division-failure precedence have dedicated controls
- Reverse slice-to-fixed conversion, mismatched scalar elements, bare reference
  forwarding, escaping references, ranges, and overlapping exclusive views
  remain rejected

The [three-module sample](../../fixtures/typed-slice-samples/README.md) is included
by the public test target, so its checked-in source bytes are tested directly.

## Verification

At the functional checkpoint, `cargo test --locked` passed 1,057 tests with 37
explicitly ignored native tests. All-target/all-feature Clippy with warnings
denied, `cargo fmt --all -- --check`, and a locked release build passed.
The historical raw/runtime representation assertions remained unchanged.

Focused new checks passed:

- 11 source/type/raw-verifier tests, including independent corruption of loan
  target and source authority views, malformed nominal identity, and reverse
  narrowing controls
- Four native structure/resource tests, including 64 source slice parameters,
  bounded flattening, emission limits, and unchanged exact-reference ABI
- Nine initial ordinary public tests, followed by a tenth test for the
  checked-in three-module sample
- The public source-free native gate in debug and release
- Three internal slice-native groups in debug and release. Each profile covers
  32 ELF artifacts and 104 executions, including fuel-before-bounds behavior
- The broader native ordinary regression set: 52 passed, 19 explicitly ignored

The reference handle remains 64 bytes and the compact borrowed descriptor is
8 bytes. Native length storage adds four bytes only for each slice reference
or loan; reference runtime storage, plan metadata layouts, and semantic fuel
are unchanged. Dynamic bounds are checked before element address formation.
No `noalias` or new unchecked `inbounds` claims are introduced.

The explicit CI step runs both the public and internal slice-native gates in
both profiles. Its registration does not establish a hosted result.

## Qualification boundary

The existing source-binding preflight correctly rejects changed compiler inputs
until a new immutable successor is recorded. That failure must not be fixed by
weakening historical hashes, archived expectations, or producer/consumer
identity checks. A successful local feature test does not replace that binding,
independent review, or the applicable exact-head CI results.
