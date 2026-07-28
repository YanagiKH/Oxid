# Diagnostics

Oxid diagnostics should be short, source-linked, and easy to read aloud.

## Preferred fields

- `kind`
- `message`
- `file`
- `line`
- `column`
- `hint`
- `snippet`
- `stage`

## Format

```text
error: unexpected token
 --> src/main.ox:12:8
  |
12 | print(1 + )
  |        ^
  = help: remove the extra operator or finish the expression
```

## Rules

- show the most relevant location first
- keep the main message short
- prefer one hint over many
- avoid internal runtime jargon
- continue parsing when recovery is possible
