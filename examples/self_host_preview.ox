use "../stdlib/prelude.ox";
use "../stdlib/compiler.ox";

fn main() {
    print "self-host preview";
    print compile_plan("oxid", "0.6.0", "oxid");
    print parse_error("compiler.ox", 3, 14, "planned self-host stage", "keep the parser and diagnostics readable");
}
