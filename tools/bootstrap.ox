use "../stdlib/prelude.ox";
use "../stdlib/compiler.ox";

fn main() {
    print "Oxid bootstrap preview";
    print compile_plan("oxid", "0.6.0", "native");
    print parse_error("src/main.ox", 1, 1, "bootstrap preview only", "move parsing helpers into Oxid modules");
}
