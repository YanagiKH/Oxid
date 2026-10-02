# Independent owned-source qualification corpus

This corpus is staged and dormant. The production source route is not admitted by
these model tests. It is a separate RFC0014 source model, not the qualified raw
consumer corpus. In particular, raw Batch fuel 1086 is not source Batch fuel.

## Frozen API

`Builder` builds tagged source nodes; `Program` contains nominal declarations and
functions. `render(Program)` returns original source plus half-open UTF-8 byte
origins. `NamesTypes.check()` performs all name resolution before complete body
typing. Static analysis saturates a finite graph whose vertices include lazy
expression and call-argument preparation control points. Dynamic interpretation
uses concrete call activations, owner generations and capability handles.

Each frozen case uses schema `oxid-owned-source-v1`, stable case ID and seed,
source SHA256 and original source file, origin map, expected stage/code/primary
span or scalar result, source event facts, a separate template charge schedule,
and independently specified source/raw correspondence facts. Compiler outputs
never determine model acceptance, names, permissions, values, or expected facts.

The eventual boundary adapter accepts immutable observations of declarations,
parameter positions/modes, scalar snapshot definitions and ordered source events.
It reports correspondence mismatches only. It cannot certify or execute raw IR.
Raw-valid changes to source mutability, same-shaped nominal identity, parameter
mode/position or snapshot timing must fail this comparison even when raw shape
and ownership verification succeeds. Malformed/E0500 mutants are a separate gate.

## Enumerable coverage ledger

Coverage is reported by semantic family, unique source program, native success,
frontend negative, compiler CLI invocation, LLVM module and process execution.
Counts are not multiplied by all equivalent argument permutations.

| Family | Bounded enumeration / required pairs |
| --- | --- |
| 1 records | Empty record; bool/unit/i32 mixed record's 3! written-field orders; nominal mismatch; duplicate/unknown/missing fields; source-order effects and overflow |
| 2 names | Forward item order; type/function/local homonyms; duplicate record/function/field; active shadow vs lexical reuse; function collision; inferred/unknown base |
| 3 exclusions | One precise source fixture for each excluded grammar form in RFC §2, plus nominal aggregate equality type error |
| 4 moves | Let/call/return/discard move and later use; empty move; self/relay replacement; moved self; one/both-arm restore; partial recovery |
| 5 ambiguity | Scalar/grouped conditions; literals in nested call and grouped comparison; unparenthesized condition literal exclusion |
| 6 flow | Local/result reuse; break/continue/return lexical-prefix cleanup; false entry; continue-carried move vs restored move; terminating moved arm; real/static-terminating loops |
| 7 argument aliases | For arities 1..4, each restricted-growth alias partition once × shared/exclusive mode vectors: 2+8+40+240=290 source programs, 74 accepted/216 rejected; separate snapshot/read/move-order pairs |
| 8 capabilities | Shared and exclusive parent × requested child modes; repeated shared children; compatible parent read vs write; exclusive child suspension; children released before parent |
| 9 nested/lazy | Outer prepared loan with nested calls and skipped/taken logical RHS; static rejected skipped RHS; earlier argument effect survives later failure |
| 10 stores | RHS move then unavailable field target; completed inner mutation then field store; nested constructors; whole replacement RHS first |
| 11 results | Immediate consume/assign/discard/return; two independent owned results; loop results; interleaved scalar/owned/ref positions; empty owners beside scalar storage |
| 12 raw controls | Retain existing raw negatives; later separate candidate-adapter E0500 operations/roles gate, not counted as source fixtures |
| 13 correspondence | Independent mutation pairs for binding mutability, nominal record/field identity, parameter mode/position and scalar snapshot timing |
| 14 diagnostics | First 8/9 missing names; UTF-8 exact/one-over component lengths; 100/101 diagnostics; source vs internal operation classification pairs |

The ledger is a coverage obligation, not a claim that all rows already execute.
Exact completed counts and any outstanding families appear in the generated
manifest. Corpus bounds are model bounds; they do not replace compiler resource
admission. Static fixed-point saturation has no path-depth cutoff.

## Cost specification frozen before raw comparison

Ownership templates and charge origins are exactly proposal §§4.2–4.4, 5.3.
Canonical scalar expression locals include literals, names, groups, fields,
unary/binary expressions, logical merge results, and call results. Name emits
Copy/Load; Group emits Copy. Scalar call result is initialized by Invoke with no
extra Assign. Immutable scalar let allocates a binding local and Copy; mutable let
allocates a place and Initialize; scalar assignment is a Store. A bare return
allocates a unit expression temporary and Unit Assign. Scalar discard adds no
operation. Owned Group adds no operation or owner.

If Branch/join use the full if span; falling-arm Goto uses its closing brace.
No else produces no extra false block/Goto; two terminating arms produce no join.
While preheader Goto/header/exit/condition Branch use the full while span; body
backedge uses its closing brace. Break/continue use their full statements. Lazy
Branch/RHS-ending Goto/BoolMerge use the full expression; RHS entry uses the RHS
span. FieldRead has one scalar result local and full expression charge origin.
Bare return's unit assignment and return use its full statement span. Lexical
ends precede outgoing Goto/return with the same brace/transfer origin.

