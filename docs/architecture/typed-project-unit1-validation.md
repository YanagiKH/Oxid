# Typed-project Unit1: private source-set groundwork

Unit1 introduces an immutable source set, file-aware source access and a private
module-discovery loader. The public `typed-preview` check/run/compile path still
accepts one source file and rejects `mod`, `use`, `pub` and qualified item paths.
It cannot resolve or execute a multi-file program. The full proposed contract is
[RFC 0015](../../rfcs/0015-bounded-typed-projects.md).

Base: `fbcfeb2a2de8fe9d335d6c8051d254cccdb663dc`, tree
`658c83465aed3a1423483dcdcc23a3d10037131f`. Private discovery is qualified on
Linux x86_64 only; the ordinary one-file route preserves its existing hosts.
Final candidate identities and qualification status are kept together in the
validation section below. No native target or feature-inventory entry is added.

## Implemented surface and later work

| Surface | Unit1 boundary |
| --- | --- |
| Public driver | Uses `ProjectSources::load_original`; existing one-file grammar and scalar/owned routes remain authoritative |
| Private source discovery | `load_modules` accepts existing scalar/owned grammar plus file-scope `mod name;` and `pub mod name;` |
| Immutable ownership | One frozen SourceMap, dense per-file ASTs and module headers; failure owns the sources retained before the error |
| File-aware access | Checked spans select their owning file; function/record/expression/block handles include their file or function owner |
| New parser mode | Extends the existing parser; no pre-scan, token erasure, second parser or source concatenation |
| Imports, public functions/structs/fields, qualified paths | Still rejected, including by the current private discovery parser |
| Shared namespace/import/visibility index | Unimplemented; Unit2, including concrete I/J/W proof |
| Global declaration mapping and linked consumers | Unimplemented; module-major handle iteration is groundwork, not a global declaration index |
| Mechanical/opaque multi-file Batch and public activation | Unimplemented; later qualification must establish their distinct operation/fuel schedules |

`ModuleId` and `SourceFileId` have equal DFS ordinals but remain distinct types.
Root is 0. A function or record handle is `(file, local index)`; an expression
handle is `(file, ExprId)`; a block handle is `(FunctionAstKey, BodyBlockId)`.
Local IDs are not silently interpreted as global IDs. Handle iterators retain no
per-use table. The future index will map dense global DefIds/RecordIds to these
original handles.

Checked access rejects unknown owners, reversed/out-of-range/non-UTF-8 spans
and absent local indices. `SourceFile` access checks that a span names that file.
The owned resolved program borrows a file-aware SourceView rather than one source
string. The public path supplies the complete SourceMap; a checked one-file
adapter supports existing private callers. Scalar and owned diagnostic schedules
remain distinct. A frozen source owner is never reopened for diagnostics or run.

## Filesystem and diagnostic boundary

Private discovery parses each file completely before reading its children,
then visits declarations depth first in lexical order. Root `mod jobs;` means
`jobs.ox` beside the invoked entry; jobs' `mod retry;` means `jobs/retry.ox`.
Imports do not discover files. There is no directory source search, alternate
`mod.ox`, cache, fallback path or module initialization.

A module-free root receives no new case/file-kind/canonicalization/path admission.
With children, the invoked entry's containing directory is canonical project root,
including when the entry itself is a symlink elsewhere. Entry display spelling
is preserved. Child display paths use that supplied parent spelling; native
canonical pathnames are separate identity keys. Child components must match case
exactly, have no symlink indirection, and be directories until the regular-file
endpoint. Canonical containment and repeated canonical pathnames are checked.
Distinct hardlink pathnames remain distinct module identities.

Each requested component is verified by a complete bounded directory scan.
Only exact/folded booleans survive a yielded name; names are not bulk-collected,
sorted or used to discover source files. An exact match wins even if a differently
cased entry exists. Without an exact match, folded-only means E0005/source and
no match means E0002/source. Either scan cap gives the same bounded diagnostic,
`module directory scan budget exceeded`, hiding partial case/missing facts.
Every yielded entry, including an iterator error, charges E. On an error entry,
E arithmetic/cap precedes the I/O error; on a returned name, E and U arithmetic
precede the common scan cap. Metadata I/O causes remain host-specific.

