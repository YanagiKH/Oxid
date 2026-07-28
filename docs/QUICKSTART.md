# Quickstart

## 1. Run the sample scripts

```bash
cargo run -- run examples/hello.ox
cargo run -- run examples/arrays.ox
cargo run -- run examples/package_workflow.ox
```

## 2. Explore the standard library

```bash
cargo run -- run examples/stdlib_smoke.ox
cargo run -- run examples/modules.ox
cargo run -- run examples/library.ox
```

## 3. Build and check the project

```bash
cargo run -- build
cargo run -- test
cargo run -- doctor
cargo run -- doc
```

## 4. Scaffold a new project

```bash
cargo run -- new my_project
cd my_project
cargo run -- build
cargo run -- script run
```

## 5. Add a local dependency entry

```bash
cargo run -- add demo ./packages/demo
```

## 6. Use manifest scripts

Define scripts in `oxid.toml`:

```toml
[scripts]
run = "oxid run src/main.ox"
test = "oxid test"
fmt = "oxid fmt"
doctor = "oxid doctor"
doc = "oxid doc"
```

Then run them with:

```bash
oxid script run
oxid script test
```
