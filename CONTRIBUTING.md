# Contributing to Oxid

Small, testable changes are welcome: a bug fix with a reproducer, a clearer error message, a useful example, or a documentation correction. Oxid 0.9 is experimental; start with the [current implementation](docs/architecture/current-baseline.md) before changing language behavior.

## Report a bug

Include the Oxid version, operating system, smallest program that reproduces the issue, command used, expected result, and actual output. Report vulnerabilities privately using the [security policy](SECURITY.md).

## Work on a change

1. Build a checkout using the [installation guide](docs/INSTALLATION.md).
2. Keep the change focused and add tests for the affected behavior, including failure cases.
3. For a semantic change, explain compatibility and migration in an [RFC](rfcs/README.md).
4. Update the affected docs and examples. Keep the English, Traditional Chinese, and Japanese READMEs equivalent; other public technical docs use English.
5. Open a pull request describing the change and the checks you ran. State any checks you could not run.

## Run the checks

From the repository root, with stable Rust, a C/C++ compiler, and Python 3.11 or later:

```bash
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-targets --all-features --locked
python3 -m unittest discover -s scripts -p 'test_*.py' -v
cargo build --release --locked
python3 scripts/verify_repo.py target/release/oxid
```

On Windows, pass `target/release/oxid.exe` to the verifier. `make verify` is a shortcut for the release build and repository verifier; it does not run every check above. CI also covers its configured host matrix, installation, and Docker.

## Keep claims tied to behavior

Describe what the production path does. Label previews, generated examples, and planned features clearly. Preserve the [legacy behavior tests](spec/legacy-0.9.md) unless a reviewed change includes a migration plan. Update the [feature inventory](docs/architecture/feature-status.md) when its scope changes.