Same-parent exact declaration duplicates produce E0201/resolve at the later name,
with the earlier name secondary. ASCII case-only collisions produce E0005/source.
Repeated canonical entry identity uses real root EOF as its earlier origin.
Child read/OXBC/UTF-8 failures point to the declaring module name, without an
invented child span. Root read/OXBC/UTF-8 precedence and old origins are preserved.

New handled loading admission/reserve errors are E0400/source-project. If no
SourceFile has been retained, a root allocation failure has null primary; a real
root EOF is used only after retention. Child failures use the declaring mod name;
lexer/parser failures retain their own origins. Checked length/product/attempt
arithmetic is classified as Overflow before a reserve. Only a reported fallible
Vec/String/PathBuf reserve error is Allocation. Impossible count/owner invariant
failures remain E0500. These distinctions do not broaden source acceptance.

The contract assumes a stable filesystem. Path-based checks/opens are not a
TOCTOU sandbox or an atomic snapshot and do not bound OS blocking. Private discovery rejects non-Linux hosts with E0005/source. Its current
implementation gates `target_os = linux`, not architecture; execution evidence
is Linux x86_64 only, and other Linux architectures have not been qualified.
Equivalent policy evidence is required before any broader host claim.

## Source and loader resource ledger

The private loader retains the following inclusive caps. All counts span the
loaded source set; file boundaries do not reset them.

| Quantity | Cap |
| --- | ---: |
| Source bytes B | 1,048,576 |
| Non-EOF tokens T, including trivia | 100,000 |
| Counted syntax nodes N | 100,000 |
| Modules M, including root | 256 |
| Depth D, root = 0 | 32 |
| Module component bytes | 255 |
| Logical relative file-path bytes, including `.ox` | 4,096 |
| Retained path payload P | 4 MiB |
| Component probes V | 8,448 |
| Yielded directory entries E | 100,000 |
| Yielded native name-storage units U | 16 MiB; bytes on the qualified Linux host |

One module declaration adds one node, charged before append; existing grammar
node charges and recovery order remain unchanged. EOF is excluded from T but
stored once per file. Token length remains 65,536 bytes, expression/block nesting
64, parameter/argument limits 256 and diagnostic count 100. Qualified-path Q=34
and index I=32 MiB, J=16 MiB, W=256,000,000 remain later-stage proposals, not
implemented Unit1 admissions.

Count-only child planning checks arithmetic and then M, D, component, relative
path, P and V before the corresponding allocation/probe. The first child's known
M/D/component/relative dimensions precede root host/canonical-path admission.
Returned canonical-path lengths can only be admitted once they are available.
Bytes are capped before encoding; lexer token length precedes aggregate tokens;
parser nodes are charged before append.

### Measured requested storage

The measurements are Rust 1.98.1 on Linux x86_64, from the
`frontend::project::tests::measured_layout_and_requested_inventory` test. They
are lengths multiplied by measured Rust layout sizes, not Vec capacity or RSS.

| Layout | Bytes |
| --- | ---: |
| SourceMap / SourceFile / Span / Token | 24 / 80 / 24 / 32 |
| Program / Function / Param / BodyBlock | 144 / 160 / 80 / 72 |
| Stmt / StructDecl / StructField / Expr | 136 / 96 / 104 / 80 |
| Argument / FieldInit / ItemId / ModuleDecl | 88 / 56 / 16 / 80 |
| ModuleHeader / ProjectSources / SourceUsage | 144 / 176 / 72 |
| FunctionAstKey / RecordAstKey / ExprKey / BlockKey / DFS Frame | 16 / 16 / 16 / 24 / 16 |

Let L be the sum of one initial line start plus one per LF byte in each file.
Let F/R/PA/BB/S/RF/X/A/FI/IT/MD count retained functions, records, parameters,
body blocks, statements, record fields, expressions, call arguments, field
initializers, lexical items and module declarations, respectively.

Exact retained requested payload components are:

- Source headers `80M`, source bytes `B`, line starts `8L`
- Token tapes `32(T + M)` and AST headers `144M`
- Nested AST payload `160F + 80PA + 72BB + 136S + 96R + 104RF + 80X + 88A + 56FI + 16IT + 80MD`
- Module headers `144M` and outer ProjectSources value `176`
- P counts each owned display string, logical relative string, canonical file
  pathname and canonical root-directory pathname once. SourceMap's inline
  header is already part of the outer owner; it is not added a second time

