# Value sharing and lifetime limits

Oxid 0.9 has no static ownership, borrowing, or lifetime checker. Arrays and records use shared mutable storage in the interpreter. Assigning a container to another binding does not copy its contents.

```oxid
fun main() {
    const original = {count: 1};
    const alias = original;
    alias.count = 2;
    say original.count;
}
```

This prints `2`. `const` prevents binding reassignment, but does not make the record immutable. The same sharing applies to arrays and containers passed to functions.

A static ownership-based core is planned. Its rules require a separate specification and migration plan; current dynamic behavior does not establish Rust-style memory-safety guarantees. See [legacy container behavior](../spec/legacy-0.9.md#containers) and the [roadmap](ROADMAP.md).
