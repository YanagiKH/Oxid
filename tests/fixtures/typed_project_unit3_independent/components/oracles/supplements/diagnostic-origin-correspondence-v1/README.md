# Exact stored diagnostic metadata for the finite origin mutations

This source-only supplement supplies the exact-origin comparison already
required by the frozen Unit3 design section 9 and mutation expectations. It
does not change any source result, denial code, mutation or expected authority.

The authored negative-exclusive_then_read fixture retained its complete source
AST origin map but no tagged body facts. The mutation comparator checked rendered
diagnostics only. E0311's selected loan/declaration notes do not necessarily use
the failing ReadField's stored cause, so that comparison missed a wrong cause.
The original mismatch and comparator are preserved separately.

build.py reconstructs the original independent AST using the frozen generator
and verifies every source byte, every frozen AST origin and the complete
module-to-model bijection. The read is the second argument of a call in a return
statement. The established predecessor lowerer passes the enclosing statement
span through nested expression lowering and records the field expression as
primary. These source facts determine both metadata spans without an observation.

The resulting obligation requires the actual ReadField at the independently
identified function/expression site to carry those exact primary/cause origins.
The unmodified baseline must satisfy it. Same-file primary and cause replacements
must fail it, even when the rendered E0311 bytes happen to remain unchanged.
The identical-clone generic verifier must still retain its denial kind.

Candidate raw snapshots, receipts and diagnostic streams are excluded from the
builder's inputs. The original frozen source/model/request packages remain
unchanged. This finite obligation is not a claim of new body-model coverage for
all authored negative controls.
