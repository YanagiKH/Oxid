# AST1 source/AST consumer boundary

Status: accepted for a closed decoder/validation admission experiment. The
combined source-to-typing candidate remains refused at I = 8,778 against the
unchanged 8,192 limit. No complete typed parity or provider activation is claimed.

This is deliberately a two-executable pipeline: the existing bounded parser
produces OPA1, and a separate Oxid consumer validates source plus a syntax-success
AST before any resolution/typing. The host only transports exact bytes. It never
lexes, parses, repairs rows, resolves names, infers types/flow, or creates compiler
owners/witnesses. The complete 128-byte ASCII scalar grammar remains the target.

## Exact record

An AST1 record consists of:

1. Four ASCII bytes `AST1`
2. One unsigned byte giving the source length, 0 through 128
3. Exactly that many original ASCII source bytes
4. Exactly the existing 1,559-byte OPA1 syntax-success frame

Total length is 1,564 through 1,692 bytes. The OPA1 source-used byte must equal the
outer length. Its error/detail/start/end bytes must be zero. No token tape,
semantic facts, optional fields, compression or multiple records are accepted.

Only whole-array views are required: read a five-cell outer header, source bytes
through a one-cell buffer, an eleven-cell OPA1 header, and twelve 129-cell byte
planes through one reused buffer. Every read requests positive capacity. Full
means capacity was filled, not EOF; any short EOF inside the record is malformed.
After the exact record, one one-cell read must observe Eof(0). A trailing byte
causes rejection after consuming only that one witness; remaining bytes are not
read. At most 143 read calls cover a maximum frame before existing EINTR retries.
Existing per-attempt fuel and I/O behavior apply without new limits or retries.

Malformed input returns 64 with empty artifact stdout; input I/O failure returns
74. No output precedes complete framing and source/AST validation. A closed probe
may return the explicit pending/internal status 70 after representative checks,
but never emits static success or invokes semantic consumers. Parser lexical,
syntax and domain-refusal observations remain producer results relayed unchanged;
they are not AST1 success inputs.

## Validation before semantic access

The Oxid consumer re-lexes the source using the existing lexer. It must prove:

- Exact framing, source bound/ASCII, all header counts and sentinel/inactive zeros
- Overflow-safe byte-plane decoding before values become indices or packed words
- Every kind, payload, next link and token reference has its allowed role/bound
- Unique owned acyclic rows, no sharing or orphan active rows, exact item/lists
- Complete source/token coverage, trivia included, ending at its single EOF
- Exact function/signature/type/block/statement/call grammar and source spans
- Operator precedence/associativity, nonchainable comparisons, negative Number
  convention and supported decimal spelling, with recomputed heights/depths
- The exact allocation-event order below, including all exceptions

The current producer sanity checks and host observation projection do not prove
all these properties at an untrusted boundary. They are not substitutes for the
consumer validator. Malformed input must be rejected before any index/read whose
safety or meaning depends on an unchecked graph property.

## Canonical allocation events

Physical row order affects current resolver DefIds and first-error order. AST1
therefore requires the exact existing parser allocation sequence, even if some
permuted graph would describe equivalent syntax. This does not narrow the source
grammar: every unchanged parser success already follows this sequence.

Maintain one next-expected row ID, initially 1. At each allocation event, require
that exact row and increment. On completion it must equal row_count + 1. Mark
unique ownership on entry, separately from possibly delayed allocation checks,
so a left cycle or shared subtree cannot defer detection indefinitely.

- Function before its parameters, result type and body; Parameter before its Type
- Block on entry, before its statement list
- Let/LetMut before annotation Type and initializer; Assign before RHS, without
  allocating a separate Name-expression row for its target
- Return, Break, Continue, If and While before their payload/condition/blocks;
  then-block before else-block
- ExprStmt after its complete expression and semicolon
- Group, unary and direct Call before their operand/arguments
- Binary after its complete left subtree and operator token, before its right
- Number, Bool, Unit and Name at their atomic parse event

Token consumption, row spans and all structural constraints are still checked;
a monotonically numbered graph alone is insufficient. After complete validation,
clear temporary marks/heights/frames and hand the resolver genuinely zero local,
semantic and traversal storage. Do not manufacture parser State/root markers to
pretend `static_state::release` established this new phase boundary.

## Admission and enablement

Measured subtraction of seven parser construction/dispatch modules from the
rejected candidate leaves a conservative 4,930 I before new transport, validation
and root changes. It still includes parser_state/output and all semantic modules.
The 3,262 remainder is not a measured validator budget. An initial target of at
most 2,500 new I would leave 762 reserve; all owners, wrappers, calls, arguments,
references and loans must be counted normally.

The first experiment links the full semantic modules but keeps their entry call
absent. It implements bounded decoding and representative real validation, then
measures ordinary check/native admission with maximum banks. It must remain
explicitly incomplete. Complete the whole validator and independent malformed
controls before requesting semantic enablement. Any gate failure stops expansion;
no cap change, source fusion, grammar narrowing or weaker input checks may make
an incomplete validator appear admissible.

A later enabled consumer may run real resolution and typing and emit the existing
OPA1+STF1 result. Even then it remains a component observation, not a TypedProgram,
source-association witness or production provider. Future provider use must bind
to the compiler's actual source identity and ordinary authoritative pipeline.

## Closed precursor measurement

The first role dispatcher exceeded the unchanged 256 scalar-slot function limit.
An acyclic declaration/statement/expression role split preserves the checks and
admits the closed precursor at I = 7,036, W = 4,199, 93 functions and 1,741 blocks.
The new transport/representative-validation modules account for 2,410 I. This is
not complete-validator admission: 1,156 I remain, and calls/group/prefix plus the
remaining statement/control/block families are still pending.

Fifty-three focused closed controls pass: 14 pending outcomes, 38 malformed
records and one I/O error, all with empty stdout and stderr. Repaired physical
row permutations preserve the logical Binary/ExprStmt graph yet reject, while
canonical allocation positives pass. A trailing-data control consumes exactly
one witness byte beyond the record and leaves the rest unread. No resolver or
typing invocation occurs in this root. Full validation, independent adversarial
review and later semantic enablement remain separate gates.
