use "strings.ox";
use "package.ox";
use "frontend/pipeline.ox";
use "frontend/modules.ox";
use "compiler.ox";
use "self_compile.ox";

fn bootstrap_goal() {
    return "Rust is only the fallback boundary";
}

fn bootstrap_command_list() {
    return join_lines([
        "oxid bootstrap",
        "oxid compile",
        "oxid self-compile",
        "oxid self-host"
    ], ", ");
}

fn bootstrap_manifest(project_name, version, entry_point) {
    return package_manifest_header(project_name, version, entry_point);
}

fn bootstrap_plan(project_name, version, entry_point) {
    return bootstrap_snapshot(project_name, version, entry_point);
}

fn bootstrap_snapshot(project_name, version, entry_point) {
    return join_lines([
        "bootstrap project: " + project_name,
        "bootstrap entry: " + entry_point,
        package_summary(bootstrap_manifest(project_name, version, entry_point)),
        frontend_bootstrap(project_name, entry_point),
        frontend_pipeline(entry_point),
        compile_snapshot(project_name, version, entry_point),
        self_compile_snapshot(project_name, version, entry_point),
        module_catalog(),
        bootstrap_goal()
    ], "\n");
}

fn bootstrap_summary(project_name, version, entry_point) {
    return bootstrap_snapshot(project_name, version, entry_point);
}

fn bootstrap_boundary_note() {
    return "keep the native runtime minimal and move workflow logic into Oxid";
}