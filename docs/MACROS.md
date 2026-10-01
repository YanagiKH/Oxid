# Preprocessing macros

Oxid 0.9 expands small source macros before lexing and parsing:

```oxid
macro twice(x) => ((x) + (x));

fun main() {
    say twice(3);
}
```

The macro definition fits on one line. Preprocessed output is cached under `.oxid/cache/preprocess/`; see [Cache](CACHE.md).

This is source expansion, not a typed or hygienic macro system. An argument appearing twice in a macro body may be evaluated twice. Avoid side-effecting arguments unless that repetition is intended.
