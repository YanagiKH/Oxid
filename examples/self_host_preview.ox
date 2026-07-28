use "../stdlib/prelude.ox";

fn main() {
    print "self-host preview";
    print frontend_compile_plan("oxid", "0.9.0", "tools/bootstrap.ox");
    print parse_failure("compiler.ox", 3, 14, "planned self-host stage");
}
