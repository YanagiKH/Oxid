# RFC0030 scalar native resource successor

This is a named repository-carrier and finite source-event proof, not a stack,
allocator-backend or RSS claim. Global ceilings and the existing work tariffs
remain unchanged.

## New caller-owned formatting storage

Qualified compiler: Rust 1.99.0 b940084d7, Linux x86_64. A probe using the exact
new checks/truncation/widening formatting templates was compiled with
`RUSTC_BOOTSTRAP=1 rustc -Zprint-type-sizes --emit=mir,link`. Its emitted MIR has
2 unique runtime arguments at each new site; the comparison site has 5. Private
`core::fmt::rt::Argument` and `ArgumentType` measure 16 bytes/alignment 8;
public `fmt::Arguments` measures 16 bytes. The two-pointer repr(C) surrogate in
`native_scalar_resource.rs` prices the private argument's complete carrier.

For each N-argument callsite the bank separately includes N constructor return
objects, the complete N-element argument array, N initial references, the full
N-reference tuple, N constructor-input references and their call transports,
the thin static-template and argument-array borrows, the Arguments constructor
input tuple and its complete return. No moved-object or ABI-elision credit is
taken. The resulting generated-callsite bank is 64N+48 bytes: 176 for N=2 and
368 for N=5. The compact placeholder program is immutable compiler data;
there is no newly allocated dynamic placeholder array. The live descriptor
borrow is charged. Only unchanged opaque standard-library frames remain out
of scope. Toolchain changes require repeating the MIR/layout qualification.

The complete new bank decomposes into admission 960, emission 1944, fixed
preflight 96, and the inventory result 8: 3008 bytes. Its single new row also
adds 24 bytes across the native inventory array, moved array and IntoIter.
The previous native local bank is preserved at 2155; successor is 5187.
These numbers are checked independently in tests against actual owning types.

## Work and output containment

The admission helper has ten syntactic declaration-lookup sites but at most
three executed on any path. Exhaustive type/opcode/malformed-ID controls cover
10240 cases, including bad destinations and operands. A lookup contains no
loop. The conservative finite census is 32 dispatch/prologue events + 3*16
lookup events + 8*4 type/predicate events +16 result events +64 diagnostic
setup events +4*62 ASCII message-copy events =440, below the inherited 512
units/statement. Allocation-backend time is excluded as in the predecessor.

The six inherited per-pass statement buckets remain metadata candidate lookup,
checked-failure classification, reverse exit-label lookup, body traversal,
opcode selection, and result/write setup. Each receives128 source events;
new branch setup is finite and introduces no OIR walk. Narrowing has one
failure, less than division's two. Failure formatting and diagnostic loops
retain their existing separately priced paths. Actual writer tests substitute
usize::MAX in every numeric local: narrow checks+value+failure, including full
possible diagnostic-length width, fit660 bytes; widening fits65, both below
2048. The output is still separately charged64 events per template byte.

## Admission order and connection work

A private fixed-bank check runs after the existing entry gates and before
native graph planning or assignment admission. Complete fixed budget minus1
is rejected with zero assignment-helper calls and zero allocator attempts.
Exact fixed budget reaches the count pass; final text budget minus1 still
makes no reserve. Exact fixed+text budget makes one exact-capacity reserve.
These are actual invocation controls, distinct from passive formula tests.

Conservatively count four native-bank evaluations: source plan, outside-bank
subtraction, early fixed check, final text preflight. The connection envelope
uses389 inventory rows*32 +19456 predecessor fixed events +128 extra handler
=32032, below unchanged32768. All unchanged-source paths pay the additive
bank; no source-name or byte-input exemption is inferred.
