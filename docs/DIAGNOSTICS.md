# Diagnostics

Oxid error messages should stay short, precise, and source-linked.

## Preferred structure

- `kind`
- `message`
- `file`
- `line`
- `column`
- `hint`
- `snippet`

## Example

```text
error: unexpected token
 --> src/main.ox:12:8
  |
12 | print(1 + )
  |        ^
  = help: remove the trailing operator or add the missing expression
```

## Rules

- show the most relevant location first
- prefer one clear hint
- avoid internal implementation jargon
- keep parser and runtime messages visually consistent