`L <= B + M` independently of token/node count. The independently measured
newline-dense fixture uses the full 1,048,576 source bytes, 32 non-EOF tokens,
zero nodes, 1,048,513 line starts and 8,388,104 requested line-index bytes. A
zero-byte file still has one line start and one EOF. Line indices cannot be
bounded by token count alone.

The independently derived nested-AST envelope is `280N`. Assign each retained
allocation to one counted parser node, using the following maximum charge:

| Counted node class | Assigned retained bytes |
| --- | ---: |
| Function, lexical ItemId and outer body block | 160 + 16 + 72 = 248 |
| Record and lexical ItemId | 96 + 16 = 112 |
| Module declaration and lexical ItemId | 80 + 16 = 96 |
| Parameter / field declaration / field initializer | 80 / 104 / 56 |
| Statement and at most two new branch/body blocks | 136 + 2 × 72 = 280 |
| Expression and at most one value-argument slot | 80 + 88 = 168 |
| Explicit borrow argument | 88 |

Each assignment is at most 280 bytes; function/item/block allocations are not
counted again, and inline Vec headers already belong to their measured parent
types. Summing over N disjoint charged nodes proves this conservative envelope.
It is an inventory bound, not a new admission ceiling or part of future index I.
The existing parser heights vector contributes up to `8N` simultaneous scratch
while a file parses; it is separate from retained AST output.

Fixed read scratch is 8,192 bytes including the sentinel; the 33-entry DFS array
is 528 bytes, for 8,720 bytes of simultaneous explicit array storage. This is
not a bound on all Rust stack frames. Existing parser scratch retains its prior
node/nesting bounds. One explicitly reserved probe pathname has the known
canonical-root length, necessary separator and relative-path length. Directory
and canonicalization APIs also return transient std/OS objects before their
lengths are known; these are outside controlled frontend requested storage.

New controlled allocations preflight checked lengths/products and reserve
fallibly. Vec growth is amortized; strings/paths reserve their known lengths.
Source bytes become a String without a second byte copy. Reserve trace storage
is test-only. Neither requested lengths nor a handled reserve error bounds
allocator buckets, fragmentation, capacities, diagnostics/rendering, LLVM memory,
total RSS, OS allocation/work or allocation success.

### Reachability and one-file argument

With M=256/D=32 and no probe cache, the maximal source-tree probe count is 7,664:
a depth-1-through-31 spine uses 496 probes, then 224 depth-32 leaves use 7,168.
The configured V=8,448 cap is masked by M/D; its boundary requires a lowered-limit
seam. Host filename/absolute-path limits can similarly mask component=255 and
relative=4,096. These are not reported as reached default source boundaries. The default P=4 MiB
boundary was not forced or claimed; its exact representation and reduced-limit
boundary are established without a global unreachability claim.

For every predecessor-admitted ordinary one-file input, M=1/D=0; no module
component, relative/canonical path, probe, E or U cap applies. Root display
spelling has no new P cap. B/T/N, token length, node charges and existing nesting
and parameter limits remain unchanged. On the measured 64-bit host, at most
1,048,577 line starts request at most 8,388,616 bytes; root source/program/module
header products are fixed 80/144/144. These new products cannot overflow in the
predecessor envelope. Local IDs, route selection, scalar/owned diagnostic order
and executable operations/fuel are unchanged. No mathematical Unit1 admission
therefore rejects that envelope; this does not promise identical allocation
success under physical memory pressure.

Loader loop bounds are separate from future index W: at most 32,385 sibling
pairs, conservatively 16,516,350 exact/folded spelling-byte comparisons; at most
32,640 canonical-path pairs with byte work O(M times retained canonical units);
and V/E/U probe visits, entries and units, with at most 2U spelling bytes
inspected apart from length tests. Source line preflight and fill each scan
admitted bytes once. Short reads may yield B positive reads; amortized reserve
avoids quadratic copying. OS interruptions and internal filesystem work are
outside these bounds.

Full original-one-file I/J/W redundancy still requires Unit2's actual original
bindings, imports, ID maps, domains, sorting scratch, lookup work and allocations.
The current loader formula is not that proof and does not qualify linked-project
ownership or runtime behavior.

## Validation: frozen core and regression package

