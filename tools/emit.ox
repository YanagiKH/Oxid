use "../stdlib/prelude.ox";

fn main() {
    print "Oxid emit";
    print emit_plan("src/main.ox");
    print emit_preview("native");
    print emit_output_hint("native");
}
