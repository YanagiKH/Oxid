# Package workflow preview

Oxid package projects should feel copyable.

## Layout

```text
project/
├── oxid.toml
├── README.md
├── src/
│   ├── main.ox
│   └── lib.ox
├── stdlib/
├── tools/
├── examples/
└── tests/
```

## Suggested flow

1. create the project
2. import `stdlib/prelude.ox`
3. keep reusable helpers in `stdlib/`
4. keep executable entry logic in `src/main.ox`
5. keep workflow previews in `tools/`
6. keep demonstrations in `examples/`
7. keep smoke tests in `tests/`

## Goal

A new user should be able to copy the template, edit one entry file, and start writing Oxid without learning Rust-like project ceremony first.
