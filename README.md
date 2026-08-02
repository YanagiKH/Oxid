<div align="center">
  <img width="304" height="282" alt="Oxid logo" src="https://github.com/user-attachments/assets/c1de7268-a168-408c-8790-f5088c50e480" />

  # Oxid

  **A compact, standalone language for fast scripts, applications, bundles, and cross-language development.**

  [![Repository CI](https://github.com/YanagiKH/Oxid/actions/workflows/ci.yml/badge.svg)](https://github.com/YanagiKH/Oxid/actions/workflows/ci.yml)
  [![Release](https://img.shields.io/github/v/release/YanagiKH/Oxid?include_prereleases)](https://github.com/YanagiKH/Oxid/releases)
  [![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue)](LICENSE)

  [English](README.md) · [繁體中文](README_ZH.md) · [日本語](README_JP.md)
</div>

Oxid 0.8 turns the project into a directly usable language surface: concise syntax, an interpreter and bundle compiler, project tooling, real C/C++ native functions, process bridges for Python/Java/Go, functional Web and Discord modules, and checksummed standalone releases. Normal users install one binary and do **not** need Rust.

## Project status

Oxid is usable today for scripts, automation, teaching, prototypes, local HTTP handlers, Discord interaction logic, and mixed-language process integration. The release binary contains the parser, runtime, package tools, C/C++ bridge, compiler bundle writer, formatter, test runner, doctor, and scaffolding commands.

The compiler implementation is currently a stage-0 Rust bootstrap with native C/C++ components. Rust is required only when building Oxid itself from source; it is not required to write, run, check, bundle, or bridge Oxid programs from a release binary. Full Oxid-authored self-hosting remains an explicit roadmap item, so the project does not present preview code as a finished self-hosted compiler.

## Why Oxid

| Daily task | Rust-style ceremony | Oxid 0.8 |
|---|---|---|
| Mutable value | `let mut total = 0;` | `var total = 0;` |
| Output | `println!("{value}");` | `say value;` |
| Short function | function body and explicit return | `fun double(n) => n * 2;` |
| Conditional | mandatory Rust expression syntax | `when ready { ... } otherwise { ... }` |
| Iteration | iterator traits or manual loop | `for item in values { ... }` |
| Pipeline | nested calls or adapters | `value |> clean |> encode;` |
| Async declaration | runtime and trait setup | `work fun fetch() => await request();` |
| Script run | project compilation workflow | `oxid run app.ox` |
| Single artifact | configure a package target | `oxid compile app.ox -o app.oxb` |
| Foreign bridge | write host glue manually | `oxid bridge all bridges` |

Oxid optimizes development speed by keeping the language small, avoiding a dependency graph for ordinary scripts, caching preprocessing, recursively caching modules, and compiling imports into one `.oxb` bundle in a single pass. Performance depends on the workload; use repository or application benchmarks instead of assuming a universal speed ratio against Rust.

## Architecture

![Oxid architecture showing source, frontend, runtime, bundles, standard library, and bridges](docs/assets/architecture.svg)

- The lexer and parser understand both classical keywords and Oxid shortcuts.
- The runtime supports numbers, strings, booleans, nulls, arrays, functions, tasks, modules, constants, files, processes, C/C++ native calls, and HTTP response serving.
- The bundle compiler recursively inlines imports, expands macros, validates syntax, and emits one `.oxb` artifact.
- The standard library is written in `.ox` modules and supplies collections, text, workflows, Web routing, Discord dispatch, and language bridge descriptions.
- Generated bridge SDKs let foreign hosts launch Oxid consistently without embedding compiler internals.

## Quick start

![Oxid terminal quick start](docs/assets/quickstart.svg)

```bash
oxid new hello
cd hello
oxid run src/main.ox
oxid build
oxid test
```

The generated project includes a manifest, source entry point, minimal prelude, example, test, and build script. `oxid build` validates the project and writes `.oxid/bin/hello.oxb`.

## Language syntax

### Classical spelling

```oxid
fn double(value) {
    return value * 2;
}

fn main() {
    let values = range(1, 7);
    print map(values, double);
}
```

### Oxid concise spelling

```oxid
fun double(value) => value * 2;
fun label(value) => "value=" + str(value);

work fun greet(name) => "Hello, " + name;

fun main() {
    const values = range(1, 7);
    for value in values {
        when value % 2 == 0 { continue; }
        say value |> double |> label;
    }

    var job = greet("Oxid");
    say await job;
    say yes all (none == null);
}
```

Supported shortcuts are aliases, not a second incompatible grammar: `fun/fn`, `var/let`, `say/print`, `give/return`, `when/if`, `otherwise/else`, `loop/while`, `import/use`, `yes/true`, `no/false`, `none/null`, `all/and`, and `any/or`. Oxid also implements `for … in`, `break`, `continue`, `%`, `|>`, `=>`, `async`, `await`, arrays, indexing, assignment, comments, and one-line macros.

## Installation

### Linux and macOS release installer

The installer detects the platform, downloads the latest checksummed release, verifies SHA-256, and installs `oxid` into `${HOME}/.local/bin` by default.

```bash
curl --proto '=https' --tlsv1.2 -sSf https://raw.githubusercontent.com/YanagiKH/Oxid/main/install.sh | sh
export PATH="$HOME/.local/bin:$PATH"
oxid --version
```

Set `OXID_INSTALL_DIR` for another directory or `OXID_VERSION=v0.8.0` for a pinned release. Published Unix assets cover Linux x86_64, macOS x86_64, and macOS arm64.

### Windows PowerShell installer

```powershell
Set-ExecutionPolicy -Scope Process Bypass
irm https://raw.githubusercontent.com/YanagiKH/Oxid/main/install.ps1 | iex
& "$env:LOCALAPPDATA\Oxid\bin\oxid.exe" --version
```

The PowerShell installer verifies the archive checksum and supports Windows x86_64. `OXID_INSTALL_DIR` and `OXID_VERSION` can override its defaults.

### Portable release archive

1. Open [GitHub Releases](https://github.com/YanagiKH/Oxid/releases).
2. Download the archive for the operating system.
3. Verify it with the adjacent `.sha256` file.
4. Extract `oxid` or `oxid.exe` into a directory on `PATH`.

No language runtime is required for a portable release binary.

### Cargo or source installation

Building the stage-0 implementation requires stable Rust plus a C/C++ compiler.

```bash
cargo install --git https://github.com/YanagiKH/Oxid --locked
# or
git clone https://github.com/YanagiKH/Oxid.git
cd Oxid
make verify
sudo make install
```

### Docker

```bash
docker build -t oxid .
docker run --rm -v "$PWD:/workspace" oxid run /workspace/examples/hello.ox
```

The container builds an optimized runtime and executes as a non-root user.

## Compile and package

```bash
oxid check src/main.ox
oxid compile src/main.ox -o app.oxb
oxid run app.oxb
oxid build
oxid clean
```

`.oxb` is an Oxid bundle: imported modules are deduplicated and inlined, macros are expanded, and the combined source is syntax-validated. It is portable across systems running the same or a compatible Oxid runtime. `oxid build` also validates manifest dependencies and records a build report under `.oxid/`.

## Cross-language bridges

![Oxid bidirectional bridges for Python, Java, Go, C, and C++](docs/assets/interop.svg)

### Call foreign programs from Oxid

```oxid
fun main() {
    say python("-c", ["print('hello from Python')"]);
    say go("tools/report.go", ["--format", "json"]);
    say process_output("java", ["-jar", "service.jar"]);
    say c_hash("native");
    say cpp_hash("bridge");
}
```

`process` returns an exit code; `process_output` returns standard output and turns a failed exit status into an Oxid error. `python`, `java`, and `go` provide concise adapters. Native `c_len`, `c_hash`, `cpp_len`, and `cpp_hash` prove the linked ABI boundary in every CI build.

### Call Oxid from another language

```bash
oxid bridge python bridges/python
oxid bridge java bridges/java
oxid bridge go bridges/go
oxid bridge c bridges/c
oxid bridge cpp bridges/cpp
# Generate every SDK at once:
oxid bridge all bridges
```

Generated files use each ecosystem's standard process API and expose a small `run` entry point. This keeps the protocol stable and the host glue replaceable. Treat file names and command arguments as trusted application input when using the C/C++ shell adapters.

## Web module

![Oxid Web routing and Discord interaction modules](docs/assets/web-discord.svg)

```oxid
import "stdlib/web.ox";

fun health(body) => web_json(200, "{\"status\":\"ok\"}");
fun echo(body) => web_text(200, body);

fun main() {
    const routes = [
        web_route_entry("GET", "/health", health),
        web_route_entry("POST", "/echo", echo)
    ];
    const response = web_dispatch(routes, "GET", "/health", "");
    web_serve_once("127.0.0.1", 8080, response);
}
```

`stdlib/web.ox` supplies route entries, local dispatch, text/JSON responses, and one-request TCP HTTP serving. Use `oxid web new my-api` to generate a runnable Web profile. Production TLS, long-running sockets, and framework-specific deployment remain adapter responsibilities.

## Discord module

```oxid
import "stdlib/bots/discord.ox";

fun ping(payload) => discord_reply("Pong: " + payload);

fun main() {
    const commands = [discord_command("ping", "Reply with pong", ping)];
    say discord_dispatch(commands, "ping", "interaction-data");
}
```

The module builds Discord interaction responses, registers commands, dispatches payloads, and launches gateway adapters through `discord_run_adapter`. Use `oxid discord new my-bot` for a token-aware project skeleton. HTTPS and WebSocket gateway transport stays isolated in a replaceable adapter instead of being hard-wired into the language core.

## Command reference

| Command | Purpose |
|---|---|
| `oxid run <file>` | Execute `.ox` or `.oxb` source |
| `oxid check <file>` | Lex, preprocess, and parse without running |
| `oxid compile <file> [-o output]` | Produce a deduplicated bundle |
| `oxid repl` | Start the interactive interpreter |
| `oxid new/init <name>` | Scaffold a normal project |
| `oxid web new <name>` | Scaffold a Web project |
| `oxid discord new <name>` | Scaffold a Discord bot project |
| `oxid bridge <target> [output]` | Generate Python/Java/Go/C/C++ host SDKs |
| `oxid build` | Validate manifest and create `.oxid/bin/*.oxb` |
| `oxid test` | Run language smoke tests and core examples |
| `oxid fmt [path]` | Format one source or an entire project |
| `oxid watch <file>` | Re-run after project file changes |
| `oxid script <name> [args]` | Run an `oxid.toml` script |
| `oxid add <name> <target>` | Add a dependency entry |
| `oxid doctor` | Check project structure |
| `oxid doc` | Generate built-in API documentation |
| `oxid clean` | Remove the `.oxid` cache/build directory |
| `oxid bootstrap/frontend/...` | Run Oxid-authored toolchain inspections |

## Repository layout

```text
Oxid/
├── src/                  # stage-0 parser, runtime, CLI, bundler
├── stdlib/               # Oxid-authored standard modules
│   ├── interop/          # C, C++, Python, Java, Go bridge helpers
│   └── bots/discord.ox   # Discord command and response module
├── examples/             # runnable language, Web, bot, and bridge examples
├── tests/                # Oxid smoke programs
├── tools/                # Oxid-authored project/toolchain scripts
├── native/               # linked C and C++ ABI implementation
├── scripts/              # repository and release verification
├── docs/assets/          # README diagrams
└── .github/workflows/    # full CI and checksummed release builds
```

## Verification and releases

Every push and pull request performs:

- Rust formatting and Clippy with warnings denied;
- unit tests for syntax, loops, pipelines, bundling, bridge generation, JSON/Web helpers, and native C/C++ linkage;
- syntax checking for every `.ox` file;
- execution of all tests, examples, tools, apps, and package demos;
- optimized builds on Linux x86_64, Windows x86_64, macOS x86_64, and macOS arm64;
- README parity, SVG XML, TOML, JSON, workflow, source-install, and Docker checks;
- project `test`, `build`, and `doctor` commands.

Version tags package standalone archives, generate SHA-256 files, and publish them to GitHub Releases only after the reusable CI workflow succeeds.

## Independence and roadmap

Oxid 0.8 achieves user-facing independence from Rust: release users work exclusively with `oxid` and `.ox/.oxb` files. The internal stage-0 implementation remains Rust-based while more compiler and tooling behavior moves into Oxid modules. The next self-hosting milestones are a serialized AST/bytecode format, an Oxid-authored bytecode emitter, deterministic bootstrap comparison, and replacing the stage-0 frontend one verified component at a time.

## Security

Process bridges execute programs requested by the Oxid application. Do not pass untrusted executable paths or shell fragments to generated C/C++ adapters. Web serving is intentionally minimal and does not provide TLS. Report vulnerabilities privately according to [SECURITY.md](SECURITY.md).

## Contributing and license

Read [CONTRIBUTING.md](CONTRIBUTING.md), run `make verify`, and keep public repository documentation written for all users. Oxid is available under the [MIT](LICENSE) or [Apache-2.0](LICENSE-APACHE) license.
