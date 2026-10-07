# Current implementation and evidence boundaries

Source baseline: `7e681d25fa58828b817d66a8701a6913a7158932` (Oxid 0.9.0).
This records part of M0, the initial implementation inventory and specification work. M0 remains in progress.

## Production path

`src/cli.rs` includes the legacy runtime in `src/main.rs`; legacy lexer, parser
and AST now live in `src/legacy/syntax.rs`. The entry point first gates the
explicit [typed-preview compiler and runner](../../spec/typed-preview.md) in `src/frontend/`.
Default execution still uses the legacy interpreter, module loader and CLI. Runtime services are split into `artifact`,
`data`, `network`, `packages`, and `benchmark` modules.

Legacy source execution parses into the dynamic `Value` interpreter. Numbers are `f64`;
arrays and records have shared `Rc<RefCell<...>>` storage. There is no OIR/CFG,
ownership checker, or `.ox` native machine-code backend in this path. The independent opt-in bool/i32/unit pipeline now has typed HIR, verified cyclic
Branch/Goto OIR, explicit bool value merges, initialized typed scalar places and calls/returns. Check remains non-executing; explicit
typed run uses bounded iterative scalar reference execution. The experimental
source integration adds nominal scalar-field/empty owned structs and call-only borrowing
through a sealed ownership verifier and its bounded reference/LLVM consumers.
The production parser and driver select this experimental route; exact source,
compiler and qualification identities are recorded in
[owned-source validation](owned-source-validation.md). Native scope is Linux
x86_64, LLVM 19.1.7 and O0. The historical
[owned-consumer report](owned-consumers-validation.md) qualifies its raw OIR
fixtures; it does not establish source qualification. Default legacy records retain their
dynamic shared-storage behavior.

The public typed source route now accepts bounded declaration-only modules,
direct imports, visibility and absolute item paths. One `load_typed` call retains
immutable file-aware sources and AST-derived OriginalSingleFile/ProjectSyntax
flavor. Original syntax preserves its scalar/owned schedules; project syntax
uses one shared declaration/import/visibility index and one complete verified
program. All declared bodies are checked before entry or native admission.
The original root-main identity is retained; imports cannot supply an entry.
Declared-child loading admits Linux; root-only syntax has no discovery host gate.
Exact current, archived and platform qualification is recorded in the
[Unit4 ledger](typed-project-unit4-validation.md).

The complete bounded capability is specified in
[RFC 0015](../../rfcs/0015-bounded-typed-projects.md). The historical
[Unit1 ledger](typed-project-unit1-validation.md) and
[Unit2 ledger](typed-project-unit2-validation.md) distinguish each source
representation and qualification. Unit2 measures ten flat index vectors, a fresh
288N AST payload envelope and explicit I/J/W admissions; it does not inherit the
old Unit1 280N envelope. The public capability has one experimental feature-inventory entry. It adds no
native target or milestone completion claim. Historical
ownership and Unit1 evidence keep their original identities.

The [Unit3 ledger](typed-project-unit3-validation.md) separates the 304 finite
source/profile comparisons, 2,798 actual reference runs and native covering
subset with 99 source-free ELF executions. It also records two newly paid origin
walks, the null-origin malformed-source diagnostic change, retained compatibility
identities and the test-only host-selection successor. These counts do not imply
native execution of every composition case or qualification of all programs.

A narrower explicit [LLVM native preview](../../spec/native-preview.md) compiles bounded, nonrecursive scalar programs, including checked i32 `+`, `-`, `*`, `/`, `%`, explicit i32/bool comparisons short-circuit boolean logic, mutable scalar locals and guarded while loops, to Linux x86_64 ELF PIE. See
[RFC 0004](../../rfcs/0004-bounded-reference-execution.md) for exact execution limits
and [RFC 0005](../../rfcs/0005-exact-i32-literals.md) for exact i32 literals.
[RFC 0006](../../rfcs/0006-checked-i32-arithmetic.md) adds ordinary checked i32
addition, subtraction and multiplication, with profile-independent runtime overflow.
[RFC 0018](../../rfcs/0018-checked-i32-division.md) extends this with checked
division and remainder, truncation toward zero and explicit zero-divisor errors.
[RFC 0011](../../rfcs/0011-mutable-scalar-locals.md) adds initialized typed scalar
places and ordered statement assignment through both reference and native execution.
The linked C/C++ helpers accelerate selected host operations; their existence
does not mean Oxid source is compiled to native machine code.

`compile_file` calls `compile_program` and then Rust's `artifact::write`.
The resulting OXBC 1.0 container holds serialized AST version 1 with source
ranges. The runtime decodes and interprets it. The FNV checksum detects selected
accidental changes; it is not a cryptographic integrity or provenance proof.

## Demonstrations and declaration-only boundaries

`stdlib/frontend/bytecode.ox` emits a textual instruction representation.
`compiler/main.ox` checks a fixed representative instruction sequence. This text
is different from the binary OXBC writer used by `oxid compile`. It is an emitter
demonstration, not a compiler for arbitrary supported Oxid source.

