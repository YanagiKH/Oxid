# First excess array element boundary supplement

This separate prospective supplement preserves corrected contracts-v2 unchanged. It resolves one source-contract edge before candidate observations: after1024 admitted literal elements and a comma, `]` is a legal trailing-comma close, EOF/nonstarter retains the normal missing/excluded-expression error, and a possible1025th expression starter gets E0400 before parsing or reserving it. Malformed content after that starter cannot outrank the already-known excess.

The six source-derived cases cover EOF, semicolon and plus nonstarters, and unary `!`, borrow `&` and array `[` prefixes. `literal-length-max-trailing-comma` in the base package already covers `]`; `literal-length-one-over` covers an ordinary numeric starter. These are expectations, not executed parser passes. No source, raw, consumer or resource cap changes. No candidate compiler has run.

E0100 reuses existing parser primary fallback text `expected a bool, i32 or unit expression`. E0101 reuses the existing unsupported-token template. E0400 uses the separately reviewed `array element limit exceeded (1024)` proposal. All primary origins are the first post-comma token, or the empty EOF span. Source is ASCII and one line, so byte offset+1 also gives its Unicode-scalar column. Missing closing source syntax is intentional: the specified first error must win.

Run `python3 generate.py verify` for source-package consistency only. Independent review is required before observing these cases. This is a small authority addition, not a widening of the89 base cases or later semantic/consumer families.
