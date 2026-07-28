use "../strings.ox";
use "lexer.ox";
use "parser.ox";
use "diagnostics.ox";
use "modules.ox";
use "syntax.ox";

fn pipeline_stage(name) {
    return "[" + name + "]";
}

fn frontend_pipeline(source_name) {
    return join_lines([
        pipeline_stage("lex"),
        pipeline_stage("parse"),
        pipeline_stage("diagnose"),
        pipeline_stage("module"),
        pipeline_stage("syntax")
    ], " -> ") + " :: " + source_name;
}

fn bootstrap_banner(project_name) {
    return "Oxid frontend bootstrap: " + project_name;
}
