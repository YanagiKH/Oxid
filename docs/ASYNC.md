# Async / await

Oxid uses a task-style async model.

- `async fn` returns a task value.
- `await` resolves a task value.
- `spawn`, `join`, `join_all`, `task_status`, and `yield_now` support task workflows.
- The goal is to move toward a real scheduler while keeping everyday syntax simple.
