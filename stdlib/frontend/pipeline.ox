use "../strings.ox";
use "lexer.ox";
use "parser.ox";
use "ast.ox";
use "recovery.ox";
use "diagnostics.ox";
use "modules.ox";
use "syntax.ox";

fn pipeline_stage(name) { return "[" + name + "]"; }

fn frontend_pipeline(source_name) {
    return join_lines([
        pipeline_stage("lex"),
        pipeline_stage("parse"),
        pipeline_stage("ast"),
        pipeline_stage("recover"),
        pipeline_stage("diagnose"),
        pipeline_stage("module"),
        pipeline_stage("syntax"),
        pipeline_stage("emit")
    ], " -> ") + " :: " + source_name;
}

fn frontend_bootstrap(project_name, entry_point) {
    return "Oxid frontend bootstrap: " + project_name + " -> " + entry_point;
}

fn frontend_compile_plan(project_name, version, entry_point) {
    return frontend_bootstrap(project_name, entry_point) + " | " + version + " | " + module_key(project_name, entry_point);
}
