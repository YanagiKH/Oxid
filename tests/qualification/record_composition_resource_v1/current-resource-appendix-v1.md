# Owned record composition: current-resource appendix v1

## Scope and evidence boundary

This is a **new current-source resource amendment**, not a rewrite or retroactive
qualification of a historical record/array/slice ledger. Composition source tree:
`f6b7dee8bac4ebcc27ad020db9940344c5e4ae41`. Declaration-layout predecessor:
`f8e2d39d8081b992969597649bad7fd9f28e7d04` (published PR33 source base).
The separately qualified native-fix e4bc checkpoint remains an integration
predecessor; this appendix does not replace its protocol or evidence.

Host of measurements: x86_64-unknown-linux-gnu; rustc 1.99.0
(b940084d7, 2026-09-28). Rust private type layout is not a portable ABI.

Two distinct kinds of evidence are retained:

1. `derive-layout-probe.py` extracts only named type declarations from each exact
   Git tree, binds original file/declaration hashes, removes visibility, shortens
   `hir::Ty` to `Ty`, and compiles a standalone declaration-only probe. No parser,
   compiler admission algorithm, runtime, or candidate expected-output routine is
   imported. The selected declarations have no repr attributes; the extractor
   rejects such attributes. Derive attributes do not affect their representation.
   Fresh probes establish the declaration/layout facts below independently of
   candidate test expectations.
2. Eight existing layout/inventory tests were freshly run in each existing local
   executable, without Cargo compilation. Their stdout/stderr and binary hashes
   are retained. Those executions corroborate wider enclosing layouts, but do
   **not independently establish the executables' build source identity**. Before
   qualification, rerun the exact names in `existing-binary-receipt.json` through
   a source-bound successor build, in both qualified profiles. The larger producer
   test counts are not resource evidence.

## Measured representation changes

Type | Predecessor bytes | Composition bytes | Alignment
---|---:|---:|---:
checked RecordDecl | 64 | 72 | 8
checked FieldDecl | 56 | 64 | 8
source HIR Field | 72 | 80 | 8
source HIR Projection | 32 | 56 | 8
Option<source HIR Projection> | 32 | 56 | 8
raw FunctionCounts | 144 | 160 | 8

New independently measured shapes: FieldInitializer=40; (FieldId,
FieldInitializer)=56; ContainmentSummary=32; ScalarLeaf=16; ScalarLeaves=2096;
its fixed 65-entry stack=2080, with 32-byte Option<(ValueTy,usize,usize)> slots.
All of those types have alignment 8.

Independently measured unchanged shapes include FieldId=16, ValueTy=16,
ParameterTy=24, RawRecordDecl=56, RawFieldDecl=64, Declarations=80,
DeclarationUsage=32, Layout=16, Operand=32, (FieldId,Operand)=48,
source HIR Record=104 and source HIR AccessBase=16.

The fresh existing-executable comparison additionally reports:

Type | Predecessor bytes | Composition bytes
---|---:|---:
ResolvedOwnedProgram | 504 | 512
TypedOwnedProgram | 528 | 536
lowering Output / Option<Output> | 464 | 480
lowering Walk | 696 | 728
array-pipeline observer Output | 512 | 528
array-pipeline observer LiteralRequest | 160 | 176
array-pipeline observer CaptureWrapper | 520 | 536

Unchanged in that comparison: RawOwnedProgram=48, RawOwnedFunction=248,
OwnedBlock=328, OwnedStatement=208, ParameterBinding=16, LocalDecl=32,
PlaceDecl=32, OwnerDecl=56, ReferenceDecl=48, CallDecl=96, LoanDecl=72,
ArgumentSlot=16, Option<DiagnosticOrigins>=56, OwnerSites=48, CallSites=64,
Option<Site>=24, TypedBody=120, TypedOwnedFunction=40; lowering fixed stacks
16248/15016/2608; BindingLocation option=16 and EvaluatedValue option=40.
The AST/project/transport layout rows also match, including ast::Program=248,
ast::StructField=144, ast::TypeSyntax=64 and ast::Stmt=136. Full measured rows,
not just this summary, are retained in the JSON and raw transcripts.

## Independent declaration and graph arithmetic

Let R be record declarations and F total declared fields. Existing inclusive
ceilings are R<=4096, F<=65536 and at most1024 fields per record.

