# Experimental typed checking and bounded reference execution

Source-integration status: experimental. The ownership source subset uses the
production `typed-preview` route. See the
[source qualification ledger](../docs/architecture/owned-source-validation.md)
for the exact source, compiler, target and validation scope. This describes the
current repository, not support in an older released binary.

Status: experimental. Checking is non-executing; explicit run is bounded reference execution. A narrower optional [LLVM native compilation preview](native-preview.md) is also available. `typed-preview` is a provisional selector,
not a final language edition or a completed M1/M2 milestone. The scoped design
and review boundaries are recorded in [RFC 0001](../rfcs/0001-typed-preview-check.md),
[RFC 0002](../rfcs/0002-verified-straight-line-oir.md) and
[RFC 0003](../rfcs/0003-boolean-branch-cfg.md) and
[RFC 0004](../rfcs/0004-bounded-reference-execution.md) and
[RFC 0005](../rfcs/0005-exact-i32-literals.md) and
[RFC 0006](../rfcs/0006-checked-i32-arithmetic.md) and
[RFC 0009](../rfcs/0009-scalar-comparisons.md) and
[RFC 0010](../rfcs/0010-boolean-logical-operators.md),
[RFC 0011](../rfcs/0011-mutable-scalar-locals.md) and
[RFC 0012](../rfcs/0012-while-runtime-fuel.md),
[RFC 0013](../rfcs/0013-loop-control.md),
[RFC 0014](../rfcs/0014-owned-structs-call-borrows.md) and
[RFC 0015](../rfcs/0015-bounded-typed-projects.md) and
[RFC 0016](../rfcs/0016-fixed-scalar-arrays.md) and
[RFC 0019](../rfcs/0019-borrowed-scalar-slices.md) and
[RFC 0020](../rfcs/0020-owned-record-composition.md) and
[RFC 0023](../rfcs/0023-bounded-enum-match.md) and
[RFC 0024](../rfcs/0024-bounded-stdin-input.md).

## Command and compatibility boundary

```sh
oxid check input.ox --edition typed-preview
oxid --edition=typed-preview check input.ox --message-format=json
oxid check --edition typed-preview -- --dash-prefixed.ox
oxid run input.ox --edition typed-preview
```

With no edition option, existing commands retain their legacy behavior.
`--edition legacy-0.9` explicitly selects that same route. Both `--option value`
and `--option=value` work for edition and message format. These two options may
appear before the command, between the command and source, or after the source.
Duplicate, missing, empty, or unknown values fail; they are never ignored.

`--` ends option recognition. For typed checking/running it must follow `check` or `run`, and all
later words are literal operands; exactly one source path is required. A typed
option written after `--` is an operand, not an edition selection. With no new
options, legacy arguments, including any separator, are forwarded unchanged.

There is one deliberate process-argument boundary: all arguments after
`script <name>` belong to that manifest script, so edition-like words there are
passed through. To select an Oxid edition for a script command, place the option
before its name. An explicit typed selection there is rejected before launching
anything. This prevents accidentally consuming an external process's options.

The preview supports `check`, explicit `run`, syntax-only `fmt`, and the separately specified
[native `compile --backend llvm`](native-preview.md). Direct-file invocation,
`ast`, project commands, and every other operation fail before legacy dispatch. The gate runs before interpreter construction, preprocessing,
dependency resolution, script execution, cache writes, or artifact generation.
Checking and reference running read the entry and its explicitly declared modules
and create no output/cache files. Native compilation has its own explicit artifact boundary.
Checking never executes; run consumes only the completely verified OIR for the selected source route. It rejects OXBC input and does not fall back to the legacy
parser, dynamic values, macro expander, interpreter, or artifact writer.

`--message-format text|json` is available only with typed preview. Unknown
preview options are rejected. Backend/target/output options are available only for the native compile route; profile requests remain unavailable.
Manifest edition propagation and typed project builds are not implemented.
Explicitly choosing a legacy command on a source remains possible; the selector
is not a file-carried or project-wide edition marker.

## Fixed scalar arrays

Explicit typed-preview `check`, `run` and native `compile --backend llvm` accept
move-only `[bool; N]`, `[i32; N]` and `[(); N]` values for decimal lengths 0..1024.
Nonempty literals infer one exact scalar element type and their element count:

```text
fn sum(a: &[i32; 2]) -> i32 { return a[0] + a[1]; }
fn main() -> i32 {
    let mut a: [i32; 2] = [5, 7];
    a[0] = a[0] + 1;
    return sum(&a) + a.len();
}
```

This program returns 15. Equal element type and length identify the same array
type across modules. Whole assignment, by-value calls and returns move the owner;
indexed reads copy scalar snapshots. Writes require a mutable local or exclusive
call-only reference. Borrow parameters use `&[T; N]` or `&mut [T; N]`, with the
same exact-mode borrowing/reborrowing and whole-owner overlap rules as structs.
Even a zero-length or unit array is move-only. `a.len()` also requires a readable,
available owner, so it cannot bypass a move or an active exclusive loan.

A named array local/parameter, or a bounded record-field path ending in a fixed
scalar array, is an index or length base. Signed i32 indexes
are checked at runtime: `0 <= i < N`; negative, upper-bound and every zero-array
index fail with E0606/oir-owned-run, exactly `array index out of bounds`. Skipped
branches do not execute bounds checks. For `a[index] = rhs`, the complete RHS is
evaluated to a scalar snapshot before the index, then one access fuel unit is
charged before checking bounds and storing. Earlier helper effects survive a
later failure. Literal elements execute once, left to right.

The empty literal has one narrow typing context: an explicitly annotated
zero-length local initializer, such as `let a: [i32; 0] = ([]);`. It cannot infer
from a call, return or replacement context. Length spellings may have leading
zeros but not signs, separators, names or expressions. Nested/reference/record
elements, repeated-element syntax, temporary or grouped
index bases, element borrowing/moving and array equality/printing remain
unavailable. Call-only whole-array slice views are the bounded exception described
below; owned unsized values, ranges and subslices remain unavailable.
Existing source, ownership, runtime fuel and native admission limits
are unchanged; native limits can reject a source-legal large program.

`check` can accept an array-returning `main`; `run` and native compile still
require an original zero-argument bool/i32/unit root main, rejecting other
entries with E0600/E0700. The [three-module sample](../fixtures/typed-array-samples/README.md)
returns 5325. Declared-child loading remains Linux-only; native compilation
remains Linux x86_64 with LLVM 19.1.7 at O0. See [RFC 0016](../rfcs/0016-fixed-scalar-arrays.md)
for exact diagnostics, costs and exclusions, and the
[public-route validation](../docs/architecture/fixed-array-public-validation.md)
for actual local evidence. Default/legacy dynamic arrays are unchanged. This is
experimental and does not complete M2 or v1.0.

## Standalone bounded byte storage

[RFC0031](../rfcs/0031-bounded-standalone-byte-storage.md) extends the fixed-array
and call-only slice contract to standalone `[u8; N]`, `&[u8; N]`,
`&mut [u8; N]`, `&[u8]` and `&mut [u8]`, with N=0..1024. This successor is
under independent resource and current-source qualification; it does not extend
the historical validation claims linked above.

A u8 scalar is exactly 0..255. Construct one through an explicitly named i32
local or parameter using `x.to_u8_checked()`; an out-of-range value fails with
E0610. Widen a named byte with `b.to_i32()`. Decimal literals remain i32 even
under byte annotations. There is no implicit coercion, u8 arithmetic or byte
literal syntax. The helper `byte` in the
[pilot](../fixtures/typed-byte-storage/main.ox) is an ordinary declared function.
That pilot moves `[0,127,128,255]` through a helper, mutates index1 through an
exclusive slice, and sums `[0,255,128,255]` through a shared slice to return638.

Nonempty byte literals infer exact u8 elements. Empty literals still require an
explicit zero-length local annotation, for example `let a: [u8;0] = ([]);`.
Arrays remain move-only including N=0; indexes and lengths remain i32. Reads
copy byte snapshots. Exact fixed-array and whole-array slice calls retain
existing whole-owner loans, explicit reborrows and evaluation/fuel order.
An indexed byte must first be bound to a local before widening; `a[i].to_i32()`
is not a supported receiver.

Byte arrays cannot be record fields, including unused/nested/imported fields
and N=0. Such fields fail E0202/resolve at the complete field type with
`u8 array record fields are not supported`. Direct u8 fields and enum payloads
remain excluded. Predecessor bool/i32/unit record arrays and projections stay
available. There are no byte-record projections, element references, subslices,
heap buffers, I/O signature changes or provider wire expansions. Existing
stdin/stdout builtins continue to use i32 arrays/slices.

Physical standalone byte array storage uses stride/alignment1 and max(1,N)
bytes. Logical width remains max(1,N), preserving all expanded-cell and fuel
charges. Source/reference and native caps are unchanged; a legal N=1024 source
constructor can exceed the narrower native per-function slot cap and be refused
before tools/output. Native qualification remains Linux x86_64 LLVM19.1.7 O0.

## Bounded owned record composition

[RFC 0020](../rfcs/0020-owned-record-composition.md) permits record fields to hold
bool/i32/unit, another nominal record, or a fixed bool/i32/unit array. Standalone
byte arrays are excluded from record fields. Forward type
references are allowed, but by-value containment must be acyclic even in unused
declarations. Records remain nominal and move-only. Containment and named-root
field paths are bounded to 64 levels, with the existing byte and work ceilings.

```text
struct Meta { completed: i32 }
struct Batch { meta: Meta, samples: [i32; 3] }
fn relay(b: Batch) -> Batch { return b; }
fn bump(b: &mut Batch) -> () {
    let mut i = 0;
    while i < b.samples.len() {
        b.samples[i] = b.samples[i] + 1;
        b.meta.completed = b.meta.completed + 1;
        i = i + 1;
    }
    return;
}
fn main() -> i32 {
    let m = Meta { completed: 0 };
    let a = [1, 2, 3];
    let mut b = relay(Batch { meta: m, samples: a });
    bump(&mut b);
    return b.meta.completed * 100 + b.samples[0] * 10 + b.samples[2];
}
```

The result is 324. Each initializer runs once in written order, independent of
field layout order. Moving an earlier child makes its source unavailable to
later initializer expressions. Constructors require every field exactly once;
only the completed outer record becomes available. For an empty array field,
move an explicitly annotated zero-length local; `field: []` does not infer a type.

