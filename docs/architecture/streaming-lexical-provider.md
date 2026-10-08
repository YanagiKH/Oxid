# Source-scale lexical component use

Status: experimental, explicit Linux x86_64 route. This is a bounded M1/M8
component-use increment, not independent compiler self-hosting.

## Authority and scope

`--experimental-lexical-provider BUNDLE` selects a real Oxid lexical process at
the retained SourceFile boundary for every root and declared child module.
Canonical Rust lexing is a separately traced comparison dependency. The checked
producer Vec, bound again to source identity/file/length, moves into the existing
Rust parser. Default routing, HIR v1/v2 observations, parser/resolver/ownership,
OIR verification and backend authority remain unchanged. No provider failure
falls back to canonical tokens, and failed compilation preserves existing output.

The five actual lexer modules total 10,470 bytes; their largest is 4,176 bytes.
The existing v2 parser/static source union contains 31 distinct ASCII modules,
110,289 bytes, largest 8, 530 bytes. This target includes modules, imports, owned
records, arrays/slices, borrows, enums/matches and Process I/O. It is not replaced
by reduced scalar fixtures.

## Reproduce the component-use gate

```sh
python3 scripts/qualify_streaming_lexer.py \
  --debug /path/to/debug/oxid --release /path/to/release/oxid \
  --llvm-bin /path/to/llvm-19/bin --output /new/component-evidence
python3 scripts/qualify_streaming_lexer_controls.py \
  --compiler /path/to/oxid --llvm-bin /path/to/llvm-19/bin \
  --bundle /new/component-evidence/seed/bundle --output /new/failure-evidence
```

The first command builds a source-bound seed lexer, then uses its actual token
provider to rebuild both v2 producer entry closures and the new lexer closure
with ordinary debug and release hosts. It verifies every loaded module receipt,
independently replays each exact LXI1 input/output, compares every token/span with
an observer built from unchanged canonical Rust sources, and replays genuine
OPA2/AST2/STF2 examples through check/run/native compile. Rebuilt lexers process
the complete actual source union. The second command checks corrupted, stale,
truncated and trailing observations, process failures, fuel refusal, output
preservation, changed modules and Process output-channel separation.

All output directories are fresh and retain commands, stream hashes, source and
executable identities. Hashes identify bytes; they are not signatures. A passing
summary is not an exact-head hosted CI result.

## Current evidence

The complete framed feasibility prototype passed all 36 genuine source modules
and 593 independent split/carry cases, including 49 exact lexical diagnostics.
A separate focused matrix passed 468 canonical comparisons, 340 diagnostic
observations, 93 malformed/domain/I/O refusals and four fragmented-pipe replays.
The largest requested module consumed 976,054 of the unchanged 1,000,000 fuel;
maximum native scalar-slot inventory was 237 of 256. These are exact-source local
measurements, not a new general source-size guarantee.

The integrated component-use recipe passes in ordinary debug and release hosts:
82 per-module provider invocations (14 parser, 22 static, 5 lexer in each profile),
all three executable rebuilds per profile, complete 36-source replay, and genuine
scalar/255-byte loop examples through OPA2/AST2/STF2 and native execution. The
independent split/limit corpus has 623 cases, including 69 exact diagnostics.
Public fail-closed controls retain 111 receipts per profile, 222 total.

Independent implementation review also checked 2,200 deterministic canonical
cases, including 1,169 exact diagnostics, and 40 malformed/domain refusals. Its
workspace-cleanup finding was corrected and independently rechecked. The reversible
source binding and full-current-source regression/hosted checks are separate
gates; their command receipts carry the exact source/compiler identities. These execution counts overlap the focused
matrices above and must not be added as unique cases.

## Resource and compatibility boundaries

The new loader phase separately accounts new provider vectors/carriers and the
coexisting canonical token capacity under the existing retained/scratch/work
ceilings. Canonical lexing keeps its historical infallible Vec growth; no claim
of process-wide OOM recovery or RSS accounting follows. Transport scratch drops
before ordinary semantic checking. The selected lexer still uses the ordinary
native 1M fuel and admission ceilings, and executes once per entire module.

Both 65, 536- and 65, 537-byte single-token native probes exhausted fuel before a
terminal. Exact token-width diagnostic completion at those inputs is therefore
unqualified; they fail closed. Small remaining-token budgets and unterminated
string/comment precedence are separately checked against genuine canonical
diagnostics. Unicode, general heap semantics, broader native targets, replacing
Rust parser/semantic/backend authority, C1/C2/C3 fixed points and clean Rust-free
reconstruction remain outside this increment.


### Platform setup and private-directory cleanup

Named loader accounting covers admitted provider carriers, retained workspace
paths and the explicit source/stream/token allocations. Standard-library
`temp_dir` lookup and path canonicalization can allocate before their resulting
path is checked; those platform-setup internals are outside this named model.
Neither whole-process OOM recovery nor an operating-system allocation/time bound
is claimed.

The owned private cwd is removed only when empty and still the same directory.
Cleanup is nonrecursive: executable-created contents leave that directory in
place rather than triggering an unbounded host traversal. A selected executable
is trusted local code, and may have filesystem capabilities outside that cwd;
process supervision is not a sandbox.


## Reversible source qualification

The lexical successor binds 345 selected inputs, 257 src/native members and 260
compiler/build bodies. Its separate producer closure pins the five Oxid source
files, their source manifest and the builder. The exact inverse restores the
340-input producer-diagnostic view before the unchanged historical chain; all 55
prior source-package bodies and the retained parser/public authority snapshots
are byte-identical. Local focused controls cover 213 source-binding, 107 current
parser, 27 public adapter and 112 CI-package tests, plus related privacy/import
controls. These are source/metadata admission results, not substitutes for actual
runtime or hosted-platform execution.
