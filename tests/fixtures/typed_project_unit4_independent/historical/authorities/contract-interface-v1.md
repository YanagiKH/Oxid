# Unit4 pre-implementation contract interface

Status: source-derived expectations only. No compiler was executed to author
these expectations. No successful baseline or candidate qualification is claimed.

## Frozen inputs and observations

`parser-cases-v1.json` is a finite roster. Each row names one exact UTF-8 source
file, its SHA-256, a source display path, parser mode, and explicit node limit.
The default node limit is 100000. Lowered limits are labeled parser-only seams.
`original` means OwnedCandidate. `activated` means ProjectCandidate with the
accepted sticky recognition/recovery change. These names do not alter production
SourceMode or SyntaxFlavor identifiers.

Every expected diagnostic is ordered and complete: code, stage, message,
file_id, byte start/end, Unicode-scalar line/column and end line/column, empty
secondary origins, empty notes, human rendering, and JSON rendering. Recovery
rows identify the error's next synchronization token and the sticky bit at that
recovery. Explicit anchors are resolved while authoring; no parser or semantic
evaluator computes expected diagnostics. Each source gets a fresh SourceMap
with file_id 0; display paths are part of the literal rendering contract.

Compare failure diagnostics, source retention, and recovery, even when parsing
does not produce an AST. Failure has no successful flavor or executable witness.
Compare successful token tapes, AST projections/local IDs and node counts across
the two parsers for old successes. Omit only allocation addresses/source-owner
generation numbers from equality; independently require genuine source ownership.
Lex exactly once per mode observation; count EOF and trivia as existing lexer
semantics require. Do not claim that parser-only controls read child files.

Optional test-only instrumentation may record: each grammar-entry event,
false-to-true bit transition, error cursor, bit at recovery, next synchronizer,
final cursor, and added public-field lookahead token inspections. It must be
separately hashed and passivity checked. There is no new public debug option.
The first transition anchor is literal in the expectation. Later grammar-entry
events are permitted but cannot reset the bit. Count a terminal EOF lookahead
against its initiating Pub. Across disjoint field prefixes the additional
inspection count must be <= the input token count; it adds no syntax nodes.

`historical_unrepaired` is a source-derived negative-control prediction for the
published baseline's private ProjectCandidate parser. It is separate from the
desired activated expectation and any later actual baseline observations.

## Public and semantic observation paths

`public-cli-contract-v1.json` is separate from parser controls. Invoke the actual
public executable with its exact argv and exact entry spelling; no private
adapter may select source flavor or inject a checked result. Bind executable,
profile, platform, toolchain, source map and actual stdout/stderr/status to a new
outer receipt. An adapter trace can establish one load/lex/parse per loaded file,
one complete route/index/check and zero consumer-time source reopen; metadata
probes are not source reads and source lifecycle counts are not syscall counts.

Reuse accepted Unit2/Unit3 corpus bytes, expectations and model results by their
existing identities. Materialize their recorded source map, invoke public check
and run, compare public projections to the frozen expectations, and bind the new
actual observation path. Keep private raw/event/exact-fuel comparisons for facts
that public CLI does not expose. Do not edit historical receipts, rewrite
expected bytes, add a duplicate semantic model, or call a private execution
receipt public. Native qualification needs actual source-free copied ELF runs
on Linux x86_64, LLVM 19.1.7/O0 in both compiler profiles.

## Observation/result protocol

A future observation record names contract_sha256, case_id, input_sha256,
source_mode, lowered_node_limit, compiler_head/tree/input_manifest,
observer_sha256, profile, platform and toolchain. It then records actual tokens,
diagnostics, rendered streams, parser success, AST projection, nodes,
source_owner_valid, grammar/recovery events and termination. Comparisons use
only pre-frozen contract expectations. Unknown fields, missing observations,
extra diagnostics, model exhaustion, unexpected case skips and changed source
bytes fail or remain explicitly incomplete; they do not become new expectations.

The source-derived parser roster is 74 cases: 8 old successes, 14 old failures,
6 malformed-prefix pairs, 18 new-grammar migrations, 12 EOF/resource controls,
4 diagnostic-cap controls and 12 lexical variants of four selected controls.
Freeze identities before candidate execution and record any later correction as
a reviewed versioned contract change with reason and affected case IDs.
