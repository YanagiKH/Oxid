# Public typed-project dispatch: Unit4 validation

The production `typed-preview` commands select a single immutable source load,
then preserve the original single-file schedule or check one complete typed
project. Public modules, direct imports, visibility and absolute item paths use
the existing shared index and scalar/owned verified consumers. The language and
filesystem boundaries are specified in [RFC 0015](../../rfcs/0015-bounded-typed-projects.md)
and [typed preview](../../spec/typed-preview.md).

## Exact source and qualification inputs

The activation patch is based on main
`be7695a8055d7181fbed0ad13fd90829bbdde0ba`, tree
`22849bbf26bf30811a582dbc8d027e936258ceb3`. Its complete nine-file patch is
28,881 bytes, SHA-256
`04f0588360aac12b96cd69a34b282329ea696eb69d7b979c8ffc385b7a42aab8`.
The first published source tree is
`fe9b9ba73ba28cb2b180cd10854eec82912cfe94`. Later qualification-only checkpoints
preserve those compiler bytes.

Two newly authored source contracts were independently reviewed before candidate
observations. They use distinct identities from historical Unit4 contracts:

| Contract | Frozen identity | Source-only scope |
| --- | --- | --- |
| Public v3 | `12b40d321014719805f4864f297ba3598a3c4c5ec6f416de99a0218de68f2a85` | 31 retained originals, 15 activation literals, 26 inherited/route cases, 15 held-out lifecycle cases and output controls |
| Parser package v1 | `7a2ec4fdf43bf94f3348a04251a9c04b77610b0e6bdd7318ce62dc2c5a5c4027` | 248 cases; audit of 533 coordinate-consistency checks, 14 token ledgers, six field scans and seven predecessor vectors |

The original 3,603 Unit2 semantic expectations and 152 Unit3 composition rows
remain unchanged. Public projection comparisons, internal raw/event/fuel tests,
logical lifecycle instrumentation and actual native execution are different
scopes. Source contracts or prepared queues alone do not count as execution.

## Fresh ordinary validation

Official Rust 1.99.0, commit
`b940084d7eb6a299eb4bfeb8e34901bc051e7ac4`, built the exact activation source.
Format and all-target/all-feature strict Clippy pass. Ordinary Cargo tests pass
789 tests per profile, zero failures, with 21 ignored qualification tests in
each profile. Those ignored gates were not executed by ordinary Cargo tests.
The fresh ordinary debug executable is
`20f2a1fc2ee0c6decba452195ffedb3e77260438e45daed43bf0cbb699bf7531`;
release is
`420d2bd1fbe7f042e2270266756f6dbd7acd9a79ebb9568f59d9f8c4abe937de`.

Initial PR26 head `d1fecf4403165002de3f310f4dbecd14b8621190` completed two
hosted workflows. Each passed six jobs and failed two historical-source binding
checks before their downstream semantic gates. Actual ordinary counts per run:
Linux 789 passed/21 ignored, macOS x86_64 and arm64 756 passed/6 ignored each,
Windows x86_64 754 passed/6 ignored. Every runtime host executed 38 source files.
The failures were explicit changed-length checks for `src/frontend/driver.rs`
in the old Unit2 input manifest and Unit3 portable bridge. They are retained as
failed attempts; a future current-source manifest must not silently retarget
historical evidence.

## Qualification boundaries

Current-source Unit1 compatibility and Unit2 tests must bind the current compiler.
Four newly accepted syntax forms intentionally migrate from old blanket parse
rejections; the other 31 original public cases retain exact streams. Archived
Unit3 checks must reconstruct and verify their original selected source bytes
before execution and label the result as archived-source evidence. New public,
parser and lifecycle gates establish activation behavior separately.

Complete source checks precede entry/native admission. New policy diagnostics
retain exact code, stage and source origins; OS-cause prose is bounded where the
contract leaves it platform dependent. Logical source-open/read counters are
observations of compiler stages, not syscall tracing. A mode-0000 unreadable-file
fixture cannot qualify that obligation when process privileges still permit a
read; it must report unavailable capability explicitly.

Declared-child filesystem loading gates on Linux, with measured predecessor
filesystem evidence on x86_64. A module-free root has no discovery host gate.
Native remains Linux x86_64, LLVM 19.1.7 and O0. Successful LLVM emission or
compiler exit is not source-free ELF execution. Copied executable runs, exact
input/build identities, full tuple inventories and negative comparison controls
are required separately. No broader platform, stable ABI, full-memory-safety,
M1/M2 completion or v1.0 claim follows.

## Completed local public and lifecycle qualification

