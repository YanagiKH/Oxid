# RFC 0025: bounded stdout and explicit process entry

Status: **accepted contract; public implementation and qualification in progress**.
Updated 2026-10-06. Source activation is not yet release-qualified.

## Outcome

An Oxid-written producer can write an exact persistent artifact to stdout and
return a meaningful process status. A separate consumer reads only the saved
artifact after the producer exits. The acceptance example is the bounded
expression stack program; the reusable capability is native process output,
not an extension of its instruction set or a self-hosting claim.

## Explicit entry policy

Proposed `--entry-mode process` applies only to typed `run` and native `compile`.
Default result mode keeps all existing bool/i32/unit rendering, JSON records,
exit statuses and source fuel behavior. The named physical-admission differences
are specified below. Process mode requires the original
resolved zero-argument `main() -> i32`; values 0..255 become exact process exit
statuses without a scalar trailer. Other values produce a runtime diagnostic
and status 1 when that diagnostic completes, or 74 if its stderr output fails;
all earlier stdout bytes remain. Never truncate the result. A source main called as an ordinary
function retains its ordinary i32 semantics.

Reject process-mode run combined with JSON reporting before source execution,
with a human diagnostic on stderr and empty stdout. Early option errors must
obey that channel rule too. Compile may use JSON build reporting because it
executes no source effects. The selected entry policy is bound into emitted
native code, not selected later from executable arguments or environment.

A verified inventory containing the stdout function requires process mode,
even if the import is unused or hidden behind an alias/module helper. Status-
only imports do not require it. Checking remains effect-free and has no entry
requirement. The first process/output execution target is Linux x86_64;
unsupported-host and entry/policy validation occur before activation/effects.
Process Run/Compile reject unsupported execution hosts before loading source or
creating an output artifact, with E0608 / oir-run / `process execution requires
Linux x86_64` when the selected reporting channel is available. Default-mode
portability is unchanged.

Before process source activation, establish ignored SIGPIPE. Setup failure exits
74 silently before source effects: attempting a diagnostic before signal safety
is established could itself terminate on SIGPIPE. Individual byte helpers do not
modify signal policy. No signal-mask fallback is introduced.
Silent setup-failure behavior and reference/native diagnostic-write failure
parity are required review gates; the existing Rust macro reporters and the
unmetered native scalar printer are not sufficient evidence for the new mode.
The I/O failure status is 74; ordinary runtime faults remain status 1 when their
diagnostic can be delivered. Terminal stderr reporting retries EINTR and positive
short progress, stops on zero progress or any other error with status 74, and
never recursively reports that failure.

On the qualified host, pure argv classification may precede policy setup, but
setup precedes every process-run diagnostic and the root activation/fuel guard.
Thus an invalid process/JSON combination can establish SIGPIPE policy before
reporting its rejection, while performing no source execution or byte-output
builtin effect. Native process entry uses the same pre-guard setup order.
Compilation only reports a build and does not execute this process-entry setup.
Diagnostic capability is separate from runtime qualification. Linux x86_64
and macOS x86_64/arm64 use the explicit platform SIGPIPE-ignore policy and
fallible direct stderr writes. Windows Process Run and process-option errors
currently return silent 74 before formatting or loading: an inherited stderr
handle has not been proven synchronous, so no potentially asynchronous write
receives temporary storage. Process Compile keeps ordinary compiler reporting
and returns E0608 there; it never installs signal policy. Other unqualified
terminal targets also fail setup silently. Hosted controls are required for
each advertised diagnostic target; Linux evidence does not qualify macOS.

Pure argv classification precedes loading. A valid Process selection remains
sticky for Run error routing, including duplicate or conflicting options.
Missing/unknown entry-mode values on explicit typed Run also use text-only
process errors. `--` ends option recognition; script arguments keep their
existing forwarding boundary. Check/Fmt never establish process signal policy.
Compile JSON remains an effect-free build report. A Process Run with JSON is
E0001 on safe stderr, with no JSON or scalar stdout trailer.

Run and Compile share the original-root zero-argument i32 signature gate:
E0600 / oir-run / `process entry requires original-root fn main() -> i32 with no
parameters`. Check remains library-capable. Default Run/Compile reject the full
stdout-function inventory using E0609 / oir-owned-run / `bounded stdout
execution requires process entry mode`, including unused imports. Raw native
admission retains its independent E0700 defense.

## Whole-view output

Proposed individual imports are `std::io::write_stdout` and
`std::io::WriteStatus`, with existing alias rules and no prelude/general std.

```text
write_stdout(bytes: &[i32]) -> WriteStatus
enum WriteStatus { Complete, InvalidInput, IoError(i32) }
```

