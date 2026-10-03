# Private typed-project execution: Unit3 validation

Status: private implementation with ordinary, retained compatibility, finite
source/reference, native/driver, finite mutation and bounded local portability
checks completed. Applicable hosted CI on the exact publication head remains
pending. Public project syntax is still disabled.

Public `typed-preview` check, run and compile remain single-file. Public `mod`,
`use`, `pub` and qualified item paths remain rejected. No command-line switch,
module initializer, import-time execution, module runtime object or fallback was
added. Unit4 remains the separate public activation boundary.

## Implementation and proof boundaries

The private project type-only and executable entry points share one route/index/
resolution/type schedule. Whole-project syntax selects scalar or owned once,
including unused declarations. Executable depth lowers the complete global
program with the existing selected lowerer, validates source origins, then calls
the unchanged authoritative raw verifier. All loaded original functions are
checked, including unused functions; a successful check does not require main.

The original root function named main supplies entry. An import alias named main
or a child's original main does not. Global declaration IDs remain those assigned
by the shared index; imports allocate no additional runtime identities. There is
no new function wrapper that would add runtime fuel.

A dedicated `oir::source::sealed` leaf constructs `CheckedSourceProgram` from
source inputs only. It binds the actual immutable SourceMap, existing verified
body and source-derived entry. Its run/native methods accept no replacement map
or entry. No raw-parts factory, mutable witness accessor or production child of
the seal was added. The owned typed bridge obtains its map from its checked
index/source owner, lowers once and returns the existing sealed owned body.

These guarantees are distinct:

- Index and type checks enforce source names, visibility and type permissions
- The source association audit checks declaration identities/origins, file
  membership and every stored span's complete range/UTF-8 validity
- The existing raw verifier certifies its existing OIR and ownership contract
- Reference and native consumers use that same bound body/map/entry and retain
  their existing admission, storage, operation and fuel rules

An in-range same-file span does not prove that an operation corresponds to the
source expression. A raw valid call to a wrong same-signature target may pass
raw verification. Exact source-to-call, staged argument, reborrow, lexical
cleanup, event and fuel correspondence is established only for the independently
modeled finite cases described below. No claim of that correspondence follows
from ordinary tests or this audit alone.

## Newly paid source-association work

The audit allocates no variable-size storage and retains no origin table. It
performs two explicitly counted walks over the existing admitted raw program:

- Vcount = D + S: one structural counting visit per declaration association and
  per stored span occurrence, without source-map or declaration lookup
- Vbind = D + S: declaration association validation and one map/file validation
  for every stored span occurrence

Repeated identical origins in separate fields count separately. S includes
nested scalar statement/operand/operator spans, both BoolMerge inputs, every raw
scalar call argument and both fields of each present DiagnosticOrigins. D is F
for scalar, or F + R + K for owned functions, records and fields. Counter addition
and final count/validation equality are checked in release builds.

Dimension comparisons are separate: one scalar function-count comparison, or
2 + R owned comparisons for function count, record count and every record's field
count. Bounded declaration/index/AST mapping, dense-ID/name checks, root-entry
comparisons and loop-control overhead remain separate from the abstract span
visit total. The existing nonresetting namespace meter W is neither reset nor
used to pay for this new work.

For source-produced scalar output, with L locals plus places, B blocks, Q
statements plus merges and Aarg the actual total raw call arguments:

    S <= F + L + 3B + 4Q + Aarg
    Vbind <= 17N
    Vcount <= 17N

The scalar producer's admitted L<=N, B<=3N, Q<=N and F<=N bounds combine with
Aarg<=N: each source argument has a distinct expression root emitted once. This
is a conservative source-producer bound, not an arbitrary-raw-OIR theorem, an
elapsed-time bound, or a claim that binding costs at most N visits. The count
pass is additional work; it is not hidden inside Vbind.

For owned output, distinct stored Span payloads are already included in the
existing 64 MiB raw-byte admission. Its bound is derived from admitted raw bytes
and host Span layout plus D. That owned byte cap is not applied to scalar output.
The existing lowerer count/preflight/emission and verifier allocation gates stay
in force; this audit adds no arbitrary rejection limit.

The independent x86_64 implementation review measured a 208-byte checked source
wrapper, 40-byte Visitor, 16-byte Counts and 40-byte BindUsage in both profiles,
and observed zero allocation attempts during successful audits. These are fixed
layout observations, not total heap/RSS measurements. SourceMap/AST/path storage
remains owned by the outer ProjectSources through consumer completion; index,
typed HIR, raw output and verifier transient storage can overlap during checking.
The wrapper's borrow does not imply those outer source objects were freed.

