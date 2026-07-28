use "../stdlib/prelude.ox";

fn main() {
    print "Oxid module command";
    print module_group("frontend");
    print import_alias("stdlib/frontend/parser.ox", "parse");
    print module_preview("frontend", "stdlib/frontend/parser.ox");
}