Named-root paths may copy or write a scalar leaf, index a contained fixed scalar
array, or call its `len()`. Visibility is checked at every intermediate and final
field. The root must remain available and its existing whole-root permissions
must allow the access. A shared/exclusive helper borrows the complete outer
record; explicit reborrows keep existing parent-suspension rules.

No aggregate field may be extracted, moved, discarded, passed or returned by
value or independently replaced. Replace the complete outer record instead.
The call-only fixed-array-field slice exception below retains whole-root authority. Partial initialization, arrays of records,
nested arrays, stored references and temporary/grouped path roots remain
excluded. Subobjects never gain independent ownership or loan authority.

Whole-value work is charged by recursive width: scalar 1, array max(1,N), and
record max(1,sum of field widths); padded storage bytes are bounded separately.
Scalar leaf accesses retain their existing fuel charge. Indexed writes evaluate
RHS then index, charge fuel, check signed bounds, and finally store. Earlier
moves/helper effects survive a later failure. Reference and native consumers
transfer initialized leaves and empty sentinels without copying padding.

## Call-only projected array slices

[RFC 0022](../rfcs/0022-projected-array-slices.md) permits `sum(&batch.samples)`
and `bump(&mut batch.samples)` when the bounded named-root field path ends in a
fixed bool/i32/unit array and the formal is exactly the matching shared/exclusive
scalar slice. Paths may contain up to 64 fields and visibility is checked at each
hop. Length zero works through an explicitly typed empty-array initializer.

`&*p.samples` and `&mut *p.samples` explicitly reborrow an array field through a
whole-record reference parameter. This restricted argument grammar parses the
star as explicit reference forwarding and the remaining dotted path as the
field selection. It does not accept `*p.samples` as an ordinary expression,
`&(*p).samples`, or `&(p.samples)`, and changes no general unary precedence.
Existing `&*p` reborrowing remains unchanged.

A view exposes only the selected complete array, while its loan covers the
entire root record. Different fields therefore still conflict for exclusive
borrows; shared/shared loans may coexist. Parent permissions restore on return.
The view cannot escape, grant field ownership, borrow scalar/record fields,
supply an exact fixed-array formal, or enable ranges/element borrowing.

Runtime and native consumers preserve field offsets, actual lengths, argument
staging, RHS/index/fuel/bounds/store order and neighboring fields. Retained view
metadata is charged against unchanged resource ceilings. This experimental
extension has separate local tests; predecessor qualification artifacts and
other-platform or exact-head hosted CI claims do not transfer automatically.

## Call-only borrowed scalar slices

Explicit typed-preview `check`, `run` and native `compile --backend llvm` also
accept shared `&[T]` and exclusive `&mut [T]` function parameters, where T is
exactly bool, i32 or unit. Each view borrows one complete existing fixed array
of length 0..1024. This lets one helper process different array lengths without
allocating or owning a collection:

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
```

`sum(&a)` accepts an available `[i32; N]` owner at any supported length, including
an explicitly initialized `[i32; 0]`. An exclusive helper takes `&mut a` and may
assign `p[i]`. A named slice parameter supports only scalar indexing, exclusive
indexed writes and `p.len()` returning i32. No owned `[T]`, slice literal, local,
field, element, result, equality or display operation is added.

Passing a reference parameter onward requires explicit `&*p` or `&mut *p`.
An exclusive parameter can supply a shared view with `&*p`; direct `&mut a`
does not implicitly match a shared formal. Element types and borrow modes must
match, including for empty arrays. A fixed-array reference may explicitly
reborrow as a slice, and a slice may reborrow as the same scalar slice type.
A slice cannot supply a fixed-array formal, even when its runtime length matches.
Bare reference forwarding, arbitrary dereferences, ranges, subslices, element
borrows, stored/returned references and heap collections remain unavailable.

Every loan still covers the whole backing owner. Shared views may coexist;
exclusive loans conflict with overlapping fixed-array and slice loans alike.
Arguments acquire loans left to right through the owning call, and nested
reborrows suspend incompatible parent access until the child call returns.
Length erasure does not weaken ownership or availability checks.

Signed indexing requires `0 <= i < p.len()` and retains E0606/oir-owned-run,
exactly `array index out of bounds`, at the complete access. Every index into an
empty view fails. Writes evaluate their complete RHS before the index; access
fuel is charged before bounds and the final read/write. Slice reads, writes and
length retain the existing one-unit operation costs; forming a view does not
copy the array. Source, storage, verification, fuel and native admission caps
are unchanged.

The [three-module sample](../fixtures/typed-slice-samples/README.md) uses shared
`sum`, exclusive `bump` and explicit reborrows on lengths 2, 3 and 0, returning
515. See [RFC 0019](../rfcs/0019-borrowed-scalar-slices.md) for the contract and
[`tests/typed_slices.rs`](../tests/typed_slices.rs) for the public acceptance
controls. Test definitions are not current hosted-CI receipts; historical
source-bound ledgers keep their original qualification identities. Declared-child
loading remains Linux-only; native scope remains Linux x86_64 with
LLVM/Clang/LLD 19.1.7 at O0 and the existing admission gates. Default/legacy and
OXBC behavior are unchanged. This remains experimental, with no stable slice
ABI, general lifetime inference or milestone-completion claim.

## Bounded nominal enums and consuming match

Explicit typed-preview `check`, `run`, native `compile --backend llvm` and
syntax-only `fmt` accept the experimental contract in
[RFC 0023](../rfcs/0023-bounded-enum-match.md):

```text
enum Token { Integer(i32), Plus, End, Invalid(i32) }
fn consume(token: Token) -> i32 {
    match token {
        Token::Integer(value) => { return value; },
        Token::Plus => { return 1; },
        Token::End => { return 0; },
        Token::Invalid(code) => { return 0 - code; },
    }
}
fn main() -> i32 { return consume(Token::Integer(7)); }
```

Each enum has 1..256 unique variants, each nullary or carrying exactly one
`bool`, `i32` or `()` value. Nullary construction is `Token::End`, without
parentheses; payload construction supplies exactly one expression. A `U(())`
variant is constructed with `E::U(())`. Types match exactly, without coercions.
Enums share the type namespace with records and builtins; existing type aliases,
absolute paths and visibility resolve the type prefix. Variants have their
enum's visibility and no separate `pub` modifier.

Enum values are whole-value move-only locals, mutable replacement destinations,
by-value parameters and by-value results. `match` is a statement over one bare,
available named local or parameter. Its arms have braced blocks, comma separators
and an optional trailing comma. Every variant of that exact enum must occur once,
including arms that will not run. The selected arm consumes the scrutinee once
and, for a payload variant, creates one immutable scalar binding scoped to that
arm. Ordinary no-shadowing, all-path-return, break/continue and loop rules apply.
Only continuing arms participate in the ownership join; a consumed owner can be
restored by an explicit same-type assignment to a mutable local. Matching an
unavailable owner reports E0310 at the complete match statement.

Arm order affects fuel. Selecting zero-based written arm k costs `k + 1` dispatch
units and 3 consumption units before its body. Construction costs 3 after its
payload expression; whole move/owned preparation/discard/StorageEnd cost 3 and
replacement costs 5. Existing call/frame costs use enum width 2. Ordinary source
temporaries, argument staging, body operations and cleanup add their own costs;
these figures are not complete source-statement costs. Fuel failure exposes no
payload and performs no unpaid consumption.

Enums and records share the 4,096 declaration cap. Variants and record fields
share the 65,536 member cap; record fields retain their 1,024 per-record cap.
The existing 100,000-node and 64-level parser limits remain. Affected enum-bearing
HIR storage and admitted projection/lowering scratch use a 64 MiB ceiling; this
is a scoped storage account, not a universal compiler-memory or RSS cap. Reference
execution retains the 200,000-expanded-cell/16 MiB bounds. Native admission uses
independent whole-program inventories I and W, each at most 8,192, plus unchanged
1 MiB aggregate/live explicit-byte bounds and its other stricter limits; see
[RFC 0027](../rfcs/0027-native-admission-inventories.md).

Enum borrows, storage in record fields or arrays, aggregate/recursive payloads,
partial moves, equality, printing and source-visible tags are unavailable.
Match expressions, guards, wildcard/alternative/nested patterns and arbitrary
scrutinee expressions are also unavailable. An enum-returning `main` may check;
run and native compile still require an original zero-argument bool/i32/unit
root `main`.

The scanner [entry](../tests/fixtures/bounded_enum_scanner/main.ox) and
[implementation](../tests/fixtures/bounded_enum_scanner/scanner.ox) use `&[i32]`
character codes, a separate mutable cursor with a private field, and an imported
`Token` alias. Input `12 + 3` returns 115 (12 + 100 + 3). Decimal accumulation
uses checked i32 arithmetic, so overflow remains the ordinary arithmetic failure.
Run it with `oxid run tests/fixtures/bounded_enum_scanner/main.ox --edition typed-preview`.
Declared-child loading remains Linux-only; native scope remains Linux x86_64
with LLVM/Clang/LLD 19.1.7 at O0. This fixed-input compiler-component example
makes no production-provider or self-hosting claim. The
[public acceptance controls](../src/frontend/enum_public_tests.rs) cover the
source path; separate current-source qualification and exact-head hosted CI
remain pending.

## Bounded stdin input

Explicit typed-preview `check`, reference `run`, native `compile --backend llvm`
and syntax-only `fmt` accept exactly these individual compiler-owned imports,
with optional aliases:

```text
use std::io::read_stdin as read;
use std::io::ReadStatus as Status;

