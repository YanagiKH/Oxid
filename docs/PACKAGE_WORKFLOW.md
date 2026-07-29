# Package workflow

## Suggested flow

1. create a project with `oxid new`
2. put reusable code in `src/`
3. put shared helpers in `stdlib/`
4. preview commands through `tools/`
5. keep runnable examples in `examples/`
6. keep smoke tests in `tests/`
7. validate with `oxid build`, `oxid test`, `oxid doctor`, and `oxid doc`
8. add bridge previews with `oxid interop` and `oxid bridge`

## Package layout

```text
project/
├── oxid.toml
├── README.md
├── src/
├── stdlib/
├── tools/
├── examples/
└── tests/
```
