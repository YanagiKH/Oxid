# Explicit local HIR producers (functional qualification)

This isolated increment connects the existing parser/static observation pair to
checked-HIR import. The explicit gate is enabled for bounded functional qualification following
independent review of the executable identity and child-lifetime boundary. No default
provider, grammar, Process-entry policy or artifact-import contract is changed.

## Selected bundle and actual executable identity

The new selector is `--experimental-hir-producers <local-bundle-directory>` on
explicit typed-preview check/run/compile, mutually exclusive with the existing
supplied-artifact option. Producer build/install stays explicit. No executable is
downloaded or compiled implicitly, and no manifest-controlled path is accepted.

The bundle has fixed filenames `parser`, `static`, and `manifest.txt`. The exact
manifest is ASCII with a final newline and lowercase hexadecimal SHA256 values:

```
OXID-HIR-PRODUCERS-1
parser <64 lowercase hexadecimal characters>
static <64 lowercase hexadecimal characters>
```

The manifest asserts byte identity, not publisher trust or a signature. Select
only local producer programs you trust. Final root and fixed leaf lookup rejects symlinks and nonregular entries,
including root spellings with trailing slash/dot. Intermediate ancestor symlinks
remain part of the trusted local path baseline. A root
directory descriptor binds fixed-name openat lookups. Each executable is streamed
through a fixed 8 KiB buffer with a 16 MiB cap into an owned executable memfd.
Its exact copied bytes must match the selected SHA256. ELF64, little-endian,
x86_64 shape is checked; the kernel remains responsible for actual ELF loading.

Before exposure, snapshots receive write/grow/shrink/seal and executable-mode
seals. Bounded readback verifies the digest of the immutable sealed bytes. Both
owned descriptors remain live across the two launches, and execution resolves
`/proc/self/fd/<descriptor>` in the child before CLOEXEC closes descriptors.
Producer changes to original paths, cwd filenames, mode bits or reopened snapshot
contents cannot substitute another executable image. The private cwd contains
no authoritative executable pathname. Image hashes do not attest dynamic-library
code; the host runtime remains a separate trusted dependency.

