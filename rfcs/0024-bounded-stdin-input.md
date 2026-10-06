# RFC 0024: bounded stdin input into an existing i32 buffer

Status: **public API proposal; no compiler or host-input activation**. Updated
2026-10-06 against merged PR37 main `ad45dc1e6da6f9f4eb86aa85c738de256a02c0ee`
and the PR38 expression tree `b57306e9e0cbb90a2df9fa8e0511bf01a9075a9f`.
A separate disconnected Rust model exercises the internal byte/fuel/commit
contract. That model is not a reference-runtime or native implementation.
Public binding and measured production representation remain unresolved.

## Outcome and boundary

Compile the expression component once, then run that identical ELF with distinct
stdin byte streams. It should return 39 for `12 + 3 * (4 + 5)` and 63 for
`7*(8+1)`, with matching public reference execution. This removes its current
source-generation/recompilation requirement without claiming compiler
self-hosting or a production compiler provider.

Introduce one synchronous input operation on an existing exclusive `&mut [i32]`
view. Keep zero-argument scalar main, the existing scalar result printer, current
array capacities and all compiler/runtime ceilings. No strings, Unicode decoder,
file paths, arbitrary descriptors, general FFI, command-line argument API,
output formatting, heap collection, enum aggregate payload or recursion feature
is included. The operation is an input effect inside the existing verified owned
OIR/reference/native paths, not a third execution runtime.

## Choosing the smallest input contract

| Candidate | Boundary | Consequence |
|---|---|---|
| One host `read` call | May return a short prefix even without EOF | Exposes scheduling-dependent chunking and forces every source program to loop correctly; not the chosen public abstraction |
| **One source operation: fill the bounded buffer or reach EOF** | Internally handles short reads; stops at capacity without probing beyond it | Chosen. Raw bytes, explicit EOF/full distinction, no line or encoding rules |
| Read through EOF with a capacity-plus-one probe | Detects over-capacity input directly | Implicitly consumes a byte outside the destination capacity; unnecessary when the application can supply one explicit witness cell |
| Line input | Stops at a newline | Adds delimiter retention, LF/CRLF/lone-CR handling and line-truncation policy; unnecessary for the existing whitespace grammar and batch evaluator |

“One operation” is not “one syscall.” It can block until capacity or EOF.
Waiting for host input is outside execution-fuel wall-clock guarantees. The
initial application is batch input through a pipe/file redirection, not an
interactive line editor.

## Result and buffer semantics

The conceptual signature is `read_stdin(buffer: &mut [i32]) -> ReadStatus` with
one compiler-owned nominal enum:

```text
enum ReadStatus { Eof(i32), Full, IoError }
```

These names illustrate the API; they are not permission to recognize an ordinary
user function by its spelling. The binding choice below remains a blocking RFC
question. Variants use the already accepted nullary/scalar enum rules.

Let C be the checked slice capacity, 0..1024. The operation accumulates bytes in
bounded private staging storage and has the following observable outcomes:

- `Eof(n)`: EOF was observed after staging n bytes, where 0 <= n < C. Commit those
  n bytes as i32 values 0..255 to indexes 0..n; preserve all remaining cells.
- `Full`: C bytes were staged. Commit all C cells and return immediately. **Do
  not read an extra byte to determine whether EOF follows.** Full means capacity
  reached, not “more input definitely exists.”
- `IoError`: a non-interruption host read error occurred. Do not change any
  destination cell. Previously consumed input cannot be restored. Expose no
  platform-specific errno or undocumented partial-count convention.
- C = 0: return Full after the operation's base charge, without staging bytes or touching stdin. It cannot establish EOF. A native
  activation may already reserve shared scratch under its frame admission.

When the stream has exactly C bytes followed by EOF, the result is Full. A later
positive-capacity invocation can observe `Eof(0)`. This avoids hidden lookahead.
No newline is removed or normalized. NUL, CR, LF and bytes >=128 are ordinary
input bytes; the expression grammar decides whether they are valid.

On Eof/Full, complete buffer mutation occurs only after the read phase has
succeeded. On IoError, or resource/fuel failure inside this input operation before
commit, the buffer is unchanged.
This is memory atomicity, **not rollback of external input consumption**. An
error after several reads can leave stdin advanced while preserving the old
buffer. A later ordinary result transfer, caller operation or cleanup can still
exhaust fuel after a successful input commit; earlier effects then remain, as for
other existing operations. This is not a transaction across the entire caller.
Source code must not evaluate the old buffer as new input after an input failure.

## No-overread and deterministic read attempts