The final v4 core contains 387 files / 5,521,755 bytes, including the corrected
Overflow-versus-Allocation diagnostic distinction. The source/code snapshot
manifest SHA-256 is
`e163437337f8690e8059308d59b42ce9296d02778212f3be7a8b9b6f87830f32`;
the producer source manifest SHA-256 is
`7506a078bdc81bb4180a2c066a85a94aea39b9dbe8c11b6d72f6e67d03167822`.
These identities describe the frozen implementation core, not the later package
containing the portable regression runner and publication documentation.

| Core gate | Recorded result and scope |
| --- | --- |
| Ordinary Rust, debug and release | 724 passed per profile across 16 targets; 21 existing opt-in tests ignored per profile |
| Formatting and strict Clippy | Passed |
| Repository verifier | Passed: 122 sources / 68 runnable programs |
| Core-only Python methods | 173 passed; two explicitly conditional native-receipt classes skipped because their collected receipt input was absent |
| Independent private core, debug and release | 69 test executions passed per profile, including 16 reserve-failure scenarios, 16 distinct checked-counter overflow scenarios and direct arithmetic controls |
| Public compatibility | 35 cases / 560 fresh candidate executions matched exact predecessor exit/stdout/stderr bytes: 31 preservation cases / 496 runs; four still-disabled migration cases / 64 runs |
| Public one-file native controls | Six actual source-free ELF executions: scalar result 42, existing single-file Batch result 816, and Unicode/CRLF overflow, each compiled with both debug and release compilers |

The compatibility matrix is check/run × JSON/text × debug/release × two repeats
for each source case. Its 560 candidate observations are separate from the 560
pinned-main baseline observations and earlier candidate runs. Independent audit
checked all 1,120 candidate stdout/stderr artifacts, 504 JSON records and 324
primary/secondary labels, including UTF-8 scalar/CRLF coordinates. Neither the
CLI corpus nor its equal-offset one-file cases directly inventories all global
IDs or proves every possible fuel schedule.

The final public compiler hashes used by compatibility and the six native
controls are:

- Debug: `44562148e2cc59dce7aa0bf1160f372d2dac8f125c6ed56cd86aa99ceadfbf02`
- Release: `04e6538e6afa1576c91196a7c538c414f9e932cad045268e9d7f1a28c5225a9f`

The compatibility observation manifest SHA-256 is
`1b14b4cb0720ceb0771780544401f7192ca4da70bf27fe49f4a40e57d18af8a4`;
the source-free native receipt manifest is
`fe673f4a907d353f1c291e16b6f420060efbfed1d1309a46207a84e57e230592`.
Independent core debug/release result manifests are respectively
`18054058a5e2e81d2c8cd4064c9f9cd0d78ab135af8aad4ede695cc10daae4d7` and
`19ab032b887bdb0538e4ffcf4021bedae38d28da0c88d561c9eae634c8d0255b`.
Measured layouts use Rust 1.98.1 on Linux x86_64; native execution retains LLVM
19.1.7/O0. The 21 opt-in Rust tests and conditional Python classes are not claimed
as executed by the ordinary suites. The six native controls are a focused
one-file regression, not a recollection of the full historical native corpus.

The two retained precedence findings were a directory-iterator error escaping
before an already-exhausted entry budget, and initial canonical-path admission
preceding the first child's known M/D/component/relative limits. Failing and
corrected observations remain separate evidence. Final counter-overflow controls
show no reserve attempt; capacity-overflow injection exercises a real fallible
reserve error with renderable null/root-EOF/child origins. Neither is observed
process OOM or a claim of total OOM recovery.

### Reusable regression package and CI registration

The [portable regression package](../../tests/fixtures/typed_project_unit1_independent/README.md)
is integrated unchanged at `tests/fixtures/typed_project_unit1_independent/`:
16 files / 860,747 bytes, sealed by manifest SHA-256
`0dea4f47aff92721cf4f03a6164f0e1bc2e996c57f4b08248894cb6701c5c3d8`.
It preserves the same 69 independent test functions and their frozen source,
resource and diagnostic expectations. The test-roster SHA-256 is
`331bf92273fb3f9904aeff24e279ff546387a211239e0ae7ce1ac20c04468f79`.