This requires Linux x86_64 with explicit executable-memfd/sealing support. A kernel
or policy refusal fails closed; the compiler does not change system policy or
fall back to a mutable executable. See the Linux
[executable memfd policy](https://docs.kernel.org/userspace-api/mfd_noexec.html).

## Source and bounded process transport

The source/AST is loaded once as genuine ProjectSources and retained throughout.
One root-only source of at most128ASCII bytes feeds the parser. A successful
1,559-byte OPA1 frame and those same original bytes are framed as AST1, at most
1,692bytes, for the static consumer. Its 2,607-byte OPA1/STF1 success must preserve
the complete parser prefix. No host semantic facts, repaired rows or normalized
source bytes are introduced. The existing paid compiler import still performs
all source/AST, canonical HIR, checker, type/flow, lowering, association and OIR
verification before any runtime result or native publication.

Children use argv directly, an empty environment plus PATH=/no-tools and LC_ALL=C,
and fixed nonblocking stdin/stdout/stderr pipes. The parser stdout cap is1,559;
static stdout is2,607; stderr is4,096. One extra byte witnesses overflow. Success
requires complete input writes, status0, empty stderr, exact success framing and
closed streams. Diagnostic/refusal observations grant no executable authority.
No child failure silently falls back to ordinary compilation.

Each child has a five-second supervision deadline. It starts a fresh session so
its leader cannot join the compiler's process group. Cleanup signals the original
group and the owned leader individually, then polls reaping for at most500ms;
it never calls an unbounded blocking wait. Missing leader-reap evidence is a
failed contract. A parent-death signal protects the direct child. Stream EOF is
rechecked after termination before a success result is accepted.

This controls the leader and ordinary descendants remaining in its group. A
trusted program that deliberately creates another group/session can escape that
descendant cleanup. Kernel stalls, host scheduling and arbitrary process RSS or
filesystem effects are not bounded by this supervisor. It is not an OS sandbox.
No claim of unconditional cleanup after SIGKILL or of containing hostile local
software is made. An incomplete cleanup is reported rather than called success.

## Records and resource scope

Each attempted verified launch records its selected sealed-image hash, input
hash, actual written-input count, captured stdout/stderr lengths and hashes,
spawn status, terminal reason, exit/signal values and whether the leader was
reaped. Captured-prefix hashes on overflow are explicitly labelled captured data.
A failed spawn is reported as such, never as an executed producer. JSON mode emits
`hir-producer` records; text mode keeps Result stdout and reports provenance on
stderr. Rust canonical validation/typechecking/lowering remain mandatory. This
is frontend-component orchestration, not compiler self-hosting.

The new options use one fallible exact reservation without growing the existing
56-byte Route carrier on the qualified 64-bit host. Named overlapping compiler
transport carriers are debited only on this opt-in path. Snapshot copying uses
bounded buffers and fallible I/O. PathBuf/formatting/Command and other standard
library allocations are not all made fallible; named-carrier accounting is not a
whole-route allocation/RSS guarantee. Hashing/file setup and external processes
have separate byte/transport limits, not the compiler's scalar work tariff.

## Reproducible local qualification

The tracked Linux x86_64 recipe builds genuine Oxid parser/static executables,
then separately compiles controlled C helpers for transport and rejection tests:

```sh
cargo build --locked
python3 -B scripts/qualify_hir_producers.py \
  --compiler target/debug/oxid \
  --llvm-bin /usr/lib/llvm-19/bin \
  --output /tmp/oxid-producer-qualification-new
```

The output directory must not already exist. It retains commands, statuses,
streams, source/C helper inputs, executable artifacts and SHA-256 identities;
an outer timeout retains partial captured streams and a failed receipt. The
recipe does not install tools or silently build the Rust compiler. Its caller
must bind the supplied compiler to the source checkpoint being qualified.
Synthetic paused helpers independently expire after 15 seconds, including fork
children, so a broken supervisor cannot leave those test helpers paused forever.
This safeguard is test infrastructure, not a production supervisor guarantee.

At compiler checkpoint `96d93ed27416523ad264b16da38f4498cab6afa7`, local
qualification passed:

- Genuine Oxid-built parser/static replay through check, run and compile, with
  each executable/input/output identity joined to manual pipeline execution.
  The 2,607-byte observation equals the frozen rich-success fixture. The manual
  supplied-artifact route produces an identical native executable. Reference,
  native and text Result stdout agree on `1`.
- 76 public CLI refusal invocations: the original 52 transport/source/identity
  probes, 16 additional absent-output compile probes, and eight complete-frame
  semantic/stale-source checked-import probes. Controlled helpers test short or
  malformed frames, nonzero status, stderr, stdout/stderr overflow, both-stage
  deadlines and ordinary descendant cleanup. These controlled helpers are not
  described as genuine producer semantics.
- The semantic and stale-source observations have successful process status,
  exact frame size and unchanged parser prefixes; the independent importer
  still rejects them with E0702. Failed compilation preserves existing output
  and publishes no new output. Successful and rejected calls leave no snapshot
  workspace, and recorded launched leaders are reaped.
- Fresh Rust all-target/all-feature regression: 1,930 passed, zero failed,
  65 explicitly ignored across 27 executables. The focused 25 bundle/supervisor
  tests also passed. Strict all-target/all-feature Clippy and formatting passed.
  Four fast harness controls cover evidence non-reuse, partial timeout capture,
  independent helper lifetime caps and refusal to run with assertions disabled.

The complete recipe also passed using the ordinary optimized release compiler
built at `b164641025bce1551bba1e5195ab92bed48c8534`, whose compiler inputs
are unchanged from the qualified checkpoint. A separate parity replay selected
the exact same trusted bundle and retained source on both debug and release
compilers: check/run/compile provenance and native executable bytes agreed.

These are local ordinary debug/release producer-route results, not hosted
exact-head CI, cross-platform support, a source-binding update or full compiler
self-hosting. The full Rust regression and Clippy results above are ordinary
debug checks; no new release all-target test matrix is claimed. Rust/compiler/test inputs remained byte-identical while adding the
Python qualification recipe and this documentation, so the regression evidence
is reused only for those unchanged inputs. No default route, source grammar,
Result-only restriction or frozen binding authority was changed.

The `local-hir-producers` CI job now independently builds ordinary debug and
release executables on the existing pinned Linux LLVM 19.1.7/Rust 1.99.0 stack.
`scripts/run_hir_producer_ci.py` selects the actual non-test executable from each
successful Cargo JSON build receipt, joins its byte identity to the exact clean
checkout and recipe report, and retains compiler copies and command evidence.
Each profile uses a distinct new qualification directory. An always-upload step
preserves failure evidence; existing jobs, triggers and permissions remain
unchanged. Hosted execution is still pending, and this bounded recipe does not
replace any existing gate.

At pre-successor checkpoint `5871a92`, the unchanged frozen source-binding
package intentionally did not admit these new dependency/module inputs. Broad Python discovery reported 615 tests,
six errors and two skips; all six errors were `changed input: Cargo.lock` from
current-source admission (including two setup-class failures). These failures
are preserved rather than bypassed.

The separately reviewed local successor preserves that 324-member HIR manifest
as `hir-import-source.json` and adds an exact 12-path producer transition. It
binds 327 selected inputs, 249 compiler-source members and 252 compiler/build
bodies. Inversion restores both Cargo files and removes exactly the three new
producer modules before the unchanged HIR inverse and earlier chain. Earlier
authorities, oracles and the 20 checked-HIR fixtures remain byte-identical.
Current adapter/Unit4 seals advance explicitly. Exact privacy staging retains
the real Cargo files and passed all 42 probes (seven positive, 35 negative).
The exact-head local CI-wrapper rehearsal at `4ba8087` passed both ordinary
profiles. Preserved current-count/outer-diagnostic test failures at that
checkpoint are corrected separately; their changes do not alter compiler inputs
or historical expectations. Local focused checks and hosted aggregate
qualification remain distinct evidence layers.
