# Bootstrap

Oxid bootstrap should be the smallest possible native edge.

## Goals
- keep Rust isolated to the fallback boundary
- move project orchestration into Oxid modules
- keep bootstrap output readable

## Helpers
- `bootstrap_plan(project_name, entry_point)`
- `bootstrap_snapshot(project_name, entry_point)`
- `bootstrap_summary(project_name, entry_point)`
- `bootstrap_boundary_note()`
