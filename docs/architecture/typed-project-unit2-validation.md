# Typed-project Unit2: private shared declaration index

Public `typed-preview` check, run and compile remain single-file. They still
reject `mod`, `use`, `pub` and qualified item paths. Unit2 implements the private
project grammar, shared declaration/import/visibility index, and actual scalar
and owned resolution and type checking described by
[RFC 0015](../../rfcs/0015-bounded-typed-projects.md). Linked project OIR,
whole-project ownership verification and multi-file execution/native consumers
remain Unit3. Public activation remains a separate Unit4 decision.

The ordinary single-file driver uses the shared index through its established
scalar and owned schedules. The private project facade returns only structural
type-check results. It supplies no linked execution or native conversion method,
and its index seal is not an ownership-verification witness.

## Qualification identity

The local qualified candidate is the 423-file v4 source freeze with manifest
SHA-256 `9aaadaf0567378a862ddfdbcf163045ba6fb696af88e0eac624b63f3a77abdfb`.
Its base is merged main `bfa4978c3bc5bc75810e247494683e5803119206`, tree
`3b45828e413d35efdfb7b68b09ae664fc1a1aee0`. That base tree is identical to the
published PR23 head used to start this implementation. Qualification ran on
Linux x86_64 with Rust 1.98.1 (compiler LLVM 22.1.8). The separate external
LLVM 19.1.7 toolchain was used for the native smoke. Hosted compiler layouts and
results must be recorded separately after execution.

The [Unit1 ledger](typed-project-unit1-validation.md) remains historical evidence
for its original source representation and qualification. Unit2 changes AST and
source headers; its measurements below replace no old receipt or manifest.

## Implemented private surface

`ProjectCandidate` and `load_project_candidate` extend the existing parser and
loader. `ModuleCandidate` and `load_modules` retain the narrower Unit1 discovery
grammar as real compatibility entry points. No source is concatenated, no token
is erased, and no second parser or namespace map is used.

The shared implementation retains immutable file-aware sources, dense per-kind
module-major declaration IDs, span-keyed names and checked file-local AST
handles. Flat syntactic alias and target groups are built before lexical semantic
import resolution. Type/value aliases and normalized-target admission commit
atomically after the last fallible operation. Absolute paths follow original
modules only; they never follow import aliases.

Original scalar resolution preserves duplicate/signature interleaving. Original
owned resolution checks conflicts before signatures, and checks record fields
before function signatures. Both use the same frozen index and queries. Project
visibility, signature exposure, construction and field access use the direct
requester's module. Inferred projections use the actual checked record type;
assignment checks the RHS before field privacy/mutability. Bounded canonical
project nominal names retain distinguishable record identities.

A constructor-owning child module seals original facts and the final index.
Immutable accessors cannot replace the source owner, declaration rows or domains.
The borrowed legacy facade validates actual source-map membership and parser
provenance, including nonzero file IDs. The process-local source generation is a
checked nonwrapping AtomicU64; identities are never reused and exhaustion fails
closed. Generation values do not affect language IDs or normal diagnostics.
Full AST/span/arena validation runs once at the metered collection boundary.
The seal certifies validated associations, original conflicts and atomic imports;
signature/body typing and ownership checks retain their separate roles.

## Requested storage and charged work

These measurements are Rust 1.98.1 layouts on Linux x86_64. Let F/R/K count
functions, records and record fields, M loaded modules, U imports, and
A = F + R + M - 1 original declarations.

| Row or header | Bytes |
| --- | ---: |
| Original / Function / Record / Field | 36 / 12 / 28 / 16 |
| Module / Import / Alias / Seen | 60 / 20 / 16 / 8 |
| Immutable index / Facts / Plan / Counts / Scratch | 280 / 472 / 96 / 72 / 96 |
| SourceOwner / prepared nominal name | 32 / 576 |
| SourceFile / Program / Function AST | 88 / 248 / 200 |

Ten retained vector lengths give:

```
I = 40A + 12F + 28R + 16K + 60M + 4(M-1) + 40U + 280
J = 4max(A,U) + 12U + 4M + 3528
```

The J constant conservatively includes all explicit fixed builder, transaction,
traversal, query, row/span and formatting state, including disjoint lifetimes.
The fresh AST payload envelope is 288N, with Program headers separate. The old
Unit1 280N envelope is not inherited. Source B/T/N admission and ordinary node
charges remain unchanged. Private paths admit at most 34 segments, including
contextual `crate`, and check arithmetic/node/path limits before reservations.

Defaults are I = 33,554,432 bytes, J = 16,777,216 bytes and W = 256,000,000
explicit work units. For parser-produced original single-file programs,
B <= 1,048,576 and T,N <= 100,000 imply:

```
I <= 6,800,340
J <= 403,532
W <= 53N + 162T + 99B + 13,011
  <= 64N + 168T + 104B + 32,768
  <= 132,284,672
```

