# Oxid commands

Run `oxid --version` to check the installed version, or `oxid help` for built-in help. Commands below describe the 0.9 toolchain.

## Run and package

| Command | What it does |
|---|---|
| `oxid run <file.ox\|file.oxb\|file.oxa>` | Execute source or load and interpret an artifact |
| `oxid <existing-file>` | Shorthand for `run` |
| `oxid check <file>` | Parse source or validate an artifact without executing it |
| `oxid compile <file.ox> [-o output.oxb]` | Package the module graph as OXBC 1.0 serialized AST |
| `oxid ast <file.ox> [-o output.oxa]` | Write the same artifact representation with an `.oxa` extension |
| `oxid inspect <artifact>` | Print versions, counts, and checksum |
| `oxid repl` | Start the interactive interpreter |
| `oxid watch <file.ox>` | Rerun after project files change |

`check` does not type-check or enforce ownership. `compile` does not emit native machine code. See [artifact format](COMPILER.md) and [module ordering](MODULES.md).

## Create and maintain a project

| Command | What it does |
|---|---|
| `oxid new <name>` / `oxid init <name>` | Create a new directory and project scaffold; the directory must not exist |
| `oxid web new <name>` / `oxid discord new <name>` | Create an HTTP or Discord starter project |
| `oxid build [--offline\|--locked\|--frozen]` | Resolve dependencies and write `.oxid/bin/<project>.oxb` |
| `oxid test` | Execute `.ox` files under `tests/` and `examples/` |
| `oxid lint` | Run source checks across the project |
| `oxid fmt [path]` | Rewrite source formatting in a file or project |
| `oxid doctor` / `oxid diagnose` | Check the manifest, entry point, and project structure |
| `oxid doc` | Write a built-in reference to `docs/API.md`, replacing an existing file |
| `oxid script <name> [args...]` | Run a manifest script with appended arguments |
| `oxid clean` | Delete the project's `.oxid` artifacts, caches, and dependency checkouts |

`lint` currently runs syntax checks, not a separate static-analysis engine. Manifest scripts launch an executable and argument vector without a shell; see [Scripts](SCRIPTS.md).

## Manage dependencies

- `oxid add <name> <path-or-pinned-git-source>`
- `oxid remove <name>` / `oxid list`
- `oxid lock [--offline|--locked]`
- `oxid fetch [--offline|--locked]`
- `oxid update`
- `oxid install [--offline|--locked|--frozen]`

See [Packages](PACKAGES.md) for source rules, lockfile changes, and offline behavior. `install` resolves dependencies and builds the project; it does not install a global executable.

## Measure and inspect

- `oxid bench [--iterations N] [--json report.json]` measures Oxid's internal workloads.
- `oxid frontend` prints provider declarations; it does not select compiler providers.
- `oxid module`, `oxid syntax`, and `oxid interop` print reference information.
- `oxid bridge <python|java|go|c|cpp|all> [output]` generates host process adapters.

## Legacy bootstrap aliases

`oxid bootstrap`, `oxid self-compile`, `oxid emit`, and `oxid self-host` all run the same [artifact round-trip check](BOOTSTRAP.md). They do not rebuild the compiler.

Each accepts `--check`. Without that flag, the command writes artifacts and a manifest under `.oxid/bootstrap/`. With it, the command skips those output writes; source preprocessing may still use its cache. See [self-hosting status](SELF_HOSTING.md) for the exact checks.
