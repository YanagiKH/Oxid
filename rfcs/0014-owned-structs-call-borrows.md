# RFC 0014: owned scalar-field structs and call-scoped borrowing

Status: design contract for an experimental ownership-foundations phase. The private declaration/identity/layout groundwork, raw owner/loan verifier and verified reference/native ownership consumers described in §10 are implemented. Struct and borrow source syntax and source ownership lowering remain unavailable. Examples below describe the complete intended phase; they are not accepted source programs yet.

Baseline: PR17 merge `f8061f403415fd728dc8beee73175a0b6b5e4e20`, tree `07c5f1d998d444baecfef8b53deb252ecd8c62ef`. This extends RFCs 0001–0013 without changing legacy-0.9, OXBC, current scalar semantics, source admission or the executable verified-witness boundary. The phase is not completion of M2 or a claim of Rust-compatible memory safety.

## 1. Outcome and boundaries

A program can construct nominal structured state, update it through an exclusive helper parameter, inspect it through shared helper parameters, move it through helpers and returns, and have ownership violations rejected across all existing branches, loops, break, continue and return paths. Scalar behavior, entry output, failure ordering and existing admission must remain unchanged.

Supported values: existing bool/i32/unit scalars and named move-only structs whose fields are only these three scalars. Supported references: shared or exclusive function parameters pointing to one complete struct, supplied by explicit call-argument borrowing. Functions may return an owned struct; executable `main` still returns a scalar. Empty structs are allowed and remain move-only.

Excluded: nested struct fields, tuples/enums/arrays/slices, partial moves, destructuring, user Copy/Clone/Drop traits, implicit aggregate equality, reference-bearing locals/fields/results, lifetime syntax/inference, NLL, two-phase borrowing, scalar/field/temporary borrows, pointer arithmetic/comparison/casts, arbitrary dereference expressions, closures, mutable parameter bindings, globals, heap resources, FFI and changes to the legacy edition.

## 2. Phase source contract (not yet enabled)

```text
item            := function | struct_decl
struct_decl     := "struct" type_name "{" field_decls? "}"
field_decls     := field_name ":" scalar_type ("," field_name ":" scalar_type)* ","?
scalar_type     := "bool" | "i32" | "(" ")"
value_type      := scalar_type | type_name
parameter_type  := value_type | "&" type_name | "&" "mut" type_name
result_type     := value_type
struct_literal  := type_name "{" field_inits? "}"
field_inits     := field_name ":" expression ("," field_name ":" expression)* ","?
field_read      := local_name "." field_name
assignment      := local_name "=" expression ";"
                 | local_name "." field_name "=" expression ";"
borrow_argument := "&" local_name | "&" "mut" local_name
                 | "&" "*" borrowed_parameter_name
                 | "&" "mut" "*" borrowed_parameter_name
argument        := expression | borrow_argument
```

Existing expression precedence remains; a struct literal or field read is an atomic primary. Fields are not methods. Borrow arguments are admitted only as the complete argument of a direct call, not as general expression values. Parentheses around an ordinary expression remain supported; parenthesized borrow/place forms, `(s).field`, `(*p).field`, `make().field`, dereference assignment and temporary borrows are not added. Use `p.field` for a borrowed parameter and bind owned results before projecting fields. Calls still require their existing no-trailing-comma parameter/argument grammar; optional trailing commas are confined to the newly introduced struct lists. No field shorthand or `..base` update syntax.

To avoid confusing a condition's body brace with a struct literal, an unparenthesized struct literal is not a primary in the top-level `if`/`while` condition grammar. A parenthesized literal remains an ordinary expression and will fail bool typing if directly used as the condition. Direct function-call arguments and nested parenthesized expressions can contain struct literals. This restriction never reinterprets an existing `if flag { ... }` or `while flag { ... }`.

Examples:

```text
struct Counter { value: i32, enabled: bool }
struct Marker {}
fn read(state: &Counter) -> i32 { return state.value; }
fn step(state: &mut Counter) -> () {
    state.value = state.value + 1;
    return;
}
fn relay(state: Counter) -> Counter { return state; }
fn forward(state: &mut Counter) -> () {
    step(&mut *state);
    read(&*state);
    return;
}
```

