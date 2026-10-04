# Fixed scalar arrays: gated Unit 2B raw verification

This is a preparatory slice of [RFC0016](../../rfcs/0016-fixed-scalar-arrays.md) Unit 2. It adds authoritative raw array
verification behind the mandatory production array gate. It does not enable
array execution or source syntax. The published parent is
`0532ae986b2b41fe03d87ebfe61da441a6453dd5`, tree
`cf1338907241149efb9266d97016a33dbb08fafb`. The local implementation checkpoint is
`5ec5d683c31e26f917cf50b07e3e27731db1c140`, tree
`a30fe4cf29cfc12e6bb91d3a39fa51af7713c017`.

## Authoritative boundary and operations

The raw instruction enum adds exactly ConstructArray, ReadIndex, WriteIndex and
ArrayLength. Constructors contain the actual ordered Vec<Operand>; destination
identity determines element type and exact length. Index operands are signed
I32 scalars; reads define one element scalar; writes consume a scalar snapshot;
length defines I32 and is a whole-base Read for availability and loan checking.

Production verify_with_limits performs resource preflight, checked declarations,
mandatory array-carrier/opcode rejection, and then one common signature,
all-operation shape, canonical scalar CFG and ownership-flow pipeline before
constructing its sealed witness. The gate includes unused/inactive carriers and
new opcodes with no array carrier, including malformed/unreachable instructions.
The cfg(test) probe calls the same preparation and validator continuation but
returns only OwnershipUsage or OwnedFailure. It borrows the raw input and returns
no body, declarations, seal, execution plan or executable witness. It is absent
from production builds. The extra production opcode scan allocates nothing and
uses already admitted global statement/block/function bounds; scalar-only
functions retain their zero ownership-work bypass.

Both shape passes use the existing canonical initialization sites and aggregate
identity facade. Every array operand and scalar destination participates directly
in the canonical definition/use/dominance verifier. Constructor completion uses
the same U-to-A transition as record construction. ReadIndex, WriteIndex and
ArrayLength require an available whole base; their distinct verifier-generated
denial operations use ArrayBase without changing record FieldBase facts. Indexed
writes require a mutable local or exclusive reference. Existing whole-owner
move, replace, staging, parameter/result, alias and reborrow rules apply unchanged.
There is no element ownership state or bounds-proof authority.

Every instruction/operand/diagnostic span is checked. The source association
count and validation walks visit every constructor element, read index, write
RHS/index and diagnostic origin. Raw validation only establishes SourceMap span
validity; the separate association boundary establishes source-function file
ownership. A structural type carries no source-file identity. New access charging
origins use the validated primary span through plan::instruction_span.

Reference execution explicitly returns its existing invariant error for any new
array opcode. The infallible plan-cost/native-emission and source-test-adapter
matches have explicit unreachable sealed-invariant assertions. They cannot be
reached through an admitted production witness. All these temporary arms must be
replaced before removing the gate; this checkpoint contains no partial execution
or fallback. Source-facing diagnostic classification remains unchanged and no
source grammar/selector or frozen binding/corpus is refreshed.

## Resource accounting and retained representation

Q is the sum of actual raw constructor vector lengths. Preflight rejects any
length above 1024 before inspecting elements. It adds Q to expanded ownership
events and n, and adds Q*sizeof(Operand) to ownership metadata and source raw
payload. The existing work formula remains `(4O+L+C+32)*n+4D`, and all count/byte
ceilings are unchanged. The source lowering count still produces Q = 0.

The raw preflight visits constructor sites/lengths only. The two authoritative
shape passes meter their actual element visits. Canonical scalar CFG consumes
all actual operands. Source association separately adds 2Q allocation-free span
visits. Index/RHS uses are fixed-arity per statement. Array construction adds no
field-seen array, element scratch, type table or verifier reservation. The raw
operand vectors are caller-owned payload that the ledger charges; they are not
new verifier allocations. Existing scratch formulas remain unchanged.

FunctionCounts grows from 136 to 144 bytes on the current 64-bit host because it
adds one usize. Source lower::Output.expected and lower::Walk.counts embed that
row as transient stack bookkeeping. Each live instance grows by 8 bytes; neither
is a per-node heap table or retained raw/AST/witness row. The enclosing raw and
runtime representation checks are retained and extended with Option<ValueTy>,
Option<ParameterTy>, OwnerSubject, DeniedSubject, OwnedStatement, OwnedBlock and
Operand. Both ordinary profiles pass these actual-crate assertions: ValueTy/Option 16, ParameterTy/Option 24, OwnerSubject 64, DeniedSubject 64, OwnedInstruction 128, OwnedStatement 208, OwnedBlock 328 and Operand 32 bytes. The other existing raw/plan/runtime envelopes remain unchanged on Linux x86_64.

## Focused validation

Thirteen new focused Rust tests passed on exact staged tree a30fe4cf before its
compiler-source-preserving commit to 5ec5d683. They include:

- Fifteen type/length programs over bool/I32/unit and lengths 0, 1, 2, 4, 1024 pass the
  non-executable probe and fail production admission. Four opcode-only malformed
  unreachable fixtures also fail production admission
- Constructor element/count/kind, read/write/length type, invalid ID/span,
  unreachable shape, canonical definition, use-before-definition, self-reference,
  duplicate destination and four wrong-branch dominance mutations
