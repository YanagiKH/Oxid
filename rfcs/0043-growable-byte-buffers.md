# Growable byte buffers and binary I/O

Status: experimental in `typed-preview`. Native execution and binary I/O target
Linux x86_64 with LLVM/Clang/LLD 19.1.7. This contract does not define a stable
ABI or a general container library.

## Buffer operations

`alloc::bytes::ByteBuf` owns a growable sequence of initialized bytes. It is
move-only and may be stored in records or enum payloads. Ordinary functions in
`alloc::bytes` provide:

```text
new() -> ByteBuf
len(&ByteBuf) -> i32
capacity(&ByteBuf) -> i32
try_reserve(&mut ByteBuf, additional: i32) -> GrowStatus
try_push(&mut ByteBuf, byte: u8) -> GrowStatus
try_extend(&mut ByteBuf, source: &[u8]) -> GrowStatus
try_resize(&mut ByteBuf, length: i32, fill: u8) -> GrowStatus
truncate(&mut ByteBuf, length: i32) -> ()
take(&mut ByteBuf) -> ByteBuf
```

`GrowStatus` has `Ok`, `CapacityOverflow`, `AllocationFailed`, and `MemoryLimit`
alternatives. Growth failure preserves the original bytes, length, capacity and
ownership. Negative lengths fail; they never wrap. Truncating to a length above
the current length does nothing. Reserve zero, empty extend and resize to the
current length neither allocate nor change the value. Growing after truncation
initializes every newly exposed byte. Spare capacity is never readable.

Capacity growth is an implementation choice. There is no implicit clone,
implicit numeric conversion, byte arithmetic or buffer literal. Existing
`u8` conversion rules apply to values passed to these functions.

`take` transfers the buffer's allocation, length and capacity to its result and
leaves the source empty with zero capacity. It neither allocates nor copies
initialized bytes, and works at the live-memory limit. The source remains
usable. It requires an exclusive buffer borrow; existing overlap and ownership
rules apply. See [owned buffer transfer](0046-owned-buffer-transfer.md).

## Borrowing and ranges

Call arguments can borrow an entire buffer with `&buffer` or `&mut buffer`, or
an initialized range with `&buffer[start..end]` and
`&mut buffer[start..end]`. Named record fields support the same forms. The
parameter selects an exact buffer reference or a byte-slice view. Both range
endpoints are required, evaluate once in source order, and satisfy
`0 <= start <= end <= len`.

Reference forwarding uses explicit dereferencing: `&*parameter[start..end]` or
`&mut *parameter[start..end]`. Views cannot escape a call, be stored or resize
the owner. Whole-root loans reject overlapping shared/exclusive access,
self-extend, or moving, resizing or releasing an owner while a view is staged.
Partial moves, element references, field-disjoint loans and stored references
remain outside this contract.

## Resource ownership and cleanup

Locals, constructor arguments, call arguments and return values each retain one
ownership obligation until they transfer it. Returning transfers the result
before local cleanup. Replacement evaluates and validates the incoming value
before releasing the previous value, including a moved-out destination such as
`buffer = relay(buffer)`. Resource-containing constructors validate owned inputs
before transferring their first resource.

Normal cleanup releases initialized locals in reverse declaration/completion
order, fields in declaration order, and only the active enum alternative.
Moved or uninitialized places do not release. Normal completion checks for
outstanding allocations before the execution-wide cleanup backstop. Fatal
cleanup drains the allocation registry without trusting possibly damaged
owner descriptors; it requires no allocation or user fuel.

This cleanup is specific to built-in resources. It does not introduce user
destructors, exception unwinding or guarantees after asynchronous termination.

## Allocation policy

The current descriptor is private: an allocation identity, initialized length
and capacity. Its 64-bit-host layout occupies 16 bytes with alignment 8; it is
not a source-visible pointer or handle. Resource layouts derive record and enum
alignment, while existing resource-free scalar layouts remain unchanged.

Allocation identities are execution-local, monotonic and never reused or
wrapped. Registry membership, capacity and initialized extent are checked
before dereferencing a descriptor. Empty, unallocated buffers use identity zero
and zero length/capacity.

Each allocation requests a 32-byte intrusive header followed by usable capacity.
The normalized per-execution limit is:

