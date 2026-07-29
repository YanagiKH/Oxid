use "../stdlib/prelude.ox";

fn main() {
    print "Oxid module";
    print module_catalog();
    print module_search_hint("frontend");
    print module_key("src", "frontend/parser.ox");
    print module_preview("frontend", "stdlib/frontend/parser.ox");
}
