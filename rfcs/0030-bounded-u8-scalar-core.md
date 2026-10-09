# RFC 0030: bounded unsigned-byte scalar core

Status: **contract approved for bounded implementation planning; implementation
and resource admission remain unqualified**. Decision date: 2026-10-09.
Draft date: 2026-10-09. Baseline: `20f8e494f872420e6eb53b518b92783c9a1c3539`.
Owner: delegated Oxid language development. Reviewer: independent reviewer to be
assigned before implementation. Publication and implementation qualification remain separate gates.

## 1. Outcome and scope

Add one unsigned integer, `u8`, with mathematical domain 0..255, to the explicit
typed-preview language. A source helper can accept, copy, compare and return a
byte, and explicitly widen it to an existing i32 main result. Checked narrowing
from i32 rejects negative values and values above 255 at execution. This is a
small successor to [RFC 0005](0005-exact-i32-literals.md), not a claim that RFC
0005 approved another integer family or casts, or that roadmap M2 is complete.

Only the current stable-Rust production frontend is in scope. Both its scalar
OIR route and its owned-program route must implement the same byte scalar
contract. Adding an unrelated supported record to a source file must not remove
u8 support or change conversion/comparison runtime semantics. Existing route-specific static
diagnostic precedence is explicitly preserved, not unified. Existing route-specific
activation/storage charges remain distinct; equal language semantics does not
mean identical whole-program fuel across those representations.

No other integer widths, unsigned arithmetic, bitwise operations, shifts,
byte literals/suffixes, generalized casts, aggregate byte storage, generics,
heap/drop support, new I/O, provider switch, or self-hosting work is included.
This draft changes no code, feature-status claim, validation ledger or accepted
predecessor RFC. Its examples are proposed acceptance fixtures, not executable
capabilities claimed at the baseline.

## 2. Recommended syntax and alternatives

Use two closed, named-receiver intrinsics:

```text
byte_conversion := local_name "." "to_u8_checked" "(" ")"
                 | local_name "." "to_i32" "(" ")"
```

`local_name` is one unqualified local or by-value parameter identifier. It is
not a function name, path, field projection, borrow, literal or general
expression. Whitespace/comments may separate tokens. The parser recognizes
exact spelling, not a function lookup. These nodes are scalar syntax in both
source routes; recognizing them must not force owned routing or require an
import. Each is a primary expression, with ordinary outer grouping and existing
unary/binary precedence. No chaining is admitted.