The independently reviewed W derivation includes complete validation
(V <= 11N + 2T + 4 callbacks), route scans, row initialization, every stable-sort
and lookup comparison, module edges, the finite signature/body query inventory,
and up to two 65-unit prepared original names for each of 100 errors. One meter
continues through signatures, bodies and owned typing. The earlier proposed
121,960,576 bound omitted work and is not an implementation result.

Checked arithmetic precedes I/J/mandatory-W admission and allocation. The
mandatory remaining build reservation is checked against work already spent;
it is not charged twice. Original aggregate declaration admission remains first.
All 14 index reservations and the four new parser reservation families (five
positions for `use crate::f;`) use actual fallible reserves. Once admitted,
index vectors fill fixed lengths without hidden growth.

I/J measure requested frontend lengths and explicit state, not allocator
capacity, fragmentation, compiler-generated stack frames or RSS. W excludes
diagnostic rendering, OS blocking, unrelated compiler/LLVM work and test tracing.
These proofs do not promise allocator success or process-wide OOM recovery.

## Executed validation

| Gate | Observed result on v4 |
| --- | --- |
| All-target Cargo tests | 764 passed in debug and 764 in release; 21 ignored in each profile |
| Formatting and all-target clippy | Passed; warnings denied |
| Unchanged Unit1 private runner | 69/69 per profile; 138 test-function executions |
| Unit1 public compatibility | 35 cases, 560 exact comparisons, including all four closed-syntax migration controls |
| Independent source-only semantic/parser/legacy corpus | 3,603/3,603 matches in each profile; no normalization/comparison errors |
| Independent source-loader entry observations | 22 logical read-source entry checks per profile, within the semantic corpus |
| Independent predecessor legacy rendering | Seven direct legacy API cases match published Unit1 raw JSON diagnostic arrays and human bytes in both profiles, using identical source paths |
| Independent resource/ownership controls | 21/21 per profile; nine final query controls with identical profile traces |
| Portable combined adapter qualification | Fresh 3,603/3,603 semantic matches and 21/21 named resource tests in both profiles; all final identity checks passed |
| Portable execution/identity controls | 13 protocol tests and 62 independent adapter controls passed; these are not compiler case executions |
| Genuine sibling privacy probes | Eight producer probes freshly pass; 18 independent probes and the unchanged 22-probe ownership gate explicitly rebound from v3 |
| Python regressions | 182 tests ran; two additional receipt-dependent classes skipped |
| Repository verifier | 122 sources checked; 68 runnable cases |
| Scalar literal and arithmetic oracles | Passed against the exact v4 binaries |
| Real LLVM/native smoke | Four source-free ELF executions: scalar helper/loop and owned Batch in both profiles |

The public compatibility gate compares 1,120 streams, 504 JSON records, 324
labels and 140 repeat/profile groups. It uses the full gate, without
`--original-only`. Three setup-only zero-case attempts are preserved separately
and contribute no passing comparisons.

The 93 authored oracle cases, 230 source files and 33 oracle-model checks are
pre-candidate authoring evidence. The 3,603 executed cases include those cases
and the independently authored supplementary suites; authoring/model counts are
not additional candidate executions. Observation records come from the real
parser, loader, shared index and scalar/owned checkers. The observer receives
source requests only; a separate process owns expectations and comparisons.

Resource controls exercise every index reserve failure and all five minimal
import parser positions; exact/one-below limits, checked overflow and admission
precedence; all paired-import budgets 0 through 46; 19,531 production merge
sequences; long names/paths; malformed associations and 8,000 concurrent unique
source generations. The finite query proof is also checked against actual
operation/origin traces. Lowered seams demonstrate concrete branches without
claiming those maxima are reachable under default source caps.

Independent review initially found reference-exposure diagnostics covering
`&T` or `&mut T` rather than their nominal referents. V4 corrects that origin
without another namespace pass. The two-file v3-to-v4 delta and all earlier
results remain preserved. Relevant sealed/source-owner/resource and ownership
carrier definitions are byte-identical, supporting the explicit privacy rebind.

The initial v3 supplementary corpus also contained 64 malformed source fixtures
declaring both `mod a` and `mod A`. The established case-fold loader policy
rejected those sources before index collection. Fresh reviewed replacements use
a B alias while preserving the independent index expectations. The original
fixtures and their actual rejected observations remain retained separately from
the two genuine reference-exposure implementation defects.

OS syscall tracing was unavailable before any traced case executed, and no
tracing retry occurred. The 22 read checks instead observe a reviewed `cfg(test)`
callback at the real `read_source` entry, retaining lossless host path bytes.
They establish logical source-read entry and ordering, not lower-level file
syscalls. File outcomes and path admission have their separate loader tests.

The ordinary Cargo tests include linked native C/C++ helpers; LLVM text checks
and mock-toolchain tests are not counted as source-native executions. The four
additional real smoke runs compile the existing scalar helper/loop and owned
Batch examples, obtain reference outputs `27\n` and `816\n`, remove copied
source files, then execute the ELF files. The owned pilot exercises construction,
moves, shared/exclusive calls, forwarded borrowing and loops. These runs preserve
existing single-file behavior; they establish no new multi-file native scope.

