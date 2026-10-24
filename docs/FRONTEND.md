# Checking Oxid programs

Oxid checks programs according to their language edition. Published 0.9 binaries
provide the script workflow; typed project checks are available in development
source.

## Script checks

For a script project, check syntax without running the program:

```sh
oxid check src/main.ox
```

`oxid lint` also performs syntax-based checks. These script commands do not
enforce static types or ownership. Use `oxid test` to run the project's tests
and examples, including code that exercises runtime behavior.

## Native project checks

A development project with `edition = "typed-preview"` in `oxid.toml` uses
static checks. From its directory, run:

```sh
oxid check
oxid test
```

`check` verifies the application's modules and declared local dependencies,
including types, visibility, moves, and supported borrowing rules. `test` checks
and runs the test entries declared in the manifest. The same checks apply before
running or building the project.

Errors identify the source location and include a diagnostic code. For tools
that consume diagnostics, use `oxid check --message-format=json`.

Start with the [native project guide](TYPED_PROJECTS.md) for setup, supported
platforms, and project settings. The [typed language reference](../spec/typed-preview.md)
describes the supported syntax and ownership rules.

## Choose the right command

Use [the command reference](COMMANDS.md) for formatting and other project tools.
For an introduction to script projects, follow the [quickstart](QUICKSTART.md).