The function writes every cell in the existing shared view, capacity C in
0..1024. There is no implicit terminator, used-prefix argument, encoding or
subslice promise. Each cell must be 0..255. Validate and stage the entire view
before the first write; invalid input returns InvalidInput without emitting
bytes. Zero cells are ordinary byte values. Empty view returns Complete without
touching fd 1.

Complete means C bytes were accepted by host writes. IoError(n) reports the
number accepted by earlier successful attempts in this call, not delivery or
durability. Each host attempt requests exactly one byte. Success advances once;
EINTR retries after a new fuel debit; zero progress and all other failures,
including EAGAIN/EPIPE, return IoError. No seek, close, buffering, polling or
change to descriptor flags is part of the operation.

The candidate operation cost is `4 + C + A`, where A counts every attempted
one-byte write including interruption/error/zero-progress outcomes. Debit 4+C
before scanning/staging, then one before each attempt. Failed subtraction
preserves remaining fuel. InvalidInput still pays 4+C; empty view pays four.
Invocation, activation, loan, return and caller transfers are separate existing
costs. This recurrence requires actual reference/native consumer review before
acceptance; it is not established by the disconnected event model alone.

Before output, preflight shared-loan origin/epoch/projection, capacity, staging,
and result storage. Use a fixed 1024-byte builtin activation scratch reservation
under existing accounting. No allocation or result-materialization debit may
be introduced after the first write. Invalid raw identity or malformed runtime
views fail closed before any source byte is read or output attempt occurs.

Written bytes cannot be rolled back. Fuel failure between attempts, a later
source failure, or an ordinary return-transfer failure may leave a partial or
complete-looking output with a nonzero process status. Count this honestly;
there is no whole-file atomicity. Source fuel bounds attempts, not the blocking
duration of an OS call or inherited terminal diagnostic retries.

## Identity, representation and admission

Keep the existing owned OIR and its independent validator. Add only a closed
finite stdin/stdout inventory, with separate nominal enum/function identities,
canonical suffix ordering and full source association. A matching user spelling
or raw shape never establishes builtin identity. Every executable consumer must
validate the new claim through ordinary signature, CFG, ownership and canonical
builtin checks. No reusable privileged constructor, alternate witness or runtime
sidecar is proposed.

Each family has exactly three states: absent, status-only, or function with its
required status. The product has nine valid states. Canonical suffixes list
input before output, ranking admitted enums and functions independently after
their source-only prefixes. Importing a function installs its status dependency
without binding an unimported status name in source. Each enum anchor is the
earliest import of that enum or its requiring function; each function anchor is
its earliest function import, ordered by module preorder then source order.
WriteStatus carries its i32 payload at member 2; ReadStatus retains member 0.
All source association and raw checks must distinguish these families even when
only one family is present. Unknown or dependency-incomplete claims are invalid.

The existing declaration-index fixed-state measurement is 4094 bytes under an
unchanged 4096-byte ceiling. Existing singleton BuiltinIds and the one-parameter
signature are concrete design constraints. Measure new/affected enclosing
carriers, complete fixed-bank coexistence, retained capacities and scratch
before choosing representation. Candidate primitive sizes alone are not an
admission proof. Preserve default rendering, status and source fuel, and all
resource ceilings; do not hide layout growth or enlarge a limit. Whole-view arity avoids expanding the signature
seam solely to support a variable output prefix.

The selected feasibility candidate retains four optional endpoint borrows tied
to the existing immutable source owner, after complete CompactSpan validation.
It narrows only the already validated std-order cursor to the reserved-domain
u32 sentinel. On the measured target, the complete substituted fixed bank remains
4094 bytes with no phase overlay and no credit for smaller endpoint transports.
This does not yet establish all nine source identities or executable admission.
The actual closed implementation additionally names and charges the two one-byte
enum/function selector arguments. Its fixed bank is therefore 4096, exactly at
the unchanged 4096 ceiling. Total requested scratch also includes existing
dynamic row storage; it is not the same quantity as this fixed bank.

**Named retained-header successor:** Tables/DeclarationIndex grow from 368 to
376 bytes; DeclarationFacts grows from 584 to 592. The actual outer IndexPlan
charge must increase by eight bytes even for absent builtin inventory. A prior
exact retained-byte endpoint can therefore reject the same program; this is an
intentional, measured physical-admission difference, not a ceiling increase.
Retain the old boundary evidence and test the new exact endpoint plus the old
endpoint/one-byte-under failures before allocation. Measure and charge all other
affected enclosing wrappers separately: the eight-byte header change is not a
claim about total program growth. The source-association candidate measured 504
bytes; the closed transport implementation required 568, including actual
receiver borrows, selectors, rank results and the retained function projection.
Its private input caller was 3072 rather than 2944. These predecessor
measurements remain distinct from the finite-family source successor below.
Resolved and typed program carriers are 608/632 rather than 600/624; production
source/type dispatch envelopes are 3272/2560 rather than 3256/2528. The observation
fixed charge is 11896 rather than 11888. Existing ceilings still apply.

