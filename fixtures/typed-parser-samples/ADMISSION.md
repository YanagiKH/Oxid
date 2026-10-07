# Carrier admission experiment

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
