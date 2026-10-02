# Independent Unit3 execution oracle package

Status: frozen source/model expectations prepared before observing or running
the Unit3 candidate. This package is not compiler acceptance, private-project
execution, public CLI, raw-verifier, native, resource, or witness-privacy evidence.

## Reproduce without a compiler

From this directory, run:

```
python3 -B generate.py
python3 -B mutation_plan.py
python3 -B selfcheck.py
```

The generator reads the corrected frozen Unit2 checkout and reviewed design
listed in provenance.json. It imports only the vendored independent Python
source model. It neither reads the Unit3 checkout nor invokes Rust, Oxid, LLVM,
an observer, or a remote service. The 461-file corrected Unit2 manifest has been
independently byte-verified; its local git HEAD is historical and is not used as
a claim about the working-tree publication identity.

requests.jsonl and sources/ are the complete source-only observer inputs.
expected.jsonl.gz must not be read by an observer or injected into production.
The request names each fixture, its real separate files, entry, operation,
profile, and permitted fuel reductions. Source hashes are transport integrity,
not a compiler provenance witness. mutation-requests.json is a separate seam
driver input; mutation-expectations.json carries its oracle, kept out of the
driver. Neither input contains expected results, bindings, route, ownership
states, or proof tokens.

## Finite source domain

152 source fixtures contain 445 source files:

| Family | Count |
| --- | ---: |
| Scalar call placement × spelling × root-main ordinal × result template | 24 |
| Owned call placement × spelling × record home × event template | 48 |
| Original Batch, mechanical Batch, opaque Batch, scalar split, Counter | 5 |
| Separate owned entry-ID1 control | 1 |
| Equivalent original scalar control | 1 |
| Paired negative controls | 18 |
| Source type/visibility/ownership negatives | 26 |
| Imported-main entry negatives, scalar and owned | 2 |
| Unused nominal route controls | 2 |
| Return/join cleanup control | 1 |
| Full scalar and owned span-variant controls | 2 |
| Used/unused mutually recursive scalar/owned native negatives | 4 |
| Four source encodings × four origin families | 16 |
| Equal-offset different-file native collision controls | 2 |

The exact names, bytes, counts and covering native subset are in coverage.json,
requests.jsonl and the manifest. The finite source-negative list is not an
unbounded combinatorial claim. Every mutator has an identified source control.
The mutation plan has exactly 114 requests; it enumerates span classes instead
of multiplying all raw mutants by all source fixtures.

The 24/48 families remain exactly the reviewed axes. A separate seven-case
native subset covers all three placements, both spellings, both scalar result
categories, both record homes and all four owned event templates. Required
project pilots/control and the eight overflow layout cases add their own
source-free native runs; collision cases add two more. Native admission-only
negative requests are counted separately from executed ELFs. No claim is made
that all 72 family cases are native-qualified.

## Source/module/body correspondence

Unique body-model labels are symbolic `(module, original declaration)` identities,
not claims that same-spelled originals are equal. The generator assigns dense
global function and record IDs independently in root/DFS file order, with
lexical order within each kind. Field IDs are `(record ID, lexical index)`.
Direct imports allocate no IDs. The original root main supplies the entry.

The independent flat-module model checks every actual call token and constructor
path against original or direct-import bindings; it separately checks function,
record, constructor-field and projected-field access. Source field privacy is
not inferred from a raw record witness. Exact pilot token bijections check every
declaration and every tagged origin. Only declared public modifiers, direct
alias replacements, source-qualified call/type spelling and whitespace may
change. No compiler consumes a concatenated substitute program.

The original/mechanical Batch bodies are identical, with an explicit original
DefId order `[retry,commit,dispatch,done,relay,finish,main]` to project order
`[main,retry,commit,done,relay,finish,dispatch]` bijection. All model origins are
relocated to checked `(file,start,end)` byte ranges. Their complete charged
operation schedules are equal under this relocation. Opaque Batch is a distinct
program with distinct calls/storage/fuel and does not use that equality claim.

Function-local model identities remain origin/role keys. A later normalizer may
join actual raw numeric IDs to actual declaration/site spans and roles; it must
not resolve source names, copy expected targets or synthesize missing events.

## Independently frozen outcomes

| Control | Route | Entry | Result | Model fuel |
| --- | --- | ---: | ---: | ---: |
| Scalar split | scalar | 1 | 10 | 165 |
| Original Batch | owned | 6 | 816 | 1297 |
| Mechanical Batch | owned | 0 | 816 | 1297 |
| Opaque Batch | owned | 0 | 816 | 1577 |
| Counter | owned | 0 | 5 | 126 |
| Owned entry control | owned | 1 | 5 | 126 |

