use "../stdlib/prelude.ox";

fn main() {
    print "Oxid bootstrap";
    print frontend_bootstrap("oxid", "src/main.ox");
    print frontend_compile_plan("oxid", "0.9.0", "src/main.ox");
    print self_host_plan("oxid", "src/main.ox");
}
