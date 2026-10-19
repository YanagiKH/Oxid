# Experimental byte-buffer and binary-I/O contract

Status: experimental integration candidate, not stable v1 or unrestricted
container support. Canonical source operations, checked call-only views,
resource cleanup and binary attempts are implemented in both consumers.
Reference execution and LLVM emission share the same multi-package `oxrev`
source. Focused Linux x86_64 / LLVM 19.1.7 native application and clean deployment
acceptance passed for the seven fixed binary inputs in the example. Three
ordinary resource-lifecycle programs also matched reference execution. These
checks do not establish unrestricted container support, complete fatal-path
coverage, cross-platform qualification, or stable v1 completion. C runtime
component tests remain separate evidence from native Oxid application execution.

## Source contract for integration

The move-only resource is `alloc::bytes::ByteBuf`, with these ordinary
canonical functions in `alloc::bytes`:

```text
new() -> ByteBuf
len(&ByteBuf) -> i32
capacity(&ByteBuf) -> i32
try_reserve(&mut ByteBuf, additional: i32) -> GrowStatus
try_push(&mut ByteBuf, byte: u8) -> GrowStatus
try_extend(&mut ByteBuf, source: &[u8]) -> GrowStatus
try_resize(&mut ByteBuf, length: i32, fill: u8) -> GrowStatus
truncate(&mut ByteBuf, length: i32) -> ()
```

`GrowStatus` alternatives are `Ok`, `CapacityOverflow`, `AllocationFailed`,
and `MemoryLimit`. Lengths and indices remain checked `i32` values. Negative
arguments fail at runtime; they never wrap. `truncate(n > len)` is a no-op.
Reserve zero, empty extend, and same-length resize neither allocate nor change
the value. Capacity is implementation policy, not a stable result guarantee.
There is no implicit clone.

The call-only views are `&b`, `&mut b`, `&b[start..end]`, and
`&mut b[start..end]`, also for a named record field. Endpoints evaluate once in
source order, then check `0 <= start <= end <= len`. The parameter's type selects
an exact buffer reference or a byte-slice view. Views cannot escape, be stored,
or resize the buffer. Whole-root loans conservatively reject self-extend and
resizing, moving, or releasing an owner with a staged view.
Forwarding a reference parameter retains explicit dereference syntax:
`&*p[start..end]` or `&mut *p[start..end]`. Both endpoints are mandatory.

Binary operations are `std::io::read_some(&mut [u8])` and
`std::io::write_some(&[u8])`. Read alternatives are `Data(i32)`, `Eof`,
`Error(i32)`; write alternatives are `Written(i32)`, `Error(i32)`. Their source
type names are `std::io::ReadSomeStatus` and `std::io::WriteSomeStatus`.
The existing canonical `std::io::ReadStatus` and `WriteStatus` retain their
identities, alternatives, and behavior for `read_stdin` and `write_stdout`.
The new names have distinct canonical identities in the source catalog.

## Allocation and representation

The descriptor is 16 bytes, aligned to 8: a private 64-bit allocation identity,
an `i32` initialized length, and an `i32` capacity. Identity zero means no
allocation, with zero length/capacity. Both component implementations use a
monotonic execution-local identity, validate membership before dereferencing,
and compare the descriptor's capacity/initialized extent with registry facts.
Identity exhaustion is terminal; an identity never wraps or gets reused within
an execution. These integers are not source-visible handles. Descriptor moves
must transfer an obligation, never clone allocation bytes.

The logical width is three cells. Resource leaves have their own
checked traversal, distinct from `ScalarLeaf`. Declarations compute enum
alignment as the maximum of 4 and the payload alignments, use a common payload
offset `align_up(4, maximum_payload_alignment)`, and round the complete size to
that alignment. A ByteBuf payload starts at offset 8. Existing alternatives
with alignment at most 4 keep their layouts and charged operations unchanged.
Record fields derive alignment normally. Inactive alternatives are never
traversed as live resources. Resource-bearing declarations require the closed
canonical ByteBuf inventory even in unused functions and declarations.

Each allocation is one fallible request containing a 32-byte intrusive header
followed by the requested usable capacity. The header stores next/previous
pointers (16 bytes), identity (8), capacity (4), and initialized length (4).
The per-execution normalized policy is:

```text
sum(live allocation requested capacity + 32) <= 16 * 1024 * 1024
```

Growth checks the simultaneous old-plus-new peak, including both headers. It
does not use `realloc`. The new header is linked and accounted immediately
after allocation, with no subsequent fallible registry operation. A failed
growth preserves the old identity, bytes, length, capacity, and ownership.
Generation consumption and diagnostic ledger counters are private and may
advance on a rolled-back attempt.

For the supported 64-bit host, actual heap requests are also `capacity + 32`.
Allocator bookkeeping, allocator rounding, fragmentation, and RSS are not
measured. The release-build execution context is 64 inline bytes in either
implementation, separate from the heap payload budget; test-only fault state
adds 16 bytes in Rust and 8 in C. Each owner descriptor is 16 bytes in ordinary
activation storage. Integrating consumers must admit the execution context and
owner storage under their existing limits without changing those limits.

Successful growth chooses `max(required, max(8, saturating_double(capacity)))`,
with saturation at `i32::MAX`. No capacity byte above length is exposed.
Resize initializes every newly exposed byte, including after shrinking.

## Failure order and cleanup protocol

After existing caller/staging charges, operations validate the descriptor,
validate signed arguments, check required-capacity arithmetic, admit the heap
peak, check identity availability, and prepay dynamic byte work. Overflow and
heap refusal return statuses before attempting allocation. Insufficient fuel
is terminal and precedes allocation or mutation. Allocation failure returns
`AllocationFailed`; prepaid fuel is not refunded. Copying and filling happen
only after the last fallible step.

