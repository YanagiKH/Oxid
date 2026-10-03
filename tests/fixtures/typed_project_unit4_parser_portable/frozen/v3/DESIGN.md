# Portable parser qualification admission

Status: v3 semantic-authority candidate, awaiting independent final review and
fresh candidate qualification. V1 and V2 remain immutable predecessors. V3
inherits the V2 source/recipe boundary while rebinding only the exact approved
comparator v8b and independently reviewed source-coordinate amendment. No
hosted or fresh candidate qualification is claimed.

V3 also contains two reviewer-requested nonsemantic recipe corrections. Relative
session arguments are converted to absolute paths for passivity receipt checks,
with explicit regular-file and symlink-ancestry rejection before session reads.
The initial absolute `rustc --version --verbose` probe now has a 15-second
timeout. A focused deterministic timeout control and a real source/synthetic
passivity fixture cover these changes; they do not launch a compiler build.

The stable trust root is the literal authority SHA in `portable.py`. It binds
the frozen parser contract, historical source commit, ordered original 283 and
derived 286 file identities, instrumentation before/after identities, the exact
ten-file v7 helper package, v8b comparator/independent decoder bytes and reviews,
Rust/Cargo binaries and runtime/sysroot libraries, dependency source/archive
identities, and the command/target/profile/environment recipe. All authority
paths are relative and unique. No CLI argument replaces the trust root.

The immutable provisional-v1 review and held-out control sources are preserved
under `reviews/provisional-v1/`. V2 fixes its three findings: all Git calls use
`/usr/bin/git` with an explicit minimal environment that ignores caller Git and
PATH overrides, then check the intended worktree's top-level directory; build
PATH exposes only two verified Rust executable links plus the explicit trusted
host `/usr/bin:/bin`; and the entire measured preparation/build/collection host
snapshot, including uname release/version and Python identity, must agree.
This snapshot requirement establishes within-session receipt consistency, not
semantic incompatibility between different Linux kernels.

The compiler selection contains 62 files: Cargo/rustc, Rust driver and LLVM
shared libraries, Linux rust-lld/ld.lld, and target standard/test `.rlib`,
`.rmeta` and `.so` files. It excludes component/install manifests, clippy/rustfmt
metadata, debugger pretty printers, wasm tools and unused sanitizer archives.
Selected extra/missing/changed entries fail. Selected symlinks are rejected;
official Rust component contents must be present as regular files.
The fresh Rust executable view is the sole deliberate symlink exception. It
contains exactly `cargo` and `rustc`, with absolute links to the selected regular
files in the verified original toolchain. Their targets and the actual Rust
sysroot are rechecked. Unselected toolchain `which`, `as` or other executables
never enter PATH; adding any member or substituting a link in the view fails.
The 64 dependency records bind the three locked crates' archives and unpacked
sources under Cargo's crates.io registry namespace. Cache roots are variable;
the registry namespace comes from the canonical registry URL. Only that closed
source/archive set is copied; index metadata is copied for offline resolution
and cannot override the pinned lockfile, archives or source identities.

`prepare --repo HISTORICAL_WORKTREE --checkout CURRENT_CHECKOUT --output FRESH`
verifies historical HEAD and every original Git-object/file pair, separately
checks that the current checkout's complete src/native/Cargo/build.rs input set
is byte-equivalent, then copies the ten helpers to a fresh directory. It invokes
the unchanged prepare helper and independently verifies the complete derived
tree. HEAD, tree and compiler equivalence are distinct facts. A shallow CI
checkout must fetch the exact historical commit before making its worktree.
Preparation also derives a hook-free control view through the unchanged
`prepare.py --control` recipe, binding a separate exact 286-member control map.
Those bytes match the previously reviewed control source. Both fresh view
manifests and preparation invocations are bound into the new session.

`build --session FRESH/session.json --profile debug|release --toolchain ROOT
--cargo-cache CACHE` creates a new profile output, an empty HOME, and a private
Cargo cache populated with verified dependency bytes. Only fixed PATH/locale,
HOME/CARGO_HOME and the recipe's bounded environment are passed to the helper.
All ambient Cargo/Rust wrapper, flag, linker and configuration settings are
excluded. Cargo configuration files in every source ancestor are rejected.
The unchanged helper writes its ordinary receipt. A separate portable envelope
records the actual helper invocation, exact environment, toolchain identity,
fresh session identity, process result and receipt, without rewriting it.
Add `--control` to build the separately bound hook-free control view into a
distinct fresh output directory. The role is checked in Cargo/body/source and
portable receipts; an instrumented receipt cannot stand in for a control build.

Each invocation measures the actual Python binary/version. Build invocations
also record native C/C++ drivers, compiler bodies, archive tool and linker;
CC/CXX/AR and the Rust native linker are explicit resolved paths. These host
tools are observed rather than preapproved as one portable binary identity.
System dynamic libraries, kernel and OS process execution remain trusted host
inputs. The package does not claim a hermetic native toolchain or remote
attestation. Tools must remain unchanged across each invocation and available
at their recorded paths for comparison.

