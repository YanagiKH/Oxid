# Staged scalar parser

The native inventory successor in RFC 0027 is qualified. The unchanged parked
parser at `8f1fe203` passed ordinary native compilation and 54 retained
reference/native cases, plus 41 separate strict-decoder controls. These comprise
35 canonical comparisons, 11 domain refusals and eight pending grammar cases.
The full scalar grammar remains unfinished; no provider activation is included.

## Expression continuations

Source checkpoint `c9a5d733` and strict projection checkpoint `00545401` implement
Group/Unit, signed and general prefixes, arithmetic, comparisons and logical
operators. Ordinary native admission passes with I = 5,258, W = 2,571,
76 functions, 1,281 blocks and 53,776 explicit bytes including the wrapper.
The existing arrays and State layout are unchanged.

Paired reference/native replay passes 87 inputs: 68 complete canonical AST or
first-diagnostic comparisons, 15 site refusals and four explicit pending cases
per mode. The 41 retained corruption controls pass in each mode; 11 authored
decoder test methods cover the new row families. Calls, let, if and while remain
pending at this checkpoint. Bare assignment and match still use unfinished
generic-error paths and are not claimed as contract-complete. Calls are the next
increment.

## Historical initial control checkpoint

The canonical parser observer copies unchanged Rust sources and projects full
scalar AST facts or the first diagnostic. Its standalone build and focused
observer checks are separate from the Oxid implementation below.

The initial Oxid dispatcher covers function headers, parameters, named/unit type
syntax, simple blocks, return/loop-transfer/expression statements and expression
atoms. Planned grammar that is not wired yet has an explicit stage-pending result
(error tag 5), never a claim of canonical rejection. Full agreed scalar grammar
and native admission remain required before this component can be qualified.

The first integration attempt failed on unsupported else-if source shorthand;
the next reached two name-shadowing errors. Both results are retained. This
checkpoint has no successful parser native execution or AST comparison claim.
The primitive storage/controller interfaces and partial implementation are saved
for recovery before those narrow corrections and the next admission measurement.

After the two naming corrections, the first real dispatcher reaches native
admission but is rejected at 12559 expanded cells against 8192. Additional loan
and reference inventories account for 4020 of its 5756-cell increase over the
synthetic carrier. Further grammar expansion is paused; no limit is relaxed.

The independent decoder `scripts/parser_ast_observation.py` checks OPA1 transport,
row ownership and spans, then projects the currently implemented syntax to the
canonical JSON shape. Reference-only validation includes 35 exact AST/diagnostic
comparisons,11 authored family refusals,8 explicit stage-pending cases and 41
malformed observation controls. This does not establish native admission or full
planned grammar coverage. Unimplemented row families are explicitly rejected by
the current projection instead of silently dropping fields.
