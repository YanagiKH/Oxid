use "../stdlib/prelude.ox";

fn main() {
    print "self-host preview";
    print frontend_plan("oxid", "0.7.0", "tools/bootstrap.ox");
    print parse_error("compiler.ox", 3, 14, "planned self-host stage", "keep the parser and diagnostics readable");
}
