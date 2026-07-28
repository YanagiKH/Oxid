use "../stdlib/prelude.ox";

fn main() {
    print "Oxid frontend preview";
    print frontend_pipeline_name();
    print frontend_plan("demo", "0.7.0", "src/main.ox");
    print syntax_summary();
    print syntax_shortcuts();
}