The two skipped Python classes require actual collected native receipts; their
18 receipt-dependent controls remain in the existing hosted native gate. The
bounded smoke does not replace the preserved hosted native corpora.

## Reproduction

The portable [Unit2 package](../../tests/fixtures/typed_project_unit2_independent/README.md)
retains a bounded static export of all 3,603 final source/expectation cases and
unchanged independent resource probes. Its separate source-input manifest binds
113 compiler/build inputs from the qualified v4 freeze. The package preserves
3,539 explicit creation orders and the 64 replacements’ source_files-order
fallback; it does not claim an earlier unrecorded order for
those replacements. Source requests alone are copied into the observer input
tree; expectations remain in the comparison
process. Exact test names, unique case IDs, both profiles and terminal success
are required. Zero/missing/duplicate/stale receipts are rejected by protocol
controls. The full derivation is in the
[resource proof](typed-project-unit2-resource-proof.md).

With the repository's dependencies already available in Cargo's cache, run:

```
python3 -B tests/fixtures/typed_project_unit2_independent/run.py --repo . --output /tmp/new-unit2-results
```

The output directory must be new and outside the input repository/package. The
runner uses an isolated instrumented copy and target directory, at most two Cargo
jobs, and preserves source/package/input/toolchain/binary/output identities with
each profile's receipt. Use the qualified compiler toolchain on PATH.

The portable package was separately executed against the unchanged v4 compiler
inputs: package manifest SHA-256
`53ed2e4c9599fc2af965c97ecabf2ed5aded02d08f19793e7cf341ad2406e6c6`, input manifest
`bdeefc08c95824180ce2dadb9ddf79642e234907c3a1a15d3056773b4cdb8cb9`, and terminal
result `3a95c668999def6238750c518a4a5e40c0c9eada03c8f2a83af1622967e9d7aa`.
Both profiles passed 3,603 exact semantic cases and 21 named resource tests;
final source/package/queue/request-byte checks passed. The CI step prints final
identities and counts, and retains compact receipts/logs plus compressed raw and
normalized observations for independent replay. Build targets and generated
source trees are excluded. Hosted Rust 1.99.0 layouts/results remain separate
until that run completes. No feature-inventory, host expansion or M1/M2/v1.0
completion claim follows from this private unit.

## Hosted CI correction

The first published Unit2 head, `47f9e168a2b5eb1d5996681e266747fb607de0b2`,
exposed two qualification gaps. Rust 1.99.0 (`b940084d7`, compiler LLVM 23.1.1)
deprecates AtomicU64::fetch_update, causing the warnings-denied Clippy gate to
fail. Windows and macOS also ran 14 tests requiring the intentionally
Linux-qualified child-module loader and reached its existing E0005 rejection.
Those failing-head results remain historical evidence.

The correction uses a checked Relaxed compare_exchange_weak loop, preserving
successful old-value identity allocation, nonreuse and terminal exhaustion.
Only tests requiring child-file loading are Linux-gated. Four mixed tests were
split to keep their single-file checks portable, and ProjectCandidate has an
explicit non-Linux E0005 test alongside the existing ModuleCandidate test.
The production loader policy is unchanged.

Official Rust 1.99.0 locally reproduces the hosted compiler identity. Both
profiles pass 769 all-target tests with 21 ignored; warnings-denied Clippy,
formatting and the focused source/index/project tests pass. The protocol suite
passes 13 controls, and the Python suite runs 182 tests plus two skipped
receipt-dependent classes. Independent Rust 1.98.1 and 1.99.0 probes exercise
contention, exhaustion, allocation failures and retained-AST provenance;
all 45 measured layouts match. Actual replacement-head Windows/macOS execution
still requires the hosted matrix.

The corrected source-input manifest is
`074b419d2af8a2c44d834ba95363b9ca0f5ac059445806adf48bc2c1db1234ad`, and the
portable package manifest is
`4dae6a8de321b09d972d7dc345d19001332fe397ed05ccd3edaaf4f38a5db916`.
Only three compiler file rows and their containing manifest identities change.
The static corpus, all expectations and observation/comparison code are exact.
The earlier v4 and first portable qualification identities above remain
historical; this correction establishes its own source/toolchain results.

The unchanged private Unit1 runner passes 69 cases per profile, the full public
gate passes 35 cases and 560 exact comparisons, and the refreshed portable
package passes 3,603 semantic cases and 21 resource tests per profile. Its terminal
result SHA-256 is
`31c08dc1fc27baee05a2d0c31f3f6d3c1eb33a8194bdd188817d9dc0d844aa1a`.
Eight source/index seal probes, the repository verifier (122 sources/68 runnable
programs) and the scalar literal/arithmetic oracles also pass. These results are
new correction evidence; no additional old native corpus is counted.