- Historical checked-table requested element payload: 64R+56F
- Current checked-table requested element payload: 72R+64F
- Delta: 8(R+F)
- At both count maxima: historical3,932,160; current4,489,216; delta557,056 bytes
- Both are below the unchanged8,388,608-byte declaration-table ceiling

These formulas count element storage, not Vec headers, allocator bookkeeping,
allocator rounding, or whole-process RSS. Declarations' inline facade is80 bytes.

A containment summary has two layout usize values, width and depth:32 bytes.
The graph additionally stores one state byte per record and a stack of two-usize
pairs, limited to min(R,64). Simultaneous graph requested element payload is
33R+16min(R,64), at most136,192 bytes; the three Vec headers add72 inline bytes.
State and traversal stack are dropped on graph return. The32R summary payload
remains while output tables are reserved, so table-payload accounting alone does
not describe peak construction storage. At R4096,F65536, the independently
calculated table-plus-summary element payload is4,620,288 bytes. These bounds
exclude raw input already owned by the caller and constant local temporaries.

Depth-bounded leaf traversal retains a2096-byte iterator, including a2080-byte
fixed stack, rather than a flattened leaf vector. Two simultaneous iterators
need4192 bytes for the iterators, not one2096-byte allowance. This is stack/local
representation, not a new global allocator cap.

## Scalar-only argument and composition counterexample

For the scalar-only subdomain, each nonempty record's padded layout is at most
four bytes per field; empty records use one byte. At the count maxima, at least
ceil(65536/1024)=64 records are nonempty, giving
4*65536+(4096-64)=266,176 summed layout bytes. The formula remains valid **only
for scalar-only record fields**. It cannot justify a blanket unchanged resource
expectation for composed records.

Independent composed DAG example: R0 contains one bool; Ri contains two complete
R(i-1) fields. Then layout(Ri)=width(Ri)=2^i and depth(Ri)=i+1, while Ri declares
only two fields. R0 through R19 have20 records,39 fields and summed layouts
2^20-1=1,048,575 bytes. Add one empty record to hit the unchanged1,048,576-byte
sum ceiling exactly; a second empty record exceeds it. All count/depth ceilings
remain far below their maxima. This separately demonstrates why the sum-layout
limit is material even when no individual layout exceeds it.

For a64-record single-child chain ending in bool, depth64 is admissible by the
RFC; a65th record crosses the unchanged new-feature depth contract. These are
independent arithmetic fixtures, not claims that unexecuted runtime cases passed.

## Payload, work and observation formulas

Use these definitions per raw function:
L=locals, P=places, O=owners, Rf=references, Pa=parameters, C=calls, N=loans,
B=blocks, S=statements, M=merges, A=descriptor arguments, Q=preparations,
F=scalar constructed fields, E=constructed array elements, K=composite initializer
fields, H=projection path FieldId elements, D=diagnostic origins, G=CFG edges.

Given the measured qualified layouts, requested source raw function payload is:

248 +16Pa+32L+32P+56O+48Rf+96C+72N+16A+328B+208S+48F+32E+56K+16H

Program requested raw payload adds48+56*record_count+64*declared_field_count.
The unchanged source raw cap is67,108,864 bytes. This formula is an inventory;
it does not confer validation, ownership, or executable authority.

For ownership-active functions:
- expanded events = S+M+A+Q+F+E+K+H
- n =1+2B+G+S+M+A+Q+F+E+K+H+O+N+C+Pa+Rf+L+P
- work =4D+(4O+N+C+32)n
- metadata =56(S+B)+48O+64C+24A+24N+32C+16Pa+48Rf+72N+48F+32E+56K+16H

For ownership-inactive functions with diagnostic origins, work is2D and metadata
is56(S+B). The no-ownership/no-diagnostics fast path contributes neither of those
usage quantities. Program sums and overflow checks remain obligatory.
Ownership scratch requested payload per function is the maximum of33B,
2B+8(2B+1+G), and min(max_constructor_fields,1024); functions reuse this bound.
Existing ownership ceilings stay100,000,000 work units and33,554,432 bytes each
for scratch and metadata. Existing owner/event count ceilings stay tied to the
unchanged scalar parser count limits.

Lowering variable scratch remains8B+16*HIR_bindings+40*HIR_expressions+8O.
The larger inline Walk and Output are distinct from this variable scratch metric.
Do not add inline types twice when one embeds the other.