## Diagnostics and compatibility change

Incorrect original source/map membership or stale parser identity now produces
E0500 at stage `resolve-project` with null primary and secondary origins before
compilation. An equal byte range in a substitute map is not an authorized origin.
This is an intentional internal malformed-input change, covered separately by
wrong-map, stale-AST and nonzero-file controls. It is not described as unchanged
Unit2 behavior.

For a real admitted source set, parse/load/resolve/type precedence stays intact,
followed by lowerer aggregate preflight and emission, source association audit,
then raw verification. Association failure is E0500 at `oir-project-bind` with
null origins and cannot be presented as a source ownership denial. Existing
ownership codes still come from verifier-derived facts, not optional origin
metadata. Whole-program checking precedes entry admission, so an unused child's
ownership error takes precedence over missing main.

## Current evidence and identities

Implementation core-v1 has 117 compiler/build inputs, current manifest SHA-256:
`53ed114674e59266c8e9a809f6cba5ef35c9ea819318ab64a7ae6d9aebd20910`.
It began on the Unit2 development tree and was reconciled metadata-only to merged
main `9e61a384e49302db6f47cde4159ae94538bb020f`, identical base tree
`017495b7f8abcc5f475994774dc522e8d6f54f84`. No source identity was changed by that
reconciliation.

Fresh ordinary checks used official Rust 1.99.0, incremental off and at most two
Cargo jobs. Debug and release each passed 778 tests, with 21 explicitly ignored
and zero failures. Clippy with warnings denied, formatting, release build and
repository verification passed. Python unittest passed 182 tests; two additional native test classes were
skipped pending actual native receipts. Repository verification counted 122 sources and 68 runnable programs.
These ordinary counts are separate from Unit3 composition qualification.

The original Unit2 package, its source/package manifests, observer, expectation
logic, resource payloads and historical receipts remain unchanged. The new
compatibility adapter requires explicit selection of the current source manifest
and creates a derived package changing only source-inputs.json and its identity
inside package-inputs.json. It then runs the byte-identical historical runner.
A fresh outer result binds the current source identity, historical and derived
package identities, adapter hash and actual child result/invocation. There is no
fallback to historical compiler inputs or retargeting of a historical receipt.

That current-input replay passed all 3,603 unique semantic cases and 21 named
resource tests per profile in debug and release. The untouched public gate also
passed all 35 cases, 560 exact predecessor comparisons and 1,120 streams,
including all four public project-syntax rejection cases. The full gate was used,
without original-only reduction or changed expected output bytes.

The initially rejected compatibility start found a generated preprocess cache in
src/.oxid after ordinary repository verification. Its exact source-membership
check rejected before compiling. The generated files were preserved outside the
source tree and the successful run used a fresh invocation/output identity.

## Native scope and remaining qualification

Unit3 reuses the existing native consumers. The selected route's whole linked
program is subject to their existing aggregate limits; module boundaries do not
reset limits or fuel. The call graph must be acyclic, including unused functions
and statically skipped calls. Native entry/admission remains before external
LLVM tools or output creation; no-clobber and no-fallback behavior is unchanged.

Native support remains experimental Linux x86_64 only, target
x86_64-unknown-linux-gnu, LLVM 19.1.7 at O0. Private project loading has evidence
on Linux x86_64; no additional filesystem host or native target is qualified.
ELF output still depends on compatible host libc/loader. There is no portable
binary, minimum-glibc, ABI, M1/M2/M3 or v1.0 completion claim.

Existing inclusive native bounds are 256 functions, 64 parameters per function,
256 scalar slots per function, 8,192 aggregate/maximum-call-path scalar slots,
4,096 aggregate blocks, depth 32 and conservative acyclic fuel 100,000. Guarded
modules retain shared dynamic fuel 1,000,000. Owned adds S+O<=256 per function,
8,192 aggregate and maximum-call-path expanded cells, 1 MiB explicit arena-byte
bounds, 16 MiB diagnostic data and 64 MiB emitted LLVM text. Scalar guarded
emission retains its existing 16 MiB diagnostic/64 MiB text limits. These limits
do not bound LLVM spills, host/tool resources or total compiler RSS.

