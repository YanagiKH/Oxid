# Bounded stack-code component

This experimental Oxid-written component lowers the existing expression arena
into at most 15 instructions and executes them through a separate code-only
verifier and interpreter. It adds no language syntax, host effect or compiler
limit. It is a small lowering/validation exercise, not a compiler backend or
self-hosting claim.

The instruction record has fixed opcode/operand columns and a used count.
Opcode 1 pushes a nonnegative i32; 2 adds and 3 multiplies the top two values.
Operator operands must be zero. The verifier checks the complete prefix before
arithmetic, including count 1..15, every row, operand availability, and final
stack height one. Invalid counts report -1, invalid rows their index, and a
residual stack the used count. Unused instruction tails are not read.

`execute` holds one immutable Code borrow across verification and execution;
it receives no arena links or cached evaluator result. Arithmetic uses existing
checked i32 operations. The lowerer reuses the arena validator without executing
arithmetic. Invalid arenas preserve every output field. For valid arenas it
sets count to zero before writing rows, preserves the unused tail, and publishes
the final count only after the complete prefix exists. A terminating runtime
failure during copying may leave unpublished rows; this is not full rollback.

For `12 + 3 * (4 + 5)`, exact rows are `(1,12), (1,3), (1,4), (1,5), (2,0),
(3,0), (2,0)`, returning 39. `7*(8+1)` lowers to `(1,7), (1,8), (1,1), (2,0),
(3,0)`, returning 63. `stack_main.ox` is the fixed example; `stack_stdin.ox`
uses the existing 128-byte input boundary and 129-cell capacity witness.

Initial local public check/reference/native admission and the unchanged stdin
ELF's 39/63 executions pass. Exact-row, malformed-code, invalid-arena preservation,
full component acceptance and hosted CI remain separate pending checks.
