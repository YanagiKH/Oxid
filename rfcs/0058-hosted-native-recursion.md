# Hosted native recursion

Status: experimental

Typed projects using `policy = "hosted"` can build native programs with direct
or mutual recursive calls. Calls support the same scalar, owned-value and
call-only borrowed parameters as other functions.

```toml
[execution]
policy = "hosted"
```

```oxid
fn factorial(n: i32) -> i32 {
    if n <= 1 { return 1; }
    return n * factorial(n - 1);
}

fn main() -> i32 {
    return factorial(6);
}
```

With `[build] entry-mode = "result"`, both `oxid run` and the executable built by
`oxid build --profile release` print `720`.

## Calls and errors

Hosted native execution permits up to 1,024 active Oxid calls, including `main`,
and 200,000 active scalar storage slots. The compiler's planned native frame and
runtime storage is limited to 1 MiB. Buffer heap allocation retains its separate
16 MiB live limit. Native frames use a different storage representation from the
reference runner; the reference runner retains its own live-storage checks.

A caller reserves the callee's storage before making the call and releases that
reservation when the callee returns. The checks report the attempted call:

| Error | Meaning |
| --- | --- |
| `E0602` | Too many active calls. |
| `E0603` | Too many active scalar storage slots. |
| `E0605` with `live native frame bytes` | The next call would exceed the native storage budget. |

These errors end the program. They do not resume execution in the caller.
Argument evaluation and checked arithmetic keep their existing order. Ownership,
borrow conflicts and type errors remain errors even in an unused function or
an unchosen branch. An unused recursive function alone does not cause a runtime
error.

Shared and exclusive reborrows retain their original owner and call lifetime.
Recursive calls do not permit aliased exclusive arguments, escaping references
or access to a moved value. Owned results transfer back to the caller before the
callee's remaining resources are released.

## Scope

This applies to native typed project builds on Linux x86_64, using either native
optimization profile and either result or process entry. Hosted execution has no
cumulative work budget. Its compiler size and allocation limits still apply.
Nonrecursive hosted call chains can also exceed the bounded policy's depth of
32 while satisfying the remaining compiler limits.

The bounded native policy retains its acyclic call-graph requirement and static
depth limit. Explicit file compilation keeps that policy. The reference runner
already supports recursive calls; its call and scalar-storage error behavior is
unchanged.

The public checks are in [hosted_recursion.rs](../tests/hosted_recursion.rs).
