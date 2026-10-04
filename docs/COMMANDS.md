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

The default legacy `check` does not type-check or enforce ownership. Legacy `compile` does not emit native machine code. See [artifact format](COMPILER.md) and [module ordering](MODULES.md).

## Experimental typed commands

`oxid check entry.ox --edition typed-preview` checks the complete explicitly
declared project without execution. `run` selects bounded reference execution;
`compile --backend llvm --output ./new-program` selects the Linux x86_64 native
preview. Each requires one entry source; a manifest build is not a typed build.
Run/compile require an original zero-argument scalar main in the root file.
Declared modules use `mod name;`, direct imports use `use crate::m::item;`, and
`pub` controls module/item/field visibility. See [typed preview](../spec/typed-preview.md),
[native preview](../spec/native-preview.md) and the
[three-file example](../fixtures/typed-project-batch/README.md) for limits and
reproduction. Module-free syntax has no discovery host gate; child loading
currently admits Linux only.

## Create and maintain a project

| Command | What it does |
|---|---|
| `oxid new <name>` / `oxid init <name>` | Create a new directory and project scaffold; the directory must not exist |
| `oxid web new <name>` / `oxid discord new <name>` | Create an HTTP or Discord starter project |
| `oxid build [--offline\|--locked\|--frozen]` | Resolve dependencies and write `.oxid/bin/<project>.oxb` |
| `oxid test` | Recursively execute `.ox` files under `tests/` and `examples/`, except explicitly registered source-data fixtures |
| `oxid lint` | Run source checks across the project |
| `oxid fmt [path]` | Rewrite source formatting in a file or project |
| `oxid doctor` / `oxid diagnose` | Check the manifest, entry point, and project structure |
| `oxid doc` | Write a built-in reference to `docs/API.md`, replacing an existing file |
| `oxid script <name> [args...]` | Run a manifest script with appended arguments |
| `oxid clean` | Delete the project's `.oxid` artifacts, caches, and dependency checkouts |

`lint` currently runs syntax checks, not a separate static-analysis engine. Manifest scripts launch an executable and argument vector without a shell; see [Scripts](SCRIPTS.md).

### Source-data fixtures

Compiler inputs that are data rather than executable language tests can be
registered by exact path in the current project's `oxid.toml`:

```toml
[test-fixtures]
"tests/fixtures/parser/invalid-input.ox" = true
```

Paths are relative to that manifest's directory and must name existing `.ox`
files beneath `tests/` or `examples/`. Directories, globs, parent traversal,
absolute paths, symlinks, duplicate entries and values other than `true` are
rejected before any tests execute. Unlisted files, including neighboring files
in the same fixture directory, remain runnable. Without this section, recursive
discovery is unchanged. Explicit `run` and `check` commands are unaffected, and
a fixture-only project still reports that no runnable tests were found.

In this repository, `scripts/verify_repo.py` requires these entries to match the
validated frozen source-data inventory exactly. Registration does not establish
compiler correctness or replace the dedicated tests for those inputs.

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