The two retained predecessor fixtures pin the distinction: absent inventory
previously requested retained/scratch 488/4102, and read_stdin requested
528/4114. The closed successor requests 496/4104 and 536/4116 respectively,
where scratch is fixed 4096 plus existing dynamic 8/20. These are named current
resource-admission changes; source behavior and the ceilings are unchanged.

**Named finite-family source successor:** exact signature and identity carriers
increase from 368/400 to 376/440 bytes. The canonical producer increases from
6456 to 6584. Source association uses one retained first-function ID and a
one-byte family permission, with its complete classifier/transport envelope
charged at 640 rather than 568. The existing private input caller consequently
increases from 3072 to 3144. The separate fresh output caller is charged at 5592,
including association; it is paid even when its sealed candidate has no imports.
Run and Emit are separate requests, so emission performs no reference effects.

The preserved input fixtures quantify the resulting admission change: fixed HIR
105888 becomes 106040 (+152); status-only total 150465 becomes 150617; one input
function total 151025 becomes 151185; the larger fixture 174091 becomes 174251.
The additional eight bytes per synthetic signature are separate from the fixed
increase. Reservation counts and retained-allocation peaks are unchanged in
these comparisons, and both private/current exact work endpoints remain 739.
These are measured control-carrier admission successors under unchanged limits,
not a claim that every prior exact byte endpoint still admits. The index fixed
bank remains 4096; that is not the total HIR or raw-lowering budget.

**Named public-admission successor:** each repeated Current/Executable versus
private-candidate guard has a complete fixed carrier in its existing paid bank.
Measured source/type dispatch increases from 3272/2560 to 3320/2608 bytes;
association increases from 640 to 672. The private input/output caller envelopes
therefore increase from 3144/5592 to 3176/5624. Lower invocation controls increase
from 4672 to 4776. Resolved608, typed632, signature376, identity440, canonical
producer6584 and the index fixed4096 remain unchanged. No new origin flag or
policy field is retained in a checked source owner.

The same one-input-function fixture now admits HIR151313 instead of151185:
its base149353 becomes149449, and the production builtin extra880 becomes912.
The exact private/current work endpoint remains739. This measured +128 is a
named admission difference under the existing64MiB HIR ceiling; it is not an
increase to that ceiling or a claim about whole-process memory. The lower
invocation change is separately charged by its existing raw/source preflights.

The scalar emitter's transient `Emission` carrier is40 bytes instead of32;
entry policy is1 byte, and its diagnostic key/table carriers remain32/48.
Process mode applies the existing16MiB diagnostic and64MiB LLVM-text caps even
to acyclic programs, while preserving their body ABI and fuel schedule.
The source process failure/result carriers are8/16 bytes, signature return40,
and checked owner320 with zero stored policy fields. The non-test argv scanner
increases from2 to5 bytes; Route remains56 and its scanner-result envelope56.
These are measurements of
named transient roles, not new retained HIR allocations or a stack/RSS bound.

## Concrete acceptance and stopping point

Produce an exact 80-byte versioned stack artifact: magic `OXS1` (four bytes),
count 1..15 (one byte), then fifteen five-byte rows, each opcode plus four-byte
little-endian nonnegative i32 operand. Unused rows are canonical zeros generated
without reading unused Code rows. A loader uses an 81-cell input witness and
rejects wrong length, trailing data, version/opcode/count errors, high-bit
operands, nonzero operator operands, noncanonical tails and malformed stack
structure before execution. No file-management builtin is required.

One unchanged producer ELF must emit separately authored exact files for the
39 and 63 examples. An independent external decoder and an Oxid-written loader
consume only saved files after successful producer exit; neither may reuse the
producer's parser/arena/evaluator or generated expected bytes. Preserve input,
output, statuses, commands and hashes. External callers publish temporary files
only after successful production and validation; this RFC does not provide
rename, fsync or crash durability.

Required controls cover byte/capacity endpoints, last-byte validation before
output, shared/projection provenance, mode/JSON/entry rejection before effects,
status 0/nonzero/-256/256, exact fuel boundaries, interruptions, zero progress,
known-prefix errors, real nonblocking and closed-reader pipes, SIGPIPE setup,
closed/broken stderr, interleaved stdin/stdout and later failure after output.
Injected outcomes and real OS evidence remain separately labeled. Historical
inputs/oracles stay immutable; default no-output programs retain their behavior.

Stop after this bounded capability and independent artifact consumer pass
applicable exact-head CI. Do not extend the sample ISA or introduce general
file management, strings, encodings, networking or provider/self-hosting claims.
