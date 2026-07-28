use "../package.ox";
use "../cache.ox";
use "lexer.ox";
use "parser.ox";
use "diagnostics.ox";
use "modules.ox";
use "syntax.ox";

fn frontend_pipeline_name() {
    return "frontend";
}

fn frontend_plan(project_name, version, entry) {
    let out = "";
    out = out + "stage=" + lexer_stage_name() + "\n";
    out = out + "stage=" + parser_stage_name() + "\n";
    out = out + "stage=diagnostics\n";
    out = out + "stage=modules\n";
    out = out + "project=" + project_name + "\n";
    out = out + "version=" + version + "\n";
    out = out + "entry=" + entry + "\n";
    out = out + "cache=" + cache_label(project_name, version) + "\n";
    out = out + "syntax=" + syntax_profile_name();
    return out;
}

fn frontend_bootstrap_hint(project_name, version, entry) {
    return package_load_hint(project_name, entry) + " | " + cache_scope(project_name, version, "frontend");
}

fn frontend_preview_error(file_path) {
    return render_frontend_error(file_path, 1, 1, "frontend preview only", "move the lexer and parser into Oxid modules first");
}