### Names and construction

- A separate nominal type namespace contains struct names. Functions retain their existing value namespace and no-active-shadowing rules; field names are local to a struct
- Collect all struct and function declarations before resolving bodies, permitting forward type references and calls. A struct and function/local may share a spelling because the syntactic position selects the namespace. A bare name continues to denote a local, never a constructor or function value
- `bool` and `i32` cannot be struct names. Duplicate struct names, duplicate fields and unknown fields/types are errors. `()` is syntax, not a name
- Each literal must name exactly every declared field once. No omissions, duplicates, unknown fields, defaults or structural conversion. Field expression types must match exactly
- Evaluate field expressions once in written source order, regardless of declaration/layout order. Construct the owned value only after every expression succeeds. Empty construction still creates a distinct owner identity
- Struct identity depends on declaration identity, not layout or field names. Two declarations with identical fields are not assignment/call compatible

## 3. Ownership and mutability matrix

A struct value has one owner. Static availability is checked for both named owners and internal owned temporaries. Scalar field reads copy the scalar without moving the struct. Binding immutability and ownership availability are independent.

| Form | Result / requirement |
|---|---|
| `let b = a;` where a is a struct | Move whole a into b; a becomes unavailable, even if all fields are scalars |
| `consume(a)` with owned parameter | Move a at its left-to-right argument evaluation point |
| `return a;` with owned result | Move a into the caller result before callee storage ends |
| `a;` as a discarded struct expression | Move then discard the value; later use of a is rejected |
| `a.value` | Copy field scalar; a must be available and readable |
| `let n = a.value;` then mutate a | n retains the copied scalar snapshot |
| `a.value = rhs;` | a must be an available mutable owned local or an exclusive reference parameter |
| immutable owner passed by value | Allowed; immutable does not mean non-movable |
| immutable owned parameter moved/returned | Allowed; parameter rebinding and direct field mutation remain disallowed |
| `let mut a = ...; a = replacement;` | Evaluate replacement fully, then install whole value; type is fixed |
| moved mutable a wholly reinitialized | Allowed, including after a conditional move; availability is restored |
| moved a initialized field by field | Rejected; a field write cannot reconstruct a missing whole value |
| `a = a;` for mutable available a | Allowed: move RHS into a temporary, then reinitialize a atomically at the store step |
| `a = relay(a);` | Allowed; the old owner is moved during RHS; no access to it before the result is installed |
| `a = a;` after a was moved | Rejected at RHS use |
| struct comparison/arithmetic | Rejected; no implicit equality, numeric conversion or Copy |

For a field assignment, resolve the destination identity statically but do not reserve an LHS loan while its RHS runs. Evaluate the RHS first, then require the destination owner to be available and writable at the final store. Thus `a.value = consume(a);` is rejected after the RHS moves a, while `a.value = mutate(&mut a);` is allowed when the nested call returns an i32 and its loan has ended.

RHS evaluation precedes replacement. If evaluation has already moved/mutated something and later overflows or exhausts fuel, earlier effects are not rolled back. A failed final store charge performs no store. No user destructor executes when replacing or discarding a value in this subset.

Availability must hold on every reaching path; constant conditions do not remove checking paths. At a join, one moved incoming path makes the owner unavailable until wholly reinitialized. At a loop header, include the preheader and all backedges, including continue. A source declaration in a loop begins a fresh owner each dynamic execution; backend slot reuse does not revive the previous instance. Break/continue/return end exited lexical storage after required value transfer. A path that terminates does not contribute an unavailable state to an unreachable successor.

## 4. Call borrowing and evaluation matrix

Borrowing selects a complete, available owned local/parameter; `&mut` additionally requires a mutable owner. An immutable binding to an exclusive reference parameter can mutate its referent: the binding cannot be reassigned, but the granted permission is exclusive.

A loan begins immediately when its argument is evaluated and ends when its owning direct call returns normally. It remains active during evaluation of later arguments and nested calls. On abrupt runtime failure, execution stops; no return/cleanup behavior is promised. Discarding a shared argument or not reading it inside the callee does not shorten the loan.

