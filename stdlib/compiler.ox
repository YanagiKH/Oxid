use "strings.ox";
use "package.ox";

fn compiler_banner(project_name, version, entry_point) {
    return package_summary(package_manifest_header(project_name, version, entry_point));
}

fn compile_phases() {
    return join_lines([
        "parse",
        "ast",
        "semantic",
        "mir",
        "codegen",
        "backend",
        "bootstrap",
        "verify"
    ], ", ");
}

fn compile_hint() {
    return "Rust stays as the fallback bootstrap boundary while Oxid owns the visible toolchain surface";
}

fn compile_command_list() {
    return join_lines([
        "oxid script bootstrap",
        "oxid script compile",
        "oxid script frontend",
        "oxid script diagnose",
        "oxid script lint",
        "oxid script emit",
        "oxid script module",
        "oxid script syntax",
        "oxid script interop",
        "oxid script bridge",
        "oxid script self-host"
    ], ", ");
}

fn compile_snapshot(project_name, version, entry_point) {
    return join_lines([
        compiler_banner(project_name, version, entry_point),
        "phases: " + compile_phases(),
        "commands: " + compile_command_list(),
        "hint: " + compile_hint()
    ], "\n");
}

fn compile_plan(project_name, version, entry_point) {
    return compile_snapshot(project_name, version, entry_point);
}
