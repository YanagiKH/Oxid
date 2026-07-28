# Commands

## Existing commands

- `oxid run`
- `oxid script`
- `oxid repl`
- `oxid check`
- `oxid new`
- `oxid init`
- `oxid add`
- `oxid watch`
- `oxid build`
- `oxid clean`
- `oxid fmt`
- `oxid test`
- `oxid doctor`
- `oxid doc`

## Added commands

- `oxid bootstrap`
- `oxid frontend`
- `oxid diagnose`
- `oxid lint`
- `oxid emit`
- `oxid module`
- `oxid syntax`
- `oxid self-host`

## Purpose

- `bootstrap`: validate the bootstrap path
- `frontend`: preview lex / parse / AST / recovery / module / syntax flow
- `diagnose`: render source-linked diagnostics
- `lint`: preview style and structural rules
- `emit`: preview emission
- `module`: preview module resolution
- `syntax`: preview the shorter syntax forms
- `self-host`: summarize the migration path away from Rust

## Fallback mode

If a binary has not yet been rebuilt with the Oxid-first command routing, these commands can still be executed through the matching Oxid scripts in `tools/`.
