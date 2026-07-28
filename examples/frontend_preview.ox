use "../stdlib/prelude.ox";

fn main() {
    print frontend_pipeline_name();
    print frontend_plan("oxid", "0.7.0", "src/main.ox");
    print frontend_bootstrap_hint("oxid", "0.7.0", "src/main.ox");
}
