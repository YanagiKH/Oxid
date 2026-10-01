# Tasks and network I/O

Oxid 0.9 supports task syntax, but has no concurrent task scheduler. Calling an async function creates a pending task; its body runs when awaited or joined. `join_all` runs tasks sequentially in input order.

- `async fn` or `work fun` declares a task-returning function.
- `await`, `join`, and `join_all` resolve tasks.
- `spawn` creates a pending task from a callable and arguments; it does not start a background thread.
- `task_status` reports `pending`, `running`, `completed`, or `failed`.
- Completed results and failures are memoized.
- `yield_now` yields the current operating-system thread, rather than scheduling other Oxid tasks.

## Network behavior

TCP sockets use nonblocking mode, but individual accept/read/write calls poll until they complete or time out. `net_try_accept` is the immediate-return option: it yields a connection or `null`. A listener can accept multiple connections over its lifetime without being recreated.

Use finite timeouts and close connections when finished. The current runtime has no concurrent event loop, built-in TLS, or authentication. See [local HTTP examples](WEB_AND_BOTS.md) and [legacy task semantics](../spec/legacy-0.9.md#tasks).
