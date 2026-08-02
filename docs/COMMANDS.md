# Commands

## Language and project commands

- `oxid run <file.ox|file.oxb>`
- `oxid check <file.ox>`
- `oxid compile <file.ox> [-o output.oxb]`
- `oxid repl`
- `oxid new <name>` / `oxid init <name>`
- `oxid web new <name>` / `oxid discord new <name>`
- `oxid bridge <python|java|go|c|cpp|all> [output]`
- `oxid script <name> [args...]`
- `oxid add <name> <target>`
- `oxid watch <file.ox>`
- `oxid build`, `oxid test`, `oxid fmt`, `oxid clean`, `oxid doctor`, `oxid doc`

## Toolchain inspection commands

- `oxid bootstrap`, `oxid self-compile`, `oxid self-host`
- `oxid frontend`, `oxid diagnose`, `oxid lint`, `oxid emit`
- `oxid module`, `oxid syntax`, `oxid interop`

Running `oxid compile` without a file and `oxid bridge` without a target keeps the Oxid-authored inspection scripts available. Supplying arguments invokes the real bundler or SDK generator.
