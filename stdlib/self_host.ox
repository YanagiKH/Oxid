use "strings.ox";
use "frontend/pipeline.ox";
use "frontend/syntax.ox";
use "frontend/diagnostics.ox";
use "frontend/modules.ox";
use "interop.ox";
use "bootstrap.ox";
use "self_compile.ox";
use "compiler.ox";

fn self_host_stage(name) {
    return "self-host:" + name;
}

fn self_host_goal() {
    return "Oxid writes the toolchain surface";
}

fn self_host_status() {
    return join_lines([
        self_host_stage("bootstrap"),
        self_host_stage("compile"),
        self_host_stage("self-compile"),
        self_host_stage("frontend"),
        self_host_stage("diagnose"),
        self_host_stage("lint"),
        self_host_stage("emit"),
        self_host_stage("module"),
        self_host_stage("syntax"),
        self_host_stage("interop"),
        self_host_stage("bridge")
    ], ", ");
}

fn self_host_summary(project_name, version, entry_point) {
    return join_lines([
        bootstrap_snapshot(project_name, version, entry_point),
        self_compile_snapshot(project_name, version, entry_point),
        self_host_goal(),
        self_host_status(),
        compile_goal()
    ], " | ");
}

fn self_host_plan(project_name, version, entry_point) {
    return join_lines([
        self_host_summary(project_name, version, entry_point),
        frontend_pipeline(entry_point),
        syntax_shortcuts(),
        module_catalog(),
        interop_catalog(),
        diag_suggestion("run", "oxid script self-host")
    ], "\n");
}

fn self_host_command_list() {
    return join_lines([
        "oxid bootstrap",
        "oxid compile",
        "oxid self-compile",
        "oxid frontend",
        "oxid diagnose",
        "oxid lint",
        "oxid emit",
        "oxid module",
        "oxid syntax",
        "oxid interop",
        "oxid bridge",
        "oxid self-host"
    ], ", ");
}

fn self_host_boundary_note() {
    return "keep Rust only as the fallback boundary";
}