Each actual host read requests **one byte**, directly through an unbuffered OS
adapter. A positive read stages exactly that byte. A zero return means EOF. An
interrupted read stages nothing and retries through the normal fuel debit. Any
other read error yields IoError. Stop as soon as n = C. Linux distinguishes
short successful reads, EOF, and EINTR before any data is read; an error does not
generally promise unchanged file position. [Linux read(2)](https://man7.org/linux/man-pages/man2/read.2.html)

This deliberately simple first implementation accepts at most C successful
bytes per operation and makes read-attempt fuel independent of pipe chunking.
Its no-overread promise means bounded one-byte requests, no prefetch, and no
post-capacity/EOF probe. On IoError it does not promise that the failing host
operation preserved stream position; the adapter cannot strengthen that host
guarantee or roll back earlier input. It does not
use buffered `getchar`, a buffered Rust stdin reader, prefetching, `read_to_end`,
line readers or hidden lookahead. Such facilities may advance the underlying
stream beyond the requested prefix. Rust stdin handles explicitly share a
global buffer, so that API alone does not establish this boundary.
[Rust stdin documentation](https://doc.rust-lang.org/std/io/fn.stdin.html)
Later bulk-I/O optimization requires an
explicitly reviewed preservation rule; it is not silently interchangeable with
this initial attempt/fuel contract.

The operation does not close stdin, alter its flags or file position by seeking,
change process signal policy, or install persistent input buffering. It operates
on the inherited standard-input stream only. Nonblocking errors are IoError;
this increment does not add polling or asynchronous scheduling.

## Evaluation, borrowing and exact fuel order

Ordinary argument evaluation and call-only exclusive borrowing keep their
existing left-to-right order and costs. Wrong element types, missing exclusive
authority, unavailable owners and invalid projections are rejected by existing
typing/raw verification. In reference execution, before deriving C or addressing storage, the input
operation revalidates the active loan/view, epochs, projected provenance and
exclusive authority. Forged or stale reference views fail before input
consumption or destination mutation. Native execution retains the existing
verified pointer-plus-length ABI: independent raw verification and lowering
establish exclusive provenance, and the operation checks permitted capacity and
preflights its destination/result storage before reading. It does not acquire a
new runtime ownership or epoch sidecar. These are distinct enforcement paths;
the standalone Rust slice model does not prove either one.

Proposed core cost is **4 + C + A**, where A is the number of attempted
one-byte host reads, including EOF, interruption and error returns. This consists
of 1 + C for the bounded operation/commit reservation, the existing width2 enum
construction amount of 3, and A for attempts. Ordinary call/loan handling and
whole-result moves remain additional and unchanged. The result construction
amount is prepaid here; lowering must not charge a second synthetic constructor.

1. Debit 4 + C before staging admission, external reads or destination mutation.
   The C component prepays maximum commit work, even for an early EOF/error;
   the additional 3 prepays result construction before the external effect.
2. Confirm the complete staging reservation, result destination, and all
   storage needed for commit before the first read. Ordinary builtin-frame
   admission/allocation may already have reserved staging before this core
   debit; failure there follows the existing Invoke ordering. Commit and result
   materialization must perform no fallible allocation. A resource failure in
   this preparation follows the existing runtime resource-failure path,
   not IoError, and consumes no input. Zero capacity uses no staging prefix,
   although per-activation reserved scratch remains subject to admission.
3. Before **each** host read attempt, debit one fuel unit. If it fails, do not
   perform that attempt. Earlier successful attempts may already have consumed
   bytes; discard staging and leave the destination unchanged.
4. On Eof/Full, commit the known staged prefix using the prepaid work, then
   materialize the prepaid ordinary enum result. IoError discards staging
   and materializes its prepaid enum result. Do not read unused staging bytes or
   the destination's preserved tail.

There is no extra fuel debit during commit that could leave a partially changed
buffer. Interruption retries cannot spin without fuel consumption, although a
single blocking read has no promised time bound. A skipped call performs no read
and no input-operation debit. Check/format/compile do not consume program stdin.
Existing programs without the operation retain their fuel behavior and ceilings.
Because interruption retries are dynamic, an input operation must enter the
native runtime-fuel discipline even if its source CFG is acyclic. It shares the
existing counter and ceiling with ordinary execution, including the existing
guarded-entry wrapper cells and their admission charges; it gets no second fuel
bucket or invented finite static retry bound. An unused input
declaration must not let unrelated functions bypass existing static admission.
The chosen lowering must make those properties explicit in native planning.

The source-level lowering must make this schedule reviewable: the new operation's
charge, each adapter attempt, prepaid result construction and subsequent ordinary
transfers are distinct. A
raw producer must not supply a trusted byte count, unchecked write offset, host
function pointer, arbitrary descriptor, or a ready-made enum tag as authority.

## Supporting the current expression component

The expression language still accepts at most 128 input bytes. Allocate an
**explicit 129-cell input buffer** so the application, rather than an implicit
runtime lookahead, owns its over-capacity witness:

- Eof(n), n <= 128: parse exactly n bytes.
- Eof(129) is impossible for capacity129 under this operation's contract.
- Full: 129 bytes were read, so report the application's InputLimit(128). Leave
  any 130th and later byte unread.
- IoError: report a distinct application input failure; do not parse the buffer.

An exactly128-byte stream followed by EOF produces Eof(128); exactly129 bytes
produce Full without asking whether EOF follows. This preserves the application's
128-byte rule without a hidden read outside the operation's declared capacity.

Slices currently expose full fixed-array capacity, not a subslice. Add an
application-level used-length parameter (or cursor limit), validate
0 <= used <= backing length and used <=128 before scanning, and use it for all
index/EOF decisions. Do not pad unused cells with whitespace. Keep the old
whole-input parser wrapper for existing fixtures if useful. Its fixed-input
contract and the original enum scanner remain separately testable.

## Binding, representation and trust boundaries

Recommended binding candidate for review: add an explicit compiler-owned
`std` root **only in imports**, admitting precisely these two items initially:

```text
use std::io::read_stdin;
use std::io::ReadStatus;
```

This is a proposed import extension, not currently supported syntax. Calls use
the imported function/alias; matches use the imported nominal type/alias. Existing
crate-absolute imports and `crate::std` source modules keep their meaning. There
is no implicit prelude, general standard-library discovery, filesystem probing,
direct `std::...` expression path, glob import or user intrinsic declaration.
Existing duplicate-name and alias rules should apply, rather than silently
reserving ordinary source function/type names.

The resolver must bind these imports to compiler-owned declaration identities;
the typechecker, source association and raw lowering follow those identities.
An ordinary user function named read_stdin remains an ordinary function. A
spelling comparison in the emitter cannot authorize an external effect.

The identity design is deliberately closed to these two items. Source function
IDs remain unchanged. If the input function is imported, append exactly one
compiler-owned function after source functions; its fixed internal body has one
exclusive i32-slice parameter, result storage, the atomic input operation and an
ordinary owned return. Existing call setup, loan suspension/completion, frame
admission, Invoke and return transfer remain in force. The special input opcode
is valid only in that canonical builtin-origin body, not in arbitrary user raw
functions. Importing ReadStatus alone appends no input function.

ReadStatus has one compilation-local enum ordinal after source enums, admitted
when either item requires it. All aliases use this same nominal identity,
independent of import order. Function-only imports admit the dependency enum
without introducing an unimported local type name. No import means no builtin
enum/function and no input scratch reservation.

Source and builtin declarations have explicit distinct origins. One closed
program descriptor has three states: None, ReadStatus, or ReadStdin (which
implies ReadStatus). Checked origin lookup maps the admitted builtin enum and
function to their canonical trailing rows; it checks dimensions before any
subtraction or indexing. All earlier rows are source-origin. This descriptor is
an untrusted raw claim until shape verification and source association validate
it. Existing row spans hold diagnostic anchors, so the descriptor duplicates
neither IDs nor spans and does not widen every declaration row. Builtin names,
variant names and scalar payload descriptions come from a closed static
descriptor, not a synthetic user AST or parsed virtual file. Index views expose
those names separately from diagnostic spans. A deterministic checked import
endpoint span, selected by module/source order, anchors builtin diagnostics;
it does not define nominal identity. Input-operation and retry failures point
to that anchor. Ordinary call/loan/transfer errors retain their existing source
origins. Per-invocation reattribution of shared builtin-body diagnostics is not
part of this increment.

The independent source association verifies the exact builtin origin, canonical
trailing ordinals, complete fixed descriptors/body and import-derived admitted
set. Merely attaching a builtin label to a user function or matching its spelling
cannot grant an input effect. The raw validator independently checks the closed
operation and body shape. The index, typing and lowering use explicit builtin
identity; none acquires authority from a diagnostic anchor's text.

Actual carrier growth and any old-program admission impact must be measured
and disclosed, not hidden by raising a ceiling. Pricing covers the affected
retained headers, descriptors, capacity reserves and scratch coexistence. No
existing general standard library, external-call registry or intrinsic mechanism
is assumed. Source parsing/typechecking activation remains gated until these
identity and resource checks are implemented and independently validated.

The independent raw validator must prove exclusive i32-buffer access, permitted
capacity/view, known input operation, result shape/nominal identity, and normal
ownership transitions. Source association must tie the effect to the admitted
compiler-owned declaration. Native/reference implementations share this contract
but independently enforce bytes, bounds, failure order and active result payload
safety. No change grants arbitrary external calls through raw OIR.

Measure the new/affected AST, HIR, raw, plan and runtime carriers before choosing
staging representation. Charge full capacity and allocator slack where relevant,
not only bytes eventually read. The selected staging design adds a checked 1024-byte scratch suffix after the
builtin activation's ordinary owner payloads. The reference frame's existing
payload allocation and native owner allocation both include this suffix; no
second scratch allocation or new runtime ownership sidecar is needed. A plan
method derives its checked range only for the canonical builtin function.
Owner extents remain their nominal sizes and cannot address the scratch suffix.
Ordinary activation admission/allocation occurs before the input core charge,
including zero-capacity calls. The operation then confirms this prepared range
before its first read. Native storage is allocated once at function entry and
reused through retries, so source loops do not accumulate dynamic allocations.

Charge the complete 1024-byte reservation, allocator capacity and affected
control carriers in existing physical-byte limits. This changes the applicable
payload-byte bound by an explicit 1024-byte term for the builtin activation;
no-input bounds stay unchanged. Scratch adds no language owner or logical cell,
so it does not change ordinary logical activation fuel. Exact sizes and endpoint
behavior remain to be measured; the reservation is not a measured layout claim. No new cap or general memory-accounting rewrite is
part of this RFC. Old programs should not acquire an unused I/O reservation.

Initial OS execution qualification should be Linux x86_64 for both reference and
LLVM19.1.7 O0 native routes. Define pre-consumption rejection on unqualified hosts
before activation rather than silently assuming Unix descriptors or Windows
console encodings. Parser/typechecker portability remains a separate scope.
Reference input-bearing execution on other hosts rejects before activation with
E0608 / oir-owned-run, "bounded stdin execution requires Linux x86_64", anchored
to the admitted input import. E0607 retains its existing division-by-zero meaning.
Native compile keeps its existing host gate. An invalid runtime capacity fails
closed as the E0500 owned-execution invariant "input capacity" before any input;
staging-resource denial keeps the existing resource-failure convention.
The fixed native capacity diagnostic is 65 bytes; its existing transient message
envelope increases from 64 to 65 only for modules that contain this operation.

## Acceptance and stopping point

- Compile one input-driven expression ELF once, hash it, and execute the same
  bytes with the two expressions above. Reference results agree. Retain sources,
  actual input bytes, commands, outputs and artifact identities.
- Exercise empty input, zero capacity, early EOF, exact physical capacity, and
  larger streams. For successful/full ordinary pipe or regular-file reads, observe remaining
  input in an independent controller to prove no extra byte was consumed. In particular: a 129-cell application buffer
  reads 128+EOF successfully and rejects 129/130-byte expressions after consuming
  exactly 129 bytes, not 130.
- Assert prefix/tail writes on Eof, all-cell writes on Full, and unchanged memory
  on IoError/resource/fuel failure before commit. Also distinguish later caller/transfer fuel exhaustion
  after a successful commit. Poison unused destination/staging regions in
  consumer tests. Include partial progress before failure and repeated calls.
- Use controlled byte adapters for short/interrupted/error results and real
  pipes/closed-descriptor failures for OS coverage. Assert exact 4+C+A core debit plus ordinary call/transfer costs,
  failure before the first and later attempted reads, and no effect from skipped
  calls/check/compile. Do not label controlled adapter faults as global OS proof.
- Feed NUL/non-ASCII bytes and malformed EOF to the existing parser using actual
  used length; retain its syntax-before-evaluation and checked i32 overflow rules.
- Reject wrong element/borrow modes and forged raw operation/view/result metadata.
  Verify old-program fuel/admission, exact resource endpoints, native source-free
  execution, independent review and exact-head CI. Reuse existing test structures.

Stop at one bounded batch-input application and this one effect contract. File
APIs, line/Unicode semantics, native recursion, richer enum payloads, unbounded
parsing and compiler self-hosting remain separate proposals.

## Implementation stages and acceptance status

1. A [disconnected controlled-event model](../tests/qualification/bounded_stdin_prototype/README.md)
   exercises exact byte, fuel and commit behavior. Its tests do not establish OS,
   runtime ownership, production memory admission or public API support.
2. Add and measure the closed origin/descriptor representation, then private
   identity and association controls. Keep public source imports denied while
   canonical alias/dependency/body proofs are established.
3. Connect the independently verified raw operation to reference and native
   consumers, with shared fuel and complete pre-effect preparation. Validate
   actual one-byte OS adapters and memory/no-overread controls.
4. Activate the bounded source imports and run the same compiled expression
   application on distinct input streams. Qualify the current source separately
   from historical frozen suites, with explicit amendments where required.

Internal effect-model prototyping is accepted; source/API activation is not.
The compiler-owned identity design is a reviewed proposal for the next private
implementation stage. The exact resource representation, host diagnostics and
public activation acceptance remain open. This RFC does not claim stdin works
in Oxid today.