`compiler/providers.toml` declares component ownership. `verify_provider_manifest`
checks for required substrings, and `oxid frontend` reports declarations. The
manifest does not dispatch production compiler components, prove the emitter
was used, or enforce a no-fallback execution mode.

`bootstrap_project` encodes a parsed compiler demonstration, decodes/re-encodes it
twice, compares bytes, and interprets the demonstration. This is an artifact
serialization round-trip, not a compiler rebuilding itself. The existing
`bootstrap`, `self-compile`, `emit`, and `self-host` commands are retained for
compatibility, as are the `stage0`, `stage1`, `stage2`, and manifest fields.
These legacy names must not be interpreted as C1/C2/C3 self-compilation evidence.

`stdlib/mega.ox` consists of generated identity functions. It is synthetic source
coverage, not evidence of standard-library API breadth. Its location and public
names remain unchanged in this baseline to avoid breaking existing imports.
Preview/demo programs under `tools/`, `examples/`, and `packages/` demonstrate
selected workflows; folder presence is not domain production certification.

## Other limits confirmed in code

- Source executes imports at their top-level position; artifacts hoist imported module statements before importer-local statements. Side-effect ordering can therefore differ. See [Modules](../MODULES.md#initialization-order).
- Tasks are lazy and memoized; `join_all` executes them sequentially.
- `net_read` returns text from received bytes. There is no general binary stream
  contract or incremental UTF-8 decoder in that API.
- The package resolver keys dependencies by name, supports paths/pinned Git,
  and uses FNV package-tree checksums. This is not multi-version registry or
  cryptographic supply-chain verification.
- Benchmarks measure Oxid startup/parser/packaging/interpreter operations.
  They do not run equivalent Rust workloads or demonstrate Rust parity.
- CI declares Linux x86_64, Windows x86_64, macOS x86_64, and macOS ARM64 jobs.
  Linux ARM64 is not in the configured matrix. Configuration alone is not a
  record of a successful run on any of these targets.

## Evidence and next boundaries

See [the machine-readable inventory](../feature-status.json),
[status definitions](feature-status.md), and
[local baseline results](baseline-validation.md).

The baseline records selected legacy behavior and its implementation limits.
Full grammar/scope, static semantics and safety, threat model, compatibility and
performance corpora, broader native compilation, provider dispatch, true self-hosting,
and native AI training remain open work. The future static-core design must use
its own specification rather than treating the legacy interpreter as its oracle.

Ordinary bool-condition while is an experimental end-to-end extension with cyclic
OIR dominance and shared one-million-operation reference/native fuel. Guarded
native modules retain static call-depth/storage bounds and add explicit 16 MiB
human-diagnostic/64 MiB LLVM-text ceilings. Existing acyclic native admission is
preserved; no native recursion or final-performance claim follows.
See [RFC 0012](../../rfcs/0012-while-runtime-fuel.md) and
[while validation](while-validation.md).

Unlabeled break/continue extend the while subset end to end. Four-outcome typed
flow summaries separate return from loop transfers; lexical targets lower to
existing charged Goto edges without unreachable joins. No ownership or new type
is implied. See [RFC 0013](../../rfcs/0013-loop-control.md) and
[loop-control evidence](loop-control-validation.md).

The owned-type declaration facade supplies nominal record/field IDs and checked
fixed scalar layouts to the private ownership/loan verifier. It checks whole-owner
availability, explicit argument preparation and exact loan/call regions before
constructing the sealed immutable witness used by both consumers.

Source parsing, resolution, typing and bounded lowering feed that
verifier. One module-wide selection sends every function through the owned route
if any owned syntax occurs, including unused declarations and skipped paths.
Scalar-only modules keep their established OIR, costs, diagnostics and native
admission. No failure falls back to scalar, legacy or reference execution.
Newly recognized but unsupported ownership forms can move to a more precise
stage: for example, an `&bool` parameter changes from E0101 at `&` to E0202 at
the scalar referent. This does not admit scalar borrowing or change an accepted
scalar-only program.

The public typed commands select the source route; the
[source qualification ledger](owned-source-validation.md) records the exact
activation identity, CLI/native results and remaining qualification limits.
The source contract includes move-only scalar-field and empty nominal structs,
whole replacement, scalar field mutation, owned helper returns, exact-mode
call-only borrows and explicit nested reborrows. Run/native entry still requires
a scalar zero-argument main. Stored/returned references, partial moves,
field-disjoint borrowing, nested owned fields, heap/destructors, unsafe/FFI
ownership and a complete static-memory model remain open. Native remains Linux
x86_64 with LLVM 19.1.7 at O0, under its inventory and explicit-byte restrictions.
This is one experimental ownership-foundations capability, with no M2 or v1.0
completion claim. See [RFC 0014](../../rfcs/0014-owned-structs-call-borrows.md),
[the typed contract](../../spec/typed-preview.md),
[the native contract](../../spec/native-preview.md),
[groundwork validation](owned-types-validation.md),
[raw verifier validation](owned-verifier-validation.md),
[historical raw-consumer validation](owned-consumers-validation.md), and
[source qualification](owned-source-validation.md).

The accepted [native admission successor](../../rfcs/0027-native-admission-inventories.md)
replaces only native aggregate/live reference-expanded-cell guards with independent
whole-program inventories `I = S + A + O + R + L + C` and owner width W, each at
most 8,192. Aggregate/live explicit storage remains capped at 1 MiB, including
canonical input/output scratch and the conditional wrapper fuel cell. This
intentionally broadens admission; it does not optimize allocation or change
reference storage, logical fuel, LLVM, ABI or other caps.

[Local successor evidence](../../rfcs/0027-native-admission-inventories.md#local-evidence-and-remaining-gates)
records activation `189cbed`, tested compiler source `ffa2e00`, unchanged LLVM
controls and an ordinary source case formerly rejected by the expanded-cell
gate. The unchanged partial parser at `8f1fe20` now compiles through normal
native CLI gates, with 54 matching reference/native corpus cases and 41 separate
malformed strict-decoder controls. Full grammar and capability/hosted
qualification remain unfinished.
Historical ledgers and oracles retain their recorded identities. Source-free
execution here means a working directory containing only the ELF and a controlled
environment, not filesystem isolation or an OS sandbox.


Fixed scalar arrays are now an experimental public typed-preview capability.
Explicit check/run/native compile accepts move-only `[bool; N]`, `[i32; N]` and
`[(); N]` for N=0..1024, named-base checked signed indexing, length and whole-array
call-only borrows. Original files and bounded linked projects use the same
checked source/ownership pipeline. Runtime writes snapshot the complete RHS
before evaluating the index; fuel is charged before bounds. Default and explicit
legacy dynamic arrays are unchanged. See the
[current array contract](../../spec/typed-preview.md#fixed-scalar-arrays) and
[public-route validation](fixed-array-public-validation.md) for new direct CLI
and source-free native evidence. The [three-module sample](../../fixtures/typed-array-samples/README.md)
returns 5325, with the test checking its complete final sequence.

The [reference ledger](fixed-array-unit2c-validation.md),
[native ledger](fixed-array-unit2d-validation.md),
[frontend ledger](fixed-array-unit3a-validation.md) and
[typing ledger](fixed-array-unit3b1-validation.md) retain their historical gated
source identities and results; they are not current public-route receipts.
Private observation and ArrayConsumer admissions still cannot be promoted into
public SourceProgram authority. Declared-child loading remains Linux-only, and
native remains Linux x86_64 with LLVM 19.1.7 at O0 under stricter whole-program
resource limits. Source-legal large arrays are not guaranteed native admission.
Exact-head hosted CI remains separate. Nested arrays, owned unsized values, heap collections,
element references and a v1.0 stability claim are outside this capability.

The experimental [call-only borrowed scalar slice extension](../../spec/typed-preview.md#call-only-borrowed-scalar-slices)
adds `&[T]` and `&mut [T]` parameters to explicit typed-preview check/run/native
compile. One helper can read or mutate complete existing fixed arrays of
different lengths, including zero, with exact bool/i32/unit elements. Explicit
`&*p` and `&mut *p` reborrows retain the backing owner's permissions and runtime
length. Indexed access and `len()` retain bounds, evaluation-order, fuel and
whole-owner conflict rules; no new collection allocation is introduced. The
[three-module slice sample](../../fixtures/typed-slice-samples/README.md) returns
515 using lengths 2, 3 and 0.

[RFC 0019](../../rfcs/0019-borrowed-scalar-slices.md) bounds this extension to
whole-array call views. Ranges, subslices, owned unsized locals/fields/results,
element borrows and escaping references remain excluded. The existing source
and native admission caps, Linux-only declared-child loading and Linux x86_64
LLVM/Clang/LLD 19.1.7 at O0 native qualification boundary are unchanged. The
[public slice controls](../../tests/typed_slices.rs) specify acceptance cases;
their presence does not establish current exact-head hosted CI. Older
source-bound validation ledgers qualify their recorded compiler identities,
not this successor. No stable ABI, self-hosting or completed milestone is claimed.

## Bounded process output and persistent artifacts

The implemented [stdout/process contract](../../rfcs/0025-bounded-stdout-process-entry.md)
adds explicit Process Run/Compile and whole-view byte output on Linux x86_64.
Default Result behavior and resource ceilings remain; named physical admission
successors are documented in the RFC. Process setup/reporting has its own host
boundary, including the current Windows silent-74 limitation.

The [OXS1 component](../../fixtures/typed-expression-samples/README.md) has a
producer, independent Oxid loader and independent external decoder. Local public
CLI/reference/native checks cover exact bytes/statuses, 39/63 from unchanged
executables, malformed framing, capacity, overflow and failed publication. The
[controller](../../scripts/verify_bounded_stack_artifact.py) retains inputs,
partial outputs and executable identities. Current-source rebinding and exact-head
hosted CI are still required; these local results are not a release or v1.0 claim.
