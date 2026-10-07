# Early control implementation checkpoint

The canonical parser observer copies unchanged Rust sources and projects full
scalar AST facts or the first diagnostic. Its standalone build and focused
observer checks are separate from the Oxid implementation below.

The initial Oxid dispatcher covers function headers, parameters, named/unit type
syntax, simple blocks, return/loop-transfer/expression statements and expression
atoms. Planned grammar that is not wired yet has an explicit stage-pending result
(error tag5), never a claim of canonical rejection. Full agreed scalar grammar
and native admission remain required before this component can be qualified.

The first integration attempt failed on unsupported else-if source shorthand;
the next reached two name-shadowing errors. Both results are retained. This
checkpoint has no successful parser native execution or AST comparison claim.
The primitive storage/controller interfaces and partial implementation are saved
for recovery before those narrow corrections and the next admission measurement.
