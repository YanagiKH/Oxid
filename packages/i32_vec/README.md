# I32Vec: an ordinary-source specialized numeric container

This typed-preview package stores signed i32 elements as four little-endian
bytes each in a private owned `ByteBuf`. It is a specialized container, not a
generic `Vec<T>` implementation or a new compiler/runtime primitive.

## Public API

- `I32Vec`: opaque owned record, with no public fields.
- `Element::{Value(i32), Missing}`: copied scalar value or absent element.
- `FromBytes::{Ok(I32Vec), Invalid(ByteBuf)}`: ownership-preserving import.
- `new_vec() -> I32Vec`: empty vector.
- `len(&I32Vec) -> i32`: element count, not byte count.
- `get(&I32Vec, i32) -> Element`: copied snapshot, with negative/out-of-range
  indexes returning `Missing`.
- `set(&mut I32Vec, i32, i32) -> bool`: replace an existing element; invalid
  indexes return false without changing the vector. Encodes all four bytes
  before writing any destination byte.
- `try_push(&mut I32Vec, i32) -> alloc::bytes::GrowStatus`: encode one four-byte
  stack array and perform exactly one existing ByteBuf `try_extend` call.
- `pop(&mut I32Vec) -> Element`: decode the last element before truncating four
  initialized bytes; empty vectors return `Missing`. Retains buffer capacity.
- `from_le_bytes(ByteBuf) -> FromBytes`: consume input and accept exactly when
  its initialized byte length is divisible by four. All four-byte patterns
  represent an i32. Invalid input returns the original unchanged owner.
- `into_le_bytes(I32Vec) -> ByteBuf`: consume the vector and transfer its owner
  using the existing `take(&mut owned.bytes)` operation.

Import and consuming export do not copy the initialized byte payload or create
another payload allocation. Capacity need not be divisible by four, including
for reserved empty buffers. There is no mutable byte export, escaping borrow,
implicit clone, iterator, arbitrary insertion/removal, or generic element type.

## Representation, failures, and costs

The initialized byte length is always four times the element count. Both index
bounds are checked before multiplying by four. Each encoded byte uses a
normalized signed remainder in 0..255. Subtracting that remainder is safe even
at i32::MIN because MIN is divisible by 256. Decoding starts with the high byte
interpreted as signed, then accumulates descending bytes without exceeding i32.
The maximum representable initialized length is 2,147,483,644 bytes, or
536,870,911 elements; existing memory, fuel, and admission limits can be much
lower. This bound is not a promise that such an allocation succeeds.

Package-level arithmetic and byte work for len/get/set/pop and import/export
are constant-size. Push has constant-size encoding plus ByteBuf growth costs:
a reallocation can copy the existing payload, and peak memory includes old
and new allocations and allocator overhead. Existing buffer-owner validation,
allocation bookkeeping, execution fuel, memory quotas, and native admission
costs still apply; no whole-runtime constant-time guarantee is made.

The existing recoverable growth statuses (`CapacityOverflow`,
`AllocationFailed`, `MemoryLimit`) are propagated without translation; the
ByteBuf recoverable-failure contract preserves initialized bytes, length,
capacity, and ownership. Terminal execution/fuel failures are not recoverable
statuses. There is no transactional rollback guarantee for terminal failure
partway through an operation, including set's destination writes.

## Validation scope

Focused execution against base `ce57d874e6a23c2902a538100c66c2ad27293443`
passed package CHECK (16 functions) and pure-contract CHECK (34 functions).
The pure contracts returned interpreter Result 42 and passed all five
interpreter tests. With LLVM 19.1.7, native BUILD succeeded; its standalone
executable returned exact `42\n` with exit 0 and empty stderr, and native TEST
passed all five tests.

The separate `oxi32sort` client passed CHECK (26 functions) and its fixed
nine-case interpreter and native packaged acceptance. After a test-only
Clippy correction, the final combined acceptance target passed both tests
with native coverage enabled; scoped rustfmt and Clippy with warnings denied
also passed. See the client README for the limits of this evidence.

These are focused results, not a full repository CI pass or completed R2.
Recoverable allocation-failure injection, capacity/fuel qualification,
terminal-failure behavior, and other outstanding qualifications remain
unverified. This remains an ordinary-source specialized I32Vec, not a generic
Vec implementation. No compiler or runtime changes are included.
