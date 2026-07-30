use "strings.ox";
use "package.ox";
use "frontend/pipeline.ox";
use "compiler.ox";

fn self_compile_goal() {
    return "Oxid compiles the toolchain surface with Oxid-authored helpers";
}

fn self_compile_command_list() {
    return join_lines([
        "oxid bootstrap",
        "oxid compile",
        "oxid self-compile",
        "oxid self-host"
    ], ", ");
}

fn self_compile_plan(project_name, version, entry_point) {
    return self_compile_snapshot(project_name, version, entry_point);
}

fn self_compile_snapshot(project_name, version, entry_point) {
    return join_lines([
        self_compile_goal(),
        compile_snapshot(project_name, version, entry_point),
        frontend_pipeline(entry_point),
        package_load_hint(project_name, entry_point),
        self_compile_command_list()
    ], "\n");
}

fn self_compile_summary(project_name, version, entry_point) {
    return self_compile_snapshot(project_name, version, entry_point);
}

fn self_compile_boundary_note() {
    return "keep native code only for the thin compatibility edge";
}