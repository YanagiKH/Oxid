# Frontend

The frontend is organized as a visible pipeline rather than a hidden compiler monolith.

## Stages

1. lex
2. parse
3. ast
4. recover
5. diagnose
6. module
7. syntax
8. emit
9. lint

## Goals

- keep the stages small
- make stage names explicit
- keep preview modules reusable from examples and tooling
- make error recovery and suggestions visible to users
- keep interop and self-host previews adjacent to the frontend view

## Entry helpers

- `frontend_bootstrap(project_name, entry_point)`
- `frontend_pipeline(source_name)`
- `frontend_compile_plan(project_name, version, entry_point)`
- `frontend_stage_banner(source_name)`
- `frontend_bridge_plan(project_name, entry_point)`
