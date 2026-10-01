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
