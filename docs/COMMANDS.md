# Commands

## Run, check, and compile

- `oxid run <file.ox|file.oxb|file.oxa>` executes source or a validated OXBC artifact.
- `oxid <existing-file>` is the direct shorthand for `oxid run`.
- `oxid check <file>` parses source or validates an artifact without executing it.
- `oxid compile <file.ox> [-o output.oxb]` writes OXBC 1.0.
- `oxid ast <file.ox> [-o output.oxa]` writes the serialized AST representation.
- `oxid inspect <artifact>` prints format, AST version, module/statement counts, and checksum.
- `oxid repl` starts the interactive interpreter.
- `oxid watch <file.ox>` reruns after project files change.

## Projects and packages

- `oxid new <name>` / `oxid init <name>`
- `oxid web new <name>` / `oxid discord new <name>`
- `oxid build [--offline|--locked|--frozen]`
- `oxid add <name> <path-or-pinned-git-source>`
- `oxid remove <name>` / `oxid list`
- `oxid lock [--offline|--locked]`
- `oxid fetch [--offline|--locked]`
- `oxid update`
- `oxid install [--offline|--locked|--frozen]`
- `oxid script <name> [args...]`
- `oxid test`, `oxid fmt [path]`, `oxid clean`, `oxid doctor`, `oxid doc`, `oxid lint`

## Measurement and bootstrap

- `oxid bench [--iterations N] [--json report.json]`
- `oxid bootstrap [--check]`
- `oxid self-compile [--check]`
- `oxid emit [--check]`
- `oxid self-host [--check]`
- `oxid frontend`, `oxid diagnose`, `oxid module`, `oxid syntax`, `oxid interop`

Without `--check`, bootstrap commands write deterministic stage artifacts and a manifest under `.oxid/bootstrap/`. With `--check`, they perform the parity validation without changing bootstrap outputs.

## Bridges

- `oxid bridge <python|java|go|c|cpp|all> [output]`

Use `oxid --version`, `oxid help`, or `oxid --help` for the installed toolchain version and concise command help.

Manifest scripts are tokenized with quote/backslash handling and launched as an executable plus argument vector. They do not use a command shell, so shell operators, substitutions, and expansions are not interpreted. Extra arguments after the script name are appended unchanged.
