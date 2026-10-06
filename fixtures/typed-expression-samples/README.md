# Bounded expression component

This component is being developed in existing typed-preview Oxid. Its target is
an iterative parser for decimal integers, `+`, `*` and parentheses, a 15-node
record-backed expression arena, and a separate iterative evaluator. It is not
compiler self-hosting or a language extension.

The initial checkpoint exercises only arena construction and scalar storage:
`main.ox` stores and returns 39. It does not yet parse an expression. Public
check, reference run, LLVM native compilation and execution have passed for that
small admission probe using the current PR37 compiler and LLVM 19.1.7 on Linux
x86_64. Complete parser/evaluator admission and boundary controls remain pending.

The intended expression `12 + 3 * (4 + 5)` must produce seven nodes and evaluate
to 39. `*` has greater precedence than `+`; both associate left. Input is bounded
to 128 character codes, and node/operator/operand capacities are independently
bounded at 15. Syntax and capacity failures return position-carrying enums;
decimal and evaluation overflow retain checked i32 E0604 behavior. No compiler
resource ceiling changes are part of this component.

The sibling scanner extends the prior scanner with `*`, `(`, `)` and token-start
positions. The original `tests/fixtures/bounded_enum_scanner` remains unchanged.
Arena kind values 1 (integer), 2 (addition), and 3 (multiplication) are application
data, not access to Oxid enum tags. Helpers retain whole-record borrowing.
