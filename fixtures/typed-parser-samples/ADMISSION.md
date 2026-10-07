# Carrier admission experiment

## Current native admission

[RFC 0027](../../rfcs/0027-native-admission-inventories.md) replaces the native
reference-expanded-cell gates with independently checked compiler inventories.
The unchanged initial parser at `8f1fe203` has I = 4,646 and W = 2,571, with
48,864 explicit native bytes including the Process wrapper. It passes ordinary
native compilation and its existing 54 reference/native cases. The remaining
inventory headroom is 3,546 items and 5,621 width cells; every later grammar
increment must still pass all native gates. This is intentional admission
broadening, not allocation reduction. Reference storage and logical fuel remain
unchanged.

The experiments below preserve the earlier representation and X-limit results.
Their 1,200-cell engineering target was superseded by the explicit inventory
contract; it is not an additional current parser limit. Full grammar, exact
canonical projection and independent decoder validation remain unfinished.

## First expression increment

At `c9a5d733`, ordinary native admission passes with I = 5,258 and W = 2,571,
76 functions, 1,281 blocks and 53,776 explicit bytes including the Process
wrapper. Compared with the initial parser this adds 612 inventory items and no
owner width. Remaining I/W headroom is 2,934/5,621. Exact syntax/first-diagnostic
parity is recorded by [the stage report](STAGE.md); this does not qualify the
unimplemented statement/call grammar.

## Call increment

At `40898984`, ordinary native admission passes with I = 5,660, W = 2,571,
80 functions, 1,343 blocks and 56,964 explicit bytes including the wrapper.
Calls add 402 inventory items and no owner width. Remaining I/W headroom is
2,532/5,621; all other native gates still apply.

## Binding increment

At `6aff6597`, ordinary native admission passes with I = 6,027, W = 2,571,
83 functions, 1,420 blocks and 59,864 explicit bytes including the wrapper.
Bindings add 367 items and no owner width; remaining I/W headroom is 2,165/5,621.

## Historical carrier experiments

The first probe shares the existing lexer modules and source bound. Its entry is
`../typed-lexer-samples/parser_admission.ox`; it constructs synthetic full-width
banks, checks push/resume/pop and duplicate/limit behavior, poisons an unused row,
and serializes checked column values. It does not parse the scalar grammar.

The initial seven-column AST and three-column continuation representation was
rejected by the current native consumer with E0700: aggregate expanded cells
exceeded 8192. Typing passed before that gate. No ELF execution or viable parser
admission is claimed for this initial layout. Existing compiler limits remain
unchanged. Constructor/initializer/binding copies make the raw resident payload
sum insufficient to predict this admission cost.

This checkpoint preserves the failed candidate before any layout change. A
successor may compact finite integer fields without reducing grammar, row count,
source bound or continuation capacity, but needs its own actual admission and
round-trip/control evidence. The full parser remains unimplemented.

Two lossless successors also reached the same E0700 aggregate-cell gate:

- Six AST columns and two continuation columns combined kind/span and state/aux
- Three AST columns and one continuation column further paired bounded fields

The current three-column row stores header_links, ab and cd. Its encodings are
`kind + 64*(start + 256*end) + 4194304*next`, `a + 256*b`, and `c + 256*d`.
With kind 1..63, offsets 0..128 and start<=end, and IDs 0..128, the first value is
at most 538976319 (<2^30); pairs are at most 32896. Non-EOF successful syntax token
references fit 0..128; diagnostic EOF token 129 stays outside these row fields.

A continuation is `node*65536 + state + 32*aux`, at most 8452117 (<2^24).
State 1..21 and aux depth/context/precedence components are checked before packing;
aux's holes with depth>64 are rejected. Root alone has node 0; pushed rows have
node 1..128. All unused physical row/frame slots are zero. Probe serialization
uses four separate byte planes and never computes 256^4 in i32.

Independent arithmetic review confirms reversibility within these checked
domains. That does not imply native admission: all three candidates were rejected
under the unchanged compiler. Broad parser work is stopped pending attribution of
the actual aggregate accounting. A future diagnostic observer must preserve the
same admission refusal and remain separate from production compiler authority.

## Separate array owners: admitted carrier, limited headroom

The same three packed AST columns and single continuation column now live in
separate array locals, with only row/stack counts in a two-scalar record. Helpers
receive borrowed column views. This removes constructor staging copies while
preserving every logical field and the full 128-row/129-frame capacities.

An isolated diagnostic observer of the unchanged admission calculation measured
8385 expanded cells for the prior nominal-bank candidate and 7845 for the separate
arrays:347 cells remain under 8192. Owner payload decreased by 1032 cells; extra
views/arguments, helper calls and stricter frame decoding offset 492 of that
saving. The observer changed only logging before the original limit; production
compiler limits and authority were unchanged. The original compiler also admits
and builds the separate-array probe.

Six input/control scenarios pass in both reference and native execution. The
successful cases fill all 128 rows, exercise maximum stack depth and in-place
resumptions, reject duplicate/full/root-pop operations, detect an unused-row
poison, and produce exactly 1553 independently checked bytes. These are synthetic
storage/control results, not AST parsing or a complete parser-control budget.

347 cells of headroom is not sufficient evidence for broad parser implementation.
The next proposed reuse boundary is a lexer core operating on caller-owned token
columns, preserving the existing Tape API through a wrapper. It can avoid the
1197-cell tape constructor in a parser executable. This requires separate review,
lexical parity and actual admission before claiming additional headroom. No
compiler lifetime/accounting change or reduced grammar is part of this probe.

## Shared column lexer core

The lexer now has one column-view core. Existing `lexer::scan` and Tape remain
compatible through temporary columns and scalar copy-back. The direct projected
adapter was rejected by E0311: current borrowing locks a whole record even when
its fields are disjoint. That rule and its existing regression remain unchanged.
The parser carrier owns its token columns directly and does not load the Tape
constructor or compatibility wrapper into its executable.

The complete compatibility executable measures 6446 expanded cells (1746 free).
The complete direct-column carrier measures 6803 (1389 free), saving 1042 versus
the preceding carrier. Removing tape::new_tape saves 1197; new core/view costs add
155. These whole-inventory measurements retain the same 8192 gate. The original
compiler admits both programs, and the integrated existing 400 lexer observations
and 12 synthetic carrier observations pass in reference/native execution.

The carrier controller is reproducible with:

```sh
python3 scripts/verify_parser_carrier_admission.py --oxid target/debug/oxid \
  --output /tmp/parser-carrier-proof --native
```

This remains an admission checkpoint. The 1389-cell remainder is actual headroom,
not a measurement of the unwritten parser. A practical next implementation budget
is at most 1200 net additional expanded cells for the first complete control and
diagnostic handlers, leaving 189 as a reserve. That is an engineering target, not
a proof they fit; every coherent parser stage must still pass the original
native admission. The synthetic fill/check driver can be replaced by real parser
logic, but its existing charge must not be counted as free until the measured
whole inventory actually removes it. No broad parser implementation is included.