- Distinct array denial operations, ArrayBase roles, actual moved causes and
  primary charging spans; structural same-layout mismatches in moves, returns
  and borrowed calls; equal type across valid raw files
- Independent Q/event/work/metadata/scratch calculations and inclusive/lowered
  limits; an actual 1025-element raw vector rejects before any allocation attempt
- Two distinct 100,000-event raw inventories: an explicitly shape-invalid
  preflight-only payload with Q = 99,901, and a structurally valid 98-owner program
  with 196 Live/Construct statements and Q = 99,804. The valid one-over changes both
  final array type and vector from 476 to 477 elements. It rejects before any
  allocation attempt while retaining structurally consistent input
- Eighteen existing ownership-budget reservation hook points fail individually
  before success. Lengths 0, 2, 1024 have the same hook boundary and scratch use.
  This is not a count of generic scalar-CFG/declaration allocations. Global
  allocator observation separately confirms zero allocation on oversized and
  valid one-over denials and successful source association walks
- Source association validates 1,027 actual array operand positions plus each
  instruction/primary/cause origin; valid-in-wrong-file mutations reject there
  while ordinary raw SourceMap validation remains correctly distinct
- The promoted 128-case shared/exclusive record loan-defect matrix preserves
  call/argument/type/mode short-circuit order and later loan-span/authority order

Eight additional independent-model Rust tests passed, with 20,076 actual raw
probe comparisons. Expectations reuse the existing independent storage,
graph/state, ordered-capability and nested-reborrow models. The mechanical
adapter emits real array operations and calls the shared validator. Counts are
actual raw comparisons, not separate test functions or executable programs:

| Family | Raw comparisons |
| --- | ---: |
| Lifecycle words, all four initial states and three access operations |1,236|
| Reachable one/two-block graphs, all entries/prologue positions |3,546|
| Unreachable two-block graph/entry subjects |378|
| Explicit joins, backedges, generation restarts and zero length |240|
| Ordered capability acquisition at argument boundaries |9,960|
| Nested reborrow subjects, read and length adapters with indexed writes |4,608|
| Local/parameter/temporary/real-call-result availability and permission |90|
| Staged-owner and shared/exclusive-reference access |18|

The focused model run precedes only later focused-test additions; model and compiler bytes are unchanged. Both ordinary final-tree profiles subsequently execute all eight model tests successfully. The model domain
is primarily I32 length 3, with lengths 0/3 for selected state/class families. It
makes no runtime bounds, payload sequence, fuel, transfer or native claim. The
larger inherited record domains are not relabeled as new array coverage.

The source-closure test is extended to nine forms in all four parser modes,
including mutable borrowed arrays and array results. The privacy script adds
checked compact-slot construction, private-slot access denial, denied infallible
RecordId conversion and production absence of the test probe. The source closure controls passed in both ordinary profiles. The privacy script passed all 26 actual-crate probes as recorded below.

## Ordinary validation and independent review

With pinned Rust 1.99.0, jobs 2 and incremental 0, both ordinary profiles passed on
implementation 5ec5d683/tree a30fe4cf. Each ran 17 executables: 684 unit plus 140
integration successes, totaling 824 passed, 0 failed and 21 ignored. The 21 new Rust
test functions are already included in these totals. Ignored tests were not run.
Strict all-target/all-feature Clippy with -D warnings and the locked release
build passed. Formatting and diff whitespace checks passed. The actual-crate privacy script passed all 26 probes: five positive controls and
21 expected compile failures. The four additions include production absence of
the cfg(test) probe; none grants an executable array witness.

An independent reviewer passed the exact implementation slice. Immutable archive
materialization matched all 1,026 input hashes. Eleven independent behavioral tests
plus an identity marker passed, covering 574 counted input cases and 57
separate allocation-hook inputs. Ten inherited predecessor controls also ran
freshly on the candidate and matched retained, identity-verified Unit2A expected
outputs: 46 raw/runtime size rows, 20 source size rows, 4 ownership usage rows, 5
source diagnostic JSON rows, 5 human rendering rows, 8 type diagnostic rows and 141
nominal failure rows. The predecessor was not rebuilt; this is a comparison to
retained qualified predecessor output, not fresh execution on the old checkout.
The producer's 20,076 model comparisons are separate and are not added to the
reviewer's 574 new cases. The independent report SHA256 is
`b540ad7a8adafea3534195f537325f210b268df8a937b648c15e910d6144aff0`;
its receipt SHA256 is
`148ea4e4614571ed7c48875ae102e64d2c03c960369bb05bd8e6c8949f053bf4`.

The first producer test build failed on missing exhaustive matches in two
current-source test adapters. Explicit unsupported-array assertions fixed the
build. The reviewer separately retained one failed fixture: an intended Dead
Local omitted its mandatory canonical live site and failed shape before flow.
Adding a later canonical live site corrected that reviewer fixture without any
production change or changed acceptance expectation. Failed attempts and their
source/log/binary evidence remain separate from passing qualification.

No array reference/native execution, E0606/bounds emission, native coordinate
batch, sentinel initialization, source syntax/routing, public activation,
source-binding refresh, new host qualification or hosted CI result is claimed.
Those remain later reviewed Units 2C–2E/3/4 work. The raw production gate remains
mandatory until both consumers are complete.
