# Private verified ownership consumers

This unit adds reference execution and real LLVM storage/calls for the private
owned OIR from [RFC0014](../../rfcs/0014-owned-structs-call-borrows.md). Both consume
the same immutable `VerifiedOwnedProgram`; its verifier remains the sole
constructor. Struct/borrow source syntax, source ownership lowering and CLI
activation remain disabled. This is a bounded ownership-foundations increment,
not a public ABI, heap/resource lifetime implementation, Rust-equivalence claim,
or completion of M 2.

The candidate began at PR 19 head `6eed37d9301b100f33ea0f869e74aae53824c312`, then
fast-forwarded over its equal-tree merge and the disjoint qualified native-suite
runner change to main `5042d356cc441b85f09c18cda49dfa8781a124c8`, tree
`24e02f5235df43bd843a797a5a8a1a56afd34f84`. All consumer edits were retained.
Results below are local qualification; hosted CI for a future publication is a
separate requirement.

## Witness, identities and ownership boundaries

The witness owns its actual raw instruction stream and checked declaration
facade. The plan has private fields, a private witness reference and immutable
accessors. Consumer internals recover their witness from that plan; they never
accept an independently supplied plan/program pair. Actual-source sibling
compile probes reject witness replacement/mutation, plan rebinding/mutation and
raw-program execution/native emission for privacy/type reasons.

The iterative reference machine uses fixed per-activation arrays. Checked
handles contain root frame index, activation epoch, owner index/generation and
loan frame index, activation epoch, loan index/instance. Indices are zero-based;
identity epochs are nonzero and use checked increments. No Rust pointer into the
growing frame-header vector survives an operation. Byte arenas use checked
record/field offsets, explicit scalar encoding, and a real identity byte for
empty records. Neither padding nor stored references are observable values.

Direct owner loans and immediate-parent child counts enforce exclusive-child
suspension and shared-child read permission. Incoming shared parameters may
alias. Caller-proven exclusivity is also checked by the reference invocation's
explicitly charged pair scan. Stale frame, owner-generation and loan-instance
checks are transition-inductive: checked release cannot end an ancestor with
live children. Arbitrary inconsistent corruption of otherwise unreachable
ancestor tables is outside that defensive claim.

PrepareScalar snapshots immediately; PrepareOwned consumes its source into
staging immediately; PrepareBorrow acquires its loan immediately. Invoke copies
owned input into independent child parameter storage. Its caller's staging
backing storage remains allocated until return despite consumed logical
ownership. An owned return transfers into the caller's already-reserved result
range before the callee is popped. The normal edge releases matching loans and
initializes the result; failure promises neither rollback nor user cleanup.

A raw CFG that re-executes the *same* canonical scalar definition after its
PrepareScalar but before Invoke cannot obtain a witness: it joins different
exact call-preparation states. Tests retain that case as verifier-negative,
check snapshot isolation directly on test-owned machine storage, and separately
execute valid later-argument loops/nested calls and repeated static call sites.

## Private native representation

Qualification uses Linux x86_64, LLVM/Clang/LLD 19.1.7, O0, and Rust/Cargo 1.98.1.
The ownership-only emitter uses entry-prologue owner byte arenas, i64 scalar and
snapshot cells, and pointer cells for references/loans. Incoming owned pointers
are copied field-by-field into callee storage; owned results use a caller-owned
output pointer and are copied before return. Empty construction initializes its
identity byte. Shared pointers may alias. The emitter adds no `noalias`,
`inbounds`, `nonnull`, `sret`, `byval` or lifetime assumptions.

Bool merges select the incoming slot pointer before loading its value, using
the true final predecessor labels after fuel/overflow splits. They do not load
an untaken uninitialized slot. The existing scalar emitter, C output/error
adapter, trusted-tool boundary and atomic no-clobber publication remain intact.
No reference fallback or source evaluation substitutes for native execution.

## Expanded storage and exact charges

For a function, let S be scalar locals plus mutable places, A all call argument
descriptors, O owners, R incoming references, L loans, C calls, and
P=sum(max(1,record field count)) across **all** owners. B is its checked aligned
owner-arena size, including staging, parameters, temporaries, results and
inter-owner padding. The plan stores direct owned-stage and borrow-loan ranges
so open/release do not hide scans of unrelated arguments.

