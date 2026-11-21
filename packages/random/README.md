# Seeded random numbers

Use `seeded(seed)` to create a generator for simulations or repeatable tests.
Call `below(&mut generator, upper)` to draw from zero up to `upper`, excluding
the upper limit. It returns `Draw::Value(number)`, or `Draw::EmptyRange` when
`upper` is zero. Use `next_nonzero(&mut generator)` to draw a positive `u32`.

Run the [example](../../examples/typed-random) from the repository root.

```sh
cd examples/typed-random
oxid run
oxid test
```

The example prints `68`.
