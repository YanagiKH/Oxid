# RFC 0024: bounded stdin input into an existing i32 buffer

Status: **implemented experimentally through the public typed-preview route;
full current-source qualification and exact-head hosted CI remain separate
acceptance gates**. Updated 2026-10-06. Public `check`, reference `run`, native
`compile` and syntax-only `fmt` admit the finite imports below. The focused
[expression runner](../scripts/verify_expression_stdin.py) has passed its 28
cases locally, including one unchanged ELF returning 39 and 63 for different
input streams and reference/native parity. These local results do not establish
merge status, a full CI pass, other execution targets or compiler self-hosting.

## Outcome and boundary

The [stdin expression entry](../fixtures/typed-expression-samples/stdin.ox)
compiles once and runs with distinct stdin byte streams. The same ELF returns
39 for `12 + 3 * (4 + 5)` and 63 for `7*(8+1)`, matching public reference
execution. Input changes require no source generation or recompilation.

One synchronous input operation accepts an existing exclusive `&mut [i32]`
view. Zero-argument scalar main, the existing scalar result printer, current
array capacities and all compiler/runtime ceilings remain unchanged. No strings,
Unicode decoder,
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

The signature is `read_stdin(buffer: &mut [i32]) -> ReadStatus` with
one compiler-owned nominal enum:

```text
enum ReadStatus { Eof(i32), Full, IoError }
```