The finite native/driver gate passed independent review against core-v1. Its
primary roster has 300 invocations: 64 default native, 36 reduced-fuel native,
36 paired reference, 144 private-driver and 20 no-clobber. Six initial debug
smokes remain separately counted. Actual source-free ELF execution covers 96
primary cases plus three smokes. Each has ELF magic, exact copied executable
identity, absent original source paths, and an execution receipt in a fresh
directory with the recorded minimal environment. All 306 compiler/driver stream
and status comparisons and all 99 ELF stream and status comparisons matched the
independent expectations. These are actual runs, not model predictions or
IR-only evidence.

Every native/driver invocation made original source paths unavailable after
loading. All 20 primary no-clobber cases preserved regular files or symlinks and
sentinel bytes. Admission denials and check/reference invocations recorded zero
external tools. The four frozen driver-boundary mutation IDs are attributed to
their existing eight profile results, without adding executions or counting
them as internal raw mutations.

This evidence is bound by native artifact manifest
`f73c125e3b3099a93d9f3be5e20a51abadc5d28feab58dfc8558024f670130ec`
and independent verdict
`9e0b6198bc17c29432921ff995dac758a6c92488625e435813f73c1e6d300ed7`.
Native qualification covers its separately frozen covering subset, required
pilots/control and selected negatives; it does not imply ELF execution of all
72 composition cases.

The 24 scalar and 48 owned composition families, separate owned main-at-ID1
control, pilots and additional controls belong to the finite source/reference
gate below. Public activation remains Unit4.

## Finite source/reference evidence

The frozen corpus contains 152 source cases and 445 source files. All 304 unique
case/profile observations matched their independent contracts in debug and
release: 244 successful checks and 60 expected source rejections. There were
2,798 actual reference runs, of which 2,764 receive full model comparison. The
105 model-backed source cases have detailed source/raw and dynamic
correspondence; the 47 authored controls have complete declaration association
and their frozen source, diagnostic and result contracts. This distinction is
part of the qualification and is not expanded by the total case count.

The independent audit verified 13,308 artifact identities and exact cross-profile
payload equality for all 152 pairs. Actual raw fields, both source-association
walks, immutable checked bodies, operation/commit/fuel journals and source-tagged
function, field, owner and loan identities remain distinct evidence. The 412
LLVM-generation attempts in this observer gate receive no native execution
credit; source-free ELF evidence is counted separately above.

The original source/model expectation package remains unchanged. The first full
debug comparison preserved a mismatch caused by an unnecessarily strict order
for independent sibling loans released within one normal return. The reviewed
projection now compares exact membership only within the same charged returning
activation and entered resume call. Physical journals and complete loan keys
remain retained. Acquisition order, ordered return groups, parent/child release
constraints, charge/commit boundaries and reverse lexical StorageEnd cleanup
remain strict. The failed report, predecessor comparator and six invalid
release-group controls are retained with the correction history.

This gate is bound by source qualification manifest
`5ad1cdcf1fbbc0fd24093c3359c2f46c4450e378bd1c40754131dc1aa1bd774a`,
comparator-v5 manifest
`22666fa44ee61e8ee70dd6f4605e87000f3872f2f2728c20c0843ff35755d32d`,
and final comparison
`d9ee15c73d7d806d09698cf51d0b8c7395ffc53bb8b4638b968a5cc1daca4a61`.
These are finite checks, not a proof for all source programs or module trees.

## Publication test selection and provenance

The separate publication successor is based on merged main
`9e61a384e49302db6f47cde4159ae94538bb020f`, with input manifest
`a8e24a8d14c8b47140297f9b2b37adc75b2932911f8745df5937d58bdb2dc963`.
Its bridge preserves core-v1's manifest and historical development-base identity.
Of the 117 compiler/build inputs, 116 are byte-identical. The sole changed input
is a test-only file: five linked-loader tests and their helpers are Linux-only;
the portable public-syntax rejection test remains active; a non-Linux test
requires E0005/source before any probe of a declared missing child. The private
loader already enforced this policy. Production code and Linux test bodies are
unchanged.

Fresh Linux ordinary checks on that successor passed 778 tests with 21 ignored
per profile, formatting, and Clippy with warnings denied. The initial Clippy
launch selected the default Rust 1.98.1 driver and stopped with an incompatible
crate-version error before checking Oxid. Its log is preserved; the explicit
Rust 1.99.0 retry passed. The successor check receipt is
`83aab9e19c4b088248f8747f6e34f68008e32dffa1a20c1498a51d08d87dd561`.
Actual macOS/Windows execution remains an exact-head CI responsibility. Existing
core-v1 source/native/resource runs keep their original identities and are not
relabeled as executions of the successor bytes.