`collect --session FRESH/session.json --profile debug|release --contract-dir DIR`
executes the unchanged v7 helper with the session as its authority checkpoint.
The collector independently measures host uname/Python width and checks Rust
process ABI. The adapter additionally measures the build/collection host and
requires consistency. Each profile's collection directory is new.

`passivity --session FRESH/session.json --profile debug|release` executes the
unchanged, bound passivity helper against that profile's fresh instrumented and
control builds. It compares `original`, `project`, and `malformed` prescribed
sources in ProjectCandidate and OwnedCandidate modes: exactly six ordinary
pairs per profile, twelve pairs across debug/release. The portable verifier
independently checks source bytes, command/binary/session bindings, raw nonce
witnesses, actual mode/ABI rosters, equality of every prescribed externally
visible field, successful node ledgers, and the complete evidence inventory.
The final comparison requires these fresh passivity results as well as both
complete contract collections. Historical local v4 passivity receipts are not
admitted, rewritten or called fresh hosted execution.

Prior source reviews support the unchanged instrumentation and observer bytes
and their intended observational scope. Fresh six-pair passivity establishes
only equality for those ordinary inputs and modes. It does not establish seam
equivalence, full-corpus equivalence, or absence of every possible instrumentation
effect. Those distinctions remain explicit in the report.

`compare --session FRESH/session.json --contract-dir DIR` validates both complete
profile envelopes, exact Cargo command/package/target/source/profile/freshness,
derived inputs, helper identities, binary paths/content and collection receipts.
It derives an executable comparator from exact pinned v8b bytes by replacing
only the initial admission prefix of `execution_manifest`; all source/token,
AST, event, diagnostic, 22 predicate, relational and raw receipt checks are
retained byte-for-byte. A transformation proof records source/replaced/retained
region hashes. Across profiles it also rejects reused execution nonces.

## Separate execution and effective-comparison identities

Collection and raw normalization retain the original decoded contract
`b19819e2e4af627dfe3877ef7753fe237aa7830b16d2a83197fae0ed02010cbc`
and package freeze
`7a2ec4fdf43bf94f3348a04251a9c04b77610b0e6bdd7318ce62dc2c5a5c4027`.
The unchanged helper does not relabel these observations or execute a rewritten
corpus. Its Rust, instrumentation, ten helper files and both derived source maps
are byte-identical to V2.

New comparisons call the exact unchanged v8b `admit_contract_amendment` loader
with four pinned artifacts: complete amendment checkpoint, descriptor,
amendment, and independent source review. The loader revalidates the entire
original base, makes a copy, permits exactly four integer coordinate leaves,
checks the original/new diagnostic and case identities, independently derives
the offending Pub token span from source, and verifies the full effective
canonical document hash. It rejects already amended or altered bases and
unreviewed/partial authority packages.

The actual effective document, not only its summary, is passed to unchanged
`compare_effective_rows`. Results report both original execution authority and
effective comparison identity `oxid-unit4-parser-v1-location-amendment-v1`,
whose canonical SHA256 is
`c2d4f8db28f7815b3e17ca13fec9a7da70f0923dd052656edb339bc0cd7740cd`.
The v8b first-admission predicate, rejected-admission/failed-reserve event
checks, all other semantic predicates and all raw/source/token/AST checks are
preserved exactly. No AST or event evidence is projected away.

The top-level session root owns all new evidence; old local rebinds and binaries
cannot be admitted. This is trusted-runner provenance, not cryptographic remote
attestation against an adversary with arbitrary control of the runner, tools or
evidence. Receipts must remain available at their measured execution paths.

## Closed controls

Source-only controls must include two unrelated fresh roots; exact original and
derived rosters; coherent altered-source plus rehashed manifest; missing, extra,
duplicate, traversal and symlink entries; substituted helper/Rust/patch bytes;
Cargo argv/name/kind/package/manifest/entrypoint/profile/target/body substitutions;
stale or reused output, executable and session receipt paths; forbidden configs
and inherited environment inputs; unsupported measured OS/architecture/width and
raw ABI mismatch; zero, missing, extra and duplicate executions; cross-profile
nonce reuse. Synthetic artifact bodies are explicitly synthetic and never count
as compiler evidence. Run the frozen normal and optimized mutation suite too.

Fresh complete debug/release builds and collection establish the 638-row gate
only after independent review. Maintainers manage publication and exact-head hosted CI.

## Remaining work at this provisional candidate

- Independently review the V2 recipe fixes and V3 exact semantic rebind,
  transformation and effective-authority admission
- Run fresh real builds and collection in both profiles, then the full effective
  contract and exact-head hosted gate; no such result is included here

Source controls use fresh real source materializations and explicitly synthetic
Cargo receipts under unrelated roots. V2 also adds integrated synthetic
session/collection and ordinary passivity controls through unmocked admission.
Synthetic bytes and receipts never count as candidate builds or observations.
Frozen v8b normal/optimized tests remain regression evidence for preserved code.

Before hosted activation, the outer CI job must enforce a job deadline and
available disk/evidence budget. Git and identity-probe subprocesses have bounded
timeouts; the inherited observer has its per-case timeout and evidence cap.
The outer build/helper process and offline index copy still rely on the job's
operational bounds. This package does not claim those operational limits are a
fully verified resource gate.
