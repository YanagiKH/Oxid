# Bounded stdout event model

This disconnected model exercises RFC 0025's candidate byte validation, partial
progress and fuel recurrence. It enables no imports, entry mode, raw operation,
host writes or executable witness. Caller-provided arrays stand in for already
admitted staging and observed output. No allocation or OS call occurs.

Events are deterministic test outcomes, not actual signal/I/O evidence. Missing
events are fixture errors. Process-status conversion rejects out-of-range i32
values without truncation. SIGPIPE setup, diagnostics, provenance, native ABI
and complete carrier accounting require separate production-path evidence.

```sh
rustc --edition=2021 --test tests/qualification/bounded_stdout_prototype/model.rs -o /tmp/oxid-stdout-model
/tmp/oxid-stdout-model
```