fn main() -> i32 {
    let mut bytes = [0, 0, 0, 0];
    let status = read(&mut bytes);
    match status {
        Status::Eof(count) => { return count; },
        Status::Full => { return 4; },
        Status::IoError => { return -5; },
    }
}
```

The signature is `read_stdin(buffer: &mut [i32]) -> ReadStatus`.
`ReadStatus` is a compiler-owned nominal move-only enum with `Eof(i32)`, `Full`
and `IoError`; the existing constructor, whole-value transfer and consuming-match
rules apply. All permitted aliases refer to the same compilation-local identity.
A user enum with identical names is a different type, and a user function named
`read_stdin` remains an ordinary function. Existing duplicate-name and alias rules
apply. Importing the function alone admits its result type internally without
binding a local `ReadStatus` name; importing the type alone admits no input function.

There is no prelude, general `std` lookup, direct `std::...` call/type/variant
path, grouped/glob import or separate variant import. Use the imported names in
source. Existing `crate::std` source modules keep their meaning. Checking,
formatting and compilation do not consume program stdin.

The argument uses existing call-only exclusive borrowing of a whole fixed i32
array, supported projected array field, or explicit reborrow. Its checked
capacity C is 0..1024; the operation stages raw bytes privately until capacity,
EOF or a read error:

- `Eof(n)`, where `0 <= n < C`, commits exactly n bytes as i32 values 0..255 and
  preserves every destination cell from n onward
- `Full` commits exactly C bytes without attempting a read past capacity; it does
  not assert that more input exists
- `IoError` leaves every destination cell unchanged, including after earlier
  successful reads; consumed input is not rolled back
- C = 0 returns `Full` after the core charge of 4, without a host read; ordinary
  builtin-frame admission still applies

Each host attempt requests one byte from fd 0 as it exists at operation time.
There is no prefetch, newline handling or encoding conversion. Interrupted reads
retry; other errors, including nonblocking errors, produce `IoError`. Process
startup can repair closed standard descriptors, so closing fd 0 before launch
does not guarantee equivalent reference/native failure behavior. A blocking read
has no wall-clock bound from execution fuel.

The core operation costs **4 + C + A**, where A counts every attempted read,
including EOF, error and EINTR returns. It debits 4 + C before reads and mutation,
preparing all storage before the first attempt, then debits one before each
attempt. This prepays maximum commit work and ordinary width-2 result construction.
Ordinary argument evaluation, loans, calls, return transfers and cleanup retain
their additional charges. A skipped call performs no read or core debit.

Validation, resource or fuel failure before commit leaves the destination
unchanged, even if earlier attempts consumed bytes. After the first destination
store, commit and result materialization contain no fallible helper, allocation,
validation or extra fuel debit. A later builtin return or caller operation can
still exhaust fuel after the successful commit; earlier effects remain.

Input-bearing reference execution is Linux x86_64 only. Other hosts report
E0608/oir-owned-run, exactly `bounded stdin execution requires Linux x86_64`, at
the admitted input import before entry validation or activation. This applies
to unused function imports and zero-capacity calls. `ReadStatus`-only use remains
portable under the existing project-loading rules; native compilation retains
its Linux x86_64 LLVM/Clang/LLD 19.1.7 at O0 policy. No new main ABI, strings,
file API, general standard library or increased compiler/runtime ceiling is added.

The [expression stdin entry](../fixtures/typed-expression-samples/README.md#bounded-stdin-entry)
uses 129 cells for a 128-byte grammar plus one explicit limit witness. It scans
only the `Eof(n)` prefix; `Full` returns -4 and leaves any 130th byte unread.
Its original parser/scanner wrappers, 15-node arena and 15-entry stacks remain.
The same ELF has returned 39 and 63 for distinct streams in the local 28-case
runner, with public reference/native parity. Syntax and node/stack failures return
-1, invalid arenas -3, input-limit failures -4 and I/O failures -5; overflow remains
E0604. These scalar values use the existing result printer. Full current-source
qualification and exact-head hosted CI remain separate acceptance gates. See
[RFC 0024](../rfcs/0024-bounded-stdin-input.md) for the complete effect contract.

## Bounded stdout and process entry

The individual imports `std::io::write_stdout` and `std::io::WriteStatus` join
the closed builtin catalog, with ordinary aliases and no implicit prelude.
`write_stdout(bytes: &[i32]) -> WriteStatus` consumes no owner: it borrows the
complete existing view, whose capacity is 0..1024. Every cell must be 0..255.
The operation validates and stages all bytes before writing any of them; there
is no encoding conversion, terminator, subslice or used-prefix argument.

`WriteStatus` has `Complete`, `InvalidInput`, and `IoError(i32)` variants.
InvalidInput writes nothing. IoError(n) records bytes already accepted, not
recipient delivery or durability. Empty views return Complete without touching
stdout. One-byte attempts retry EINTR after another fuel debit; zero progress
and other errors stop with IoError. The core cost is `4 + C + A`, including
validation/staging and every attempted write. Borrow/activation/call/return
costs remain separate. The result commit cannot fail after output begins.

```sh
oxid run program.ox --edition=typed-preview --entry-mode=process
oxid compile program.ox --edition=typed-preview --entry-mode=process --backend=llvm --output=program
```

Process entry requires the original root `fn main() -> i32` with no parameters.
Run and Compile share this signature gate (E0600/oir-run); Check remains
library-capable. Values 0..255 become exact process statuses, with no scalar or
JSON stdout trailer. Other values produce E0600 and status 1 if the diagnostic
finishes, otherwise 74. Ordinary calls to main retain normal i32 semantics.
Default `--entry-mode=result` keeps prior output, status and body fuel behavior.
A stdout-function import requires Process for Run/Compile even if unused or
aliased: Result rejects it with E0609/oir-owned-run before effects or emission.
A WriteStatus-only import has no such requirement.

Process Run rejects JSON reporting with E0001 on safe stderr before loading.
Valid Process intent remains sticky for argv errors; missing/unknown entry-mode
values on typed Run also use that text-only channel. `--` and script forwarding
retain their existing boundaries. Compile JSON remains a compiler build report;
Check, Fmt and Compile never establish process signal policy or execute I/O.

Runtime execution is qualified on Linux x86_64. Process Run establishes ignored
SIGPIPE before source loading, diagnostic rendering, activation or fuel checks.
Setup failure returns silent 74. Terminal stderr retries Interrupted/positive
short writes and returns 74 on zero/error, preserving any prefix without a
recursive report or stdout fallback. macOS x86_64/arm64 has diagnostic capability
only and reports E0608/oir-run before loading. Windows Process Run and process
option errors currently return silent 74 before formatting/loading because
synchronous inherited stderr has not been established; Process Compile uses
ordinary E0608 reporting. Other unqualified terminal targets also fail silently.
Default-mode host behavior is unchanged. Hosted checks remain necessary for
platform-specific claims.

Earlier output cannot be rolled back if a later operation, return or fuel check
fails. The [OXS1 artifact component](../fixtures/typed-expression-samples/README.md)
uses one producer executable and a separate loader plus independent decoder.
Callers publish saved output only after successful production and validation.
See [RFC 0025](../rfcs/0025-bounded-stdout-process-entry.md) for resource-admission
successors and the complete effect contract; no general file API or self-hosting
capability is implied.

## Single-file formatting

```sh
oxid fmt --edition typed-preview input.ox
oxid --edition=typed-preview fmt --check input.ox
```

The experimental formatter writes the complete formatted UTF-8 source to stdout.
It normalizes horizontal spaces and four-space delimiter indentation, preserving
interior line breaks and exact token/comment spelling. Empty files format to
empty output; nonempty output ends in LF. Comments are opaque, including their
internal whitespace and line endings. Compact code stays compact.

`--check` writes no source output: exit 0 means already formatted; exit 1 means
formatting is required, with that message on stderr. All selected formatter
errors exit 2 and use escaped text diagnostics on stderr. `--message-format=text`
is accepted; JSON formatter diagnostics are unavailable. A stdout I/O error may
occur after a prefix was written.

Exactly one regular source file is required. Symlinks to regular files are
allowed; directories, devices, FIFOs and stdin (`-`) are rejected. Use `--` for
dash-prefixed filenames and `./-` for a file named `-`. There is no write-in-place,
recursive, output-path or width option. Default/explicit legacy `fmt` retains
its existing file-writing behavior.

Formatting validates full-file syntax twice, including fixed arrays, borrowed slices,
enum declarations/construction and consuming matches,
and checks token/comment and interior-newline preservation before output.
Brackets count toward delimiter indentation; types/literals format as
`[i32; 2]` / `[1, 2]`, with name-based indexing tight as `a[0]`. It never loads modules, resolves
names, typechecks, executes source or starts native tools. Unresolved imports,
unknown types and other semantic errors therefore do not block syntactically
valid formatting. Malformed input produces no candidate source.

Input and candidate each have a 1 MiB source ceiling and the existing lexer and
parser limits, including 100,000 tokens, 65,536 bytes per token, 100,000 syntax
nodes, 64 expression/block nesting, 256 parameters/arguments, 34 path segments,
1024 array elements/declared length, 256 variants per enum and 256 match arms.
The formatter adds a 128-entry delimiter stack and an 8 MiB work-table ceiling;
the current table uses one byte per source byte. These are logical payload
limits, not whole-process memory or host-I/O guarantees. Full layout policy:
[RFC 0017](../rfcs/0017-bounded-typed-formatter.md).

## Bounded typed projects

The entry file may declare `mod state;`, import original declarations with
`use crate::state::Batch;`, and mark modules, functions, structs or fields `pub`.
Absolute item paths work in direct calls, constructors and nominal types.
Declarations load one bounded tree; they do not execute module initialization.
The [three-file Batch example](../fixtures/typed-project-batch/README.md) combines
an opaque record API, moves, call-only borrowing and loops. Its source-derived
result is 816. Exact execution coverage is recorded in the
[project qualification ledger](../docs/architecture/typed-project-unit4-validation.md).

A declaration `mod state;` in the root maps only to the entry directory's
`state.ox`; `mod jobs;` within state maps only to `state/jobs.ox`. There is no
package search, alternate `mod.ox`, implicit directory discovery or import alias
chasing. Crate imports target original functions and/or nominal types; module
imports, reexports, grouped/glob imports and `self::`/`super::` relative paths
are absent. A function and a nominal type can share a spelling and import together
atomically. The four individual `std::io` imports described above are a closed
compiler-owned exception; they perform no source-file discovery.

A private declaration is accessible to its declaring module and descendants.
A private module restricts its contents to its parent's subtree. Public exposure
is checked structurally even when no current caller uses the function. A public
struct does not expose its fields: construction requires access to every field,
while whole-value move/borrow/return can use an opaque value. Nominal types remain
distinct across modules. [RFC 0015](../rfcs/0015-bounded-typed-projects.md) specifies
the exact namespace, visibility, phase-order and diagnostic contracts.

Root is parsed once before declared children are loaded in depth-first declaration
order. Every loaded body is checked, including unused modules and functions.
One whole-project selection preserves the scalar route unless any owned syntax
occurs. Both routes consume the same declaration/import/visibility index and
produce one complete verified program. There is no per-file fallback. Run and
native compile require an original zero-argument scalar `main` declared in root;
an imported or child `main` never supplies the entry. Check requires no main.

Declared-child filesystem loading currently admits Linux only; non-Linux hosts
reject it with E0005/source. Linux x86_64 has the predecessor filesystem evidence;
other Linux architectures are not qualified. A module-free root has no discovery
host gate, including root-only visibility/import/absolute-path syntax. Native
remains Linux x86_64 with LLVM 19.1.7 at O0. The loader checks a stable filesystem;
it does not promise a concurrent-filesystem snapshot or sandbox.

Source bytes (1 MiB), non-EOF tokens including trivia (100,000), syntax nodes
(100,000), declaration/verifier/storage limits and execution fuel are aggregate,
not per-file allowances. Modules are capped at 256 including root, module depth
at 32 edges, and qualified paths at 34 segments including `crate` and endpoint.
Shared index requested payload is at most 32 MiB, peak scratch 16 MiB and work
256,000,000 units. Requested payload does not bound capacity, allocator overhead,
RSS, host I/O time or LLVM memory. Exact loader/path and measured resource limits
remain in RFC 0015 and its implementation ledgers.

## Grammar

```text
file           := item*
item           := function | struct_decl | enum_decl | module_decl | import_decl
module_decl    := "pub"? "mod" name ";"
import_decl    := "use" import_path ("as" name)? ";"
import_path    := absolute_path | "std" "::" "io" "::" ("read_stdin" | "ReadStatus" | "write_stdout" | "WriteStatus")
absolute_path  := "crate" "::" name ("::" name)*
item_path      := name | absolute_path
function       := "pub"? "fn" name "(" parameters? ")" "->" value_type block
parameters     := name ":" parameter_type ("," name ":" parameter_type)*
scalar_type    := "bool" | "i32" | "u8" | "(" ")"
array_element_type := "bool" | "i32" | "u8" | "(" ")"
enum_payload_type := "bool" | "i32" | "(" ")"
fixed_array_type := "[" array_element_type ";" decimal "]"
slice_type     := "[" array_element_type "]"
value_type     := scalar_type | item_path | fixed_array_type
referent_type  := item_path | fixed_array_type | slice_type
parameter_type := value_type | "&" referent_type | "&" "mut" referent_type
struct_decl    := "pub"? "struct" type_name "{" field_decls? "}"
field_decl     := "pub"? name ":" value_type
field_decls    := field_decl ("," field_decl)* ","?
enum_decl      := "pub"? "enum" type_name "{" variant_decl ("," variant_decl)* ","? "}"
variant_decl   := name ("(" enum_payload_type ")")?
variant_path   := name "::" name | absolute_path "::" name
match_arm      := variant_path ("(" name ")")? "=>" block
block          := "{" statement* "}"
statement      := "let" "mut"? name (":" value_type)? "=" expression ";"
                | name "=" expression ";"
                | field_path "=" expression ";"
                | array_base "[" expression "]" "=" expression ";"
                | expression ";"
                | "return" expression? ";"
                | "break" ";" | "continue" ";"
                | "if" expression block ("else" block)?
                | "while" expression block
                | "match" name "{" match_arm ("," match_arm)* ","? "}"
