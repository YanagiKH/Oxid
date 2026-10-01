# RFC 0013: unlabeled while transfers

Status: experimental implementation extending RFC 0012. This closes a bounded
source-control-flow gap; it does not complete M1/M2, ownership, termination or
production safety. Legacy syntax/runtime and OXBC serialization are unchanged.

## Source contract

`break;` and `continue;` are statements with mandatory semicolons. Both target the
nearest lexically enclosing while in the same function. Break leaves that loop
without evaluating its condition again. Continue starts the next evaluation of
the complete condition, including calls and short-circuit expressions. Neither
accepts a value, label or expression position. No loop/for expression, never type,
constant-condition return inference, new scalar type or reference is introduced.

Conditions, bodies, unused functions and unchosen arms remain fully resolved and
checked. A called function cannot transfer control to a caller's loop. A loop's
false edge remains conservative, including literal true; functions still require
the existing explicit terminal return. Body-local declarations retain their
lexical scope and fresh dynamic initialization, and outer mutable state persists.

A statement after an unconditional return, break or continue is rejected. So is a
statement after an if whose two arms cannot fall through, including mixed
return/break, return/continue and break/continue outcomes. Missing else supplies a
fallthrough path. A nested while consumes its own break/continue outcomes rather
than terminating the enclosing body. No unreachable code is silently discarded.

## Representation and flow

The lossless token tape recognizes both keywords. AST statements retain their
full source spans, including trivia before the semicolon. Resolution records a
function-local nominal LoopId indexed by the unique while-body block; iterative
enter/leave frames preserve lexical loop context separately from name scopes.
Both contexts are restored before siblings, and loop context starts empty for
each function.

Typed block summaries separately track fallthrough, return, break and continue.
Sequential composition enters the next statement only on fallthrough paths while
preserving prior terminal outcomes. If combines both arm outcome sets; while
consumes its body transfers, preserves returns and adds its conservative false
edge. A complete function summary must contain only return. No scalar `never`
type is inferred. The same fallthrough predicate drives OIR block preflight and
actual if-join allocation, avoiding unreachable synthetic joins.

Lowering resolves each active LoopId to the reserved original condition header
and exit. Break emits an existing Goto to the exit; continue emits a Goto to the
header, not the condition's final call/logical continuation. Release-enforced
producer assertions check loop identity and nearest active targets. The ordinary
OIR verifier still independently checks arbitrary reachable graphs, types,
initialization/dominance and actual predecessor-edge availability. It does not
prove that a structurally valid target matches source intent; dedicated lowering
and source-model tests cover that separate obligation.

A transferred path has no subsequent arm-close or body-close Goto. Transfers add
no value/place slots or blocks. Existing exact preflight and the conservative
block ceiling F + C + 3I + 3W + 2S remain; no new resource cap is added. Traversals
remain iterative with linear bounded block/loop tables and existing depth limits.

## Execution, fuel and native admission

Each executed transfer is one precharged Goto. Its origin is the entire transfer
statement, including its semicolon and internal trivia. Insufficient fuel reports
E0601 there without making the transfer. No closing-brace cost follows a transfer.
Existing root, storage, call, expression, merge and normal-edge costs are unchanged.
Skipped effects and overflow do not execute; static checking still covers them.

The ungrouped break-only main `while true { break; } return 7;` has two slots and
costs nine fuel. An ungrouped counter initialized to zero, updated by one, with
condition `n<2`, explicit continue and final `return n;` has nine slots and costs
37. Every lower budget is checked against a hand-written source-operation
schedule. A checked-overflow-before-break witness independently checks all cuts
through fuel twelve, including fuel-before-overflow priority.

No executor, raw OIR verifier algorithm, LLVM emitter or C adapter change is
needed. CFG shape, not while spelling, determines guarding: a break-only or
return/break body may be acyclic; continue or ordinary body fallthrough can make
it cyclic even under a literal-false condition. Any unused cyclic function still
makes the module guarded. Existing transitive acyclic 100,000 static admission,
shared 1,000,000 runtime fuel, native call-recursion rejection and all resource
ceilings remain. LLVM phi predecessors use the existing actual guard/arithmetic/
call exit blocks. Qualification remains Linux x86_64 LLVM19.1.7 native O0.

## Diagnostics

- E0204/resolve: break/continue outside an enclosing while in the same function,
  at the full transfer statement
- E0100/parse: missing semicolon, value/label suffix, or a recognized transfer
  keyword in an invalid grammar position. Other unsupported syntax retains its
  existing diagnostic policy
- E0303/type: statement after terminal control transfer, at the next statement.
  The old terminal-return-only wording is retained for return-only outcomes
- E0302/type: existing missing explicit terminal return; a while is never treated
  as proof that this return is unnecessary

Phase precedence remains parse, resolve, type, verified OIR, native admission.
Malformed or statically invalid sources fail before native tools or output.
Runtime E0601/E0604 retain exact human text, escaped path, empty stdout and exit1;
adapter write failures retain exit74. No JSON executable ABI is introduced.

## Evidence and limitations

[Loop-control validation](../docs/architecture/loop-control-validation.md) records
source, outcome-algebra, target, exact fuel, real LLVM and differential gates.
The independent oracle tracks explicit source outcomes and charged edges, not
compiler-produced IR or a legacy interpreter. Full scalar and previous while
corpora remain regression requirements. Tests are bounded evidence rather than a
proof of every source-to-IR mapping, safe ownership or every backend target.
