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

Local public check/reference/native acceptance passes all 45 independently
authored cases in `stack-cases.json`: ten exact lowering programs, 23 standalone
code programs, and twelve malformed-arena preservation cases. The separate
stdin runner passes its 28 cases through one unchanged ELF, including 39/63,
capacity and checked overflow. All 45 existing arena cases still pass.

```sh
python3 -B scripts/verify_stack_component.py --oxid target/release/oxid \
  --output /tmp/stack-component --native
python3 -B scripts/verify_expression_stdin.py --oxid target/release/oxid \
  --output /tmp/stack-stdin --native --component stack
```

Output directories must be new. These controllers preserve literal expectations,
generated inputs, commands, streams, statuses, and source/compiler/ELF hashes.
Standalone malformed code imports only the code module. The native programs run
with an empty working directory and cleared environment, not filesystem isolation.
CI runs both groups with its pinned LLVM toolchain. Exact-head hosted acceptance
remains separate from these local results.

The composed stdin ELF contains 30 emitted owned functions; its emitted owned
call graph has maximum depth six. Its Code buffer holds 31 i32 cells, the VM
stack 15, the input buffer 129 and the arena 61. Actual public native admission
passes unchanged compiler limits. These are bounded component facts, not a
general memory or performance claim. The first test generator inlined enough
assertions to exceed the native 256-slot per-function cap; literal initialization
and a shared row-check helper keep the same complete expectations within that cap.