expression     := logical_or
logical_or     := logical_and ("||" logical_and)*
logical_and    := comparison ("&&" comparison)*
comparison     := sum (comparison_op sum)?
comparison_op  := "==" | "!=" | "<" | "<=" | ">" | ">="
sum            := product (("+" | "-") product)*
product        := unary (("*" | "/" | "%") unary)*
unary          := ("!" | "-") unary | primary
primary        := "true" | "false" | name | "(" ")" | "(" expression ")"
                | item_path "(" arguments? ")" | decimal | "-" decimal
                | field_path | item_path "{" field_inits? "}"
                | variant_path ("(" expression ")")?
                | array_base "[" expression "]" | array_base "." "len" "(" ")"
field_path     := name ("." name)+
array_base     := name | field_path
decimal        := ASCII_DIGIT+
field_inits    := name ":" expression ("," name ":" expression)* ","?
arguments      := argument ("," argument)*
argument       := expression | borrow_argument
borrow_argument := "&" name | "&" "mut" name
                 | "&" "*" name | "&" "mut" "*" name
```

Identifiers are case-sensitive ASCII letters/underscore followed by ASCII
letters/digits/underscore. Keywords are reserved. Whitespace, `//` line comments,
and non-nested `/* ... */` comments are retained as trivia and ignored by the
parser. Unicode is permitted in comments; Unicode identifiers are not supported.
`as`, `crate`, `self` and `super` remain contextual ordinary identifiers;
`crate` gains path meaning only before adjacent `::` in an item-path position.
The two colons remain separate lexer tokens and both count in the resource
ledger. Trivia may surround the delimiter but cannot split its two bytes.
Public-field recognition requires `pub` followed by a nontrivia identifier.
Optional trailing commas are confined to struct field declarations, literal
initializers, enum variants and match arms. Function parameters and call
arguments still reject trailing commas. The two arrow bytes in `=>` must be adjacent. Legacy concise-keyword
aliases are not supported.

In an unparenthesized `if`/`while` condition, the restriction on struct literals
applies through the whole top-level precedence expression: its body brace is not
a constructor brace. Parentheses and direct-call arguments admit literals again;
the resulting condition must still be bool. A field projection has exactly the
form `name.field`; `(s).field`, `(*p).field`, `make().field`, chained fields and
methods are unavailable. A borrow is the complete direct-call argument, never a
general expression or parenthesized place. Standalone block statements are not
part of this grammar.

Scalar values are `bool`, `i32` and unit `()`. The owned addition admits nominal
move-only structs (including bounded record composition), fixed scalar arrays
and the bounded nominal enums described above.
Reference types occur only in function parameters. A file may contain no
functions and does not require `main`; checking is not execution. `main` may
return an owned type when checking, but run and native compile require a
zero-argument main returning bool/i32/unit.

- Function parameters and return types must be explicit
- Locals are initialized immediately, with an optional annotation or inferred
  scalar or owned value type; the initializer sees only earlier locals. Ordinary `let` and
  parameters are immutable; `let mut` enables later same-type statement assignment
- Functions are collected before resolving bodies, so forward direct calls and
  recursive calls resolve. Run uses isolated iterative activations; no termination
  claim follows and recursive programs can exhaust execution limits
- Function names are unique within a module. Parameters, function bodies, branch arms and while bodies have
  lexical scopes. Bindings cannot duplicate a name in the same scope, shadow an
  active ancestor, or shadow a value declaration/import in the current module. Sibling arms and declarations
  following a closed child scope may reuse names; every declaration has its own ID
- An arm-local is visible only after its initializer and within that arm or its
  descendants. It cannot escape to a sibling or the surrounding block
- A bare name denotes a local; functions are not first-class values
- Calls must resolve to declared functions and match their arity and types
- There are no builtins or implicit conversions
- Every function, including unit functions, requires explicit returns on every
  reachable intraprocedural path. `return;` and `return ();` return unit. Statements
  after a return or an if with two returning arms are rejected
- An if condition must be bool. Both arms are checked even for literal conditions.
  A missing else leaves a reachable empty false path. Empty arms are legal. An if
  is a statement with no value or trailing semicolon; else requires braces, so
  else-if, if-expressions and naked block statements are unavailable
- Expression statements may discard any supported type

Example:

```text
fn identity(value: bool) -> bool {
    let answer = value;
    return answer;
}
fn main() -> () {
    identity(true);
    return;
}
```

### Exact decimal i32 literals

One or more ASCII decimal digits, optionally preceded by one minus token, denote
an exact i32 in [-2147483648, 2147483647]. Existing trivia may separate minus and
digits: `- /* comment */ 2147483648` is valid. Leading zeroes are decimal, not
octal. `0`, `0000`, `-0` and `-0000` all print `0`. An explicit i32 context and
an unconstrained literal (`let x = 1`) both produce i32; bool/unit contexts give
E0300, with no coercion or truthiness. Other widths are E0202 unknown types.
This provisional single-width default establishes no promotion algorithm.

A minus immediately followed by a decimal token (ignoring trivia) is part of
that signed literal, before the general unary rule is considered. Thus
`-2147483648` and `(-2147483648)` retain exact literal conversion and costs.
General `-(1)`, `-x`, `-f()` and `--1` are checked unary negation; unary `+1`
remains E0101 unsupported. Suffixes, separators, radices, floats,
exponents and non-ASCII digits (`1i32`, `1_000`, `0xff`, `0o7`, `0b1`, `1.0`,
`1e9`, `１`) are E0101/parse. The parser validates the complete numeric token
before conversion; a long invalid suffix is not misreported as a range error.
Digits beyond the i32 range produce E0203/resolve at the complete literal span,
including sign/trivia. A 65,536-byte all-zero token is valid; many digits alone
are not a range error. Full-file checking includes unused functions and unchosen
branches. Exact integer conversion never uses f64 or a bigint.

### Checked i32 arithmetic

Binary `+`, `-`, `*`, `/` and `%` require i32 operands and return i32. `*`, `/` and `%` bind more tightly
than `+`/`-`; each level associates left. Parentheses override precedence. The
left operand is fully evaluated before the right, then the operation executes.
Calls execute exactly once in that order, and the first error stops execution.
`1--2` subtracts the signed literal -2. Prefix `-` takes an i32 expression and
returns i32, at the same precedence as `!`, above multiplication, with prefixes
applied right to left. The decimal-token preference above takes priority:
`--1` negates literal -1, while `-(2147483648)` is E0203 at the positive literal.
The operand is evaluated exactly once; then one arithmetic assignment charge
occurs before checked negation. Negating MIN is E0604 at the outer minus;
`--2147483648` therefore checks successfully and overflows when executed.
Wrong bool/unit/owned operand types are E0300 at the operand. Unchosen branches
retain ordinary short-circuit behavior. No constant folding erases a charge or
an error. Signed literals gain no new node, local, instruction or fuel charge.

Division truncates toward zero; remainder has the dividend's sign (or is zero).
A zero divisor in either operation is E0607/oir-run at its operator. Both
`-2147483648 / -1` and `-2147483648 % -1` are checked overflow.
Every operation checks its result against the i32 range. Overflow is
E0604/oir-run at the operator's one-byte source span, with exit 1 and no partial
result. Host debug and release builds behave identically. No folding, wrapping,
saturation, widening or reassociation occurs: `2147483647 + 1 - 1` and
`0 * (2147483647 + 1)` both overflow. Checking those expressions succeeds without
execution; out-of-range literals still fail E0203 before running. Unchosen arms
do not execute, but every arm is name/type checked. Wrong arithmetic operands
produce E0300 at the first wrongly typed operand. Discarded arithmetic still runs.
The bounded [native preview](native-preview.md) supports these same five checked
operations, with the reference human overflow diagnostic and exit 1. Its stricter
whole-file admission bounds and native I/O failure status 74 still apply.

