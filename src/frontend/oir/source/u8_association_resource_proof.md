# Local RFC0030 scalar association resource successor

This is a local candidate proof for the repository-owned logical storage/work
model. It is not a Rust stack bound, allocator-internals bound, process RSS
measurement, source-size expansion, or public qualification claim.

## Authority and lifetime

`authenticate_scalar` consumes the exact raw `Program`, borrows its `SourceMap`,
and uses an immutable Original AST or frozen declaration index. It constructs a
private `AssociatedScalar` only after independent association succeeds. The
wrapper owns the original Program header, not a cloned owner payload. Consuming
it transfers that same Program and source borrow into `verify_associated`.
The actual source-authorized parsed AST is immutable grammar authority at this
boundary. It is neither relexed nor reparsed; a fully fabricated or mutated
supposedly trusted AST lies outside this proof. Parser-derived owner/node bounds
below apply to that admitted authority, not arbitrary AST vectors supplied by an
untrusted producer. The prefix/bitset tracker is construction-local and is dropped before the
wrapper returns. Raw verification without that wrapper rejects conversions with
no diagnostic origin, even if a forged span happens to be in bounds.

Conversion authentication proves the actual source expression's operation,
whole/receiver/intrinsic-name origins, owning function and source, adjacent
snapshot origin/destination correspondence, and unique source occurrence.
The binding is checked for kind, same-function interval and receiver spelling.
This is not a general source-to-OIR lexical-binding or lowering-equivalence
proof. Typed source validation precedes this route; arbitrary raw type, ID,
initialization and dominance correctness remain independent verifier duties.

A genuine index may have a synthetic builtin suffix. For a conversion-bearing
scalar route, function_count must equal source_function_count before tracker
construction. This prevents a source-only function lookup from constructing a
nested indexed error with live tracker payload. The no-conversion predecessor
route does not acquire a new builtin policy.

## Derived source extent and payload

Let O be the admitted source-owner count and N the sum of those owners' retained
AST expression counts. Original has O=1. Project loading enforces O<=256 in
`ChildDimensions::admit` with `limits.modules.min(256)`. Its `parse_file` passes
`min(limits.nodes, parser::MAX_NODES) - usage.syntax_nodes` to each parser and
adds the returned actual node debit before the next module. Successful parser
expression construction is node-debited, hence the aggregate N<=100000, rather
than 100000 separately for every module. These are existing admitted-source
bounds, not new limits imposed on a different phase or caller-supplied counts.

The tracker requests exactly:

- prefix: (O+1) * sizeof(usize)
- bitset: ceil(N/64) * sizeof(u64)

All arithmetic is checked, both reservations use try_reserve_exact, and both
observable capacities must equal the admitted slot count before retention. The
model prices requested/accepted observable payload; it makes no assertion about
opaque allocator metadata or transient allocator-internal allocation size. An
excess-capacity result is rejected. Prefix success followed by word failure
leaves the prefix only in the constructor's local `offsets`; error propagation
drops it before returning. It cannot escape as a tracker or source witness.

On the measured x86_64 host, N=100000 needs 1563 words / 12504 bytes. Project's
maximum 257 prefix slots need 2056 bytes, so dense payload is 14560 bytes.
Original's two prefix slots plus the same bitset need 12520 bytes.

The shared additive control model is:

- ConversionSeen headers: 48 bytes
- ConversionAuthenticationCarriers: 664 bytes
- SeenCarriers: 1328 bytes
- finite association error construction/payload model: 628 bytes