The reviewed public adapter executes the amended source contract without changing
its scalar/ownership expectations. The initial combined attempt is retained:
public and lifecycle each had six mismatching source coordinates, while 4,408
predecessor rows failed a source-inventory ordering check before semantic
comparison. A canonical path sort preserves all source bytes. The independently
source-derived [public location amendment](../../tests/fixtures/typed_project_unit4_public_location_amendment_v1/artifacts/amendment.json)
changes exactly four location integers for the `pub` token in one source case.
The original contract and failed result remain separate artifacts.

The fresh amended local run passes all 8,690 section observations:

| Section | Observations |
| --- | ---: |
| Public activation | 288 |
| Original compatibility | 248 |
| Predecessor projections | 7,814 |
| Lifecycle observer | 318 |
| Output guards | 22 |

Including ordinary observer counterparts, this run executes 9,030 compiler
processes and 102 source-free ELF runs: 34 public, 34 lifecycle-observer and 34
ordinary-counterpart runs. All 88 negative comparison controls reject. The local
non-root host demonstrates the twelve required permission-denial tuples; it does
not qualify the hosted root-controller branch. The complete raw comparison was
independently replayed, with zero remaining capability gaps. Result SHA-256 is
`44c2ce66674f443fd0bc5e5e7c65c92fa6d2c432e35bf8002d38b2a9e1a8f8f9`;
the independent replay report is
`4ed796d94be98f4ec05a73c482702d458b4f8ab65b5869109403d1b277d12c7a`.
The retained inventory contains 66,394 regular files and ten intentional guard
symlinks; its independent audit hashes files without following those symlinks.

The predecessor projections are 7,114 complete-check, 560 complete-check/run,
62 first-diagnostic, 48 first-source-failure check/run, 24 frontend-prefix and six
schema/source-only comparisons. They do not establish equivalence of the private
raw OIR, event or fuel schedules.

The registered three-file opaque Batch pilot also has a separate fresh result:
each ordinary profile checks all eleven functions, runs to 816 and compiles with
LLVM 19.1.7; two copied ELF executables return 816 while all three source paths
are unavailable. Its six public CLI invocations and two ELF runs are recorded by
result SHA-256
`abed9f2101be815da00e360399fcdb2099398ed2e22af4bd51b41813ef4dba85`.

## Completed local parser qualification

The first 638-observation parser comparison retains six mismatching rows. Two
cases required a first-admission predicate to select the relevant prefix while
preserving legal recovery events; one source case had four off-by-one location
integers. The initial 533-location audit checked coordinate consistency, not
independent intended-token selection. The separate
[parser amendment](../../tests/fixtures/typed_project_unit4_parser_location_amendment_v1/artifacts/amendment.json)
anchors all three affected root sources independently, changes only the four
incorrect integers, and preserves all original source and execution records.
The comparator's later admission/occurrence checks have independent negative
controls; no production parser change or semantic expectation relaxation was
needed.

The [portable parser package](../../tests/fixtures/typed_project_unit4_parser_portable/README.md)
binds current and historical selected source bytes separately, official Rust
1.99.0 inputs, measured host, Cargo recipe, observer patch, passivity and every
collection session. Its actual fresh local run on checkout
`03a3a93dd90074484cfc2d0aa976b1327d757c79` passes all thirteen stages: four fresh
observer/control builds, twelve ordinary passivity pairs and 638 observations
from 248 source cases (319 mode observations in each profile). The fresh
comparison SHA-256 is
`b3fde4b8ce24ae22dd2070f602ae8daeecb7d05af55fc5638fab20976e9e7305`.
An independent read-only replay produces the identical result and verifies
4,371 retained files and eight symlink targets unchanged. It does not recollect
compiler observations. Selected result receipts and their transport boundary are
published in the portable package; they are not a substitute for the full raw
streams and executable evidence.

## Hosted integration status and scope

The earlier exact `03a3a93d` push and PR workflows each pass eight jobs. Their
current-source Unit2 gate executes 3,603 semantic cases and 21 resource tests per
profile; the archived-source Unit3 gate separately passes 304 source, 300 native
and 220 translation-mutation comparisons. The existing native suites report
7,792 observations across seven suites. These are their own exact-head results;
they do not qualify subsequent checkpoint heads or the new activation gate.

The [hosted capability controller](../../tests/qualification/unit4_hosted_capability/README.md)
has independently reviewed source admission. Its exact root-hosted execution
still must retain the twelve unavailable-capability rows from the main run,
execute the corresponding six public and six lifecycle tuples under an existing
transient non-root identity, and join their raw records. Synthetic controls and
the local non-root run do not satisfy that requirement.

Final activation admission requires the integrated exact-head Linux root/native/
parser job, three measured non-Linux host jobs and their mandatory raw-evidence
join. The frozen execute rosters require 8,690 Linux section observations and
532 on each non-Linux host, totalling 10,286; excluded and unavailable rows do
not count as execution. Final hosted collection and joining remain pending at
this documentation checkpoint. All existing gates remain applicable. Historical
Unit4 attestations are provenance only and are never relabeled as fresh results.