These choices were agreed against the reviewed contract and baseline scalar
semantics before seeing any candidate lowering output.

## Staged implementation status

The currently enumerated corpus has 431 unique source programs. This is a model
count, not a CLI or native qualification count. It includes the exact source
Batch plus seven source variants; the eighth fuel-before-write variant uses the
same source with independently predicted private budgets rather than pretending
there is a public fuel option. Reference argument modes match exactly:
`read(&mut owner)` for an `&T` formal and `mutate(&owner)` for an `&mut T` formal
are E0300. This increment does not add an exclusive-to-shared argument coercion;
shared access from an exclusive reference uses explicit `&*p`.

Static generations use the finite quotient justified by lexical cleanup: a slot
is dead, available, or moved; ending it requires no outstanding capability rooted
there, so its next lifetime can reuse the abstract root without conflating any
live generations. The dynamic oracle records each concrete `(activation, site,
generation)` and every parent capability handle. Every complete state at every
source control point is saturated; exceeding 100,000 states returns an explicit
MODEL_INCOMPLETE error. Dynamic interpretation separately stops after 100,000
semantic events with MODEL_BOUND and never calls that a compiler rejection.

The alias family is arities one through four only. A zero-argument call is covered
by source helpers elsewhere, not included in the 290 alias total. Each family’s
observed maximum points, reached states, owners and calls is written to the model
manifest; those values are corpus sizes, not new compiler resource ceilings.

The independently predicted source Batch ledger is 506 charged operations and
1297 fuel, with 24 loan acquisitions/releases, 19 field writes and 27 function
activations. Unit tests check every lower budget and the absence of the selected
unpaid write from the model prefix. This remains a model prediction until the
source candidate and real guarded LLVM stores have been checked.

## Repository fixture dispatch boundary

The exact required RFC pilot is checked in at
[`fixtures/owned_source/batch.ox`](../../../fixtures/owned_source/batch.ox).
It lives outside `tests/` because the legacy `oxid test` command recursively
executes `.ox` files there. Repository verification dispatches this positive
fixture explicitly through `--edition=typed-preview`; the old legacy suite
retains its existing inputs.

The other original source strings are retained in `corpus-sources.json` and are
materialized only into a dedicated evidence directory by this harness. Every
production harness `check`, `run`, and `compile` invocation explicitly supplies
`--edition=typed-preview`. No generated source negative enters the legacy scan.

`corpus-manifest.json` binds source hashes, expected result/stage/code/exact span,
and separate correspondence, event and schedule hashes. Full frozen source,
origins/facts, dynamic events and charge schedules are reproducibly materialized
by `verify_owned_source.py --mode freeze`; the evidence manifest binds the exact
model bytes before any raw observations or compiler processes are admitted.

The source/operation classifier tests intentionally distinguish ordinary source
errors from E0500-producing internal failures. They only qualify the independent
classification matrix. Actual malformed raw mutants and raw-valid producer
mutants still require candidate integration; no matrix unit test is represented
as a real verifier, execution or native test.

## Independent-review corrections before compiler observation

The first frozen model remains preserved as historical review evidence. A fresh
revision closes held-out scalar/renderer and harness findings from RFCs0005,
0006,0009 and0010: i32 representability is E0203/resolve; general unary minus and
unsupported division/remainder remain parse failures; all comparisons share one
non-associative tier; type mismatches use the first invalid operand; and an
ungrouped tagged tree cannot silently render a different expression. Sixteen
additional scalar contract cases extend the original corpus; none was removed.

A fail-closed tagged-domain pass checks exact scalar payload types, ASCII and
reserved identifiers, recognized node/operator/type forms, complete unique item
order, condition-root literal restrictions, and trivia/newlines that cannot
inject or comment out code. Noncanonical expression trees require explicit Group
nodes, which participate in origins, types, slots and charges. The finite tagged
model is not a general source parser; out-of-domain input raises MODEL_DOMAIN or
another explicit ValueError rather than producing a compiler expectation. Valid
unresolved or duplicate identifiers remain ordinary modeled semantic errors.

Dynamic event or Python interpreter-depth exhaustion is MODEL_INCOMPLETE and
blocks freezing a complete runtime expectation. It is never labeled a compiler
runtime error. JSON, human reference output and ELF output must each independently
match the frozen expected result or expected failure code, exit and origin. Two
paths agreeing with each other cannot replace the independent expected outcome.
Only real matching check/run/compile summaries are accepted. POSIX worker tests
are explicitly skipped elsewhere; the descendant `/proc` proof is Linux-only.

Source/raw facts now also freeze every field-read base/nominal field identity,
every literal’s record identity and written field/value-snapshot order, and all
initialization/whole/field-store targets, RHS sources and origins, including unused
helpers. Generic changed-fact tests still do not establish actual raw-valid
producer mutation coverage; that requires the later real boundary inspector.

