# Commands

## Existing direct commands

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

## Added command surface

- `oxid bootstrap`
- `oxid frontend`
- `oxid diagnose`
- `oxid lint`
- `oxid emit`
- `oxid module`
- `oxid syntax`
- `oxid self-host`

## Purpose

These commands split the future compiler front-end into smaller Oxid-owned workflows:

- `bootstrap` validates the boot path
- `frontend` previews the lexer/parser pipeline
- `diagnose` renders errors
- `lint` checks style and structural rules
- `emit` previews code emission
- `module` previews module resolution
- `syntax` previews syntax rules
- `self-host` assembles the self-host migration plan

## Fallback mode

If the binary has not been rebuilt with the CLI patch yet, run the corresponding Oxid script from `tools/` through `oxid script` or `oxid run`.