```
X = S + A + P + 4O + 8R + 12L + 2C
Dref = 8(S+A) + B + 32O + 64R + 96L + 16C
Dnative = 8(S+A) + 8(R+L) + align4(B)
```

On this host Scalar/Option<Scalar> are 8 bytes; OwnerRuntime, ReferenceHandle,
LoanRuntime and CallRuntime are 32/64/96/16 bytes; Frame is 272 bytes. Owned return
scratch is zero additional variable storage: caller result owners are already
in P/B. Reference requested runtime bytes are reserved frame-header capacity
plus active Dref plus one 8-byte Scalar. Plan vectors have a separate 32 MiB
checked, fallibly reserved metadata limit.

Reference keeps maximum 1,000,000 fuel, 1,024 frames and 200,000 scalar slots, adding
200,000 live expanded cells and 16 MiB requested-byte limits. Exactly 200,000 cells
executes in a real verified fixture;200,001 rejects before activation allocation.
A recursive shared-reborrow fixture admits 1,024 frames and fails the next call
at its Invoke origin. Loops reuse activation storage; they do not allocate per
owner lifetime or instruction.

Native retains 256 functions, 64 user parameters, 4,096 blocks, 32 call depth,
whole-call-graph recursion rejection, 256 scalar slots/function and 8,192 aggregate
scalar slots. Ownership additionally requires S+O<=256/function, summed and
maximum-call-path X<=8,192, and summed/path explicit arena bytes<=1 MiB, including
the guarded wrapper's 8-byte fuel cell. Static acyclic cost remains<=100,000;
cyclic cost stays unknown and uses shared guarded fuel. Unused functions remain
admitted/checked. Raw/reference parameters remain 256.

These byte caps are supplementary for the current representation. Each record
has size<=4w and alignment 1 or 4. Since 4P is aligned, induction gives B<=4P even
with inter-owner padding. Thus Dref <= 8X and Dnative <= 8X. Requested reference
runtime storage is conservatively bounded by 1024*272+200000*8+8=1,878,536 bytes,
below 16 MiB. Summed native explicit storage is at most 8*8192+8=65,544 bytes,
below 1 MiB. These are not total RSS or LLVM spill/ABI-stack promises.

The Batch pilot's maximum path has 154 cells, 1,152 dynamic reference bytes and 432
explicit native bytes before the wrapper. With the default 1,024 header
reservation it requests 279,688 reference bytes. Its deliberately lowered
three-header boundary requests 1,976 bytes; 1,975 rejects at dispatch-to-commit.
The matching exact frame/scalar/cell limits are 3/30/154. Separate release test-process probes using Linux wait4
reported maximum RSS of 4,540 KiB for the exact-cell fixture and 4,284 KiB for
the complete pilot budget sweep. These include raw IR, verifier/test harness
and code pages; they are contextual observations, not the charged payload
or a total-process memory guarantee. A real native
8,192-cell/depth 32 fixture has 32,264 explicit arena bytes and a measured 32,992-byte
backend frame-size sum; these distinct measurements are not conflated.

Fuel is an abstract work ledger, charged before an operation:

| Event | Charge |
|---|---:|
| Root | 1+X(root) |
| Scalar statement, bool merge, branch/goto, live, field read/write, scalar preparation, borrow preparation | 1 |
| Construct, move-initialize, discard, storage-end, owned preparation | 1+w |
| Whole replacement | 1+2w |
| OpenCall | 1+owned argument count |
| Invoke | 1+argc+X(callee)+owned argument widths+r(r-1)/2 |
| Scalar return | 1+P+L+C+R |
| Owned return | scalar-return charge+w(returned owner) |

Return R charges normal-edge release. Explicit storage end and fixed frame
teardown are distinct charged events even for moved/uninitialized slots.
Embedded scalar statements retain their own original spans; other ownership
operations use their statement/terminator origins. Arithmetic overflow retains
the operator span after the statement's charge. Activation failure priority is
fuel, frames, old scalar slots, expanded cells, bytes, then allocation. Root and
child allocation errors carry entry/Invoke origins; prior plan admission remains
separate.

