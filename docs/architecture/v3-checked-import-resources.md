# Experimental V3 checked import: implementation resource ledger

The experimental host supports explicit V3 producer bundles and supplied
checked-HIR observations. This ledger describes their implementation resource
accounting; it does not establish source-bound compiler qualification. Historical
proof files and compiler-source bindings remain unchanged. The existing
main/data-only binding does not qualify these frontend changes; a separately
named frontend source generation and fresh execution qualification are required.

## Closed selection and representation

Validated OPA3 magic selects the u16 endpoint leaf before constructing its banks.
V1/V2 select u8. Invalid magic follows the legacy refusal/preflight ordering and
never reaches a successful Wire. Wire and BoundObservation contain no protocol
field or larger enum. Wire derives protocol from validated bytes. Row IDs,
function/local/token references remain u8; only spans and optional endpoint events
are generic. There is no largest-variant carrier. Each pre-existing named-role
array keeps its component order, shape and multiplicities; V3 additional arrays
are separately summed including both complete array transports and iterator.

V3 success is1304-byte OPA3 plus1052-byte STF3=2356; diagnostic is1304+20=1324.
OPA has14-byte header and five129-cell little-endian u16 plane columns: kind/next,
start,end,ab,cd. STF header20 and two129-cell i32 columns. Active raw cells are
validated before masking/narrowing, including next<=rows, byte-sized ab/cd<=128,
start<=end<=actual source length<=511. Every inactive OPA/STF cell is zero.
Mixed versions, high endpoint/header bits, truncation and trailing bytes reject.
The live static result must exactly echo the parser OPA3 prefix.

The source remains one root ASCII file<=511 bytes,128 non-EOF tokens including
trivia,128 rows,64 depth. No compiler ceiling increases. Genuine Rust parsing,
resolution, checking, lowering, source association and OIR verification remain
mandatory. Supplied diagnostics are not successful imports; live diagnostics
match the exact first canonical error and return its complete original vector.

## Source work

Retain the original seven-term formula unchanged for V1/V2:
173+2B+30C+5E+T*(3*(B+1)+258)+R*(B+80)+R,
with C=T=129,R=128,E=16R+1. V1=124501,V2=190160.
B511 alone gives322512 but does not pay V3 decoding.

V3 adds135912, for total458424:
- 24*(5C+15R): each five-column decoder read and at most three comparator
  Row::read calls per row, with five raw wide_cell calls each. The24-unit cell
  allowance includes protocol/bounds checks, fixed offset arithmetic, two
  checked byte reads, assembly and returns.
- 64*(C+3R): raw kind/next/span/reference/tail validation and row construction,
  including checked endpoint conversion. No narrowing precedes raw validation.
- 16E: up to two checked token endpoints per event, eight units each.
- 32*(C+R): two typed columns,16 units of version/start dispatch per read.
- 512: fixed headers, first/source-length projection, version dispatch,
  diagnostic four-u16 assembly and checked supplemental formula arithmetic.

These are conservative logical visits, not elapsed time or machine instructions.
The full source prepayment precedes source/row walks and candidate allocation.
The diagnostic plan adds unchanged CHECK_WORK and MATCH_WORK; longer source
streaming is covered by source B511, and the four raw endpoints by the fixed512.

## Repeated candidate reads: independent supplemental work

The legacy builder formula512+256*15*(rows+1) and typed formula
1024+256*8*(rows+1) are retained exactly. V3 must not absorb a five-wide-cell
read into unproved slack of the256-unit legacy nested-call allowance.

Each wide row read is charged184=5*24+64. A typed-column version/start selection
is16. The builder adds512+(rows+1)*(20*184+16*16). Typed comparison adds
256+(rows+1)*(4*184+3*16). The same typed total is used by preflight WorkPlan
and the comparison itself. The combined candidate/helper/checker work plan is
admitted and debited before the first candidate reserve.

Global bounds, over all functions together:
- complete mapping validation: window row scans R plus at most2R direct/nested
  reads, totaling3R;
- signatures: function+result-type reads2F, parameter+type reads2P, <=2R;
- local construction: window R, row scan R, annotation/declaration<=2R, <=4R;
- block/statement construction: row scan R, block reread B, up to3S statement
  reads (If rereads two target blocks). B+3S<=3R, thus<=4R;
- expression mapping<=R; final function body<=2F<=2R.
Total<=16R, conservatively charged20(R+1). Typed-resolution helper chains
are<=12R, conservatively charged16(R+1).

Typed comparison uses window R, active row pass<=2R, postorder expression R,
thus4(R+1). Typed word reads: window positive<=R, declaration positive<=R,
compare_cell across nonexpression and expression rows combined exactlyR,
thus3(R+1).