Casts, shifts, explicit wrapping,
other numeric types and their overflow rules remain unavailable. Literal range
validity remains a separate compile-time rule.

### Scalar comparisons

`==` and `!=` accept matching i32 or matching bool operands. `<`, `<=`, `>` and
`>=` accept i32 only and use signed order. Every result is bool. Unit equality,
mixed-type operands, bool ordering and implicit conversions are unavailable.
E0300 points to the first invalid operand; all declarations and branches are
checked. Equality first validates the left type as i32/bool, then requires the
right type to match it. Ordering requires i32 at each operand.

All six comparisons share one non-associative tier below arithmetic.
`1 + 2 < 4 * 2` is valid. `(1 < 2) == true` explicitly compares bool results.
An unparenthesized second comparator, as in `1 < 2 < 3` or `1 == 2 < 3`, fails
E0100/parse at the second operator's full span. Parenthesized comparisons used
where i32 is required instead fail type checking. The multibyte operators must
be adjacent; comments/whitespace do not join separate punctuation tokens.

Operands execute exactly once, fully left before right, then the comparison
executes. No comparison short-circuits, including discarded results and bool
equality. Operand arithmetic can fail E0604 before comparison; comparison itself
cannot overflow. Unchosen branches remain unexecuted. The reference and bounded native backends share
this table and order; see [RFC 0009](../rfcs/0009-scalar-comparisons.md).

### Boolean logical operators

`!` accepts bool and returns its inverse. `&&` and `||` require bool/bool and
return bool. `a && b` evaluates a once and skips b if a is false; `a || b` skips
b if a is true. Otherwise b is evaluated once and becomes the result. Both
operands are always resolved and type checked, even a statically skipped RHS.
There is no truthiness or conversion from i32/unit. Executed work preserves
left-to-right, first-error and discarded-expression rules. Skipped RHS calls,
arguments and overflow operations have no runtime effect.

`!` binds above arithmetic/comparisons, then `&&`, then `||`. Binary logical
operators associate left at each tier, and prefix `!` nests right. `!a == b`
means `(!a) == b`; `!1 < 2` fails E0300, while `!(1 < 2)` is valid. Existing
comparison chaining restrictions remain. `1 < 2 && 2 < 3` is valid.
Adjacent `!=` remains one token; separated `& &`, `| |`, standalone bitwise
operators and textual `and`/`or` are unavailable. Newly recognized `!` in invalid
grammar positions receives ordinary E0100 rather than unsupported E0101.
See [RFC 0010](../rfcs/0010-boolean-logical-operators.md).

### Initialized mutable scalar locals

`let mut x (: type)? = expression;` introduces an initialized mutable bool/i32/unit
local with a fixed type. `x = expression;` is a statement, not an expression, and
requires an existing mutable bare-name target. Evaluate the RHS completely once
before storing; errors or exhausted store fuel perform no write. Existing if
branches can mutate outer places. Reads, immutable copies and call arguments are
scalar snapshots, and each activation has independent state. Both branches and
all RHSs remain statically checked. Existing lexical scope/shadowing rules apply.

This scalar-local increment adds no uninitialized declaration, mutable parameter,
compound/index assignment, parenthesized target or assignment expression.
Scalar-field assignment on an owned struct is specified separately below. Invalid
positions for `mut` and `=` now receive E0100; immutable targets receive E0304 at
the name with a declaration label, unknown targets E0200, and fixed-type mismatch
E0300 at the RHS with a declaration label. See
[RFC 0011](../rfcs/0011-mutable-scalar-locals.md).

### Ordinary while and operation budget

`while condition { statements }` reevaluates a bool condition before each iteration;
false skips its body. Body-local declarations initialize freshly on every dynamic
execution, using reusable activation slots. Outer mutable places persist and
immutable snapshots remain values. Conditions and bodies are fully checked even
when never executed. Return exits the function, but while itself never proves a
terminal return, even for literal true. No trailing semicolon, while-else,
loop expression or implicit truthiness is introduced.

Reference and guarded native invocations share a total 1,000,000-operation budget
across loops/calls, with E0601 before the next charged operation. This is not a
wall-clock limit or final performance mode. Existing costs and frame/slot limits
remain; loop execution does not accumulate frame storage. See
[RFC 0012](../rfcs/0012-while-runtime-fuel.md) for exact costs, origins, cyclic
verification and native guarded-only representation bounds.

### Unlabeled loop transfers

`break;` exits the nearest enclosing while in the same function. `continue;`
reevaluates its full condition, including calls and lazy logic. Both are
semicolon-only statements, with no values or labels. Statements after any
unconditional transfer, or an if whose arms all transfer, fail E0303 even when
arm outcomes differ. All source paths remain statically checked. Nested while
consumes its own transfers; conservative while false edges and explicit function
return requirements remain unchanged.

Typed block summaries distinguish fallthrough, return, break and continue.
Only falling paths create join edges. In scalar-only modules each transfer lowers to one existing,
precharged Goto at its full statement span, without a subsequent closing-brace
edge. No value/place slots, OIR blocks, runtime frame allocation, new verifier
algorithm or resource limit is added. Out-of-loop transfers give E0204/resolve;
malformed/value/labeled transfer statements give E0100/parse. See
[RFC 0013](../rfcs/0013-loop-control.md).

### Nominal owned structs and call-only borrowing

The owned contract is [RFC 0014](../rfcs/0014-owned-structs-call-borrows.md).
Type names have a separate namespace from functions/locals; fields belong to
their declaring struct. All declarations are collected before bodies. Types are
nominal: equal layouts do not make two structs interchangeable. A literal names
every field exactly once, with an exactly matching scalar or complete owned
value expression. Fields evaluate once in written source order; each owned
initializer moves into staging before the next initializer runs, then complete
construction occurs. There are no
defaults, shorthand fields, spread/update syntax or implicit conversions.

| Form | Contract |
| --- | --- |
| `let b = a;`, `consume(a)`, `return a;`, discarded `a;` | Move the complete owned value; the source becomes unavailable |
| `a.field` or reference parameter `p.field` | Copy a scalar snapshot without moving the owner |
| `a.field = rhs;` | Evaluate RHS first, then require an available mutable owner or exclusive reference permission |
| `let mut a = expr; a = replacement;` | Evaluate replacement fully, then replace the complete value of the same nominal type |
| Reinitialize a moved mutable owner | Allowed by whole replacement; field writes cannot restore a moved owner |
| `a = a;` or `a = relay(a);` | Move the available RHS to separate temporary storage, then replace a |
| `&a` / `&mut a` as an entire call argument | Borrow a complete available owner; exclusive borrowing also requires a mutable binding |
| `&*p` / `&mut *p` as an entire call argument | Explicitly reborrow a reference parameter; exclusive permission requires an exclusive parent |

Empty structs still move. Owned parameters are immutable bindings and may be
moved/returned but cannot be rebound or directly mutated. An exclusive reference
parameter has an immutable binding and permits mutation of its referent.
Unavailable owners fail on reads, moves and borrows. Availability must hold on
every reaching path, including loop backedges and continue. A terminating arm
does not contribute state to a successor it cannot reach. Whole reinitialization
restores availability. Loop-local lifetimes restart each iteration; break,
continue, return and normal block exits end the storage they leave. These are
logical lifetime operations, not user destructors.

Arguments are prepared immediately, left to right: scalar snapshots are saved,
owned arguments move, and borrows acquire their loans before the next argument
runs. Loans last until the owning call returns normally, including evaluation
of later arguments and nested calls. Shared loans may alias each other and allow
reads; they prohibit writes, moves, replacement and exclusive borrowing of that
owner. An exclusive loan excludes every other overlapping loan and direct owner
access. Discarding an argument in the callee does not release its loan early.

Reference argument modes match exactly. An `&T` parameter requires `&a` or
`&*p`; an `&mut T` parameter requires `&mut a` or `&mut *p`. Neither `read(&mut a)`
for `read(p: &T)` nor `write(&a)` for `write(p: &mut T)` is a coercion: both fail
E0300. To obtain shared access from an exclusive reference parameter, explicitly
write `&*p`. Bare forwarding `callee(p)` and `callee(&p)` are unavailable.
Repeated shared children of an exclusive parent are permitted; while they live,
the parent permits only reads and more shared reborrows. An exclusive child
suspends all parent access. A shared parent cannot grant exclusive access.

For example, `take(a.field, &mut a)` saves the earlier scalar before acquiring
the loan; `take(&mut a, a.field)` conflicts at the later read. Likewise,
`a.field = mutate(&mut a);` can complete its inner loan before the final write,
whereas `a.field = consume(a);` fails when the final target is unavailable.
Earlier moves and writes are not rolled back if a later operation fails; an
unpaid store performs no write.

Partial moves, aggregate-field extraction/replacement, destructuring, aggregate
equality, stored or returned references, scalar/field/temporary borrows, general dereference,
reference coercions, mutable parameter bindings, heap resources and user
destructors remain unavailable.

The [Batch pilot](../fixtures/owned_source/batch.ox) combines nested loops,
break/continue, exclusive reborrowing, shared inspection and owned returns.
Its scalar result is 816. Its command contract is:

```sh
oxid check fixtures/owned_source/batch.ox --edition typed-preview
oxid run fixtures/owned_source/batch.ox --edition typed-preview
oxid compile fixtures/owned_source/batch.ox --edition typed-preview \
  --backend llvm --output ./batch
```

The source program has its own lowering/fuel schedule; the historical raw
adaptation's 1,086 fuel is not its cost.

Strings, null, package imports, module initialization, macros, heap containers, for/loop and other
control flow, other operators, async, closures, generics, FFI, host I/O beyond
the bounded stdin/stdout operations, and undeclared builtins are unavailable. Recognized
unsupported syntax produces E0101; other invalid syntax produces E0100 or a
resolution error. There is no
silent approximation or legacy execution of these features. Now-recognized if/else/while/break/continue
keywords in invalid positions produce ordinary syntax errors (E0100), replacing
the predecessor's unsupported-keyword E0101 for those newly enabled keywords.
The owned parser also recognizes struct/field/borrow syntax. Previously
unsupported forms may therefore receive more precise errors: an `&bool`
parameter changes from E0101/parse at `&` to E0202/resolve at `bool` because only
record, fixed-scalar-array and scalar-slice referents are supported. This does not enable scalar borrowing. Accepted
scalar-only programs retain their behavior; byte-identical diagnostics are not
promised for every formerly unsupported ownership token sequence.

## Compiler representation

