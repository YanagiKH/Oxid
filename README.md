<div align="center">
  <img width="304" height="282" alt="Oxid logo" src="https://github.com/user-attachments/assets/c1de7268-a168-408c-8790-f5088c50e480" />

  # Oxid

  **A compact, standalone language for fast scripts, applications, bundles, and cross-language development.**

  [![Repository CI](https://github.com/YanagiKH/Oxid/actions/workflows/ci.yml/badge.svg)](https://github.com/YanagiKH/Oxid/actions/workflows/ci.yml)
  [![Release](https://img.shields.io/github/v/release/YanagiKH/Oxid?include_prereleases)](https://github.com/YanagiKH/Oxid/releases)
  [![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue)](LICENSE)

  [English](README.md) · [繁體中文](README_ZH.md) · [日本語](README_JP.md)
</div>

Oxid 0.9 is a directly usable language toolchain with concise and classical syntax, source interpretation, deterministic versioned artifacts, records and JSON, persistent network adapters, locked path/Git dependencies, project tooling, native C/C++ functions, and process bridges for Python/Java/Go. Normal users install one binary and do **not** need Rust.

## Project status

Oxid is usable today for scripts, automation, teaching, prototypes, local HTTP services, Discord interaction logic, and mixed-language process integration. The release binary contains the parser, runtime, OXBC compiler/reader, package resolver, benchmark harness, C/C++ bridge, formatter, test runner, doctor, and scaffolding commands.

The bytecode emitter is written in Oxid and bootstrap artifacts are checked for deterministic stage-0/stage-1/stage-2 equality. The lexer, parser, diagnostics, and module providers still use the stage-0 Rust bootstrap; full independent self-hosting is therefore not claimed. Rust is required only when building that bootstrap from source, not when writing, running, checking, compiling, packaging, or bridging Oxid programs with a release binary.

## Why Oxid

| Daily task | Rust-style ceremony | Oxid 0.9 |
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
| Locked dependency | select and wire a package client | `oxid add codec <pinned-git-url>` |
| Structured data | add a serialization crate | `{name: "Oxid"}` / `json_parse(text)` |
| Foreign bridge | write host glue manually | `oxid bridge all bridges` |

Oxid optimizes development speed by keeping the language small, caching preprocessing, resolving modules once, and allowing the same program to run directly or compile into one `.oxb` artifact. Performance depends on the workload; use `oxid bench` or application-specific measurements instead of assuming a universal speed ratio against Rust.

## Architecture

![Oxid architecture showing source, frontend, runtime, bundles, standard library, and bridges](docs/assets/architecture.svg)

- The lexer and parser understand both classical keywords and Oxid shortcuts.
- The runtime supports numbers, strings, booleans, nulls, arrays, deterministic records, JSON, tasks, persistent TCP handles, modules, files, processes, C/C++ native calls, and bounded HTTP parsing.
- The compiler resolves imports deterministically and emits OXBC 1.0 with serialized AST version 1, source ranges, size limits, and a payload checksum.
- The standard library is written in `.ox` modules and supplies collections, text, workflows, Web routing, Discord dispatch, and language bridge descriptions.
- Generated bridge SDKs let foreign hosts launch Oxid consistently without embedding compiler internals.
- `oxid.lock` records recursive path and commit-pinned Git dependencies with package-tree checksums.

## Quick start

![Oxid terminal quick start](docs/assets/quickstart.svg)

```bash
oxid new hello
cd hello
oxid run src/main.ox
oxid build
oxid test
```

The generated project includes a manifest, empty valid `oxid.lock`, source entry point, minimal prelude, example, test, and build script. `oxid build` validates the project and writes `.oxid/bin/hello.oxb`.

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

Supported shortcuts are aliases, not a second incompatible grammar: `fun/fn`, `var/let`, `say/print`, `give/return`, `when/if`, `otherwise/else`, `loop/while`, `import/use`, `yes/true`, `no/false`, `none/null`, `all/and`, and `any/or`. Oxid also implements `for … in`, `break`, `continue`, `%`, `|>`, `=>`, `async`, `await`, arrays, deterministic records, property and string-key access, assignment, comments, and one-line macros.

## Installation

### Linux and macOS release installer

The installer detects the platform, downloads the latest checksummed release, verifies SHA-256, and installs `oxid` into `${HOME}/.local/bin` by default.

```bash
curl --proto '=https' --tlsv1.2 -sSf https://raw.githubusercontent.com/YanagiKH/Oxid/main/install.sh | sh
export PATH="$HOME/.local/bin:$PATH"
oxid --version
```

Set `OXID_INSTALL_DIR` for another directory or `OXID_VERSION=<release-tag>` for a pinned release. Published Unix assets cover Linux x86_64, macOS x86_64, and macOS arm64.

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
oxid inspect app.oxb
oxid run app.oxb
oxid lock
oxid build --locked
oxid clean
```

`.oxb` is an OXBC 1.0 artifact containing serialized AST version 1, module count, cross-module source ranges, and a deterministic payload checksum. The runtime rejects incompatible, malformed, oversized, truncated, or corrupt artifacts before execution. `oxid ast` writes the same representation with an `.oxa` extension. `oxid build` resolves the manifest, validates `oxid.lock`, and writes the application artifact and build report under `.oxid/`.

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

`stdlib/web.ox` supplies route entries, local dispatch, and text/JSON responses. Native `net_listen`, `net_accept`, `net_try_accept`, `net_read`, `net_write`, `http_read_request`, `http_write_response`, and `net_close` expose reusable nonblocking sockets with bounded data and timeouts; `web_serve_once` remains available for simple one-request programs. TLS, authentication, rate limiting, and production scheduling remain adapter responsibilities.

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
| `oxid run <file>` | Execute `.ox` source or a validated `.oxb`/`.oxa` artifact |
| `oxid check <file>` | Lex, preprocess, and parse without running |
| `oxid compile <file> [-o output]` | Produce a deterministic OXBC artifact |
| `oxid ast <file> [-o output]` | Emit the versioned serialized AST |
| `oxid inspect <artifact>` | Show artifact versions, counts, and checksum |
| `oxid repl` | Start the interactive interpreter |
| `oxid new/init <name>` | Scaffold a normal project |
| `oxid web new <name>` | Scaffold a Web project |
| `oxid discord new <name>` | Scaffold a Discord bot project |
| `oxid bridge <target> [output]` | Generate Python/Java/Go/C/C++ host SDKs |
| `oxid build [dependency flags]` | Resolve, lock, and create `.oxid/bin/*.oxb` |
| `oxid test` | Run language smoke tests and core examples |
| `oxid fmt [path]` | Format one source or an entire project |
| `oxid watch <file>` | Re-run after project file changes |
| `oxid script <name> [args]` | Run an `oxid.toml` script |
| `oxid add <name> <target>` | Add a dependency entry |
| `oxid remove/list/lock/fetch/update/install` | Manage path and pinned Git dependencies |
| `oxid bench [options]` | Measure cold start, parsing, packaging, and runtime operations |
| `oxid doctor` | Check project structure |
| `oxid doc` | Generate built-in API documentation |
| `oxid clean` | Remove the `.oxid` cache/build directory |
| `oxid bootstrap/self-host [--check]` | Verify or write deterministic compiler stage artifacts |

## Repository layout

```text
Oxid/
├── src/                  # stage-0 parser, runtime, CLI, bundler
├── compiler/             # Oxid compiler entry and frontend provider manifest
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
- unit tests for syntax, OXBC round trips, source ranges, records/JSON, network limits, package locking, bootstrap parity, bridges, and native C/C++ linkage;
- syntax checking for every `.ox` file;
- execution of all tests, examples, tools, apps, and package demos;
- optimized builds on Linux x86_64, Windows x86_64, macOS x86_64, and macOS arm64;
- README parity, SVG XML, TOML, JSON, workflow, source-install, and Docker checks;
- project `test`, locked `build`, bootstrap parity, benchmark schema, and `doctor` commands.

Version tags package standalone archives, generate SHA-256 files, and publish them to GitHub Releases only after the reusable CI workflow succeeds.

## Independence and roadmap

Oxid 0.9 delivers versioned artifacts, an Oxid-authored emitter, deterministic bootstrap comparison, and an explicit provider manifest. Release users work with `oxid`, `.ox`, `.oxb`, and `.oxa` without installing Rust. Lexer, parser, diagnostics, and module providers remain stage-0; they will be replaced one verified component at a time, and the self-hosted path will become the release default only after independent cross-platform equivalence is demonstrated.

## Security

Process bridges execute programs requested by the Oxid application. Do not pass untrusted executable paths or shell fragments to generated C/C++ adapters. Git dependencies require HTTPS and full commit pins; review lockfile checksum changes. Network and JSON parsers enforce limits but do not provide TLS or application authentication. Report vulnerabilities privately according to [SECURITY.md](SECURITY.md).

## Contributing and license

Read [CONTRIBUTING.md](CONTRIBUTING.md), run `make verify`, and keep public repository documentation written for all users. Oxid is available under the [MIT](LICENSE) or [Apache-2.0](LICENSE-APACHE) license.
