# Commands

## Current commands

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

## Planned additions

- `oxid bootstrap`
- `oxid diagnose`
- `oxid lint`
- `oxid emit`
- `oxid package`
- `oxid release`
- `oxid self-host`

## Script convention

Use `oxid.toml` scripts for repeatable workflows. Keep the script names short and the command bodies explicit.

Example:

```toml
[scripts]
run = "oxid run src/main.ox"
test = "oxid test"
doctor = "oxid doctor"
doc = "oxid doc"
bootstrap = "oxid run tools/bootstrap.ox"
diagnose = "oxid run tools/diagnose.ox"
```
