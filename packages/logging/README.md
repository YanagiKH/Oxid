# Structured logging

Create one UTF-8 JSON record per line with a timestamp, severity, and message.
Each encoded line can be written to a file or sent to standard output.

```toml
[dependencies]
logging = "../../packages/logging"
```

Import `Level`, `Line`, and `encode` from `crate::logging`. Call
`encode(Level::Info, &message)` to use the current time. Use `encode_at`
to supply a timestamp in milliseconds since the Unix epoch.

Match `Line::Encoded(bytes)` to get the line, including its newline. An
invalid message reports the byte offset. The remaining results report an
unavailable clock or insufficient space for the record.

To add structured data, pass a single-line JSON object to `parse_fields`.
Match `FieldsResult::Ready(fields)` and pass `&fields` to `encode_with_fields`
or `encode_at_with_fields`. The object appears under `fields` and can be
reused across events. `FieldsResult::Invalid(offset)` reports malformed input,
including a literal newline.

Try writing an event to standard output on Linux or macOS.
[Example source](../../examples/typed-logging/src/main.ox).

```sh
cd examples/typed-logging
oxid run
```