The scalar scheduler independently applies the established scalar contract:
root costs `1 + locals + places`; a call costs `1 + arity + callee slots`; a
statement, merge, branch, goto and return cost one. It uses source-expression
and binding counts, not owned `X`. A separate hand ledger gives the scalar
pilot 165: root entry8 + argument1 + sum invocation22 + sum body115 + choose
arguments3 + choose invocation10 + choose body4 + root addition1 + return1.
Both ledgers are in selfcheck.model-only.json. The equivalent original scalar
control has the same complete charge sequence.

The inherited owned scheduler remains unchanged. Its frame counts and complete
dynamic event/charge schedules are stored per source case. Original and
mechanical Batch require 1297; opaque requires 1577, with final ReturnScalar
from1529 to1577. The separate entry control requires126, with final
ReturnScalar from117 to126. Exact fuel succeeds; one less denies the whole
next operation. Selected first/final writes, Invoke, normal returns,
StorageEnd, borrow acquisitions and nested Invoke boundaries carry exact
budgets, next denied operation/origin, paid-operation prefix and already
committed writes. Abrupt denial promises no unwind or rollback.

The route control deliberately demonstrates that adding an unused nominal
declaration changes a scalar program's route and fuel: 15 on scalar, 21 on
owned. Imports and file boundaries alone add no operation; changing routes
does not preserve fuel.

## Failure authority stays separate

1. Source resolution/type/ownership failures reject at their specified source
   stage, with exact full-file primary and ordered secondary byte origins
2. Source/map/parser association substitution fails internally with null
   origins until ownership of the selected map is established
3. The narrow post-lowering association audit rejects declaration mismatches,
   wrong-function-file spans, invalid UTF-8/ranges/files, visitation omissions,
   duplicates and checked count failures as E0500/oir-project-bind
4. Malformed raw calls, signatures, nominal fields, staging, return transfer,
   loan regions and duplicate/continue cleanup fail the existing raw verifier
5. A valid same-signature wrong target may pass both raw verification and source
   association. The source-to-call comparator rejects it. Independently safe
   reordered or omitted terminal cleanup can also pass raw verification but
   fails the exact source-cleanup correspondence contract

For each of 114 mutations the expected authority is explicitly frozen. In
particular a missing terminal StorageEnd is not automatically called malformed
raw IR: where no subsequent access/re-lifetime occurs, the generic raw contract
can accept it. The source contract still requires the cleanup. Diagnostic-origin
metadata does not confer ownership authority or change the generic denial kind.

## Observer/audit contract and remaining gates

raw_span_oracle.py defines a canonical full raw snapshot shape and independently
enumerates all stored span occurrences, including nested Scalar statements,
both BoolMerge inputs, call arguments and both DiagnosticOrigins fields. It
compares actual audit visits by structural-path multiset. Equal stored spans in
different fields count separately. Missing one and duplicating another cannot
cancel numerically. `D=F` for scalar, `D=F+R+K` for owned, and `Vbind=D+Sspan`.
Global dimension and root-entry equality are additional fixed checks, excluded
from that convention. Count and validation passes must each report their cost.

The exact Vbind computation is frozen as a function of an actual full raw
snapshot. Numeric per-fixture Vbind totals are not yet independently precomputed;
the source model does not reproduce all raw CFG/span-table expansion. A later
snapshot/visitor comparison must report this distinction and cannot use the
producer's aggregate count as the independent expected answer. Unknown raw enum
variants fail closed. The synthetic visitor self-test is package evidence only.

The package does not yet provide actual adapter/reference/native receipts,
full raw-site normalization, privacy/lifetime compile probes, aggregate resource
admission proofs or driver spy observations. Those remain required gates owned
by the implementation/review phase. The adapter contract may rename fields
mechanically after freeze, but semantic outcomes, alternatives, budgets, targets,
origins and mutation authorities must not be widened from observed output.
Any fixture defect must receive a separately reviewed correction with preserved
old/new identities and a source/spec reason before another candidate run.

The independent source model has finite state/event ceilings. Exhaustion is
MODEL_INCOMPLETE and package failure, never compiler rejection or acceptance.
All model/source-only evidence is explicitly distinct from private candidate
source, public one-file, actual native and rebound predecessor evidence.