The final protocol guard additionally requires exactly one terminal command
summary, only preceding diagnostic records, matching typed-preview/schema/command,
consistent success/error/exit state, null failed payloads and exact JSON scalar
types. Python’s bool/int equality is not accepted as protocol equivalence.
Reference types in non-parameter tagged positions are explicitly outside the
model domain before name resolution; authored source negatives retain their exact
E0101 ampersand expectations. Valid reference parameter types retain a separate
referent-name origin for unknown-type E0202 diagnostics.

## Reviewed post-observation grammar and diagnostic amendment

The first pinned candidate corpus exposed two invalid standalone-block fixtures.
The published statement grammar admits if/while bodies, not standalone block
statements. Their exact original texts are retained in
`historical-standalone-blocks.json` and remain authored E0100 negatives; the
`active-shadow` and `lexical-reuse` semantic variants now use `if true` body scopes.
The tagged renderer rejects standalone block statements. The corpus grows to431;
no original source is discarded. The scoped reuse result remains2, with a separately
hand-checked main frame X24,20 charged operations and56 fuel.

Across independent owner roots the contract promises deterministic authoritative
verification, not a globally earliest source loan conflict. A separate source-only
inventory therefore defines all216 rejected alias cases uniformly:189 exact
singletons and27 two-root alternatives. Each tuple is the first conflicting
acquisition for that root, its earliest still-live acquisition cause and that
root’s declaration. Later already-invalid requests, later same-root causes,
cross-paired labels and unrelated origins are excluded. Exactly one diagnostic
must match a tuple with ordered related spans `[cause, declaration]`.

The matcher returns and persists the observed tuple, requiring unchanged
selection across check/run/compile and debug/release. Candidate receipts use the
same tuple rule and retain selections for repeat/profile comparison. This
allowance is confined to the direct-owner alias partition family; it does not
relax other diagnostic expectations or change verifier ordering. Original
pre-observation expectations and first mismatching receipts remain historical
evidence, with the specification basis and independent review of this amendment
recorded separately.

## Reproducible receipt adapters

The source model remains frozen at the independently reviewed v7 identity. The
receipt adapter in `scripts/verify_owned_source.py` now consumes actual candidate
observations with an explicit collection manifest:

```sh
python3 scripts/verify_owned_source.py --mode candidate \
  --frozen /absolute/path/to/frozen-source-directory \
  --candidate-observations /absolute/path/to/collected-receipts \
  --collection-manifest /absolute/path/to/collection.manifest.json \
  --evidence /absolute/path/to/comparison-output
python3 -m unittest discover -s scripts -p test_owned_source.py
```

The collection manifest must bind every original source and receipt digest, the
original expectation manifest, an exact compiler binary, its collector command
and successful terminal status. Candidate comparison checks source acceptance,
exact diagnostics, actual reference results, frame census, ordered charge costs
and spans, and independently specified raw declaration/operation correspondence.
It returns a command-level candidate PASS only. Ordinary CLI/native/resource
qualification remains separate.

`producer-translations.json` preserves nine actual sealed-valid test-only
producer mutations and their original observations from debug and release.
There are six unique source texts, and repeated receipt comparison does not add
unique program coverage. Expected original behavior is recomputed from separately
authored tagged source and exact token-aligned original spans. Mutants cover
binding mutability, nominal identity, diagnostic origin, original parameter
positions, reference mode, field identity, scalar rereading, nested argument
snapshot timing and borrow weakening. The last original is an E0311 rejection;
its exact frozen cause is required once, while the additional observed declaration
label is preserved without a claim that it was predicted before collection.
The stricter ordered complete-label rule remains confined to the reviewed alias
family.

`native-probe-sources.json` reproduces the independently frozen eight original
source probes and all 1,932 expected budget/store-prefix rows. The budget matcher
requires the complete ordered inventory, compares exact scalar types and failure
origins, and keeps missing actual store observations PENDING. A matching free-form
summary cannot satisfy the native store, wrapper identity, CFG guard, resource or
production CLI gates. The six extra guarded acyclic source programs supplement
the existing Batch and Batch-overflow cases; they do not replace the 431-case
source corpus.

The `reviewer/` files are unchanged portable independent controls and a handwritten
Batch ledger. Their manifest preserves their original bytes. These regression
methods repeat some main-suite checks and are reported as test methods, not
additional source/native executions or additional unique corpus programs.

For CI after production activation, use the explicit scoped CLI mode:

```sh
python3 scripts/verify_owned_source.py /absolute/debug/oxid /absolute/release/oxid \
  --mode cli --frozen /absolute/path/to/frozen-source-directory \
  --evidence /absolute/path/to/cli-evidence --jobs 2
```

`cli` exits zero only after the complete ordinary CLI/ELF comparison passes and
writes `status: CLI_ELF_PASS` with `full_qualification: false`. Historical
`production` mode retains its `PARTIAL` status and exit two after that same scoped
success. Both modes propagate worker failures, timeouts and diagnostic-selection
mismatches. Neither accepts caller-provided pass summaries to waive separate
native budget/store, resource or review gates.
