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
| Parser package v1 | `7a2ec4fdf43bf94f3348a04251a9c04b77610b0e6bdd7318ce62dc2c5a5c4027` | 248 cases; independent audit of 533 source locations, 14 token ledgers, six field scans and seven predecessor vectors |

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

Historical Unit4 attestations and input hashes are provenance records only.
This ledger does not reuse historical observations as fresh qualification.
The current adapters, full combined runs and applicable exact-head hosted
qualification are recorded as they complete. At this checkpoint those gates
remain pending; the source-only contracts and ordinary checks above are the
completed evidence.