## Independent and actual execution evidence

Frozen hand schedules cover empty ownership 17 fuel, owned relay 52, shared read 49,
shared children 113/116, mixed bool/unit/i32 relay 90, loop-local reuse 66, whole
replacement 33/35, simultaneous distinct owned results 104, and nested
later-argument snapshot loops 292. Every lower budget is checked against the
first unpaid complete operation and exact origin.

The raw Batch pilot implements the RFC's nested retry/job loops, exclusive helper
reborrow, shared completion check, owned relay and final consumption. Independent
state arithmetic yields 210+600+6=816. Its schedule has 404 charged events and
1,086 fuel; all 1,086 lower budgets fail at their independent expected origins.
The 19 observed writes, 24 acquisitions, 24 releases and six whole transfers
match the model. A checksum-MAX variant preserves earlier retries/completed writes and
fails at 235 fuel; a 230-fuel run stops before the designated write. Failure does
not invent normal-return cleanup.

The 19 reviewer-owned reference tests are retained unchanged. Their separate
families include 192 mixed-layout/permutation/reinitialization cases, 50 alias and
distinct-root calls, 164 capability-tree variants, 1,416 acquisitions and 2,664
meaningful direct checked-release denials. Raw programs, direct machine probes
and process runs are different counts. Other held-out cases cover 10,000-iteration
identity reuse, 64-deep owned returns, 256 parameters, snapshots, requested bytes,
nonzero-entry merges and all root/child allocation origins.

Pinned LLVM gates produce actual ELF binaries, run source-free with a clean PATH,
and compare stdout/stderr/status against independent expectations and reference
execution. To avoid recompiling the same Batch body 1,087 times, an argv-only test
wrapper injects each budget while asserting byte-identical owned function bodies;
unchanged production wrappers are tested separately at boundary/default budgets.
The test wrapper is not a production entry/API. Debug and release evidence lives
in separate directories. All 196 retained LLVM modules and all 196 ELF
artifacts match byte-for-byte across profiles; all 171 pre-count-fix artifact
pairs are unchanged.

Sentinels exercise multiple owned inputs/results, interleaved scalar/reference
arguments, aliased shared parameters, empty values adjacent to live bool/i32
storage, padding, relay chains and repeated calls. Independent LLVM CFG checks
prove successful fuel/overflow edges dominate named stores and failure paths
terminate. A valid-LLVM hoisted-store mutant is rejected by the checker despite
unchanged process error output; output parity alone is not claimed to prove
failure-before-write. The standalone audit is retained as
`scripts/verify_owned_native_cfg.py`, alongside the integrated Rust checks.

Independent review found and fixed an originless reference allocation error, a
test-only transfer-destination generation inconsistency, and a native unknown-
cyclic-cost placeholder overflow. Red regressions and corrected evidence are
retained. The native LLVM count pass now uses checked byte accounting and stops every
variable expansion loop as soon as its byte cap is crossed. Lowered-cap
64-field alternating-replacement programs with 32 versus 32,768 replacements
visit identical 0/6/240 fields at 1,024/4,096/65,536-byte caps; rejected output
never reserves its final text buffer. Independent 32/256-replacement fixtures
confirm bounded prefixes, exact-cap acceptance and one-byte-under rejection.
All previously admitted modules remain byte-identical. The remaining
byte-free scans retain existing raw/arity bounds; no second semantic
field-expansion restriction was introduced. None of the bounded models, generated cases or checks is a proof
over every program or of future source/heap/FFI behavior.

A final predecessor run exposed a test-only `/proc` race: a terminated child
can disappear during a stat read with ESRCH instead of ENOENT. The stopped-
process assertion now catches only FileNotFoundError and ProcessLookupError.
A deterministic red/green regression covers ESRCH; unrelated permission and
I/O errors still propagate. The production dispatcher is unchanged from
main `5042d356`, SHA-256
`e3cf670556a6218bee44895812a1e93892c8ec20c3d12bf2e1bf3855ceb488af`.
The original failed gate log is retained; this is adjacent harness maintenance,
not a relaxation of worker cleanup or oracle requirements.

## Reproduction and final qualification ledger

