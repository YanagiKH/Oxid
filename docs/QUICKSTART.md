# Run your first Oxid project

[Install Oxid](INSTALLATION.md) first. The commands below use a new project directory and require `oxid` on `PATH`.

## Create and run

```bash
oxid new hello
cd hello
oxid run src/main.ox
```

The program prints `Hello from Oxid`. Open `src/main.ox` to see the entry point:

```oxid
fun main() {
    say "Hello from Oxid";
}
```

`oxid new` also creates a manifest, an empty lockfile, a small prelude, an example, and a smoke test.

## Work with data

Replace `src/main.ox` with:

```oxid
fun main() {
    const config = json_parse("{\"port\":8080}");
    config.host = "127.0.0.1";
    say json_stringify(config);
}
```

Run it again with `oxid run src/main.ox`. It prints `{"host":"127.0.0.1","port":8080}`. Records use sorted keys when encoded as JSON. `const` fixes the binding; record contents remain mutable.

## Check, test, and package

```bash
oxid check src/main.ox
oxid test
oxid build
oxid inspect .oxid/bin/hello.oxb
oxid run .oxid/bin/hello.oxb
```

`check` validates syntax without running the program; it does not provide static type or ownership checking. `test` runs the project's test suite. `build` packages the entry point and imported modules into a serialized-AST artifact. To run that artifact on another machine, install a compatible Oxid runtime there.

For a single file outside a project, use `oxid compile app.ox -o app.oxb`.

## Keep going

- [Syntax](SYNTAX.md) for loops, pipelines, functions, and records
- [Packages](PACKAGES.md) to add local or commit-pinned Git dependencies
- [Commands](COMMANDS.md) for formatting, scripts, and project tools
- [Implementation limits](architecture/current-baseline.md) before relying on a feature