The portable semantic gate uses an explicit production-byte identity bridge.
It verifies the selected successor manifest, permits exactly the reviewed
test-file difference, restores that hash-bound core-v1 test file in a fresh
derived tree, and verifies all 117 original inputs before applying the unchanged
observer adapters. Its semantic executions are labeled derived core-v1 linked
to the selected successor, not direct successor test builds. Ordinary CI builds
and tests the actual publication-head files. The quality and native jobs pin
Rust 1.99.0 for reproducibility; the separate four-host ordinary matrix continues
to follow stable Rust. Workflow registration is not a completed CI result.

## Finite mutation evidence

All 110 internal requests pass independent comparison in debug and release:
208 new results plus 12 retained smokes, totaling 220 unique case/profile
receipts. Four driver boundary requests have eight separately qualified profile
attributions and add no executions to the native gate. The independent audit
verified 7,749 identity items, exact application of the prescribed faults, all
110 profile payload pairs and all 12 original baseline passivity joins. Raw-safe
wrong-call and cleanup mutations fail source correspondence; they are not
misreported as raw-verifier denials. The generic metadata probe verifies a deep
clone of the exact mutated raw body and cannot supply a source wrapper.

The first complete comparison was 218/220 and remains preserved. Mutating the
failing ReadField's in-range same-file cause leaves E0311 unchanged, because the
acquired-here cause comes from the PrepareBorrow acquisition. The reviewed
source-only supplement reconstructs the original independent AST, source bytes,
98 frozen origins and module/model mapping, and applies the predecessor
lowerer's rule: field-expression primary and enclosing-statement cause. All
four retained same-file primary/cause controls fail that source-prescribed raw
metadata obligation, while baseline metadata matches. No candidate observation
defines an expected origin, and the frozen denial result and authority remain
unchanged. The supplement is
`755fcf1e92d695ab5202481e3d88edd349d4faf382be8eb4a5db4e3df759f11d`.

The source-only correction for a unit-call control with no scalar result and the
passive stale-parser identity observation are also separately bound. Both are
restricted by exact mutation ID; the original 114 requests and earlier evidence
remain retained. The final comparison is
`86691bf1857e674b73e1a761761875b3cff51c884900738f45f880c65bf705ff`,
comparator-v3 manifest is
`f34a680ebedc8a380d137ffd93eeb6c16fd737299ae4785bb8e473a776537d6e`,
and independent verdict is
`d4dc25c3964eecf444430b58f00a91279bce4d5882537be22da7514c108ed87b`.

## Portable package and hosted CI boundary

The [portable package](../../tests/fixtures/typed_project_unit3_independent/README.md)
contains one source archive and separately bound compiler, observer, oracle,
comparison and mutation components. The originally qualified package manifest is
`607e40ae9f61855eaa09341f08e272c79f7564255fa1f2b78ad65bb615ad1ebf`.
The documentation-transport successor is
`9610eeb61f9a6a783141318681511f9459e0a37559da507c8eac7bc1a1a6dd95`.
It corrects one relocated README link and restores that exact original document
before native preparation. Source-only checks require all 81 auxiliary files,
198 prepatch inputs and 199 overlay inputs to match their frozen identities.
The source archive has 13,128 compressed bytes and SHA-256
`1e6107e272f4cec23a7183eea8be23350b81b6702fe73c1c6e8d7687fe744293`.
It decodes to a 450,560-byte canonical USTAR containing exactly 445 regular
source files and 69,388 source bytes. Sorted member order, mode 0644, zero
UID/GID/mtime, empty owner names and gzip level 9/mtime zero are bound by the
transport manifest. No expanded private .ox corpus is checked into the
repository, and no broad source-verification exclusion was added.

The completed bounded local portability checks cover four source observations,
six native invocations including four actual source-free ELF executions, and
six mutation envelopes spanning all three fixed controller dispatches. All six
mutation baselines also match their same-profile counterparts. The reviewed
relocation permits exact source-prefix strings and their derived stored-path
byte delta, plus LLVM diagnostic constants and their checked byte lengths;
null canonical-path slots remain null. These are not extra full-corpus runs.
The final portability comparison manifest is
`835cb2ae9c39659b77924b1849fdd82fe91c63182df149b6f0e0abf24502ec86`.