Individual imports bind these compiler-owned identities; an ordinary user
function or enum with the same spelling remains distinct. Variants use the
existing nullary/scalar enum and consuming-match rules.

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
- C = 0: return Full after the core charge of 4, without staging bytes or
  touching stdin. It cannot establish EOF. Ordinary builtin-frame admission still
  reserves the fixed input scratch before the core operation.

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
change process signal policy, or install persistent input buffering. It reads
file descriptor 0 as it exists when the operation runs. Process startup may
repair closed standard descriptors before source execution, so closing fd 0
before launch is not a cross-runtime IoError guarantee. The qualified Rust
reference startup can supply `/dev/null`, producing EOF, while the native
startup can leave fd 0 closed. See the [application control](../fixtures/typed-expression-samples/README.md#bounded-stdin-entry).
Nonblocking read errors are IoError; this increment adds no polling or
asynchronous scheduling.

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

Core cost is **4 + C + A**, where A is the number of attempted
one-byte host reads, including EOF, interruption and error returns. This consists
of 1 + C for the bounded operation/commit reservation, the existing width-2 enum
construction amount of 3, and A for attempts. Ordinary call/loan handling and
whole-result moves remain additional and unchanged. The result construction
amount is prepaid here; lowering must not charge a second synthetic constructor.

1. After view/result validation, debit 4 + C before external reads or
   destination mutation and before confirming the prepared scratch range.
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

From the first destination store through result materialization there is no
fallible helper, allocation, validation or further fuel debit. The ordinary
builtin return and caller transfers remain charged afterward. Interruption retries
cannot spin without fuel consumption, although a
single blocking read has no promised time bound. A skipped call performs no read
and no input-operation debit. Check/format/compile do not consume program stdin.
Existing programs without the operation retain their fuel behavior and ceilings.
Because interruption retries are dynamic, an input operation must enter the
native runtime-fuel discipline even if its source CFG is acyclic. It shares the
existing counter and ceiling with ordinary execution, including the existing
guarded-entry wrapper cells and their admission charges; it gets no second fuel
bucket or invented finite static retry bound. An unused input
declaration must not let unrelated functions bypass existing static admission.
Native planning records these properties; an unused input import still enables
the shared guards, without waiving unrelated static admission.

The source-level lowering must make this schedule reviewable: the new operation's
charge, each adapter attempt, prepaid result construction and subsequent ordinary
transfers are distinct. A
raw producer must not supply a trusted byte count, unchecked write offset, host
function pointer, arbitrary descriptor, or a ready-made enum tag as authority.

## Supporting the current expression component

The expression language still accepts at most 128 input bytes. The application
allocates an **explicit 129-cell input buffer**, making its last cell the
application's over-capacity witness:

- Eof(n), n <= 128: parse exactly n bytes.
- Eof(129) is impossible for capacity 129 under this operation's contract.
- Full: 129 bytes were read, so return the application's input-limit value -4.
  Leave any 130th and later byte unread.
- IoError: return the distinct input-failure value -5 without parsing the buffer.

An exactly 128-byte stream followed by EOF produces Eof(128); exactly 129 bytes
produce Full without asking whether EOF follows. This preserves the application's
128-byte rule without a hidden read outside the operation's declared capacity.

Slices expose full fixed-array capacity, not a subslice. The application's
`parse_prefix` and `next_token_prefix` take a used length, validate it before
indexing, and apply it to every scan/EOF decision. Unused cells are not padded
with whitespace or scanned. The original whole-input `parse` and `next_token`
wrappers remain available, and the original enum scanner is unchanged.

The 15-node arena and 15-entry operator/operand stack bounds remain unchanged.
The stdin entry returns -1 for syntax and node/stack failures, -3 for invalid
arena storage, -4 for input-limit failures, and -5 for input errors. These are
ordinary scalar results, not process exit statuses. Checked decimal and
evaluation overflow still report E0604 and exit unsuccessfully.

## Binding, representation and trust boundaries

The compiler-owned `std` root is accepted **only in individual imports**, for
precisely these two items:

```text
use std::io::read_stdin;
use std::io::ReadStatus;
```

Each import may use an explicit alias, such as `use std::io::read_stdin as read;`
and `use std::io::ReadStatus as Status;`. Calls use the imported function/alias;
types, constructors and matches use the imported nominal type/alias. Existing
crate-absolute imports and `crate::std` source modules keep their meaning. There
is no implicit prelude, general standard-library discovery, filesystem probing,
direct `std::...` expression/type path, grouped/glob import, variant import or
user intrinsic declaration.
Existing duplicate-name and alias rules apply, rather than silently
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

The admitted descriptors, retained headers, capacity reserves and scratch
coexistence are included in existing resource accounts. This adds no general
standard library, external-call registry or source intrinsic mechanism. The
[descriptor controls](../src/frontend/oir/owned/builtin_descriptor_tests.rs) and
[source association controls](../src/frontend/oir/owned/source/builtin_source_tests.rs)
exercise these identity and admission boundaries; their definitions alone are
not a full current-head qualification receipt.

The independent raw validator must prove exclusive i32-buffer access, permitted
capacity/view, known input operation, result shape/nominal identity, and normal
ownership transitions. Source association must tie the effect to the admitted
compiler-owned declaration. Native/reference implementations share this contract
but independently enforce bytes, bounds, failure order and active result payload
safety. No change grants arbitrary external calls through raw OIR.

The implementation charges full reserved capacity and allocator slack where
relevant, not only bytes eventually read. It adds a checked 1024-byte scratch
suffix after the
builtin activation's ordinary owner payloads. The reference frame's existing
payload allocation and native owner allocation both include this suffix; no
second scratch allocation or new runtime ownership sidecar is needed. A plan
method derives its checked range only for the canonical builtin function.
Owner extents remain their nominal sizes and cannot address the scratch suffix.
Ordinary activation admission/allocation occurs before the input core charge,
including zero-capacity calls. The operation then confirms this prepared range
before its first read. Native storage is allocated once at function entry and
reused through retries, so source loops do not accumulate dynamic allocations.

The complete 1024-byte reservation and affected control carriers count toward
existing physical-byte limits. Scratch adds no language owner or logical cell,
so it does not change ordinary logical activation fuel. On the qualified x86_64
representation, the canonical builtin has 1,032 payload bytes (8 result bytes
plus scratch), 1,044 native bytes (including pointer and slice length), 16
admitted expanded cells and 14 logical activation-fuel cells. The
[native resource controls](../src/frontend/oir/owned/builtin_input_native_tests.rs)
cover the exact inclusive byte endpoint and its one-byte-below rejection.
Programs without the input function reserve no input scratch. No cap is raised.

Input execution is limited to Linux x86_64 for both reference and
LLVM/Clang/LLD 19.1.7 at O0 native routes. On other hosts, reference execution of
any program importing the input function rejects before entry validation or
activation with E0608 / oir-owned-run, "bounded stdin execution requires Linux x86_64",
anchored to the admitted input import. This includes unused imports and
zero-capacity calls. ReadStatus-only imports remain portable and admit no input
function; existing declared-child loading and native compilation gates still
apply. Checking and formatting do not consume input. E0607 retains its existing
division-by-zero meaning. Native compile keeps its existing host gate. An invalid
runtime capacity fails
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
  pipes and Linux directory-descriptor errors for OS coverage. Closed fd 0 at
  process launch is not an equivalent reference/native failure control. Assert
  exact 4+C+A core debit plus ordinary call/transfer costs,
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

## Implementation and acceptance status

- The finite imports, compiler-owned nominal identity, canonical raw operation,
  reference consumer and LLVM consumer are connected to public typed-preview.
  [Public controls](../src/frontend/stdin_public_tests.rs) cover status-only use,
  borrowing/nominal failures, unused imports and host rejection before activation.
- The [stdin application](../fixtures/typed-expression-samples/README.md#bounded-stdin-entry)
  passed the focused 28-case runner locally with public check, reference run and
  native compile/ELF execution. One compiled ELF retained its hash across inputs,
  including results 39 and 63. Pipe/file controllers checked unread tails and
  check/compile sentinels; a directory descriptor exercised actual Linux I/O
  failure. The reported scope includes E0604 parity and input/node/stack endpoints.
- Controlled adapter and raw-consumer tests separately exercise interruptions,
  partial reads before errors, exact fuel ordering, atomic commit and forged
  metadata. The earlier [disconnected model](../tests/qualification/bounded_stdin_prototype/README.md)
  remains historical model evidence, not an implementation oracle or OS proof.
- Full current-source compatibility qualification, independent acceptance review
  and all applicable CI checks on the exact PR head remain separate gates.
  Public activation and the focused local pass do not claim a merged release,
  completed roadmap milestone or compiler self-hosting.
