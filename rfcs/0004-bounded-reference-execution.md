# RFC 0004: bounded bool/unit reference execution

Status: restricted experimental implementation for review. This extends the
[RFC 0003](0003-boolean-branch-cfg.md) preview with one explicit production
consumer of verified OIR. It does not complete M2/M3, establish a stable edition
or ABI, or add numeric semantics, ownership, host I/O or native compilation.

## Public operation and entry

`oxid run input.ox --edition typed-preview` requires exactly one source path.
The existing global-option placement, separator and script-payload rules remain.
Direct-file invocation, program arguments, project/artifact commands and backend,
profile, target, entry-selection or resource-tuning options remain unavailable.
No legacy preprocessor, interpreter, module/package loader, dynamic Value,
artifact loader/writer, bridge or host builtin is reachable from this operation.

Compile and verify the complete file before validating the entry or executing.
The compiler identifies the declaration named `main` in source and carries its
resolved DefId with the verified program to the entry adapter. That adapter
consults the verified signature, never guesses a name from raw IR origin text,
and requires zero parameters and a bool/unit result. The closed type enum already
limits verified results to these two types. An absent entry or main with
parameters is E0600; a compiler-carried ID inconsistent with the verified table
is E0500. `check` continues to accept files with no main or with main parameters
and never executes even a diverging recursive program.

Text success is exactly `true\n`, `false\n` or `()\n`, all exit 0. Failure emits
no result, diagnostics on stderr, and exit 1 (ordinary errors) or 2 (E0500).
JSON uses the existing schema-1 diagnostic envelope and a single terminal
`run-summary` containing `schema_version: 1`, `edition: "typed-preview"`,
`success`, `errors`, and `result`: `{ "type": "bool", "value": true|false }`,
`{ "type": "unit" }`, or null on failure. It never emits a check success first.
Check's terminal record remains byte-compatible. Errors during global-option
validation retain check-summary; after valid options explicitly select typed
run, source-operand/command-option and all later failures use run-summary.
Operation metadata, not diagnostic wording, determines this boundary.

## Machine semantics

Each activation owns its function ID, current block, next statement index,
private optional scalar slots and return continuation. Bool and unit are closed
scalar values; an uninitialized read is an invariant failure. Assign evaluates
before initializing its destination. Branch follows exactly one bool-selected
successor; Goto follows its explicit ID, regardless of table index ordering.

Call arguments have already been evaluated left-to-right by lowering. Call
copies their recorded scalar operands into a fresh activation and suspends the
caller. Only a normal return initializes the caller's destination and resumes
its explicit continuation at statement zero. Return reads its operand before
popping its activation. Root return yields the result. Discarded calls still
execute. There is no eager unchosen arm, folding, memoization or tail-call
elimination. Direct and mutual recursion are permitted and may exhaust limits.

An explicit heap-backed activation vector implements calls; the Rust stack does
not recurse with the source call graph. OIR and the stack are not cloned on call.
Verified programs remain immutable and repeated invocations start with fresh
state. Private invocation validates entry identity and supplied argument
arity/types, even though the production adapter passes zero arguments.

## Resource accounting and diagnostics

Production ceilings are 1,000,000 fuel units, 1,024 live frames (including main),
and 200,000 live scalar slots (sum of every active function's entire local-table
length, including statically unused slots). No unlimited sentinel or CLI override
exists. Test-only smaller limits cannot raise these absolute ceilings.

Charge before doing work:

| Operation | Fuel |
| --- | --- |
| Root activation | 1 + main's local-table length |
| Assign, Branch, Goto, Return | 1 each |
| Call | 1 + argument count + callee local-table length |

Costs and counters use checked arithmetic. Failure order is checked cost/fuel,
frame cap, live-slot cap, then allocation/action. A failed preflight never pushes
a partial frame or initializes a caller destination. Returning releases the
callee's full slot count. Header capacity is separately bounded by the frame
cap. Argument copies use at most 256 scalar entries and do not accumulate on
suspended callers. Abstract execution is amortized by charged fuel plus compiled
program setup, not fuel times the largest activation. Scalar destruction does
not recursively traverse user structures.

| Code | Stage | Meaning and origin |
| --- | --- | --- |
| E0600 | oir-run | Missing main (no location) or invalid main (function-name span) |
| E0601 | oir-run | Insufficient fuel before the next operation |
| E0602 | oir-run | Next activation would exceed live frame limit |
| E0603 | oir-run | Next activation would exceed live slot limit |
| E0500 | oir-run | Detected invariant/counter failure; invalid origins filtered |

Resource errors identify the next unperformed assignment/terminator, or the
main declaration for root allocation. Compilation keeps its existing stages and
errors. Fuel is deterministic preview reference accounting, not seconds, source
complexity, a termination proof or a stable profiling ABI. Host allocation,
source I/O, compiler work and output are outside this execution budget; the
runner is not an OS sandbox. No arbitrary panic-catching is introduced.

## Acceptance and deferred boundaries

Tests must exercise actual source lowering and raw verified programs; verifier
acceptance by itself does not establish source equivalence. A test-only independent
structured-source evaluator provides values and call/branch/return traces for
all 16 two-input truth tables, lexical bindings, groups, unit, discarded calls
and mixed-return paths. The expected interpreter does not use OIR or the legacy
VM. Observation hooks exist only in unit-test builds. Diverging selected/unchosen
paths, left-argument failure origins and independently counted fuel cases test
ordering that pure return-value equality cannot expose.

Exact small costs, zero/one-short budgets, live storage reclamation, frame/slot
boundaries, recursion, many arguments/locals, malformed raw IR and safe invariant
diagnostics supplement public CLI/error compatibility and all legacy regressions.
The raw constant/return case costs 4. A source `if id(true)` choosing `return
false` costs 15 under the documented lowering, and budget 14 fails at that return.

No source grammar or raw OIR shape changes. Numeric literals need a separate
exact-integer contract; ownership, native backend/ABI, typed artifacts and other
roadmap milestones remain separate work. See the
[validation report](../docs/architecture/reference-execution-validation.md).