The workflow preserves the existing gates and adds exact full plans for 304
source observations, 300 primary native/driver invocations and 220 internal
mutation envelopes in one qualified Linux job. Three mandatory independent
comparisons follow collection; mutation comparison joins its eight driver
attributions directly from the full native comparison. The shared source
materialization and successor bridge run before cache-producing legacy checks.
Quality's retained Unit2 gate explicitly selects the successor manifest through
the reviewed outer adapter. Historical manifests are not rewritten.

Every new row has the reviewed bounded worker timeout and terminal-failure
receipt. Source/native collection allows 360 seconds per row; mutation
collection allows 600 seconds per row, covering the controller's separately
bounded baseline/mutant processes. The combined qualification step allows 120
minutes and the native job 240 minutes, leaving room for its retained gates.
An always-run index and lossless artifact archive retain plans, original/derived
identity receipts, logs, compressed observations, IR/ELFs, exact Rust test
executables and comparison reports. Regenerable target caches are excluded.
These budgets are operational ceilings, not performance claims.

Local evidence does not establish the result of hosted CI for a new head. This
work adds no public syntax, stable ABI, new target or M1/M2/v1.0 completion claim.

Hosted build failures expose only bounded, JSON-escaped native build log excerpts
and the wrapper error field. A separate diagnostic archive is capped at 16 MiB
uncompressed and records exact selected receipt bytes, observed status/count
fields, and explicit omissions or truncated log prefixes. It supplements the
complete lossless archive. Receipt hashes in the compact archive do not replay
binary bytes, and absent comparison reports provide no qualification evidence.
The documentation correction does not claim to fix an undiagnosed native build
failure; exact-head hosted qualification remains required.

### Qualified LLVM override library directory

The later compact evidence from head
`2c327c607c7c498861580546fb434777ca27a8a8` identifies a post-build qualification
failure: both native Rust adapters built successfully, with empty stderr, before
the wrapper rejected a library-directory symlink whose resolved target was not a
regular file.
Head `9481918` had the earlier README failure and an opaque wrapper failure;
`2c327c6` corrected the README and exposed this environment mismatch. Each hosted
run of `9481918` ended with six passing jobs and two failures (full repository
verification and native). Each hosted run of `2c327c6` ended with seven passing
jobs and the native job failing. Although 304
source rows were collected, none of the three independent comparisons ran, and
downstream native steps were skipped. These runs are not semantic qualification
passes. The exact hosted offending entry was not recorded. A local Debian-like
`perl/5.40 -> 5.40.1` directory alias reproduces the same rejection in both
unchanged inventory implementations.

The correction stages only the installed Debian amd64 `libllvm19` and
`libclang-cpp19` runtime entries from version `1:19.1.7-3+b1` into a fresh LLVM
override directory. It validates exact package version/status/architecture and
dpkg path ownership, regular-file SHA-256/size and SONAMEs, and the exact relative
`libLLVM-19.so -> libLLVM.so.19.1` alias. The two regular files total 200,716,880
bytes, with content identity
`31f2db375a0008638deb261dd8789c5d16a797a2ecd9e7074a7d0752922a5597`.
That identity excludes source/output paths and timestamps. Missing, changed,
nonregular, aliased or escaping selected files, wrong packages or SONAMEs, and
reused/aliased destinations fail closed; partial copies receive no success
receipt. Every native build and collection uses the same staged path.

Both original wrappers, their inventory caps, immutable historical controller,
81 auxiliary files, 198/199 compiler input inventories, source corpus and
expectations remain unchanged. Existing tool byte/version checks and the
per-invocation full staged inventory comparison remain mandatory. The supplied
directory binds the LLVM override libraries; ordinary host loader/libc, C++ and
other system dependencies remain required, as in the original extracted-package
environment. This is not a full dynamic dependency closure or a compiler fix.

Local verification covers the Debian directory-alias regression, required-file
and package rejection controls, deterministic identity, post-stage byte/alias
changes, partial-copy failure and compact receipt retention. Real pinned library
bytes pass both inventories. The real LLVM 19.1.7 clang/opt/LLD version probes,
LLVM verification, clang/LLD link and generated ELF execution pass using that
stage. This local package-metadata check uses real dpkg-query against an isolated
database reconstructed from the retained Debian archives, with explicit source
path relocation to their existing extraction; it does not establish installed
package status on the hosted runner. No compiler corpus replay was added.

The existing compact archive additionally retains the small staging receipt and
qualified-build-tools receipt under unchanged byte caps, with no library binary
payload. The complete lossless archive mechanism is unchanged. Exact-head hosted CI and
all three comparisons must still complete before a successful qualification is
claimed.