The compiler path is UTF-8 source → lossless token tape → spanned AST →
resolved HIR → typed HIR → verified OIR. The source integration selects one
route for the entire parsed project. Any struct or enum declaration, non-scalar named
type annotation/signature, reference parameter, struct literal, field access,
borrow argument, fixed-array type/literal, indexing, length access, enum
construction, match or any admitted compiler-owned `std::io` import selects
owned HIR and owned OIR for every function. This includes unused declarations
and statically skipped paths; comments containing owned spellings do not select
that route. Unknown nominal names also select it
and fail resolution. Scalar-only modules retain the existing scalar pipeline,
diagnostics, costs and native admission. There is no per-file or per-function mixture and
no fallback after an owned parse, resolution, type, verification, runtime or
native-admission failure. It lives in `src/frontend/` independently of the legacy
syntax module and runtime. The token tape retains trivia and invalid tokens; it
is not a complete formatter/LSP CST. Parsing synchronizes at the next top-level
`fn`, `struct` or `enum` for original syntax. After actual project-grammar recognition,
recovery also recognizes project declaration boundaries. Recovery scanning alone
does not activate project grammar; erroneous ASTs never enter name resolution.

AST names are source spans. Numeric AST nodes store exact digits spans and a
literal-only sign; resolution uses checked signed integer accumulation to create
representable i32 HIR constants, retaining full literal origins. MIN is accumulated
negatively, never by negating an unrepresentable positive i32. HIR replaces value
uses with function/local IDs,
allocated deterministically in source order. Local IDs belong to one function.
Signatures resolve before bodies. Successful typed construction has one supported
type for every expression and local; incomplete tables cannot be constructed
outside the checking pass. AST/HIR statement blocks use arenas and ID edges.
Each typed body also records every block's return flow. Lexical resolution and
statement checking use explicit traversal frames. Type inference walks child-before-parent arena entries,
not an unbounded recursive chain. There is no runtime `Value` in this pipeline.

The old AST, parser, lexer and lexical helpers now live in
`src/legacy/syntax.rs`. The extraction preserves their behavior and OXBC 1.0
encoding. The existing single-binary Cargo package and Rust edition are unchanged.

## Verified scalar OIR with cyclic control flow

Every successful check or run lowers actual typed bodies and passes the
independent verifier for its selected route before returning success. This
section describes scalar-only modules; owned modules have the additional
authoritative availability/loan verification described below. Immutable typed views expose
complete types and block return flow; there is no reparsing or legacy adapter.
Raw IR is private to `src/frontend/oir/`; only successful verification constructs
the immutable witness used by the driver. No public IR loader, dump, stable
serialization or optimization contract is added by the scalar representation. The bounded reference
consumer below accepts only that immutable verified witness.

Function-local value slots contain bool, i32 or unit and are classified as
parameters, immutable bindings or expression temporaries. Mutable places have a
separate typed declaration table and PlaceId namespace. A mutable binding uses
one place, without also allocating an immutable binding slot. Assign evaluates a bool/i32/unit constant or
copies a typed operand, computes CheckedI32 from two ordered i32 operands, or
computes CompareScalar from the explicit scalar comparison type table, or
computes NotBool from a bool operand into a bool destination, or snapshots a
mutable place through Load. Ordered block statements distinguish SSA Assign,
Initialize(place, value) and Store(place, value); stores define no SSA value.
Lowering completes the whole initializer/RHS before its initialization/store.
CheckedI32 retains its operator origin separately from the full assignment span.
Both arithmetic operands must have dominating initialized definitions; its
destination must be i32. CompareScalar independently checks each operand ID,
span and allowed type pair, both dominating initialized definitions and a bool
destination. Equality permits i32/i32 or bool/bool; ordering permits i32/i32 only.
Call has a direct DefId, ordered arguments, result slot
and one normal continuation. Return uses an explicitly initialized operand.
Branch has a bool operand and two successors; Goto has one successor. Calls in
conditions and arms remain explicit terminators, never hidden in Branch.

Lowering traverses structured source bodies in lexical depth-first order using
root-directed expression continuations and a checked postorder completion cursor. Conditions lower before Branch; then/else expressions
lower only in their respective paths. A join exists only if some arm falls
through or else is absent. Falling arms end in Goto(join); returning arms do not.
An absent else branches directly to the join. Two returning arms produce no
synthetic join. Table order is not topological: joins may be reserved before
arm-call continuations. Statement-if joins introduce no value. Logical expressions
reserve two blocks (RHS and join), evaluate RHS only on its required path and
define one explicit BoolMerge at the join from the selected left/right input.
The existing expression temporary is the result; no opposite-arm writes occur.

Verification validates aggregate bounds, every signature/reference/type/span and
terminator before following graph edges, including unreachable raw blocks.
Reachability rejects unreachable blocks; arbitrary reachable cycles, including
irreducible raw graphs, are independently verified. Equal Branch targets are legal and their edge multiplicity is handled
consistently for non-merge targets. A BoolMerge requires exactly two distinct
incoming edges matching its input predecessors and is forbidden at function entry.
Direct and mutual recursive call graphs remain legal because calls
to other function entries are not intraprocedural edges.

Parameters are entry definitions and cannot be overwritten. Every other local
has at most one definition globally, including definitions in mutually exclusive
arms. An unused undefined slot is legal; every read needs a dominating definition.
Within one block an assignment must precede a read, including its own RHS. A call
result requires strict dominance by its call block, so it is unavailable in its
own arguments/statements or at a join reachable by bypassing that call. Canonical
single-definition rules also cover BoolMerge destinations. Each fixed two-input
merge independently requires bool destination/input types, valid IDs and origins,
and availability of each input on its own predecessor edge. A call result is
available on that call's normal edge into a merge, never in its own arguments.
The merge destination is available from block entry and in dominated blocks.

Every place has exactly one canonical initialization in its separate table,
including unused places. Initializer/store inputs must be available SSA values.
Each load/store requires earlier same-block or dominating cross-block
initialization. Opposite-arm, self-initialization and pre-init access fail;
stores never establish initialization or weaken unique SSA definitions.
All place IDs/types/origins and store operator spans are checked structurally.

The verifier uses iterative Lengauer–Tarjan simple link/eval with path compression,
then iterative dominator-tree intervals for constant-time dominance queries. There is no blocks-by-locals
matrix, per-block initialization-set cloning or recursive graph traversal.

Every block has a valid terminator; the source checker separately enforces its
conservative return rule. Cyclic raw IR need not reach any Return. It does not establish termination, user-stack safety, executable call
safety, ownership, memory safety or source-to-IR equivalence merely from spans.

All declarations, blocks, assignments, operands and terminators retain original
source spans. Existing function-entry, call-continuation, copy/use and bare-return
origins are preserved. Branch uses its full if statement; its operand uses the
condition expression. Arm entries use full source blocks; arm-end Gotos use their
closing braces. Synthetic joins use the full if statement. Exact Unicode/CRLF
provenance is tested separately from valid file/range/UTF-8 boundaries. For logical
expressions, Branch, RHS-ending Goto, merge and join block use the full expression;
RHS entry uses its operand expression, and both input operands retain their spans.
NotBool and BoolMerge also retain their exact operator origins. Place declarations
and targets retain name spans; Load uses its name-expression span. Initialize
and Store use full statement spans, Store also retains `=`, and their input
operands retain complete initializer/RHS spans.

See [RFC 0003](../rfcs/0003-boolean-branch-cfg.md) for the bounded decision and
[RFC 0002](../rfcs/0002-verified-straight-line-oir.md) for its predecessor.

## Source and diagnostic contract

Each immutable source has a file ID, display path, original UTF-8 bytes and
line-start index. Spans are half-open byte ranges `[start, end)` with that file
ID, including empty EOF spans. Internal span constructors check bounds and UTF-8
boundaries. Offsets are stored as `usize`; source size is bounded before parsing.

Line/column positions are one-based Unicode scalar counts, not display-cell or
LSP UTF-16 counts. LF starts a line, including in CRLF; the CR counts as a scalar
on the preceding line. Tabs and combining marks each count as one scalar. No
source normalization or macro rewriting changes reported offsets.

Text errors go to stderr, with code/stage, primary location, optional secondary
labels and notes. Successful text checking writes a check-only summary to stdout.
JSON mode writes newline-delimited records only to stdout and leaves stderr
empty for ordinary frontend errors:

- Diagnostic: `schema_version: 1`, `edition: "typed-preview"`,
  `kind: "diagnostic"`, `severity: "error"`, `code`, `stage`, `message`,
  `primary`, `secondary`, `notes`
- A location contains `file_id`, `path`, `start`, `end`, `line`, `column`,
  `end_line`, `end_column`. Primary is null for CLI/read/UTF-8/artifact failures
- Secondary labels have `{ "span": location, "message": string }`
- Terminal record: `kind: "check-summary"`, the same schema/edition fields,
  `success`, `errors` (emitted count), and `functions` (count on success, otherwise null)

The JSON envelope is provisional version 1. It is also used for malformed CLI
options when JSON was selected before the separator/script-payload boundary.
It does not imply the requested edition was valid. JSON strings escape all C0
control characters, quotes and backslashes. Text rendering escapes control
characters instead of emitting source-controlled terminal commands.

| Code | Meaning |
| --- | --- |
| E0001 | Invalid CLI options or unavailable operation |
| E0002 | Source file read failure |
| E0003 | Invalid UTF-8 source |
| E0004 | Legacy OXBC input unavailable |
| E0100 | Invalid or incomplete lexical/syntax input |
| E0101 | Recognized unsupported preview syntax |
| E0200 | Unresolved local or direct function name |
| E0201 | Duplicate binding or unsupported shadowing |
| E0202 | Unknown type |
| E0203 | Exact decimal literal outside i32 range (resolve stage) |
| E0204 | Loop transfer outside a while in the same function (resolve stage) |
| E0300 | Binding, argument, scalar/logical operand, condition or return type mismatch |
| E0301 | Call arity mismatch |
| E0302 | Missing explicit terminal return |
| E0303 | Statement after terminal return or loop control transfer |
| E0304 | Mutation or exclusive borrowing of an immutable binding |
| E0305 | Unknown field, field projection on a non-record, or indexing/length on a binding that is neither an array nor a slice reference (type stage) |
| E0310 | Owned value unavailable on a reaching path (ownership stage) |
| E0311 | Access conflicts with an active loan (ownership stage) |
| E0312 | Unsupported reference value use or forwarding form (type stage) |
| E0313 | Shared reference does not permit exclusive access (ownership stage) |
| E0400 | Frontend/lowering resource limit |
| E0500 | Internal OIR lowering/verification/execution invariant failure |
| E0600 | Missing or invalid zero-argument main for run |
| E0601 | Execution fuel exhausted |
| E0602 | Live call-frame limit exceeded |
| E0603 | Live local-slot limit exceeded |
| E0604 | Checked i32 arithmetic overflow at its operator |
| E0607 | Checked i32 division or remainder by zero at its operator |
| E0605 | Owned execution-plan, expanded-cell, requested-byte or allocation limit (oir-owned-run stage) |
| E0606 | Signed array/slice index out of bounds at the complete access or store target (oir-owned-run stage) |
| E0608 | Unsupported bounded I/O or Process execution host, before activation; see the distinct entry/terminal policies above |
| E0609 | Stdout-function inventory requires Process entry (oir-owned-run stage) |