```text
sum(live requested capacities + 32 bytes per allocation) <= 16 MiB
```

Growth admits the simultaneous old-plus-new peak, including both headers. It
uses a separate fallible allocation rather than `realloc`; the old allocation
remains live until the new value commits. Allocation failure or a pre-commit
failure reclaims staging storage and leaves the old value intact. Physical
allocator bookkeeping, fragmentation and process RSS are not measured by this
normalized limit. Activation and compiler control storage have separate checks.

The current growth policy chooses the larger of the required length, eight
bytes, and twice the old capacity, saturating at `i32::MAX`. This policy is not
a stable capacity guarantee.

After caller/staging charges, each operation validates the descriptor and
signed arguments, checks capacity arithmetic and the heap peak, checks identity
availability, then prepays byte work before mutation or allocation. Overflow
and heap refusal return statuses. Insufficient execution fuel is terminal.
Prepaid fuel is not refunded after an allocation failure. Dynamic byte work is
the initialized old length copied during growth plus newly appended or filled
bytes. No-ops, truncation and physical release add no dynamic byte work. New
canonical primitive instructions cost four logical units; existing instruction
charges and execution limits remain unchanged.

Containment information uses a paid, memoized declaration summary. Each record
or enum is visited a bounded number of times, even when many variants share the
same nested suffix. Resource-free programs allocate no resource summary and
keep the preceding declaration-table accounting. Resource-bearing summaries
count their actual allocated capacity against the existing table limit.

## Partial binary I/O

```text
std::io::read_some(&mut [u8]) -> ReadSomeStatus
std::io::write_some(&[u8]) -> WriteSomeStatus
```

`ReadSomeStatus` has `Data(i32)`, `Eof`, and `Error(i32)` alternatives;
`WriteSomeStatus` has `Written(i32)` and `Error(i32)`. These identities are
distinct from the existing `ReadStatus` and `WriteStatus` used by bounded
`read_stdin` and `write_stdout`; those operations retain their prior behavior.

Each nonempty attempt makes one unbuffered Linux syscall over the supplied
initialized extent. Short reads and writes are ordinary results. `EINTR` returns
`Error(4)` without retrying; other errors preserve the immediate errno. A
nonempty read returning zero means EOF. Empty reads return `Data(0)` and empty
writes return `Written(0)` without a syscall. A write returning zero is visible
to the caller, which decides whether to retry or report failure. No bytes above
the reported read count are changed by transport.

These operations require process entry mode. The primitive validates its view,
result storage, generation changes and loan release, and prepays both the full
supplied extent and its immediate return before the syscall. Returning the
sealed status then performs no allocation or fallible admission. A later user
instruction can fail after a completed I/O attempt; its earlier external effect
is preserved. Process entry returns an exit status without printing a value
trailer. Diagnostics use stderr.

## Compiler and runtime boundary

The canonical built-in catalog supplies exact allocator and I/O identities.
Matching a spelling, diagnostic span or raw instruction does not grant resource
authority. Independently checked declarations, signatures and operations are
required, including unused functions. Existing old scalar and owned-enum
compatibility checks remain separate from new resource acceptance.

Reference execution and native emission use the same checked source program.
Resource-bearing native modules use guarded execution and the embedded buffer
runtime. Development builds compile program and runtime at O0; release builds
use O2 with the same checks. Native object caching reuses only the verified
program object; runtime compilation and linking remain fresh.

The [byte reversal application](../apps/byte-reverse/README.md) exercises real
workspace packages, buffer growth, byte ranges, record/enum transport and
partial I/O. Its independent controller compares output with Python byte
reversal, including embedded NUL, all byte values and growth boundaries. Native
checks also cover source-free deployment and actual I/O failures. C component
checks, compiler compatibility checks, application checks and native corruption
controls have separate scopes; none alone establishes a completed language or
release.

## Compatibility and limits

The default legacy edition remains unchanged. This extension is opt-in and
retains bounded execution, source, compiler and native storage limits. It adds
neither generic containers nor text/Unicode semantics, concurrency, new native
targets, a stable allocation ABI or general unsafe/FFI support. The descriptor
and runtime interfaces are private implementation details.
