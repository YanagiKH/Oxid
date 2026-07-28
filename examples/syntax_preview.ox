use "../stdlib/prelude.ox";

fn main() {
    print syntax_summary();
    print exported_fn("run");
    print local_group("parser");
    print import_alias("stdlib/frontend/parser.ox", "parse");
    print record_preview("User");
    print typed_record_preview("Config");
    print one_line_helper("id", "value");
}
