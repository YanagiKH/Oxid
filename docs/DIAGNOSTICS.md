# Diagnostics

Oxid should report errors in a consistent shape.

## Standard fields

- `kind`
- `message`
- `file`
- `line`
- `column`
- `hint`
- `snippet`

## Desired style

A good error should be:

- short
- specific
- actionable
- tied to a source location
- easy to read in a terminal

## Suggested format

```text
error: unexpected token
 --> src/main.ox:12:8
  |
12 | print(1 + )
  |        ^
  = help: remove the trailing operator or add the right-hand expression
```

## Reporting rules

- show the most relevant location first
- keep the original message short
- prefer one direct hint over many vague hints
- avoid dumping internal Rust terminology to users
- if recovery is possible, continue parsing and show follow-up notes instead of stopping immediately
