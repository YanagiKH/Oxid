use "../stdlib/prelude.ox";

fn main() {
    print "Oxid module";
    print module_group("frontend");
    print import_alias("stdlib/frontend/parser.ox", "parse");
    print module_preview("frontend", "stdlib/frontend/parser.ox");
    print module_key("src", "frontend/parser.ox");
}
