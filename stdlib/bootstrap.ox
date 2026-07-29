use "strings.ox";
use "frontend/pipeline.ox";
use "frontend/modules.ox";
fn bootstrap_goal() { return "Rust is only the fallback boundary"; }
fn bootstrap_command_list() { return join_lines(["oxid bootstrap", "oxid compile", "oxid self-host"], ", "); }
fn bootstrap_plan(project_name, entry_point) { return join_lines(["bootstrap project: " + project_name, "entry: " + entry_point, frontend_bootstrap(project_name, entry_point), frontend_pipeline(entry_point), module_catalog(), bootstrap_goal()], "
"); }
fn bootstrap_summary(project_name, entry_point) { return bootstrap_plan(project_name, entry_point); }
fn bootstrap_boundary_note() { return "keep the native runtime minimal and move workflow logic into Oxid"; }
