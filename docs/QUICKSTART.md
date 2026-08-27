# Quickstart

## Create, interpret, and compile

```bash
oxid new my-project
cd my-project
oxid run src/main.ox
oxid check src/main.ox
oxid compile src/main.ox -o app.oxb
oxid inspect app.oxb
oxid run app.oxb
oxid test
```

The generated project includes an empty valid `oxid.lock`. Interpret source while iterating; use the versioned artifact when a single deterministic program file is more convenient.

## Add data

```oxid
const config = json_parse("{\"port\":8080}");
config.host = "127.0.0.1";
say json_stringify(config);
```

## Lock dependencies

```bash
oxid add shared ../shared
oxid lock
oxid build --locked
oxid list
```

Remote Git entries require an HTTPS URL and full commit hash. Use `--offline` for cached-only resolution or `--frozen` for both locked and offline behavior.

## Measure and verify bootstrap

```bash
oxid bench --iterations 20 --json target/benchmark.json
oxid bootstrap --check
oxid frontend
```

See the top-level README for release, source, Cargo, and Docker installation paths.