A separate real package smoke passed 69 debug test executions, zero ignored,
exit 0. Its TMPDIR parent contained spaces, Unicode, double quotes and a
backslash; fixture inventories and exact named-test execution were independently
audited. The package smoke's binary SHA-256 is
`55b9425db409c3ce2c528f71699d886a042599dea1baf86b4b6087ef1b97cc35`,
and its result SHA-256 is
`c1d5cd4d10fd3dab4b374dce611168c80dde176caf678c9efaf52e5c5cc90835`.
These 69 runs are separate from the original core's 138 debug/release executions.
No new package release smoke is claimed.

The separate [runner protocol tests](../../scripts/test_unit1_runner_protocol.py)
preserve nine fast controls; all nine passed against the sealed package.
Their file SHA-256 is
`8bd45e039c4b96fc08014c146f14961c3a8c177f8b2c9f20ca70c691df525a6d`.
They use a deterministic fake libtest executable and are harness tests, not
additional compiler executions. They accept exact success and reject empty,
duplicate or substituted rosters, listing failure, zero actual execution,
ignored execution, the wrong executed name and nonzero test exit. The zero-test
control was also demonstrated failing against the original defective runner.

The subsequent integrated scripts suite passed 182 tests with two conditional
native-receipt class skips, including these nine protocol controls. This result
is separate from the historical core-only 173-test run above; it does not
retroactively change that run's count or execute its skipped classes.

Historical package findings remain part of the evidence:

- The original runner could claim 69 successes while executing zero tests.
  The corrected runner requires the frozen roster and exactly one requested
  named passing, nonignored test per invocation, and propagates failure
- Python optimization could remove assertion-based qualification checks.
  Relevant package entry points now refuse optimized Python
- Direct path interpolation and JSON Unicode escapes could generate invalid
  Rust. A shared Rust string encoder and the unusual-TMPDIR smoke qualify the
  correction without changing source fixtures or expected values
- Output/cache/temp overlap could write inside the input tree; listing/toolchain
  failures also needed stronger retained evidence. The runner rejects overlap
  and unavailable toolchain evidence, bounds listing/test calls to 25 seconds,
  and retains failure results. The separate timeout probes are not part of the
  nine fast controls
- A local Python import could create installed-package bytecode. The final
  entry point disables helper bytecode generation before importing; an
  installed-layout control preserved inputs without relying on an environment
  bytecode setting

These are corrected harness findings. Compiler-core and semantic expectation
bytes remained unchanged through packaging. The package requires Linux x86_64
and exits 77 with an explicit skipped result on unsupported hosts. Its own
platform check does not change the loader's Linux OS gate or extend its host
qualification. The fast protocol controls require POSIX executables and
explicitly skip elsewhere.

### Public compatibility replay package

The [public compatibility package](../../tests/fixtures/typed_project_unit1_compatibility/README.md)
is staged at `tests/fixtures/typed_project_unit1_compatibility/` with seven
byte-verified files. Its [package manifest](../../tests/fixtures/typed_project_unit1_compatibility/package-manifest.json)
SHA-256 is
`0f331a077a7e67b8e8943dfd463273f61ebbb64f08e2a60e0545b391479d3215`;
the complete package seal is
`22377ef35af826678099fea8bf010cb8f08bcf10efb210c1fe6865eeb047f7e4`.
The frozen generator, semantic intent and corrected CLI engine are preserved
byte for byte. Packaging deduplicates the original baseline to 140 canonical
case/operation/format results only after checking equality of all four original
profile/repeat observations. Expected stdout/stderr bytes are retained without
normalization; they are not generated from candidate output.

One real package run passed 560 exact predecessor comparisons, retaining 1,120
streams, 504 JSON records and 324 diagnostic labels. These 560 package executions
are additional to, and reported separately from, the earlier final-core
560-comparison run. Expanding the canonical baseline into the engine's temporary
comparison-key table creates derived expectations with zero executed observations.
It does not count as another compiler run.