Thus controls are 2668 bytes; tracker-plus-helper maxima are 17228 bytes for a
project and 15188 for Original. The finite error term includes 440 bytes of
constructor/transport roles, 136 bytes of boxed Diagnostic payload, and at most
52 bytes of constant message text, with no labels, notes or source origins.
`SeenCarriers` includes the closed owner selector, checked borrowed source access,
constructor and reserve arguments/results, prefix/word locals, observed
capacities, arithmetic, loops, O(1) marking and modeled-byte query transports.
Its nested ByteFormulaCarriers is 488 bytes (alignment 8): separately live
formula/helper parameters, caller arguments, nine checked Option-to-Result
arithmetic transports, seven named subtotal values, modeled extent lookup and
return transports. The constructor's dimensions are still inventoried alongside
these helper roles. Formula code uses explicit subtotals instead of capturing
arithmetic closures. The closed selector is separately inventoried for `new`,
`new_impl`, `count(self)` and `extent(self, owner)`, including both by-value
Indexed branch bindings, Original's borrowed AST and the extent owner parameter;
the by-value extent receiver is not represented as a borrowed pointer.

`ScalarAssociationCarriers` independently prices the scalar caller and consumer:
raw/source/declaration arguments, count/validation visitors, tracker header,
function/block/statement iterators and ordinals, scalar-function arguments/results,
count-walk empty tracker, previous-index Option/capture/lookup receiver, direct
lookup and authentication transports,
builtin-count guard, usage/results, wrapper literal/result/consumer/unpacking
roles, returned raw/source tuple, verifier return, and checked model arithmetic.
Its measured size is 1168 bytes, alignment 8. It deliberately gives no credit for
optimized copies, lifetime slot reuse or owner-header transfer. It does not
charge multiple copies of the owned AST, SourceMap, index or raw payload.

The scalar association successor is therefore:

    scalar caller bank + max(shared tracker model, finite association error bank)

The measured maximum is 18396 bytes for an admitted project or 16356 bytes for
Original. With no conversions, the tracker model is zero and no prefix/bitset is
allocated; caller/error control model is 1796 bytes. A raw forged conversion
against a genuine no-conversion AST can request only the genuine AST-derived
extent, then fails the direct expression-kind check. Its raw metadata cannot
supply a larger extent or authorize a witness.

The enclosing source phase retains its existing SourceMap/AST/index, typed and
raw owners while the new association term is live. This term must be added to
that live-owner inventory; it does not consume an alleged spare index 16 MiB
bank. There is no invented production scratch quota. Owned callers use the same
shared helper bank with their separately inventoried caller and existing paid
HirPlan phase; this scalar caller bank is not their accounting authority.

## Counted work and controls

The tracker constructor charges two owner walks, one prefix-zero write, one
zero per word and two observed-capacity inspections: 2*O+1+ceil(N/64)+2. Its
maximum admitted project count is 2078. A successful mark charges five direct
index/bit inspections. No owner scan, sorted-span assumption, source reparse,
per-function reset or quadratic duplicate search is used.

For A scalar assignments, every validation assignment contributes one real
conversion discriminator, including no-u8 predecessor assignments. Each genuine
conversion adds 29 fixed structural/mark inspections plus actual intrinsic-name
and receiver-name byte comparisons. The fixed intrinsic lengths are 13 and 6.
Each loop iteration is counted before inspection. This is separate from the
existing count/validation origin walks and source-owner function-count check;
conversion-bearing Project also has the one source-only count guard. The
fixture's expected work is computed independently from these roles and compared
with the actual Visitor counter, rather than copied from the observed total.

For the predecessor project fixture helper/main/sum/child-main, the new seven
assignment discriminators are: helper literal (1), two call-argument literals
(2), sum's two named snapshots and arithmetic assignment (3), and child-main's
named snapshot (1). The predecessor one dimension therefore becomes eight.

Tests cover exact/one-byte-short test-only model budgets, overflow, each real
fallible reserve failure, mocked excess capacity on each reservation, observed
prefix success before word failure, the last bit, cross-owner/reordered marks,
exact duplicate source occurrences, full admitted extent arithmetic/capacities,
real no-conversion association under both reserve sentinels, genuine builtin
index rejection before allocation, forged no-conversion source mismatch, and
valid-span raw-seam failures without a diagnostic origin. The test-only lower
byte budget is not a new production limit.
