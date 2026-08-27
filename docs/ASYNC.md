# Async tasks and network I/O

Oxid uses task values for concise asynchronous syntax.

- `async fn` or `work fun` returns a task.
- `await`, `join`, and `join_all` resolve tasks.
- `spawn` creates a task from a callable and arguments.
- `task_status` reports `pending`, `running`, `completed`, or `failed`.
- completed and failed task results are memoized.
- `yield_now` yields the current operating-system thread.

The network adapter uses nonblocking TCP sockets and timeout-bounded polling. A listener remains open across repeated `net_accept` calls, while `net_try_accept` returns a connection or `null` immediately so idle loops can perform other work. Accepted connections can be read, written, parsed as HTTP, and closed independently. This supports long-running server loops without recreating the listening socket.

Oxid 0.9 does not yet provide a concurrent event-loop scheduler: an individual accept/read/write call waits cooperatively until data, completion, or its timeout. Applications should use finite timeouts and keep TLS, authentication, rate limiting, and production connection management in an audited adapter.
