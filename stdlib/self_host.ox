use "strings.ox";
use "frontend/pipeline.ox";
use "frontend/syntax.ox";
use "frontend/diagnostics.ox";
use "frontend/modules.ox";

fn self_host_stage(name) {
    return "self-host:" + name;
}

fn self_host_goal() {
    return "Oxid writes the toolchain surface";
}

fn self_host_status() {
    return join_lines([
        self_host_stage("bootstrap"),
        self_host_stage("frontend"),
        self_host_stage("diagnose"),
        self_host_stage("lint"),
        self_host_stage("emit"),
        self_host_stage("module"),
        self_host_stage("syntax")
    ], ", ");
}

fn self_host_summary(project_name, entry_point) {
    return frontend_bootstrap(project_name, entry_point) + " | " + self_host_goal() + " | " + self_host_status();
}

fn self_host_plan(project_name, entry_point) {
    return join_lines([
        self_host_summary(project_name, entry_point),
        frontend_pipeline(entry_point),
        syntax_shortcuts(),
        module_catalog(),
        diag_suggestion("run", "oxid self-host")
    ], "\n");
}

fn self_host_command_list() {
    return join_lines([
        "oxid bootstrap",
        "oxid frontend",
        "oxid diagnose",
        "oxid lint",
        "oxid emit",
        "oxid module",
        "oxid syntax",
        "oxid self-host"
    ], ", ");
}