For check, exit 0 means successful type checking, lowering and OIR verification of this
subset; for run it additionally means a bool/i32/unit result (including false, zero and negatives); ordinary source/CLI/resource failures still exit 1. Scalar lowering budget errors use E0400 with stage `oir-lower`; owned source
lowering/verification resource errors use E0400 with stage `oir-owned-lower` or
`oir-owned-verify`. Detected OIR invariant failures use E0500,
explicitly say `internal compiler error`, use the relevant lowering, verification or execution stage,
and exit 2. They retain the same JSON envelope with unsuccessful summary and
`functions: null` for check, or `result: null` for run. An invalid OIR origin is omitted (`primary: null`) rather than
passed to the asserting renderer. Only the first deterministic IR failure is
reported. The compiler does not catch arbitrary panics: earlier producer-invariant
assertions, host allocation failures and broken output pipes remain host-process
failures, not ordinary source type errors.


Owned source failures preserve resolve → type → OIR shape → ownership order.
Unknown/duplicate literal fields are E0200/E0201 at resolution; missing fields
are E0300 at the literal; projected-field lookup is E0305 during typing. E0310,
E0311 and E0313 are derived from authoritative verifier denial context, including
the operation, access role, subject and state. A bad internal temporary, cleanup
event or malformed IR stays E0500; a source span alone does not make it a user
ownership error. Primary locations identify the rejected operation and related
labels identify a valid causal move/loan and declaration. When several independent
owner roots fail, the selected complete diagnostic is deterministic; it need not
be the earliest failure across all roots in textual order.

New owned diagnostic builders retain at most 1,024 message bytes, two 256-byte
labels and two 256-byte notes: at most 2,048 text bytes per diagnostic and 204,800
at the existing 100-diagnostic cap. Names retain at most 64 bytes including any
`...` suffix, cut at a UTF-8 boundary. Missing-field messages show at most the
first eight names in declaration order plus the omitted count. These bounds
apply only to new owned diagnostic text. They exclude preexisting shared
lexer/parser formatting, vector/String capacities, headers, allocator overhead,
rendered JSON/human output, path escaping and total RSS. They are not a bound on
all diagnostic bytes emitted by the frontend.

## Bounded explicit run

The complete file must pass the same compiler/verifier path before execution,
including statically erroneous unchosen branches. Run then requires a declared
`fn main() -> bool`, `fn main() -> i32` or `fn main() -> ()`. Missing main is E0600 without a location;
main parameters produce E0600 at the main name, and an owned result also fails E0600. Checking itself has no entry
requirement. The compiler carries main's resolved DefId; the runner inspects the
verified signature instead of reconstructing names from OIR spans. On unsupported
hosts, a program importing `read_stdin` reports E0608 before these entry checks
or any activation, even when the import is unused.

In default Result mode, text success stdout is exactly `true\n`, `false\n`, `()\n` or a canonical
signed decimal i32 followed by newline; each exits 0. Numeric results are never
used as process exit codes.
Failure writes no partial result. JSON replaces the check-summary with one
`run-summary`: the same schema/edition fields, `success`, `errors`, and `result`
containing `{ "type": "bool", "value": true|false }`, `{ "type": "unit" }`,
`{ "type": "i32", "value": -2147483648 }` (an exactly serialized JSON integer),
or null on failure. No successful check record precedes a failed run. Errors while
validating global edition/format options retain the existing check-summary;
once valid global options select explicit typed run, command-option/operand,
compile, entry and execution errors use run-summary. Check records are unchanged.

In scalar-only modules, each activation has isolated optional bool/i32/unit value and place arrays.
Initialize executes a place's unique static declaration and resets its reused
storage on reexecution; Store requires an initialized place and Load snapshots it
into a unique static SSA value. Nonparameter definitions can execute repeatedly;
parameters remain immutable. A merge reads its input before replacing its result. A failed RHS performs no store. Assign reads before
writing; Branch executes exactly one arm; Goto uses its explicit target.
Calls copy arguments in recorded order and suspend callers until normal return,
then replace that static call result's current value and resume at the explicit continuation. Bare
returns contain explicit unit. Discarded calls still run. There is no folding,
tail-call elimination, memoization or execution of an unchosen arm. Fresh calls
and repeated invocations share no mutable execution state. The activation stack
is iterative; the host call stack does not track source recursion.

For scalar-only modules the execution ceilings are 1,000,000 fuel units, 1,024
live frames and 200,000 live slots. The scalar costs and examples in this section
apply to that route; they must not be reused as owned-module fuel totals. An i32 counts as one slot, with the same instruction costs as bool/unit.
These are slot counts, not bytes; host scalar sizes are measured in the
[i32 validation report](../docs/architecture/i32-literal-validation.md).
Main counts as a frame and every activation counts its complete value+place tables,
including places declared in unchosen arms. Root allocation costs 1 + combined
slot count. Assign/Initialize/Store/Branch/Goto/Return cost 1 each; a Load is one
assignment. `let mut x = 1; x = 2; return x;` has four slots and eleven fuel;
a unary/arithmetic/comparison assignment or block-entry bool merge costs one
(in addition to operand evaluation), charged before reading any operand or
checking overflow. A merge reads only its selected incoming operand and needs
no extra slot or variable-sized scratch. Short-circuiting skips RHS instruction
and call costs, but whole-function allocation includes its temporary slots.
Ungrouped `return !true;` costs 6; `return false && true;` / `return true || false;`
cost 8; `return true && false;` / `return false || true;` cost 10.
For example, `return 1 + 2;`, `return 1 < 2;` and `return true == false;` each
cost exactly eight including root allocation/return;
Call costs 1 + argument count + callee combined value/place slot count. Costs are charged before work.
Checked cost/fuel, frame count and live-slot count are checked in that order,
before allocation. Returning releases the callee's slots. Argument scratch is
bounded by 256 scalar entries; frame-header capacity by the fixed frame cap.

E0601–E0603 point to the next unperformed operation, or main's name for root
allocation. Counter overflow/invariant failure is E0500, distinct from resource
exhaustion. No entry override, program arguments or budget flags are exposed.
Fuel measures deterministic reference work, not wall-clock time, source-level
complexity or a stable profiling ABI. Source reads, compiler work, allocation
success, blocking stdin reads and output-pipe behavior are not bounded by fuel.
This is not an OS sandbox. Full details are in [RFC 0004](../rfcs/0004-bounded-reference-execution.md).

## Owned verification, execution and accounting

Owned source lowering is a producer, never a proof. It emits explicit owner
storage, construction/transfer/replacement/discard, field/index/length operations
and ordered call/loan events. The independent raw verifier checks declarations, scalar CFG
shape/dominance, availability and exact call/loan regions before constructing the
sealed immutable owned witness. Reference and native consumers require that
same witness through an immutable witness-bound plan. No mutable raw program,
standalone plan or source-side acceptance summary can authorize execution.

For each function, let S count scalar locals and mutable places, A all call
argument descriptors, O all owner slots, R incoming references, L loans, C call
sites, and P the sum of each owner's checked recursive width: scalar leaves
count 1, records use `max(1, sum(field widths))`, fixed arrays use `max(1, N)`,
and enums use 2. This includes zero-length and unit arrays. B is the
checked aligned owner arena, including parameters, locals, expression temporaries,
argument staging, call results and inter-owner padding. Let T be 1024 scratch
bytes for each canonical input or output builtin and 0 otherwise; it reserves
private scratch in the same payload allocation, outside all owner extents.
All declared storage is counted, including unused/skipped work. On the qualified
x86_64 representation:

```text
X = S + A + P + 4O + 8R + 12L + 2C
Xphysical = X + 2(R+L)
Dref = 8(S+A) + B + T + 32O + 80R + 112L + 16C
```

X is the logical activation-fuel count. Xphysical includes the existing
reference/loan view metadata and remains the reference expanded-cell admission
count; the metadata and builtin scratch do not add logical activation fuel.

Native admission separately uses whole-program `I = sum(S + A + O + R + L + C)`
and `W = sum(P)`, each at most 8,192, including all argument positions, unused
functions and untaken branches. [RFC 0027](../rfcs/0027-native-admission-inventories.md)
replaces only the native aggregate/live Xphysical gates with these inventories.
This intentionally broadens admission without allocation optimization or changes
to reference storage, logical fuel, LLVM or ABI. The native 1 MiB aggregate/live
explicit-byte checks still include canonical builtin scratch and the conditional
8-byte wrapper fuel cell; all other caps remain.

Reference execution retains 1,000,000 fuel, 1,024 live frames and 200,000 live
scalar slots, and additionally caps live Xphysical at 200,000 and requested runtime
storage at 16 MiB. Requested bytes include reserved frame-header capacity,
active Dref and one 8-byte scalar scratch cell. On the qualified representation
each header is 272 bytes; these formulas are not portable layout promises.
Execution-plan metadata is separately capped at 32 MiB. Owned return storage is
already reserved in the caller's owner arena; no extra variable return scratch
is hidden. Loops reuse activation storage with checked owner generations.

The owned ledger charges before work. Let w be the affected owner's width,
as defined above (2 for an enum), and r be the call's number of borrowed arguments:

| Event | Fuel |
| --- | ---: |
| Root activation | `1 + X(root)` |
| Scalar statement/merge, Branch/Goto, StorageLive, field read/write, scalar/borrow preparation | `1` |
| Complete record/array/enum construction, move-initialize, discard, StorageEnd, owned preparation | `1 + w` |
| Each enum match dispatch / selected arm consumption | `1` / `1 + w` |
| Array/slice index read/write, after all operands | `1` before bounds and load/store |
| Array/slice length | `1` before consumer validation and result |
| Bounded stdin core, capacity C and A attempted reads | `4 + C + A`; result construction and commit prepaid |
| Bounded stdout core, capacity C and A attempted writes | `4 + C + A`; validation/staging and infallible result commit prepaid |
| Whole replacement | `1 + 2w` |
| OpenCall | `1 + owned_argument_count` |
| Invoke | `1 + argc + X(callee) + sum(owned_argument_widths) + r(r-1)/2` |
| Scalar return | `1 + P + L + C + R` |
| Owned return | `scalar_return_charge + w(returned_owner)` |

