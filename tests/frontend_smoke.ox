use "../stdlib/prelude.ox";

fn main() {
    print frontend_pipeline("tests/frontend_smoke.ox");
    print parse_function_header("main");
    print parse_public_function("run");
    print module_group("demo");
    print command_catalog();
}
