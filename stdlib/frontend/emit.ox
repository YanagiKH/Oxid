use "../strings.ox";

fn emit_stage_name() { return "emit"; }
fn emit_preview(target_name) { return "emit:" + target_name; }
fn emit_plan(source_name) { return "lower " + source_name + " -> " + emit_stage_name(); }
fn emit_output_hint(target_name) { return "output target: " + target_name; }
fn emit_banner(source_name, target_name) { return emit_plan(source_name) + " | " + emit_preview(target_name); }