Current typed projection arrays retain56 bytes per expression/statement slot,
including None slots, versus32 previously. For unchanged expression/statement
counts their slot-payload increase is24*(HIR_expressions+HIR_statements), plus
16*sum(path_lengths) for populated path vectors. Field retention increases8 per
declared source HIR field; the resolved owner increases8 bytes. TypedOwnedProgram
embeds that resolved owner, so its8-byte increase is not an additional independent
resolved owner allocation. Path accounting admits a cumulative maximum of
67,108,864/16=4,194,304 FieldId elements and length1..64 per path. Existing one-hop
paths retain their previous work-query charges; longer paths debit their length.
These are logical/requested capacities, not allocator peak/RSS claims.

Observer fixed auxiliary allowance in array_pipeline.rs is
sizeof(Output)+sizeof(Allocator)+sizeof(RefCell<Option<Output>>).
It changes from512+72+520=1104 to528+72+536=1136, exactly+32 bytes,
even when observing an unchanged historical program. Do not silently retain an
exact old auxiliary-byte expectation. The literal request's+16 is already
embedded in Output; do not charge it a second time. Other case-specific transcript,
diagnostic and allocation data must be rechecked, not globally offset without
establishing which branch produced each receipt.

## Retained ceiling matrix and integration gates

- Checked declarations:4096 records /65536 fields /1024 per record;
  table8MiB; summed padded layouts1MiB; containment/path depth64
- Source raw output64MiB; ownership metadata/scratch32MiB each; work100M
- Runtime expanded cells200,000; reference storage16MiB; plan metadata32MiB
- Native expanded cells8192; native arena/live bytes1MiB; existing other native
  function/parameter/block/call-depth/IR/diagnostic limits unchanged

Original historical JSON, compressed corpora, frozen adapters and resource
receipts remain byte-identical. A successor binds a named current-resource
amendment, original and derived hashes, exact substitutions, and reverse identity.
An amendment must say whether a changed number is representation, logical source
payload, observer accounting or language semantics. Do not use these different
categories interchangeably or raise ceilings to recover old passes.

Required before declaring current qualification: source-bound layout tests in
both profiles; exact-/over-limit declaration table and composed sum/depth checks;
new constructor/path count and overflow rejection; unchanged historical-program
semantic checks; reviewed current-resource observer comparisons; preserved PR33
native-fix protocol followed by the separate composition discovery migration.
The existing independent composition review supplies prior bounded execution
evidence, but this appendix does not convert it into exact successor-head evidence.

## Source anchors

- RFC0020: Representation and resource contract; Qualification boundaries
- owned_types.rs:249–314 checked declarations;316–335 ceilings;371–431 admission;
  606–619 leaf iterator;679–805 graph;813–835 checked-table count arithmetic
- owned/source/hir.rs:26–31 fields;205–211 projection
- owned/source/typeck.rs:15–26 retained facade;843–850 projection slot arrays
- owned/source/resolve.rs:69–109 path payload admission
- owned/budget.rs:78–99 counts;113–224 ownership formulas
- owned/source/budget.rs:6 source cap;97–116 function payload;170–192 program payload
- owned/source/lower.rs:132–150 enclosing types;1792–1809 variable scratch
- owned/source/array_pipeline.rs:254–274 LiteralRequest/Output;570–585 fixed auxiliary
- owned/plan.rs:6–11 runtime limits; owned/native.rs:7–16 native limits


## Reproducing the declaration-only generation

The extractor requires an explicit Git checkout, an existing source tree/ref,
and a fresh output directory. It never guesses a sibling checkout, fetches Git
objects, or overwrites the independently measured evidence in this package.
For the published composition source use:

```sh
python3 -B tests/qualification/record_composition_resource_v1/derive-layout-probe.py \
  --repo /path/to/Oxid --tree f6b7dee8bac4ebcc27ad020db9940344c5e4ae41 \
  --output /fresh/output/composition-layout
rustc --edition 2021 /fresh/output/composition-layout/declaration-layout-probe.rs \
  -o /fresh/output/composition-layout/probe
/fresh/output/composition-layout/probe
```

An explicit `--tree HEAD` is also supported. The generated input receipt records
the resolved tree identity, requested ref, source-file and declaration hashes.
A missing requested object produces a clear error; supply a checkout containing
that object rather than silently substituting a different source. The original
measured probe outputs and declaration-input receipts above are unchanged. The
extractor packaging correction is separate from those historical measurements.
