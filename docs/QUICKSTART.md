# Run your first script project

[Install Oxid](INSTALLATION.md) first and put `oxid` on your `PATH`. This guide
uses the script workflow in the published 0.9 release. For the development
version's typed native workflow, see [native projects](TYPED_PROJECTS.md).

## Create and run

```sh
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

`oxid new` also creates a manifest, an empty lockfile, a small prelude, an example,
and a smoke test.

## Work with data

Replace `src/main.ox` with:

```oxid
fun main() {
    const config = json_parse("{\"port\":8080}");
    config.host = "127.0.0.1";
    say json_stringify(config);
}
```

Run it again with `oxid run src/main.ox`. It prints
`{"host":"127.0.0.1","port":8080}`. Records use sorted keys when encoded as JSON.
`const` fixes the binding; record contents remain mutable.

## Check, test, and package

```sh
oxid check src/main.ox
oxid test
oxid build
oxid inspect .oxid/bin/hello.oxb
oxid run .oxid/bin/hello.oxb
```

For this script project, `check` validates syntax without running the program.
`test` runs the project's tests and examples. `build` bundles the entry point and
its imports into a script package. Running the `.oxb` file requires a compatible
Oxid runtime, including on another machine.

For a single file outside a project, use `oxid compile app.ox -o app.oxb`.
See [script packages](COMPILER.md) for compatibility and import limitations.

## Keep going

- [Syntax](SYNTAX.md) for loops, pipelines, functions, and records
- [Built-in functions](API.md) for working with data, files, and processes
- [Packages](PACKAGES.md) to add local or commit-pinned Git dependencies
- [Commands](COMMANDS.md) for formatting, scripts, and project tools