Whole-owner identity is the overlap unit. Any number of shared loans may coexist. An exclusive loan conflicts with every other live overlapping loan and with direct owner reads, writes or moves. A shared loan permits other reads/shared loans but prohibits owner writes, moves, replacement or exclusive loans.

| Sequence (assume matching signatures) | Result |
|---|---|
| `pair(&a, &a)` | Accept shared/shared alias |
| `pair_mut(&mut a, &mut b)` | Accept distinct owners |
| `pair_mut(&mut a, &mut a)` | Reject overlapping exclusive loans at second borrow |
| `mixed(&a, &mut a)` / `mixed(&mut a, &a)` | Reject at second borrow |
| `mut_and_value(&mut a, a.value)` | Reject later read while exclusive loan is active |
| `value_and_mut(a.value, &mut a)` | Accept scalar snapshot acquired first |
| `shared_and_value(&a, a.value)` | Accept shared reading |
| `shared_and_result(&a, mutate(&mut a))` | Reject nested conflicting mutable loan |
| `value_and_mut(read(&a), &mut a)` | Accept: inner shared call returned before outer exclusive loan begins |
| `owned_and_shared(a, &a)` | Reject borrow after move |
| `shared_and_owned(&a, a)` | Reject move while loan is active |
| `borrow(&Counter { ... })` / `borrow(&make())` | Reject temporary borrowing in this phase |
| `let r = &a;`, ref field, ref return | Reject reference storage/escape |
| forwarding `callee(p)` where p is a ref parameter | Reject bare reference-value forwarding; explicitly reborrow |

### Explicit nested reborrowing

`read(&*p)` is allowed when p is either shared or exclusive. `step(&mut *p)` is allowed only when p is exclusive. These are the only dereference-bearing source forms. Each child loan belongs to the nested call and must end before its parent call can return.

Precise permission rule: while an exclusive child is live, the parent permission cannot be used. While shared children are live, the parent may be used only for reads or additional shared reborrows, and cannot create an exclusive child. Thus `two_reads(&*p, &*p)` is legal even when p is exclusive; `mut_and_value(&mut *p, p.value)` is rejected. A shared parent is never upgraded to exclusive. Once all children return, the parent regains its former permission. The old shorthand “parent suspended” must not incorrectly reject compatible shared children.

At each function boundary, distinct exclusive parameters must denote disjoint owners and cannot alias shared parameters. Shared parameters may alias one another. The callee can assume only this contract; its verification cannot assume all parameters have different runtime roots. Check entry/provenance defensively in the reference consumer. Native receives these guarantees only through the final verified witness.

## 5. Integrated pilot: bounded batch job accounting

This is one real source program exercising structured state through multiple helper layers, nested control transfers, owned return and scalar output. It deliberately models a small background-job state machine without claiming I/O or a scheduler.

```text
struct Batch { completed: i32, retries: i32, checksum: i32, active: bool }
fn retry(state: &mut Batch) -> () {
    state.retries = state.retries + 1;
    return;
}
fn commit(state: &mut Batch, job: i32) -> () {
    state.completed = state.completed + 1;
    state.checksum = state.checksum + job * 10;
    return;
}
fn dispatch(state: &mut Batch, job: i32) -> () {
    commit(&mut *state, job);
    return;
}
fn done(state: &Batch) -> bool { return state.completed >= 6; }
fn relay(state: Batch) -> Batch { return state; }
fn finish(state: Batch) -> i32 {
    return state.checksum + state.completed * 100 + state.retries;
}
fn main() -> i32 {
    let mut state = Batch { completed: 0, retries: 0, checksum: 0, active: true };
    while state.active {
        let mut attempt = 0;
        while attempt < 3 {
            attempt = attempt + 1;
            if attempt < 2 {
                retry(&mut state);
                continue;
            }
            let job = state.completed + 1;
            dispatch(&mut state, job);
            break;
        }
        if done(&state) {
            state.active = false;
            break;
        }
        continue;
    }
    let completed = relay(state);
    return finish(completed);
}
```

Expected independent calculation: six jobs, six retries, checksum `10*(1+2+3+4+5+6)=210`, final `210+600+6=816`. The proof fixture records state transitions, owner transfers and loan begin/end events separately from the compiler. Required variants inject a conditional move, continue-carried move, alias conflict, nested reborrow conflict, use after relay, shared borrow after reinitialization, overflow after an earlier write and exact fuel exhaustion before a write. They are acceptance tests, not merely illustrations.

