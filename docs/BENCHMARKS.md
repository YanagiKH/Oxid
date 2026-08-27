# Benchmarks

`oxid bench` measures four toolchain paths with bounded repeat counts:

- cold start: launch the current executable and run `--version`;
- parser throughput: parse a generated source corpus and report bytes per second;
- packaging: compile the project entry and encode its OXBC artifact in memory;
- runtime operations: execute a fixed loop-heavy Oxid program.

```bash
oxid bench
oxid bench --iterations 50
oxid bench --iterations 50 --json target/oxid-benchmark.json
```

Iterations must be between 1 and 10,000. Human output reports minimum, median, and maximum nanoseconds. JSON uses schema version 1 and includes the Oxid version, sample counts, nanosecond statistics, parser source size, and parser throughput. JSON output is published atomically.

## Comparing results

- use an optimized build;
- keep CPU, operating system, power settings, and repository revision fixed;
- warm filesystem caches before recording a baseline when cold disk behavior is not the subject;
- compare medians across several complete runs, not a single minimum;
- retain the JSON report with hardware and revision metadata in the surrounding benchmark system.

The harness measures regressions in Oxid itself. It does not establish a universal speed comparison with Rust or another language; fair cross-language claims require equivalent workloads, build modes, dependencies, and environments.