Array literal operands retain their own left-to-right evaluation costs before
construction. Indexed writes evaluate the complete RHS before the index;
operand failure skips the final access charge. Insufficient access fuel wins
before bounds failure and performs no final access. Length still requires a
statically available/readable base. A fixed array has constant N; a borrowed
slice gets its length from the checked backing owner without copying elements.

Return's R term accounts for normal-edge loan release. Explicit lexical storage
ends and frame teardown are distinct charged events, even for moved owners.
Source lowering may introduce several of these events for one expression; a
source move is not one fuel unit. Owned groups forward their result owner without
another owner/event; owned name expressions move through explicit temporaries.
Scalar expressions in owned modules still preserve scalar snapshots and ordered
checked arithmetic, but whole-module calls/storage use the owned ledger.

Reference activation admission checks fuel, frames, scalar slots, expanded cells,
bytes, then allocation. An exhausted charge performs no part of the operation. Earlier
effects remain; abrupt failure promises no rollback, normal-return loan release,
destructor execution or unwinding. Root errors retain entry origins and call
activation errors retain Invoke origins. Existing scalar overflow retains its
operator span.

## Resource and trust bounds

| Resource | Current maximum |
| --- | --- |
| Source bytes | 1,048,576; reader stops after maximum + 1 bytes |
| Non-EOF tokens, including trivia | 100,000 |
| Bytes in one token, including whitespace/comment tokens | 65,536 |
| Syntax nodes counted by parser | 100,000 |
| Nested expression parser frames and total expression-tree height | 64 (63 grouping wrappers/operators above a literal) |
| Parameters or call arguments | 256 each |
| Emitted diagnostics | 100 |
| Active statement block frames | 64, counting the function body as frame 1 |
| Scalar OIR combined value/place slots and instructions (including merges) | 100,000 of each, aggregate per program |
| OIR blocks | 300,000, aggregate per program |
| OIR successor edges | At most 600,000, two per block |
| Dominator ancestor cells | At most 5,700,000 usize entries (19 levels) |

All limits are engineering defaults for this experimental subset. Token limits
may be reached before source-size or node limits. Expression recursion and total tree height are bounded
at parsing; this includes flat left-associative chains, so 64 literal terms pass
and 65 terms fail E0400. Height tracking uses one usize per expression, at most
800,000 bytes on a 64-bit host (excluding vector capacity). Statement-block nesting has a separate pre-entry bound. AST/HIR arena
ownership avoids recursive block-drop chains; later block/CFG passes are iterative.

Scalar-only lowering preflights exact aggregate expansion before IR/maps are allocated. For
F functions, C calls, I if statements, W while statements, S logical expressions, P parameters,
L lets (including M mutable), A reassignment statements, X expressions and Rb bare
returns: values = P + (L - M) + X + Rb; places = M; combined slots = P + L + X + Rb;
instructions (including merges/init/store/load) = X - C + L + A + Rb;
blocks <= F + C + 3I + 3W + 2S. These are bounded by the existing parser nodes, with only
the block budget raised to three times that limit. Old accepted source programs
are not excluded by a tighter unrelated cap. Call vectors stay capped at 256.

Verification independently checks these limits for arbitrary private raw IR and
checks graph/scratch arithmetic before allocation. Iterative cyclic dominance
uses O(locals + blocks + edges) scratch and O((blocks + edges) log blocks) graph
work. Peak dominator/predecessor tables use 11B + E + 1 usize cells, 31,200,008
bytes at B=300,000/E=600,000 on a 64-bit host, plus about 2.4MB for at most 100,000
combined optional value-definition/place-initialization records. This excludes
raw IR/source, vector headers and allocator costs; only one function's scratch
is live at once. Full verification also walks functions/slots/instructions/operands.
No host allocation-success, OS-sandbox or blocking-I/O guarantee follows.

Owned source additionally applies the following inclusive preflight limits:

| Resource | Maximum |
| --- | ---: |
| Nominal record and enum declarations combined | 4,096 |
| Fields per record / variants per enum | 1,024 / 256 |
| Record fields and enum variants combined | 65,536 |
| Fixed array length / elements per literal | 1,024 / 1,024 |
| Checked declaration-table payload / sum of padded declaration layouts | 8 MiB / 1 MiB |
| Aggregate scalar value/place plus owner slots | 100,000 |
| Aggregate statements plus merges / expanded ownership events | 100,000 / 100,000 |
| Aggregate blocks | 300,000 |
| Counted ownership-analysis work | 100,000,000 |
| Ownership metadata / peak ownership-flow scratch | 32 MiB / 32 MiB |
| Requested source-produced raw payload | 64 MiB |

Raw verification independently recounts actual nested vectors, rather than
trusting source counts. Expanded events include statements/merges, call argument
descriptors and preparation sites, constructed field operands, every array
constructor operand and complete enum match descriptors/arms. Work is a
checked inventory-based bound, not CPU instructions or a timeout. Declaration,
raw-output, verifier, plan and consumer ledgers are separate admissions.

The source producer counts before reserving raw output or maps. It makes three
count traversals and one emission traversal per function; count-only cleanup
uses a saved lexical-prefix length rather than walking every exited owner. All
new variable output/map reservations are fallible and append/count parity is
checked in release builds. The raw ledger includes each nested payload and
its containing headers once, including diagnostic-origin fields.

On the qualified x86_64 representation, one active function's requested
producer-map payload is `8B + 16N + 40E + 8O` bytes, where B is its block count,
N its binding count, E its expression count and O its owner count (these B/E
symbols are local to this map formula). Conservative independent source maxima
give 8,800,000 bytes; this does not assert all maxima are jointly attainable.
Fixed iterative traversal capacities are 203 expression frames, 268 body frames
and 65 loop targets. Inclusive source nesting needs at most 127 expression
frames, 253 body frames and 63 loop targets. Their measured fixed representation
is 33,872 bytes, not a compiled-machine-stack or total-memory bound.

Source raw/map ledgers exclude source/AST/typed frontend storage, new bounded
diagnostic text and rendering, checked declaration tables, verifier/plan/consumer
storage, allocator overhead or excess capacity, LLVM processes and total RSS.
The raw 300,000-block resource fixture exceeds the source 64 MiB payload ceiling;
it is raw-only evidence, not a promise that a source program reaches that limit.

Errors stop later compiler phases, and diagnostics beyond the cap are omitted.
No total-error-count claim is made when the cap is reached.

This constrains source parsing and forbids user-program effects during checking;
it is not a general OS sandbox or a memory-safety proof. Filesystem read blocking
and host allocation failures remain outside this resource contract.

## Evidence and deferred work

[Validation evidence](../docs/architecture/typed-preview-validation.md) identifies
the exact snapshot and actual host. Predecessor public CLI fixtures are embedded in
`tests/typed_frontend.rs` and `tests/edition_boundary.rs`. The owned Batch pilot
is registered in `scripts/verify_repo.py` for explicit typed check/run with
`--edition=typed-preview`. Other discovered source files retain their legacy
checks. `tests/typed_owned_boundary.rs` covers the public owned entry and early
failure boundaries; actual final counts and results are recorded in the source
qualification ledger. Unit tests inspect source/diagnostic boundaries,
lossless numeric tokens, resolved IDs and complete type tables.

[OIR validation evidence](../docs/architecture/oir-validation.md) records the
straight-line increment, malformed-IR cases, call-order/provenance checks and
resource/long-chain tests. The straight-line and boolean-CFG increments did not add execution. No
ownership/borrow checking, numeric type system, native backend, typed artifact
schema, self-hosting or AI capability is added by the reference runner. [Boolean CFG evidence](../docs/architecture/boolean-cfg-validation.md)
records the later restricted branch/scope/all-path-return increment and its
dominance, graph and resource checks. The subsequent [reference execution evidence](../docs/architecture/reference-execution-validation.md)
records the bounded opt-in runner and its independent source oracle. The later
[i32 literal evidence](../docs/architecture/i32-literal-validation.md) adds exact
decimal constants/copies, strict i32 types and result serialization. Broader
numeric operations remain separate decisions. [Checked i32 arithmetic evidence](../docs/architecture/i32-arithmetic-validation.md)
records the subsequent +/−/* increment and its runtime overflow policy; no later
roadmap capability is implied.

[Scalar comparison evidence](../docs/architecture/scalar-comparison-validation.md)
records the explicit i32/bool comparison table, non-associative grammar and
reference/native/Python checks. No generalized equality or later operator is implied.

[Boolean logic evidence](../docs/architecture/boolean-logic-validation.md) records
short-circuit value joins, exact path costs and reference/native differential checks.

[While evidence](../docs/architecture/while-validation.md) records cyclic verifier, shared fuel and actual LLVM gates.

[Loop-control evidence](../docs/architecture/loop-control-validation.md) records source transfers and precise fallthrough/exit qualification.


[Owned source qualification](../docs/architecture/owned-source-validation.md)
records preactivation versus actual production CLI results, exact compiler/source
identities, resource gates and source-free native execution. The historical
[raw-consumer report](../docs/architecture/owned-consumers-validation.md) remains
evidence for its own raw fixtures and frozen snapshot. Its counts, fuel and
artifact hashes are not source-integration results. Ownership foundations are
one experimental capability; stored-reference lifetimes, partial moves,
field-disjoint loans, heap/drop semantics, unsafe/FFI contracts and complete
static-core or v1.0 qualification remain open.

## Experimental source-scale lexical provider

Explicit `--experimental-lexical-provider BUNDLE` on typed-preview check/run/compile
selects an Oxid-authored ASCII streaming lexer for each genuinely loaded source.
Its validated, source-bound token Vec reaches the real Rust parser after a
separately traced canonical lexical comparison. Process compile/run remain
available; Process Run JSON and combination with HIR import/producer selectors
are rejected. Default routing is unchanged and failures have no fallback.
See the [complete LXI1/LXS1 contract](streaming-lexical-provider.md),
[actual source component](../fixtures/typed-streaming-lexer/README.md) and
[qualification boundary](../docs/architecture/streaming-lexical-provider.md).