With Rust and the pinned LLVM 19.1.7 tool environment configured:

```sh
cargo fmt --all -- --check
cargo clippy --all-targets --all-features --locked -- -D warnings
python3 scripts/verify_owned_witness_privacy.py
cargo test --all-targets --all-features --locked
cargo test --release --all-targets --all-features --locked
OXID_OWNED_NATIVE_EVIDENCE=target/owned-native-debug \
  cargo test --locked --bin oxid frontend::oir::owned::native::tests:: \
  -- --ignored --nocapture --test-threads=1
OXID_OWNED_NATIVE_EVIDENCE=target/owned-native-release \
  cargo test --release --locked --bin oxid frontend::oir::owned::native::tests:: \
  -- --ignored --nocapture --test-threads=1
python3 scripts/verify_owned_native_cfg.py target/owned-native-debug
python3 scripts/verify_owned_native_cfg.py target/owned-native-release
cargo build --locked
cargo build --release --locked
python3 scripts/verify_native_suite.py target/debug/oxid target/release/oxid --jobs 2
```

Also run the four predecessor ignored scalar raw LLVM gates and maximum-block
ownership-verifier resource gate in both profiles, i32 literal/arithmetic
oracles, Python metadata tests, feature-status and repository verification, as
listed in [the verifier report](owned-verifier-validation.md). The new
`typed_owned_boundary` integration test confirms check/run/compile all reject
struct/borrow source before tools/output. CI runs the whole ignored ownership
native group sequentially; the unchanged seven-source dispatcher retains its
separately qualified two-worker bound.

### Final local result (2026-10-01)

The final combined debug and release suites each pass **522 ordinary tests**
(392 unit + 130 integration), summed from all 16 test-result lines per profile.
Each ordinary run reports 14 intentional ignores; all are separately executed:
nine ownership LLVM gates, four predecessor scalar raw LLVM gates and one
maximum-block ownership-verifier resource gate, in both profiles. Formatting,
strict Clippy and all ten actual-source witness/plan/raw-entry probes pass.

The nine ownership LLVM gates execute 3,055 processes per profile, including one
intentional misordered-store negative-control binary. The 3,054 normal runs are
124 tiny fixtures, 1,090 Batch runs, 241 Batch write-failure runs, 1,250 extended
storage runs, 336 held-out runs, eight other guard failures, four merge/depth
runs and one expanded-stack boundary. Runs are not distinct programs: budget
harnesses reuse binaries. All 196 LLVM modules and 196 ELF artifacts compare
byte-for-byte across debug/release, including the negative-control artifact.
The independent global CFG audit covers 195 normal modules, 538 guarded
function instances, 2,290 named-store instances, 173 overflow-result stores and
3,874 terminal error sinks. The mutant is excluded from that positive count and
separately required to fail the audit.

All seven unchanged source/native oracles pass through the two-worker dispatcher
on the final CLI binaries. The final release binary differed from the initial
oracle run, so all seven suites and the i32 literal/arithmetic oracles were
rerun; no earlier-binary pass is substituted. Final CLI SHA-256 values:

- Debug: `dde763dc16a56b15d7360d431dcc11fab7a9c2ea25e7fdef2c057877dbb612ef`
- Release: `4e3bfb2be75dd18ba853a5c44a298e945062f751b4fdedcc40f6f0f02416b1bc`

The final i32 arithmetic gate checks 1,025 source cases in 4,100 invocations;
the literal gate checks 502 literals, 320 source programs and 13 unsupported
cases in 1,670 invocations. All 23 Python tests pass after the narrow `/proc`
helper correction. The feature inventory remains 24 entries; repository
verification passes 121 sources and 67 runnable programs. Source ownership
activation remains closed.

The final independently reviewed reference core SHA-256 is
`bd23c1a0df31bedf3e9f7f7ba5411360a6541f6dcf023f7f066d831cca9d3231`;
the count-pass-hardened native core is
`fb5440fbaa507338da7063b053df01d2807dc142a498499b1750a951ae84c36a`.
Both independent implementation reviews have no unresolved findings. These
local results do not assert a publication commit, hosted CI result, other
native targets, O2/LTO qualification, or completed source integration.
