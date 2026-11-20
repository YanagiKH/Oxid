# Option and Result

Use `Option<T>` for a value that may be absent and `Result<T, E>` for a
value that may fail. Both hold scalar and owned values in Oxid typed projects.

Add the package to `oxid.toml`, adjusting the path for your project.

```toml
[dependencies]
types = "../../packages/core_types"
```

```rust
use crate::types::option::Option;
use crate::types::option::ok_or;
use crate::types::result::Result;

fn main() -> i32 {
    let value = Option::<i32>::Some(42);
    let result = ok_or(value, 1);
    match result {
        Result::Ok(number) => { return number; },
        Result::Err(code) => { return 0 - code; },
    }
}
```

For a default value, use `option::unwrap_or` or `result::unwrap_or`. Use `result::ok` and
`result::err` to keep one side of a result as an option.
These functions move the selected payload and release any unused value.

Try the example from the repository root. [Example source](../../examples/typed-option-result/src/main.ox)

```sh
cd examples/typed-option-result
oxid lock --offline
oxid run --frozen
oxid test --frozen
```
