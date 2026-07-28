use "strings.ox";
use "frontend/pipeline.ox";
use "frontend/diagnostics.ox";
use "frontend/syntax.ox";
use "frontend/modules.ox";
use "self_host.ox";

fn command_entry(name, summary) {
    return name + " :: " + summary;
}

fn command_catalog() {
    return join_lines([
        command_entry("bootstrap", "prepare the runtime boundary"),
        command_entry("frontend", "show the frontend pipeline"),
        command_entry("diagnose", "format structured diagnostics"),
        command_entry("lint", "run style and workflow checks"),
        command_entry("emit", "preview lowering and emission"),
        command_entry("module", "preview module resolution"),
        command_entry("syntax", "preview compact forms"),
        command_entry("self-host", "move work into Oxid")
    ], "\n");
}

fn command_help(command_name) {
    return "oxid " + command_name + " -> " + self_host_stage(command_name);
}

fn command_preview(project_name, entry_point) {
    return join_lines([
        frontend_bootstrap(project_name, entry_point),
        frontend_pipeline(entry_point),
        frontend_stage_banner(entry_point),
        syntax_summary(),
        module_catalog(),
        lint_suite()
    ], "\n");
}

fn command_error(file_path, line, column, message) {
    return fatal_diag(file_path, line, column, message, "use a shorter form or split the command");
}