Dynamic byte work is the initialized old length copied on growth plus the
newly appended/filled byte count. Truncation, no-ops, and physical release add
no dynamic work. Each new canonical primitive instruction costs four logical
units, in addition to ordinary call/staging/teardown charges and dynamic byte
work. Existing instructions retain their charges. Test-only failure points are allocation, initialization
after registration, and commit before mutation; rollback releases the new
allocation without touching the old value.

The cleanup protocol releases initialized locals in reverse
declaration/completion order, fields in declaration order, and only the active
enum alternative. Staged constructor fields, arguments, and results remain
owners until transfer. Returning transfers its result before cleanup.
Replacement evaluates the new value first, then releases the old one, including
the moved-out case `b = relay(b)`. Moved/uninitialized places never release.

Registry unlink and free are allocation-free and do not spend user fuel.
Execution teardown walks the intrusive registry without reading descriptors;
the Rust heap also does this in `Drop`. Resource-bearing native emission calls
the registry drain before terminal diagnostics. Both consumers check for
outstanding obligations on normal completion before the backstop can hide a
missing ordinary release. Focused native normal-completion checks cover record/enum moves, replacement,
discard, scope exits and initialized shrink/regrowth. A successful normal
completion establishes the zero-live gate on those paths, not exact allocation
counts or complete fatal-path coverage.
This is built-in resource reclamation, not user destructors
or general exception unwinding. Fatal cleanup has no order guarantee and no
SIGKILL or asynchronous cancellation guarantee.

## Binary attempt boundary

Linux x86_64 is the only execution host for these adapters. Each nonempty
attempt makes one unbuffered syscall over exactly the supplied extent.
Interrupted attempts return Linux errno 4 (`EINTR`) without retrying. Other
errors preserve the immediate errno. A nonempty successful read reports
1 through length; zero means EOF. Empty read returns Data(0) and empty write
returns Written(0), without a syscall. A write can report short or zero progress.
Ordinary source `write_all` must treat zero progress as a write-zero error.

The primitive attempt boundary includes delivery of its status into the direct
caller's already-admitted result storage, including ignored results and results
staged as arguments to another call. It does not include later caller statements,
user wrapper returns, or subsequent calls.

After ordinary argument preparation, call admission, local storage activation,
and the four-unit primitive instruction charge, the consumers prepare the direct
return before external I/O. Preparation validates the local and caller result
storage, the canonical closed status layouts, required owner generations, the
call continuation, and release of the call's loan. It validates the supplied
initialized byte view and signed extent. Native result pointers and loan
authority come from the sealed call ABI; reference runtime identities are
checked before the attempt. No producer-supplied layout or prepayment flag
grants this authority.

Admission then prepays both the full supplied extent and the canonical
primitive's immediate `ReturnOwned` cost, derived from the sealed execution
plan. The existing return formula currently gives six logical units for these
width-two status results. Successful calls retain their existing total charge;
the return debit occurs before the external attempt. Bounds/validation or
admission refusal performs no I/O and leaves the destination bytes untouched.
Empty attempts still require result-delivery admission and make no syscall.

After admission, the one attempt produces a closed status and commits it into
the prepared caller storage. This completion performs no allocation or fallible
fuel, storage, layout, generation, or loan-release check. The primitive's frame
then completes without a second return debit. Only the witness-identified
`ReadSome` and `WriteSome` functions use this path. Ordinary source returns,
including returns from wrappers around these primitives, keep their existing
checks; a later failure cannot roll back the already completed I/O.

Preparation uses fixed control storage rather than a heap allocation or new
owner/frame fields. Reference execution admits `size_of::<ControlCarriers>()`
for its prepared delivery, preparation result, byte region, status and loan
roles against the existing execution byte ceiling. Native admission adds the
C adapter's asserted 16-byte inline delivery carrier to runtime control storage.
These are separate from dynamic payload capacity, leave logical activation
cells and owner layouts unchanged, and are not machine-stack or RSS guarantees.

Only reported read bytes are modified. Earlier I/O effects cannot be rolled
back by later failure. There is no decoding, hidden prefetch, or scalar-result
printing. Process entry still owns signal setup.

## Integration and remaining acceptance

Canonical imports have a closed catalog distinct from user declarations.
The existing ReadStatus/WriteStatus identities and variants are unchanged;
the binary attempt results are ReadSomeStatus and WriteSomeStatus. A checked
import inventory appends canonical signatures and enum identities, with an
independent raw descriptor check for every synthetic function. A spelling,
diagnostic span, or producer-supplied operation is never allocator authority.
Supplemental compiler control storage is admitted only for this experimental
catalog, within the existing compiler ceilings and frozen enclosing carriers.

Both consumers dispatch verified primitive instructions and stage checked
views. Declaration containment, align-8 layouts, active resource traversal,
and cleanup do not independently grant builtin authority. Malformed resource
instructions reject even in unused functions. Minimal `new`/`len` imports use
the same paid source path without requiring a status-enum import.

The application in `examples/projects/oxrev` is an ordinary multi-package `oxrev`: library source
implements read-all and write-all with initialized chunks and subranges, returns
an enum containing a record with ByteBuf, and invokes a separate package's
mutable-slice reversal. The same source must execute through reference and
LLVM/native paths, with source-free native deployment. Its interpreter manifest
route supports explicit process entry and writes diagnostics to stderr without
a result trailer. `scripts/accept_oxrev.py` records the seven independent binary
oracles and has a separate native build/package/deployment mode requiring an
already installed LLVM 19.1.7. Native execution, deployment and native dynamic
checking are pending independent acceptance. Local logs are not tracked here.