These are not per-function maxima multiplied by function count. Source AST
comparison enforces unique sequential Allocate events and consumes all rows.
Function rows are source ordered, so window row intervals partition all rows.
The next expression_begin equals previous window.end; total expression scanning
is E plus at mostF lookaheads per window pass, not F*E. Near-cap genuine fixtures
exercise10 functions/40 rows and109 rows; synthetic128/129-row count/tail
mutations are rejection controls, not claims of grammar reachability.

## New complete carriers

Existing role arrays and all their sum transports stay unchanged for legacy.
Each generic bank instantiates its actual E, never a largest variant. V3 adds:
- leaf outer:11 rows for decoder inputs/results/five raw values/two-byte assembly,
  scalar/formula locals, concrete request/denial dispatch arguments/results and
  source-plan results;
- AST reader:6 rows for five-u16 locals, checked u16 results, cell input tuple,
  assembly bytes, endpoint input/results and predicates;
- builder formula:4 rows for input tuple, four-u64 intermediates, complete
  result and predicates; typed comparator independently pays the same4 rows;
- diagnostic:6 rows for four wide spans, five byte metadata fields, two-byte
  assemblies, exact match/execute inputs and full rejection transport.
Every bank includes complete construction/move/iteration transport. They are
logical enclosing-carrier sums, not physical stack-frame/RSS bounds.
Canonical/inherited compiler storage keeps its existing accounting.

Emit uses a separate V3 connection prepayment36864=32768+4096 before bank
calculations. The new finite allowance is3808:25 rows*32=800; four bank
prologue/epilogues*128=512; four enclosing branch/add handlers*128=512;
at most17 source-formula checked sites*32=544; two scalar supplemental
formulas at most6 sites each*32=384; eight dispatch/extraction/receipt
handlers*128=1024; native diagnostic selector32. Sum3808<=4096.
No source-work allowance is borrowed to pay pre-source bank construction.
The connection and formula calculations are implemented in
`src/frontend/oir/native_emit_cost.rs` and the checked-import leaf. The legacy
32768 connection allowance, 4096 formula allowance and native text bytes remain
unchanged. Fixed V3 location rendering bound16384 covers source<=511, including
three-digit line/column positions<=512; this changes the prepaid worst-case work,
not the actual human diagnostic rendering.

## Acceptance evidence

Tests exercise Verify/Run/Emit complete-pipeline retained/scratch/work exact and
minus-one thresholds, source preflight refusal before any candidate reserve,
Emit connection failure preserving the original meter prefix, and native
formula/body exact/minus-one controls. Live diagnostic tests separately test
prepaid and canonical admission thresholds, canonical vector identity, all
header/payload mutations and supplied-diagnostic refusal. These are experimental
controls; source-bound compiler/bundle qualification remains a separate gate.

## Historical producer-capacity failures and current finite scope

Earlier V3 producer revisions exhausted the unchanged static fuel cap on a
valid padded 511-byte, 113-row if source. A 110-row boolean source could emit
all 2356 bytes and then exit with E0601. A complete stdout frame never overrides
a failed producer exit: the live route must reject both outcomes. These are
historical failures, not successful full-capacity evidence.

Subsequent producer optimizations and exact-name comparison corrections passed
the retained 124-case corpus and the 64-case residual packet in local debug and
release host qualification. The latter comprises 62 legal successes and two
canonical-invalid refusals. The packet preserves earlier common-prefix-name
failures and strict/advisory reference expectations. The current static closure
uses all 8192 admitted instructions, with zero instruction headroom; the parser
uses 6847. The worst measured retained stress fuel is 989408 of 1000000.

These finite observations establish neither a worst-case fuel guarantee for
every admitted input nor qualification of a future integrated source checkpoint.
Retain the source, instruction, work, token, row and depth limits unchanged.
Successful observations require both producers to exit zero with empty stderr,
exact framing and exact source/executable/invocation bindings. Canonical user
program runtime failures remain distinct from producer failures.

## Producer error-order compatibility revision

Non-ASCII input and source lengths above511 reject before any bundle access,
using the exact historical255-byte source error. That message is deliberately
the unadmitted/unknown-version error, not a claim that validV3 capacity is255.
For bounded ASCII256..511, only a fully validated immutableV3 bundle grants
wide capacity. Failed bundle admission or a loadedV1/V2 bundle yields the same
legacy source error and starts neither producer. ASCII<=255 keeps original
bundle errors. There is no manifest peek, loader bypass or execution fallback.

This preserves error priority and no-spawn behavior, not identical filesystem
I/O: ASCII256..511 legacy/invalid bundles now undergo the existing bounded
load/hash/snapshot admission needed to establish explicitV3. Those existing
filesystem costs remain outside the private compiler-work tariff; no zero-I/O
contract is asserted. Immutable descriptors, hash checks, seals and workspace
cleanup are unchanged. Tests include absent/malformed/symlink/hash-badV1/V2/V3
bundles and valid legacy snapshots, with no process records on refusal.
