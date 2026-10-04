# RFC 0019: call-only borrowed scalar slices

- Status: experimental implementation for independent review
- Baseline: commit `c5798a232ebdacaf720d580007ee8d760957a081`,
  tree `ce5c204372c9e2d6b8f746c10e968703258d7207`
- Owner: Oxid language/tooling maintainers
- Review: independent review and applicable exact-head CI required before merge
- Acceptance decision: bounded typed-preview extension, without a stability or milestone-completion claim
- Scope: shared and exclusive whole-fixed-array views in function parameters,
  checking, reference execution and native compilation

## 1. Decision and compatibility

Permit a single helper body to read or mutate fixed scalar arrays of different
lengths through a call-scoped borrowed slice parameter. This extends
[RFC 0014](0014-owned-structs-call-borrows.md) and
[RFC 0016](0016-fixed-scalar-arrays.md). The existing fixed-array descriptor,
owned-value rules, source limits, bounds behavior and scalar operations,
including [checked division](0018-checked-i32-division.md), remain authoritative.

Only explicit `typed-preview` gains these forms. The default and explicit
legacy edition, OXBC, nominal records and fixed-array ownership retain their
existing behavior. Historical ledgers, archived manifests and frozen observer
inputs continue to describe their original compiler and semantic authority;
this extension does not retrospectively qualify those artifacts.

The only backing storage is an existing fixed array with a scalar element and
length in the inclusive range 0..1024. Borrowing creates no collection or new
owned storage. An unsized slice is never an owned value, local, record field,
array element or function result. There is no heap allocation, vector, range,
subslice, pointer arithmetic, element borrowing, reference return or lifetime
inference. Native qualification remains Linux x86_64 with LLVM/Clang/LLD
19.1.7 at the existing optimization level and admission limits. Linked project
loading retains its existing Linux-only qualification.

## 2. Source forms

```text
scalar_type    := "bool" | "i32" | "(" ")"
slice_type     := "[" scalar_type "]"
parameter_type := existing_parameter_type
                | "&" slice_type
                | "&" "mut" slice_type
borrow_argument := "&" owner_name | "&" "mut" owner_name
                 | "&" "*" reference_parameter_name
                 | "&" "mut" "*" reference_parameter_name
```

Both `&[i32]` and `&mut [i32]` are accepted parameter types; whitespace follows
the existing lexer rules. Scalar elements are exactly bool/i32/unit. Owned
`[T]`, nested/reference/record elements, borrowed local annotations and borrowed
results remain excluded. Existing `[T; N]`, `&[T; N]` and `&mut [T; N]` syntax
and the narrow typed-zero-length initializer rule are unchanged.

A named slice parameter permits only the already-supported scalar projections:
`p[i]`, `p[i] = rhs` when exclusive, and `p.len()`. The result of `len` is i32.
Indexing takes a signed i32 index and copies one scalar; it never moves the
referent. Unit indexing still validates bounds and permissions. A slice has no
constructor, literal, owned assignment, equality or display operation. Bare `p`
cannot be passed as a value, stored, returned or discarded as a reference value.
Use the existing explicit reborrow spelling instead.

Fixed-array-to-slice compatibility occurs only at a direct borrow argument:

- `&[T; N]` authority can supply `&[T]` through an explicit shared borrow
- `&mut [T; N]` authority can supply `&mut [T]` through an explicit exclusive reborrow
- A slice can explicitly reborrow as the same scalar slice type
- An exclusive reference can supply a shared view with explicit `&*p`
- Borrow mode must otherwise match exactly; `&mut a` does not implicitly supply
  a shared formal parameter
- Element types must match exactly, including for empty arrays
- A slice cannot supply a fixed-array formal, even when its runtime length
  happens to equal that formal's static length
- Exact record and fixed-array parameters retain exact nominal/structural
  identity; a record never converts into a slice

This conversion is one-way type erasure of a complete array's static length.
No length is invented, narrowed by source syntax or inferred back into a fixed
type after erasure. No implicit reference forwarding is introduced.

## 3. Example and observable result

```text
fn sum(p: &[i32]) -> i32 {
    let mut i = 0;
    let mut total = 0;
    while i < p.len() {
        total = total + p[i];
        i = i + 1;
    }
    return total;
}
fn bump(p: &mut [i32]) -> () {
    let mut i = 0;
    while i < p.len() {
        p[i] = p[i] + 1;
        i = i + 1;
    }
    return;
}
fn relay(p: &mut [i32]) -> i32 {
    bump(&mut *p);
    return sum(&*p);
}
fn main() -> i32 {
    let mut a = [1, 2];
    let mut b = [3, 4, 5];
    let mut empty: [i32; 0] = [];
    let first = relay(&mut a);
    let second = relay(&mut b);
    let zero = relay(&mut empty);
    return first * 100 + second + zero;
}
```

The same `sum`, `bump` and `relay` bodies accept all three lengths. `first` is 5,
`second` is 15 and `zero` is 0, so the program returns 515. Shared-only sums
before mutation are 3, 12 and 0. Declared modules preserve the same structural
slice element type and dynamic length across calls.

## 4. Whole-owner permissions and lifetimes

All slice loans cover their complete backing owner. Length erasure does not
make loans disjoint or weaken availability, origin, generation or permission
checks. The ownership verifier remains the only authority for executable
admission. A slice loan overlaps every fixed-array or slice loan to the same
backing owner, regardless of the apparent referent descriptor.

Multiple shared views can coexist. Exclusive/exclusive and shared/exclusive
aliases are rejected, including mixed fixed-array/slice signatures and explicit
reborrows. Slice `len` is a whole-base read, just as fixed-array `len` is: a
pending exclusive loan prohibits later length/index reads from the owner or
suspended parent. Shared parameters cannot mutate or exclusively reborrow.
An exclusive parameter permits mutation without making its parameter binding
reassignable.