## 6. Diagnostics and command contract

Preserve lex → parse → resolve → type → OIR shape → ownership → admission/execution order. All function bodies and both condition arms remain checked. Well-typed, well-shaped ownership/loan violations get ordinary exit 1 errors with stage `ownership`, not E0500. Forms excluded by §2 grammar, including a borrow expression in a local initializer or a reference field/result type, retain earlier parse/resolve/type diagnostics. A grammatical bare reference-parameter value use such as `let r = p;` is rejected during typing, before ownership lowering. E0500 remains a malformed compiler/raw-IR invariant failure; invalid private IR must not obtain a verified witness.

Proposed diagnostic families (engineering numbering may be adjusted before publication): E0310 use/move/borrow of unavailable owner; E0311 conflicting loan or operation; E0312/type unsupported bare reference-parameter value use; E0313 illegal reborrow permission. Reuse E0304 for mutation of immutable source bindings, E0300 for type mismatches, E0200/0201/0202 for name/duplicate/type resolution where appropriate. Missing/duplicate literal fields need stable type/resolve diagnostics with exact field or literal origins.

Primary span is the rejected use, borrow or assignment target. Secondary spans identify the last conflicting move/loan and declaration; when several incoming paths move a value, choose a deterministic earliest source origin and explain “not available on every path.” Preserve full borrow syntax (including comments) and transfer statement spans. Ownership analysis must never invent a prior move on a path where none occurred merely to produce a label.

`check` accepts a well-typed owned-returning main. `run` rejects it with explicit E0600 entry-result diagnostic; native compile rejects unsupported entry result before tools/output with E0700. Helper-owned returns remain supported. Existing JSON scalar output, native output/no-clobber behavior, stderr and exit status contracts stay unchanged.

## 7. Verified representation and safety boundary

Internal identities are compilation-local nominal RecordId/FieldId, function-local OwnerPlaceId, and explicit LoanId/CallSiteId. Source mutability remains distinct from storage identity: immutable owned locals require stable identity too. Compiler-side Rust Copy for an ID does not give source Copy semantics.

Keep existing scalar SSA values/places and accounting unchanged. Introduce explicit owned-value transfers, full initialization/replacement, scalar field reads/writes, owner storage live/end boundaries and loan acquire/call-return release. Do not disguise a record move as unrestricted scalar Copy. Owned temporaries/results must participate in availability and storage accounting, including unused/discarded results.

Verification layers:

1. Preflight lengths/expanded sizes with checked arithmetic before allocation or traversal
2. Validate all raw declarations, type/field/root/loan identities, signatures, source origins and operation typing, including unreachable malformed input before reachability checks
3. Reuse arbitrary-entry/cyclic CFG structural and scalar dominance verification, without assuming producer order or reducibility
4. Verify whole-owner availability and legal consumption/reinitialization over all reachable edges
5. Verify loan provenance, acquisition order, alias exclusion, matching direct-call argument/return boundaries and parent-child permissions; reject a loan active on only one join edge or surviving its owning call/scope
6. Construct the only immutable VerifiedProgram witness after every layer succeeds. Both reference and native require that witness

A structure-only intermediate may be private but cannot expose run/native methods. Source ownership diagnostics can be derived from the same authoritative dataflow failure after valid lowering, while malformed raw IR remains a distinct internal error. Avoid an independent permissive HIR checker bypassing the OIR boundary.

### Availability algorithm and resource gate

An intentionally simple independent baseline is per-owner finite-state reachability: initial/live/unavailable states, deterministic read/move/reinitialize/end transfer, joining all reachable states and following the worklist until saturation. A later implementation may use a must-availability bitset algorithm, but must match the baseline and prove convergence. Arbitrary raw cyclic/irreducible CFGs and nonzero entries are in scope for verification.

