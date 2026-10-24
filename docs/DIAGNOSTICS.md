# Understanding errors

Oxid reports source locations to help you find the code that failed. The output
format depends on the project's language edition.

## Script errors in Oxid 0.9

Syntax errors include the source name and token position when available. Runtime
errors can include a statement's source range:

```text
path/to/module.ox:start_line:start_column-end_line:end_column
```

Imported modules retain their own source locations. Script packages (`.oxb`)
also preserve these locations, so an error can identify the original module.
The script workflow uses text messages rather than typed diagnostic codes.

## Typed project errors in development builds

From a project with `edition = "typed-preview"` in `oxid.toml`, run:

```sh
oxid check
```

Source errors include a code, message, and location. For example, `E0300` reports
a type mismatch. Related locations and notes can explain a conflicting binding
or an ownership error. Text errors go to standard error.

For tools that read diagnostics, request JSON:

```sh
oxid check --message-format=json
```

This command writes one JSON record per line to standard output: diagnostic
records followed by a check summary. It exits with a nonzero status if checking
fails. The development format and codes are described in the
[typed language reference](../spec/typed-preview.md).

See [checking programs](FRONTEND.md) for the difference between script checks and
static type and ownership checks.

For native runtime errors with project-relative locations, build with
`oxid build --reproducible`. Compile-time error locations still identify the
original source files.

