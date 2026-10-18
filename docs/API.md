# Oxid runtime API

## Core values

Oxid 0.9 supports numbers (`f64`), strings, booleans, null, arrays, records, functions, lazy task values, TCP listeners, and TCP connections. Records use deterministic key ordering and support literals, property access, string-key indexing, and assignment.

```oxid
var user = {name: "Ada", active: yes};
user.name = "Grace";
user["visits"] = 1;
```

## Built-ins

- time: `clock`, `now`, `sleep`, `sleep_ms`, `yield_now`
- values: `len`, `push`, `pop`, `range`, `str`, `number`, `type_of`, `assert`
- text: `split`, `join_text`, `replace`
- tasks: `spawn`, `join`, `join_all`, `task_status`
- records: `record`, `keys`, `has_key`, `get`, `set`, `remove`
- JSON: `json_parse`, `json_stringify`, `json_escape`
- files/environment: `read_text`, `write_text`, `exists`, `env`, `cwd`, `list_dir`
- processes: `process`, `process_output`, `python`, `java`, `go`
- networking: `net_listen`, `net_local_addr`, `net_accept`, `net_try_accept`, `net_read`, `net_write`, `net_close`, `http_read_request`, `http_write_response`
- Web: `web_response`, `web_serve_once`
- native ABI: `c_len`, `c_hash`, `cpp_len`, `cpp_hash`

## Record keys and deprecated tables

The native `keys(value)` accepts one record and returns a fresh array of its
string keys in sorted order, rather than insertion order. An empty record yields
`[]`. Importing `stdlib/records.ox` provides `record_keys(value)`, a wrapper around
that builtin. `len(record)` counts fields. For example, `{z: 2, a: 1}` has keys
`["a", "z"]` and length `2`. Key strings retain their case and Unicode characters.
The `record_from_pairs` helper accepts a flat alternating array such as
`["a", 1]`, not a nested pair array such as `[["a", 1]]`.

`stdlib/tables.ox` is deprecated compatibility scaffolding. Its zero-argument
`keys()` and `values()` currently return fresh empty arrays, and `entry_count(xs)`
only calls `len(xs)`, including for strings and arbitrary arrays. These
placeholders do not define a supported table API for v1.0. This deprecation
changes neither their public signatures nor runtime behavior.

Legacy imports share global bindings: importing `tables.ox` replaces native
`keys` with its zero-argument function. Calls such as `keys(record)` and
`record_keys(record)` then fail with an argument-count error. Importing
`records.ox` afterward does not restore native `keys`; reversing the two imports
also leaves the collision. Avoid importing tables when using record APIs. See
[binding and packaging limits](MODULES.md#binding-and-packaging-limits) for the
source/OXBC difference that also makes saving the builtin before import unsafe.

Use native `keys(record)`, or `record_keys(record)` from `records.ox`, together
with `len(record)` for record enumeration and counting. There is no native
`values` function or supported values-projection contract yet. Replacement
record-value and pair-sequence APIs remain deferred. This guidance applies to
the legacy runtime and OXBC; it does not extend typed-preview or change text APIs.

## Network signatures

```text
net_listen(host, port) -> listener
net_local_addr(listener) -> {host, port}
net_accept(listener, timeout_ms) -> connection
net_try_accept(listener) -> connection | null
net_read(connection, max_bytes, timeout_ms) -> string
net_write(connection, text[, timeout_ms]) -> byte_count
http_read_request(connection[, timeout_ms]) -> record
http_write_response(connection, response[, timeout_ms]) -> byte_count
net_close(listener_or_connection) -> null
web_serve_once(listener, response[, timeout_ms])
web_serve_once(host, port, response[, timeout_ms])
```

`net_try_accept` returns immediately with a connection or `null`, allowing an idle-resilient loop to perform other work. `http_read_request` returns structured request data and applies limits to request lines, headers, and body. `http_write_response` safely encodes either a raw HTTP string or a `{status, headers, body}` response record before writing it. Network and JSON functions reject oversized or malformed input rather than allocating without bounds.

See [task semantics](ASYNC.md), [container sharing](LIFETIME.md), and [network limitations](WEB_AND_BOTS.md) before relying on concurrency, immutability, or binary I/O.
