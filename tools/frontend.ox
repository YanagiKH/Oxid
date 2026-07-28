use "../stdlib/prelude.ox";

fn main() {
    print "Oxid frontend";
    print frontend_pipeline("frontend");
    print frontend_compile_plan("oxid", "0.9.0", "src/main.ox");
    print ast_module_summary("frontend");
}