Do not allocate a dense blocks × owners matrix under the existing 300,000-block/100,000-slot limits. A viable baseline processes one owner at a time with O(B+E+S+O) scratch, and O(O*(B+E+S)) worst-case work. This needs a checked ownership-only work envelope measured before source enablement; cap rejection is a resource error, never “safe by timeout.” Scalar-only programs bypass this new work and retain existing limits/admission. Loan-state traversal must have its own bounded representation/convergence argument; nested argument expressions can branch, so linear token scanning is insufficient.

Declaration-table limits are implemented in §10. Final expanded-storage and ownership-analysis caps require measured bounds before source activation; any material reduction of the specified envelope must be documented as a support limitation. The implementation report must state formulas, inclusive boundaries, peak-memory evidence and one-over rejection before large allocation. Reusing the current limits without charging expanded aggregate storage is not acceptable.

## 8. Reference, layout, native and fuel policy

Reference ownership is represented by activation identity + owner slot + generation, with checked indirection for references. Do not store host-language pointers into growable frame vectors. Entering a loop-local lifetime creates a fresh generation, moving transfers the payload and consumes its source, and ending storage invalidates access. A reference cannot outlive the call supplying its parent permission. Dynamic checks supplement rather than replace static verification.

Private Linux x86_64 layout recommendation: declaration-order fields, i32 4-byte size/alignment, bool and unit 1-byte storage, natural padding with checked offsets; empty structs reserve one private identity byte. Layout is not a stable source or FFI ABI. Reference handles are pointer-like backend values, but source exposes neither address nor representation. Field order in memory must not change source initializer evaluation order.

Owned argument/result transfer may use a private by-value or indirect convention with bounded scratch. The chosen convention must be tested with real cross-call storage; do not expose host Rc/RefCell dynamic records, serialize through legacy values, silently fall back to reference execution or attach LLVM noalias/inbounds/lifetime attributes without sufficient proof. Uninitialized padding must never affect observable comparisons/output; structs have no equality/serialization operation in this phase.

Recommended aggregate accounting: scalar slots keep weight 1; a struct slot has weight max(1, scalar field count) plus explicitly counted owner metadata; references/loan tables and call-result scratch are separately bounded. Charge full layout-byte storage as well as abstract cells. Field reads/writes and loan acquire/release are constant operations; constructing/transferring/dropping struct storage charges expanded work before the operation. Define exact costs and origins in a lowering/accounting appendix before integration, then hand-count traces independently. Old scalar costs and diagnostic origins must remain byte-for-byte unchanged. Do not add speculative aggregate fuel numbers before the chosen representation is measured.

Preserve current reference fuel/frame/live-slot policy, native whole-call-graph recursion rejection, 32-depth/64-parameter boundaries, acyclic conservative static admission versus cyclic shared guarded fuel, and pinned LLVM 19.1.7 O0 Linux x86_64 qualification. New ownership-only caps may supplement them but may not silently relax them. Abrupt failure executes no user cleanup or rollback; this phase does not certify heap/resource release or unwinding.

## 9. Required phase qualification

1. Tagged source model built from the RFC, with explicit owner state, scalar snapshots, child loans, call-stack identity and state-machine outcome; it imports neither compiler IR nor legacy semantics. The pilot must calculate 816 independently
2. Exhaust all reachable raw CFGs up to three blocks, at most two successors, arbitrary entries, one owner, and per-block actions no-op/read/move/reinitialize; compare explicit finite-state reachability to the production analysis until both reach a fixed point, never path-length truncation. Add storage end/live and two-owner targeted families
3. Enumerate set partitions of up to four borrow arguments, all shared/exclusive modes, source orders and explicit nested reborrow cases. Compare a capability-tree model independently of compiler loan IDs
4. Independently corrupt raw IDs, field types, layouts, loan roots/parents/ends, scope exits, call arguments/results and ownership transfers; no invalid witness may execute or compile
5. Hand-count new operation/fuel traces with every lower budget and exact next source origin; force guarded native execution using an unused cyclic helper for otherwise acyclic samples
6. Check debug/release compiler parity and actual source-free ELF binaries with pinned tools. Re-run existing scalar logical/mutable/cyclic/loop-control raw gates and all seven source-native oracles
7. Independently review the final production diff and a held-out generated corpus. Report actual counts and mismatches; generated tests and sanitizers are bounded evidence, not a proof of every program or Rust-level safety