Call arguments evaluate left to right. Each loan is acquired when its argument
is evaluated and remains active through all later argument evaluation until
normal return of its owning call. An explicit child reborrow keeps the same
root owner and bounded view length, suspends incompatible parent accesses and
releases at the child's call return. Returning from the nested helper restores
the parent's usable permission. No escaping, stored or returned references are
added. The fixed-array owner can be used normally after the outer call returns.

## 5. Runtime bounds, ordering and diagnostics

For every slice read or write, require `0 <= i < length`. Negative indices,
index equal to length and every index into an empty view fail with the existing
E0606/oir-owned-run diagnostic, exactly `array index out of bounds`. The primary
origin is the full `p[index]` access, including the correct child module file.
The native executable retains this source-rendered diagnostic after source
removal. There is no clamping, unsigned wraparound or speculative memory access.

Existing evaluation order remains binding: evaluate a store's complete RHS,
then its index expression, then charge the access, check bounds and perform the
final store. A helper called while evaluating the index may mutate the backing
array; the final successful store still writes the earlier RHS scalar snapshot.
A failing RHS skips index evaluation. Bounds failure performs no final read or
write and does not roll back earlier completed helper effects. Fuel failure
precedes bounds failure at the final access. Read/write/length operations retain
the existing one-unit operation cost; making or passing a view is not a copy of
all elements. Backing-owner initialization, moves and teardown retain their
fixed-array width-dependent costs.

Static validation covers unused helpers and untaken branches. Wrong element or
borrow mode, and slice-to-fixed conversion, are E0300/type; bare reference
forwarding is E0312/type. Immutable-owner exclusive borrowing is E0304/type;
shared-reference mutation/exclusive reborrow is E0313/ownership. Conflicting
live loans remain E0311/ownership; moved owners remain E0310/ownership. Source
rejection precedes native tool discovery and output artifact creation. Missing
fixed-array punctuation, excluded slice placements and excluded range forms
retain the existing parser diagnostic categories.

## 6. Private representation and executable boundary

Owned value and owner descriptors remain exact `AggregateTy` identities.
`BorrowedTy` distinguishes `Exact(AggregateTy)` from `ScalarSlice(Ty)`;
`BorrowedSlot` is a checked compact retained descriptor for reference and loan
rows. It cannot be used as an owner slot. Every nominal ordinal is checked
before compact conversion, preserving malformed-ID behavior. A slice element
type alone never grants access to storage.

Raw verification separately checks declaration identity, authority, permission,
formal argument compatibility, call staging and source origins before sealing
an executable witness. Authority compatibility is directional: a fixed array
may satisfy a scalar slice, while a scalar slice cannot satisfy an exact fixed
array. In particular, a malformed raw loan cannot claim a more precise exact
referent than its slice authority or use a slice element descriptor to excuse a
mismatched formal. Runtime roots still resolve to exact live owners.

The reference consumer obtains length from the actual checked backing owner.
Native slice calls carry the backing address and checked runtime length through
the private lowering/ABI; exact fixed parameters retain their static type.
Nested explicit reborrows preserve both components. Native checks signed lower
and upper bounds before address formation or loads/stores. No public pointer,
slice ABI, artifact encoding or externally constructible reference is promised.
Zero-length arrays retain their existing initialized positive-size sentinel;
it is never a readable slice element. Unit storage remains initialized.

## 7. Resource limits and evidence

Existing source/token/node, function/argument/module, fixed-array length,
verification work, metadata, flow scratch, plan, native admission and fuel caps
must not be increased for this feature. The source grammar introduces no slice
literal or element allocation. New borrowed metadata, native parameter staging
and length propagation must be counted in their existing resource families and
verified with retained/enclosing representation measurements. An eight-byte
borrowed descriptor does not by itself prove unchanged enclosing enum layouts.
No total-RSS or universal allocation-recovery claim is made.

`tests/typed_slices.rs` defines public check/run controls for:

- One shared sum body used with lengths 2, 3 and 0
- Exclusive bump plus explicit relay producing sums 5, 15, 0 and result 515
- The equivalent linked-module program
- Bool and unit elements, zero-length inputs, mixed exact/slice shared aliases,
  shared/exclusive explicit reborrows and RHS-before-index snapshots
- Signed -1, length, zero-length, i32 minimum and maximum bounds failures,
  including exact text/JSON access origins and module filenames
- Exclusive and mixed alias rejection, wrong scalar types/modes, implicit
  forwarding, reverse slice-to-fixed conversion, local/result/field escapes,
  nested/non-scalar elements, arbitrary dereferences and ranges
- The unchanged inclusive 1024-element backing-array limit and one-over failure
- An explicit ignored LLVM 19.1.7 native parity gate; generated ELF executables
  run with cleared environments after all their source files are removed

Existing raw tests are migrated by explicitly wrapping their historical exact
referents without changing their expected semantic observations. Focused raw
slice controls, malformed authority/call descriptors, ownership overlap,
resource measurements and native emission checks complement these public tests.
Ordinary tests must not depend on LLVM. Native gates and Linux module gates keep
the repository's established host restrictions.

Test definitions are acceptance criteria, not proof of a green implementation.
Fresh integrated checks, focused native gates, format/Clippy, ordinary
regressions and proportional release/repository verification must be recorded
separately from independent review and exact-head hosted CI. No acceptance here
claims completed M2, Rust-compatible memory safety, a general unsized type
system, collection heap support or a broader native target matrix.