The final independent public-CLI package review passed with no material finding.
Its saved-evidence audit proved all 140 canonical outputs against every original
profile/repeat and all 560 real saved package invocations. Eighteen additional
corruption/path controls passed with zero compiler reruns. They checked the
strict 560-invocation default versus the explicit 496-invocation original-only
mode, evidence-root/symlink overlap rejection and retained failures under unusual
paths. These controls are harness evidence, not additional compiler executions.
The README includes a [standard-library audit recipe](../../tests/fixtures/typed_project_unit1_compatibility/README.md#narrow-independent-check-of-one-retained-full-run)
that independently checks a retained full run using JSON/base64/hash/filesystem
operations, without importing package code or running the compiler.

The Unit1 gate must use all 35 cases, including the four still-disabled
mod/use/pub/qualified-path forms. `--original-only` is explicitly forbidden for
this gate. Its future 31-case option does not authorize changed expected
migration outputs or activate source syntax. The original baseline engine's
post-manifest finalization exception and historical receipts remain preserved;
this package uses the already-corrected frozen engine.

### Public rerun commands

From the repository root, with its Rust toolchain and cached dependencies, run
the complete private suite in both compiler profiles. Builds are offline;
output, Cargo cache and TMPDIR must be outside the repository, and the selected
output directory must be new or empty:

```sh
python3 -B tests/fixtures/typed_project_unit1_independent/run.py \
  --repo . --profile both --output ../oxid-unit1-regression-results
```

The [runner](../../tests/fixtures/typed_project_unit1_independent/run.py) creates
an isolated source copy, injects the held-out test module there, verifies input
and fixture identities before and after each profile, and keeps result manifests
and logs while cleaning its temporary source/fixture/build trees. `--profile
debug` or `--profile release` selects one profile. A result must show actual
named executions; compilation or test discovery alone is not a pass.

Run just the nine fast controls, or the existing scripts suite including them:

```sh
python3 -B scripts/test_unit1_runner_protocol.py -v
python3 -B -m unittest discover -s scripts -p 'test_*.py' -v
```

To replay public compatibility against existing debug/release CLIs, Python 3.11
or newer is sufficient; the wrapper performs no compiler build or repository
write. Its fresh retained evidence directory is outside the input repositories:

```sh
python3 -B tests/fixtures/typed_project_unit1_compatibility/run.py \
  --debug target/debug/oxid --release target/release/oxid
```

Use an existing external directory with `--evidence-root` to choose its temporary
parent. Package/binary/fixture identities, actual compiler observations, raw
streams and the completeness audit remain in the printed evidence directory on
both success and failure.

The Linux quality job in the [CI workflow](../../.github/workflows/ci.yml)
registers the scripts discovery command above and this exact package command:

```sh
python3 -B tests/fixtures/typed_project_unit1_independent/run.py --repo . --profile both --output "$RUNNER_TEMP/typed-project-unit1-results"
```

After the release build, the same job registers the full public compatibility
replay with this exact command:

```sh
python3 -B tests/fixtures/typed_project_unit1_compatibility/run.py --debug target/debug/oxid --release target/release/oxid --evidence-root "$RUNNER_TEMP"
```

These are verified CI registrations, not hosted execution. The combined
[publication input manifest](typed-project-unit1-inputs.json) records the final
publication inputs and excludes its own file to avoid self-reference. It identifies
the unchanged qualified core separately from later documentation, test-package
and CI additions. Exact publication-head hosted CI has not yet run and must pass
before merge; local core/package results are not a substitute for those checks.

Held-out coverage includes entry symlinks, canonical/display identity, exact and
folded case, missing children, duplicate/case-only declarations, final and
intermediate symlinks, nonregular children, distinct hardlinks, DFS ordering,
aggregate byte/token/node admission, lowered resource caps, malformed owners and
equal-offset UTF-8 spans, multi-file primary/secondary labels, root-null/root-EOF/
child failure origins, public rejection and old diagnostic schedules. An optional
Unix-socket fixture could not be created because the environment denied socket
creation; it supplies no socket-file qualification. FIFO and directory fixtures
are separate observations.

Final observations must bind command argv, exit/stdout/stderr, input/source hashes,
profile, compiler/toolchain, host and the specific admission seam. Public CLI,
private facade, lowered-limit and unreachable-bound evidence are identified
separately. Profile counts, process counts, cases, distinct sources and artifacts
must not be substituted for one another.

Historical [owned-source validation](owned-source-validation.md),
[owned-consumer validation](owned-consumers-validation.md) and their immutable
input manifests/receipts keep their original identities. None is refreshed or
silently relabeled as Unit1 execution. Real module imports/privacy, cross-file
ownership, linked consumers, source-free native project pilots and source
activation remain later-stage obligations.