Baseline reproduction commands remain those in `docs/architecture/loop-control-validation.md`; this includes full debug/release Rust suites, four explicit raw LLVM tests per profile, loop/while/mutable/logical/comparison/arithmetic/native/literal oracles, metadata tests and repository verification. Final CI must be checked on the exact proposed commit, separately from the already merged PR17 baseline.

## 10. Implemented groundwork and staged activation

`src/frontend/oir/owned_types.rs` is a production-private immutable declaration facade, registered from `oir/mod.rs`. It defines compilation-local RecordId, record-qualified FieldId and distinct function-local OwnerPlaceId/LoanId/CallSiteId wrappers. ValueTy contains only existing scalar types or an owned record ID; reference types are confined to ParameterTy. Raw declarations deliberately permit malformed IDs/types so the facade checks them independently. It validates every declaration/field span, positional identity and scalar-only field before allocation, then owns flat immutable record and field arrays. Checked lookups cannot expose mutable raw state.

Current private ceilings are 4,096 records, 65,536 aggregate fields, 1,024 fields per record, 8 MiB requested declaration-table payload and 1 MiB sum of padded declaration layouts. Counts and table bytes are preflighted before inspecting field contents; all validation/layout checks finish before either fallible output reservation. The private test seam may lower each ceiling, never raise it. These are declaration limits, not new source, activation, live-slot, fuel or ownership-analysis admission rules.

For R records and F fields, requested persistent payload is `R*sizeof(RecordDecl) + F*sizeof(FieldDecl)`. Work is O(R+F), persistent storage O(R+F), auxiliary scratch O(1), with two flat output allocations and no per-record output vector. On the measured Linux x86_64 host the declaration sizes are 64 and 56 bytes; the maximum-count fixture requests 3,932,160 bytes. Field layout uses bool/unit size/alignment 1/1, i32 4/4, checked natural padding, and one private identity byte for an empty record. There is no stable/public/FFI ABI.

At current scalar field types total padded layout is bounded by `4F+R`, at most 266,240 bytes under the count limits. Both byte ceilings are therefore redundant on the measured representation but retained as checked defenses against later representation changes. The payload formula excludes caller-owned raw input, source storage, vector headers and allocator overhead; it is not a total process-memory promise. Future expanded runtime/work limits must be independently specified before source activation.

The facade is consumed by the private raw ownership verifier. That verifier has no source producer yet. Private reference and LLVM consumers now require its immutable witness; their source-gated interfaces remain production-compiled. Existing hir::Ty, Scalar, scalar places/SSA and runtime/backend paths are unchanged. Canonical CFG/scalar shape/dominance logic is shared through direct private adapters; the existing scalar VerifiedProgram route remains intact. No ownership source feature is added to the public feature inventory.

The private RawOwnedProgram → VerifiedOwnedProgram route now checks whole-owner availability, exact call/loan regions, argument preparation order and the caller/callee alias contract. Its witness is fully sealed in a dedicated verification module and has no run/native methods. See [raw verifier validation](../docs/architecture/owned-verifier-validation.md) for its bounded models, resource accounting and compile-fail witness tests.

Private reference owner identities and real native storage/call execution are implemented and independently reviewed; see [consumer validation](../docs/architecture/owned-consumers-validation.md) for the exact resource/fuel contract, bounded raw models and actual LLVM evidence. Source integration and complete source-level phase qualification remain the next review unit. Enable new syntax only when that source subset works through the checked boundary and both consumers. The raw Batch adaptation is executable; the source example above remains unavailable. See [groundwork validation](../docs/architecture/owned-types-validation.md) for declaration evidence; §9 source requirements are not silently claimed by raw-consumer results.

## 11. Following capability gaps

After this phase, M2 still requires stored-reference lifetimes/NLL, partial moves and field-disjoint loans, resource-bearing owned types and drop/error-path cleanup, numeric/type breadth, generic/trait/closure/async and unsafe/concurrency contracts. The next useful scope should be chosen around a real CLI/background-job or typed-buffer workload. Stack-only success must not be presented as safe allocation, FFI ownership, CPU/GPU buffer lifetime, Rust replacement or native AI completion.
