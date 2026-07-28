use "core.ox";
use "strings.ox";

fn script_line(name, command) {
    return name + " = \"" + command + "\"";
}

fn workflow_header(project_name) {
    return "# " + project_name + " workflow";
}

fn workflow_step(step_name) {
    return "- " + step_name;
}

fn workflow_preview(project_name) {
    let lines = [];
    push(lines, workflow_header(project_name));
    push(lines, workflow_step("run"));
    push(lines, workflow_step("script"));
    push(lines, workflow_step("fmt"));
    push(lines, workflow_step("test"));
    push(lines, workflow_step("doctor"));
    return lines;
}
