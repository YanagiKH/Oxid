use "../strings.ox";

fn recovery_stage_name() { return "recovery"; }
fn recovery_hint(text) { return "recover: " + text; }
fn sync_to_delimiter(delimiter) { return "sync until " + delimiter; }
fn skip_trivia() { return "skip trivia"; }
fn recovery_plan(file_path) { return "recovery plan for " + file_path; }
