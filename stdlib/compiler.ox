use "strings.ox";
use "package.ox";
use "frontend/pipeline.ox";
use "frontend/modules.ox";

fn compile_goal() {
    return "Oxid compiles Oxid source without leaving the language surface";
}

fn compile_command_list() {
    return join_lines([
        "oxid bootstrap",
        "oxid compile",
        "oxid self-compile",
        "oxid self-host"
    ], ", ");
}

fn compiler_banner(project_name, version, entry_point) {
    return package_summary(package_manifest_header(project_name, version, entry_point));
}

fn compile_plan(project_name, version, entry_point) {
    return compile_snapshot(project_name, version, entry_point);
}

fn compile_snapshot(project_name, version, entry_point) {
    return join_lines([
        compile_goal(),
        compiler_banner(project_name, version, entry_point),
        frontend_compile_plan(project_name, version, entry_point),
        frontend_pipeline(entry_point),
        module_catalog(),
        compile_command_list()
    ], "\n");
}

fn compile_summary(project_name, version, entry_point) {
    return compile_snapshot(project_name, version, entry_point);
}