This follows [RFC 0016's narrow `.len()` intrinsic](0016-fixed-scalar-arrays.md),
while deliberately supporting fewer receiver positions than later projected
array length. `x.to_u8_checked` without parentheses remains an ordinary field
read. Global functions, locals and fields called `to_u8_checked` or `to_i32`
remain legal, and `to_i32(x)` retains ordinary function-call resolution. The
existing `.len()` grammar and its type rules remain unchanged. This is neither
method dispatch nor a general extension-method namespace.

Examples:

```text
fn pass(x: u8) -> u8 { return x; }
fn byte(x: i32) -> u8 { return x.to_u8_checked(); }
fn main() -> i32 {
    let lo = byte(0);
    let a = byte(127);
    let b = byte(128);
    let hi = pass(byte(255));
    if lo < a && a < b && b < hi {
        return hi.to_i32();
    }
    return -1;
}
```

The result is 255 in existing result mode, printed as `255\n` with status 0.
The same i32 main in existing process mode returns status 255 with no trailer.
The latter is an entry-policy distinction, not u8-to-process-status coercion.

Alternatives considered:

1. Bare `u8(x)` / `i32(x)` or `u8_checked(x)` / `u8_to_i32(x)`: short but
   intercepts presently ordinary function calls, or needs new name-resolution
   precedence. Reject that silent compatibility change.
2. Finite `std::num` imports: follows the I/O identity model, but introduces
   declaration inventory, import aliases, builtin identities and potential
   routing/storage changes for two pure scalar operations. Defer that machinery.
3. General `as` or literal suffixes: requires broader grammar, cast policy or
   contextual literal rules. Defer rather than suggest Rust-compatible casts.

Recommend the narrow receiver syntax. A local binding is intentionally required
for `(x + 1).to_u8_checked()`, `f().to_u8_checked()`, `255.to_u8_checked()` or
`r.field.to_i32()`; these forms remain unsupported. This costs a source binding
but makes receiver evaluation and intrinsic admission small and explicit.

## 3. Type, position and operation whitelist

Unqualified `u8` denotes the new primitive in admitted type positions. It is
copyable like bool/i32/unit, has no ownership identity, and requires no user
trait. Parameters/results stay explicitly typed; local annotations are optional.
An unsuffixed decimal is **always i32**, including in a u8 annotation, argument,
return or comparison context. `let x: u8 = 255;` is E0300, not contextual
construction. All i32 literal grammar, range errors, signed-MIN handling and
existing arithmetic remain unchanged. No implicit promotion or coercion exists.

| Position or operation | Proposed admission |
| --- | --- |
| By-value function parameter/result, local, temporary, mutable scalar place | u8 allowed |
| Local initialization, scalar copy, assignment, argument, return | Exact type equality only |
| Existing groups, if/else, loops, break/continue and calls | u8 values may flow; conditions remain bool |
| `x.to_u8_checked()` | Receiver exactly i32; result u8 |
| `x.to_i32()` | Receiver exactly u8; result i32 |
| `== != < <= > >=` | u8/u8 allowed; result bool; unsigned mathematical order |
| Mixed u8/i32 comparison, identity conversions, bool/unit conversions | Rejected |
| `+ - * / %`, unary `-`, `!`, `&& ||` on u8 | Rejected; no unsigned arithmetic or truthiness |
| Struct fields, enum payloads, fixed-array elements, slices | u8 excluded |
| Reference to u8, aggregate byte projection, byte index or byte length | Excluded; indexes/lengths stay i32 |
| Match on a byte, byte patterns, constants/globals, FFI | Excluded |

`parser/arrays.rs::array_element_type` keeps its closed `ScalarTypeSyntax`
representation; `parser/enums.rs::enum_payload_type` also remains unchanged.
Do not add a syntax-only U8 member to either carrier just to delay their existing
parse errors into resolution. Fixed/slice array rejection above includes length
zero. Record fields use the existing general TypeSyntax, so their new explicit
u8 exclusion belongs in resolution. Unannotated array literals still parse
ordinary expressions and reject their first u8 element during typing. All
shown diagnostics assume no earlier error under the existing route's schedule.

Existing signed-i32 `+`, `-`, `*`, `/`, `%` (remainder), and unary negation
retain their current checked semantics, types, diagnostics and evaluation order.
Existing logical operators likewise retain their bool-only rules. No listed
i32 arithmetic operation is extended to u8.
Existing enum matches can contain local byte computations in their arms, but
cannot carry a u8 payload. An array literal containing a u8 expression is
rejected even without an annotation. Generic uses of “scalar” in predecessor
aggregate specifications do not automatically expand their explicit bool/i32/unit
allowlists. In particular, no `[u8; 0]` or `&[u8]` exception is introduced.

Equality extends RFC 0009's same-type table; ordering now accepts either i32/i32
(signed) or u8/u8 (unsigned). At the comparison node, reject an unsupported left
type first; otherwise require the right type to equal the admitted left type.
This local rule does **not** change when each route reaches that node: scalar
`typeck::check_body` checks its postorder expression sequence, whereas owned
`typeck::expression_type` recursively checks the left child and its admissibility
before descending into the right child. Preserve those existing schedules,
including existing code/stage/message/span and diagnostic-vector ordering for
legacy cases. Do not unify them as part of introducing u8.

Paired mandatory legacy control:

```text
fn main() -> bool { return () == (true + 1); }
```

On the scalar route, E0300/type identifies `true` in the right-hand arithmetic
(`type mismatch: expected i32, found bool`). Prefixing the source with
`struct R {}` selects the owned route, where E0300/type identifies the left `()`
(`equality requires i32 or bool operands`) before checking the right arithmetic.
Keep both predecessor observations; adding u8 must not rewrite even the existing
message for this old invalid source. New conversion nodes obey their own route's
existing traversal. Their sole child is the named receiver; after its ordinary
resolution/type restrictions, conversion rejects a wrong receiver at that name.
There is no second operand whose error can be reordered by conversion itself.

Runtime rules are common: all six comparisons evaluate the complete left operand
once before the right operand once and do not short circuit. Existing comparison
non-associativity and logical short circuit stay. Static diagnostic differences
do not authorize different runtime evaluation of a well-typed program.

### Entry and external contracts

`check` remains library-capable and may accept `fn main() -> u8`. Default Run
rejects that entry with E0600/oir-run at the main name; default native Compile
rejects it with E0700/native-admission at the main name, before constructing an execution/native storage plan, invoking tools or
creating/publishing output. Whole-source checking and independent verification
still precede entry validation; this does not move entry ahead of source errors. The source main, when called as a helper, has ordinary u8
semantics. Process entry retains RFC 0025's original-root zero-argument i32 gate,
including its existing E0600/oir-run diagnostic and policy ordering.

There is no new u8 result text or schema-1 JSON variant. Bool/i32/unit output is
unchanged. `read_stdin`/`write_stdout`, their i32 arrays/slices, status payloads,
capacities, byte validation, fuel and signal/error policies are unchanged.
Explicit scalar conversion does not reinterpret a buffer or bypass I/O checks.

## 4. Conversion semantics, errors and origins

Narrowing returns the exact mathematical i32 value as u8 iff 0 <= x <= 255.
Otherwise it raises proposed **E0610 / oir-run / `checked i32 to u8 conversion
out of range`**. The same code, stage and message apply on scalar and owned
reference/native routes. E0610 is unused at this draft's baseline and must be
rechecked for collision before implementation. This is not E0604 arithmetic
overflow, a recoverable Result/Option value, saturation or truncation.

Widening returns the exact value in i32 and cannot fail a range check. Neither
operation accepts its destination type as an identity conversion. A runtime
range check applies even to a binding initialized by a literal: `let x = -1;
return x.to_u8_checked();` is well typed and fails when executed. The same holds
for 256. A dead conversion in a well-typed unchosen arm does not fail at runtime.
Do not fold such a check into a compile-time rejection or remove its fuel.
In contrast, an unrepresentable i32 literal fails existing E0203/resolve even in
dead code; a wrong receiver type fails during typechecking even in dead code.

| Failure | Code/stage and primary span |
| --- | --- |
| Missing intrinsic punctuation | E0100/parse at expected-token position; EOF may be zero-width |
| Argument supplied to a recognized intrinsic | E0101/parse at first unexpected argument token |
| Chaining or excluded receiver form | E0101/parse at unsupported postfix punctuation |
| Conflicting nominal type/import binding named u8 | Proposed E0208/resolve at binding identifier; see migration below |
| Unknown named receiver | E0200/resolve at receiver identifier |
| Receiver has wrong scalar/owned type | E0300/type at receiver identifier |
| Bare reference parameter receiver | Existing E0312/type at receiver identifier before conversion typing |
| `[u8; N]`, `&[u8; N]`, `&mut [u8; N]`, `&[u8]`, `&mut [u8]` | Existing E0101/parse at the two-ASCII-byte `u8` token |
| `enum E { V(u8) }` | Existing E0100/parse at `u8`, `only bool, i32 and () enum payloads are supported` |
| `struct R { x: u8 }` | New excluded-position E0202/resolve at `u8`, `u8 record fields are not supported` |
| Parameter `x: &u8` / `x: &mut u8` | Existing E0202/resolve at referent `u8`, `reference parameter requires a record type, found` followed by the quoted name |
| Reference local/field/result type beginning `&` | Existing E0101/parse at `&`, `unsupported typed-preview construct` followed by the quoted token; reference syntax stays parameter-only |
| Inferred u8 array element | E0300/type at first u8 element expression |
| Assignment/call/return or mixed comparison mismatch | Existing E0300/type at first mismatched value/operand |
| Insufficient conversion fuel | E0601/oir-run at whole conversion expression |
| Paid out-of-range narrowing | E0610/oir-run at intrinsic identifier `to_u8_checked` |
| Malformed OIR/metadata | E0500 at existing verification/invariant stage; only validated origins rendered |

The whole expression spans receiver through closing `)`, including intervening
trivia. Preserve a distinct intrinsic-name span without punctuation. AST/HIR
stores the receiver expression and operation discriminator plus both origins.
Each conversion counts as one expression node above its receiver toward the
existing height limit. Do not synthesize a fake call or operator span. Source
association validates the named receiver, operation, spans and full source
ownership; valid raw span bounds alone are not source-authentication proof.

### Evaluation and fuel

Lower the receiver through the existing scalar local-read path, producing one
snapshot with its existing cost; then lower one ordinary conversion assignment
into a fresh typed temporary. Do not fuse away that local read or add a call,
constant, synthetic argument or activation. Charge one fuel for the conversion
assignment **before** reading its operand or performing the range check. On fuel
failure the destination is uninitialized and no conversion is performed. After
payment, an out-of-range value fails before destination initialization. Widening
also costs one, even though it cannot fail a range check.

Calls, groups, stores, argument transfers, returns and frame/slot admission keep
their current costs. Failure inside a preceding call or scalar expression wins
before reaching conversion. The named-only receiver cannot itself call a helper;
write `let x = effectful(); let b = x.to_u8_checked();` to express that sequencing.
A discarded conversion still executes. Failed conversion does not undo earlier
stdout or other already-performed effects. Runtime failure exits 1 when its
diagnostic succeeds, 74 on the existing applicable diagnostic-I/O failure path;
no successful result/trailer is printed. JSON failures retain null result.

## 5. Private representations and independent rejection

Add `Ty::U8` and a concrete tagged `Scalar::U8(u8)` value; never store source
bytes as bool, f64 or an untagged i32 in reference execution. Keep private HIR
operations closed: checked i32-to-u8 and exact u8-to-i32. No source u8 literal
or private u8-constant Rvalue is required in this increment.

Both raw scalar OIR and the owned scalar-assignment seam admit exactly two new
Rvalues, each with one operand, its operation origin and ordinary assignment
origin. Independent verification must require:

- Narrow: operand i32, destination u8; widen: operand u8, destination i32
- All conversion/read/copy/call/return IDs and types valid, exact signatures,
  initialization, ordered use/definition and dominance on arbitrary valid CFGs
- u8 comparison operands same-type and destination bool; unsigned interpretation
  selected from verified type, never source spelling or host signedness
- No u8 arithmetic, negation, bool condition, mixed comparison, identity cast,
  aggregate component, borrowed scalar, array index or new entry-result bypass
- Existing declaration, ownership, mutable-place, source association, span,
  resource, whole-file and immutable verified-witness checks before execution

Raw i32 constants assigned to u8 must fail, even for 0..255. Forged conversion
operand/result types, invalid discriminants, impossible raw aggregate types,
stale source identities, swapped origins and malformed native-plan claims must
not obtain an executable witness. Native admission must independently reject
unsupported operation/type pairs rather than accepting every newly added scalar
enum variant. Reference impossible tagged values remain invariant failures;
there is no public raw-OIR execution route or new privileged constructor.

The owned route must admit u8 in scalar locals/places, scalar helper signatures,
argument staging and returns while retaining explicit predecessor aggregate
allowlists. Merely extending a shared scalar enum is insufficient: every layout,
array, enum, projection and borrowing consumer must be audited for accidental
admission. An unused record/enum or harmless module import must not turn a valid
byte helper into an unsupported construct or reinterpret its comparisons.

## 6. Native lowering and bounded resources

Qualification scope remains Linux x86_64, pinned LLVM/Clang/LLD 19.1.7, O0 and
existing native nonrecursive admission. No optimization, platform or ABI
stability claim is added. Scalar-route helper arguments/results use LLVM i8.
Equality uses `icmp eq/ne i8`; ordering uses `icmp ult/ule/ugt/uge i8`, never
signed comparison or subtraction. Widening is `zext i8 to i32`, not `sext`.
Narrowing checks i32 < 0 or i32 > 255, branches to the ordinary failure path on
failure, and executes `trunc i32 to i8` only on success. No unchecked truncation,
wrap, poison-producing arithmetic flags or host/C scalar conversion implements
the language rule.

The owned native route preserves RFCs 0026/0027's i64 scalar arenas and eight-byte
scalar argument positions. Store byte values zero-extended, load/transport them
as validated scalar bytes, and compare/widen with the same unsigned meaning.
Do not repack arenas, shrink admission prices or change aggregate layouts.
Its private helper ABI must follow existing scalar transport conventions with a
canonical 0..255 representation, rather than assume every helper becomes i8.
Source exposes no stable FFI or cross-module object ABI.

Each u8 local/temporary is one scalar slot, not one byte of admission credit.
Preserve reference 1,000,000 fuel, 1,024 frames, 200,000 live scalar slots and
256 argument-scratch scalars, and all current per-route limits. Preserve source,
parser, HIR, compile-work, raw verification, native I/W, byte, metadata,
diagnostic and LLVM-text ceilings and failure ordering. RFC 0027's I/W inventory
and native byte formulas are not replaced by source byte widths.

Before activating implementation, measure actual affected enum/AST/HIR/OIR,
request/result, runtime-value, source-owner, verifier, diagnostics and emitter
carriers, retained capacities and simultaneous scratch roles. Also measure all
added traversal, comparison and validation **work**, including the reservation
scan and any changed no-u8 path. Charge storage and work growth under existing
limits; unchanged carrier sizes do not prove unchanged work admission. Do not
assume adding an enum variant is free or that the declaration fixed bank has
spare capacity. Any changed exact **work or byte endpoint** requires a separately
named, independently reviewed admission successor before activation, even when
all ceilings remain unchanged. Preserve predecessor endpoint evidence and add
exact successor/one-under controls; neither the bounded language decision nor
passing ordinary fixtures authorizes an unmeasured resource regression. No ceiling increase, omitted
charge or fake whole-process/RSS claim is permitted. If growth cannot be
admitted honestly, stop and revise the design before activation.

## 7. Compatibility and unchanged boundaries

This opt-in typed-preview successor expands previously rejected syntax/types.
Legacy dynamic numbers, normal legacy calls, OXBC, OXA and provider selection
are unchanged. Existing bounded parser/static and HIR observation protocols do
not gain a u8 tag. New byte programs use only the stable-Rust frontend.

### Bounded producer and observation compatibility

Audit the supported v1 **and v2** routes, not only the original RFC 0028 precursor:
`hir_protocol.rs` selects 128/255-byte ASCII source caps; public documentation
specifies unchanged OPA1/AST1/STF1 and OPA2/AST2/STF2 markers, 1,559-byte parser
frames, 2,607-byte success observations and 1,575-byte diagnostic observations.
The HIR-import/producer boundary has no general u8 schema authority; an OHIR
label must not be treated as permission to extend these concrete formats. No new
OHIR marker, opcode/type code, version, producer source,
source cap or manifest selector is introduced here.

The frozen **accepted semantic domain** remains bool/i32/unit scalar syntax.
Do not infer that any spelling transported by a numeric/name row is semantically
supported. In particular, an old producer can transport the identifier `u8` as
an unknown type and emit its old diagnostic; the new canonical compiler may
accept that type, so its diagnostic no longer matches. Such an observation must
fail the existing exact canonical comparison, not be normalized into success.
Compare the complete supplied **first diagnostic** against the canonical first
diagnostic: code, stage, message, source identity, primary/secondary spans,
secondary labels, notes and kind-specific payload. Only an exact match permits
returning the genuine checker's unchanged complete canonical error vector in
its original order. The producer does not supply a full diagnostic vector, so
there is no claim of comparing two supplied/canonical full vectors. A canonical success where an old diagnostic
was supplied remains E0703/hir-producer. Supplied diagnostic artifacts remain
unimportable (existing E0702/hir-import). New conversion syntax is outside the
bounded grammar; producer refusal or source/domain/AST mismatch remains a closed
failure under the existing route, with no Rust fallback or publication. A
forged successful observation encoding u8 as i32 must fail domain/fact checking.

Unknown-type diagnostics need particular care: do not globally rewrite old
`expected bool, i32 or ()` wording just to advertise u8. Preserve the canonical
messages for unchanged invalid source on which exact producer diagnostics rely;
new u8-specific cases use their explicitly named diagnostics. Qualification
must compare unchanged valid sources and still-invalid unknown names (e.g.
i64), preserving their complete old observations and error vectors. Keep frozen
u8-as-unknown-type observations if present as **historical** evidence, and add
new-version compiler controls showing their intentional refusal; never rewrite
old bytes/oracles or claim these now-divergent cases still have current parity.

The unchanged producer grammar's old coverage and current compiler coverage
must be reported separately. Run focused compatibility inventories for old
valid sources, unchanged invalid sources, and changed-domain sources (u8 type,
conversions, reservation), including public-function/unknown-type schedules.
Audit canonical helper changes and exhaustive host matches without expanding
wire enums. Current-source qualification must explicitly record any changed
acceptance/refusal while retaining all old artifact identities. Production
provider switching and producer language expansion remain separate work.

There is one explicit **source naming compatibility issue**: `u8` was previously
available as a nominal type/import alias. Recommend reserving this spelling
**only for type bindings**, with a migration error rather than silently shadowing
an existing nominal type. A struct or enum declaration named `u8`, or an import
that introduces a nominal type binding named `u8`, fails proposed **E0208 /
resolve / `type name u8 is reserved for the unsigned-byte primitive; rename the
type or import alias`**. The primary span is the declaration identifier or the
import's explicit alias (last imported item identifier when no alias exists).
This applies even to unused declarations/imports. The reservation is a distinct
new pass **after every existing declaration/import-graph validation phase has
succeeded**, and before type/body checking or lowering. Any predecessor graph
error anywhere wins over every E0208 candidate, including one textually earlier;
preserve the full existing error vector/ordering rather than interleave E0208
with old duplicate/import checks. On an otherwise valid graph, scan candidates
in existing module preorder and within each module in merged declaration/import
source order; return the first conflicting type binding. A validated alias with
both a type and function target fails E0208 because of its type target; a
function-only alias remains legal. A missing/inaccessible target or conflicting
alias retains its ordinary predecessor diagnostic instead of E0208.
Insertion audit: `DeclarationFacts::finish` runs original-conflict and enum-
variant checks, then `CleanOriginals::freeze` groups/stages imports, returns all
import errors, and performs final freeze validation. A reservation scan can run
only after that final successful validation, using validated alias type-target
metadata rather than re-resolving imports. Merge each module's existing source
positions for declarations and imports without a new uncharged retained table;
charge traversal/comparison work and temporary roles. Do not add u8 to the
existing bool/i32 conflict marking in `collect_originals`: that would report it
too early. Preserve scalar OriginalSingleFile's `original_signatures` call before
`finish`; it cannot contain a nominal/import reservation candidate, and moving
it would change unrelated signature/duplicate precedence. Project and owned
consumers already finish the index before resolving their type bodies. The
insertion therefore appears feasible without moving predecessor phases, but
requires focused implementation proof, accounting and review before activation.
Do not add a late duplicate pass or change nominal lookup to make it feasible.

E0208 is unused at this baseline and must be rechecked before implementation.
This restriction must be checked independently by source declaration admission;
raw nominal metadata must not manufacture a conflicting source declaration.

Concrete previously valid source:

```text
struct u8 { value: i32 }
fn pass(x: u8) -> u8 { return x; }
fn main() -> i32 {
    let x = pass(u8 { value: 7 });
    return x.value;
}
```

Before this successor, both annotations refer to the record. Afterward the
program fails E0208 at `u8` in `struct u8`, before those annotations can silently
change meaning. Rename the declaration, both annotations and constructor to
`ByteRecord`; the resulting program keeps its old meaning. Likewise rename an
import alias `u8` to `ByteRecord` and update its uses. A qualified path does not
bypass the ban on declaring a type named `u8`; its defining module must migrate.
No new primitive path such as `std::num::u8` is introduced.

This is distinct from the function/value namespaces: `fn u8(x: i32) -> i32 {
return x; }` and `u8(255)` remain an ordinary user function and call. Local and
field names `u8` remain legal. An import of a function under alias `u8` remains
ordinary too. Do not implement the type restriction as a lexer keyword or a
blanket identifier ban. Other primitive-name rules are not expanded by this
RFC. This breaking preview change and its migration diagnostic require explicit
review approval; implementation is not approval. Preserving nominal shadowing
instead would require a revised type-resolution policy and revised RFC.

Formerly rejected method-like syntax can acquire a more specific type error,
as happened for `.len()`. Existing legal ordinary calls and field reads with the
conversion spellings must remain unchanged. Previously accepted no-u8 source
semantics, i32 inference, main signatures, process/I/O policies and public output
must remain unchanged, apart from separately measured and independently reviewed
work/byte admission successors.
No production-readiness, complete numeric system, Rust compatibility, memory
safety, full M2, native AI or v1.0 completion claim follows.

## 8. Acceptance plan and stopping condition

The bounded planning decision below does not establish implementation evidence.
The following are required evidence, **not tests run by this docs draft**:

1. Public stable-Rust check/reference-run/native-compile of the helper example,
   plus all 256 valid narrow/widen round trips and all ordered pairs of byte
   values compared by an independent integer oracle. Partition the 65,536 pairs
   into independent bounded source/runner batches under unchanged source, slot,
   call, compile-work and fuel caps; do not put the entire Cartesian product in
   one oversized source or raise a cap. Record complete pair coverage across
   batches, including all six comparison operators. Explicitly distinguish
   0/127/128/255 and detect signed-i8 comparison/sign-extension bugs.
2. Public failing executions for -1, 256, i32 MIN/MAX; static failures for wrong
   receiver/types, implicit construction, mixed comparison, byte arithmetic,
   aggregate positions, indexes and entries. Pin code/stage/span/message and
   unchosen-branch checking versus execution. Keep signed-i32 boundary controls.
3. Parser/formatter fixed-point and comment/origin tests; missing tokens,
   unsupported receivers/chains/arguments, height/source/token limits and
   function/field-name collisions. Exercise nominal `u8` migration explicitly:
   duplicates (including a later
   duplicate with an earlier u8 declaration), invalid/inaccessible import targets,
   alias conflicts, type-only/dual/function-only aliases, qualified references
   to a prohibited declaration, renamed qualified paths, and ordinary `fn u8`.
   Pin the graph-error-before-reservation order and first-candidate scan order.
4. Distinguish two exact fuel exhaustion points: receiver snapshot assignment
   (E0601 at the named read before any conversion charge) and conversion assignment
   (receiver snapshot paid, E0601 at the whole conversion before its operand read
   or range test). With both paid, invalid input reaches E0610 at the intrinsic
   name; valid input succeeds. Test narrowing and widening on both routes.
   Add mutable snapshot/copy controls,
   argument order, helper forward calls/returns, recursion/reference limits,
   discarded conversions, short circuit, loops and failure after earlier stdout.
   Derive expected event/cost traces independently of implementation constants.
5. Repeat source behavior through scalar and owned routes, including an unused
   aggregate and a multi-file helper. Test mutable scalar places and enum-arm
   byte locals without enabling aggregate byte storage. Preserve route-specific
   activation accounting rather than normalize it away. Include annotation-only
   `fn pass(x:u8)->u8{return x;}` and conversion-only unannotated locals; neither
   requires an aggregate/import to select a working route. Repeat with an unused
   aggregate, an unused supported import and a module helper. Pin the paired
   legacy static error-order control from section 3. Entry rejection controls
   instrument that no execution/native plan, output creation or tool launch occurs.
6. Malformed raw-OIR tests for every new operation/type pair and aggregate
   exclusion, IDs, spans, initialization/dominance, source association, signedness
   selection and native-plan correspondence. Swap two individually in-bounds,
   valid conversion-name spans (and receiver origins) between source nodes;
   raw bounds validity must not let source association accept the wrong mapping.
   Test every independent consumer.
7. Real source-free ELF runs on the pinned host/tools; compare values, failure
   diagnostics, statuses and fuel with reference plus independent expectations.
   Inspect actual emitted checked truncation, unsigned predicates and widening.
   Preserve effect-free check/compile, no fallback and no-clobber publication.
8. Fresh carrier and traversal/comparison/validation-work accounting evidence,
   separately reviewed changed admission endpoints, and exact/one-over predecessor/successor
   endpoints, followed by repository-prescribed stable-Rust checks, all affected
   native/source qualification, independent review and applicable exact-head CI.
   Unsupported hosts retain rejection and no new native support claim.

Implementation entry points include `src/frontend/{ast,parser,hir,typeck}.rs`,
`declaration_index`, formatter/source routing, scalar OIR lowering/verification/
execution/native emission, and `oir/owned` source/types/shape/execution/native
consumers. Existing entry adapters, builtin signatures and serialization seams
need rejection/regression audits, not an I/O or provider redesign.

Stop at the independently reviewed byte-scalar public vertical slice with its
explicit exclusions. Byte arrays, enum payloads, arithmetic, richer receiver
syntax, imports/general conversion libraries and I/O migration each require a
separate decision and evidence.

## 9. Decision and remaining review gates

Decision, 2026-10-09: **contract approved for bounded implementation planning;
implementation and resource admission remain unqualified**.

The bounded language tradeoffs are approved: named-only conversion intrinsics,
runtime E0610 and exact origins, no aggregate/arithmetic expansion, E0208
migration for type bindings named `u8`, and intentional refusal of changed-domain
old producer observations without wire expansion. Preserve route-specific static
errors, aggregate parse exclusions and the post-graph E0208 phase. These are not
alternative behaviors for an implementer to choose silently.

This decision does not approve any unmeasured resource regression, limit increase,
unsafe bypass, completed implementation or release. In particular:

- Implementation feasibility must prove the identified reservation seam and
  routing controls without moving predecessor resolution phases. If that proof
  fails, return for a contract revision rather than change error precedence.
- Actual storage and traversal/comparison/validation-work measurements and exact
  admission endpoints are not yet known. Every changed exact work or byte endpoint
  requires its own named independently reviewed admission successor under the
  unchanged ceilings, preserving historical evidence.
- Current-source provider qualification must name any changed-domain refusals
  without changing old observation protocols or historical coverage claims.
- Code integration, independent implementation review, qualification and exact-head
  CI remain subsequent gates; a planning decision is not evidence they passed.

No compiler change, new-language test pass, publication, hosted-CI result or merge
is asserted by this document. Baseline code inspection and small predecessor CLI
diagnostic probes informed this revision; they are not implementation qualification.
