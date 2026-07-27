# Async / await

Oxid currently uses a simplified task-style async model.

- Calling `async fn` returns a task instead of running the body immediately.
- `await task` runs the task at that moment and collects the result.
- `spawn`, `join`, `join_all`, `task_status`, and `yield_now` are available as helper primitives.
- This model keeps syntax readable first, then can grow into a true scheduler and I/O async system later.
