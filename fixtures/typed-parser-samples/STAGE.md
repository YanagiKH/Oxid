# Early control implementation checkpoint

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
