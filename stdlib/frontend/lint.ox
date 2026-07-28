use "../strings.ox";

fn lint_stage_name() { return "lint"; }
fn lint_rule(name) { return "rule:" + name; }
fn lint_suite() {
    return join_lines([
        lint_rule("short forms"),
        lint_rule("consistent imports"),
        lint_rule("diagnostic hints"),
        lint_rule("module resolution"),
        lint_rule("bootstrap reduction")
    ], ", ");
}
fn lint_preview(target_name) { return lint_stage_name() + ":" + target_name; }
