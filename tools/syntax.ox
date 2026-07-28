use "../stdlib/prelude.ox";

fn main() {
    print "Oxid syntax";
    print syntax_summary();
    print exported_fn("run");
    print local_group("parser");
    print import_alias("stdlib/frontend/pipeline.ox", "pipeline");
    print match_preview("value");
    print try_preview("expr");
    print defer_preview("cleanup()");
    print pipe_preview("value", "step()");
    print record_preview("User");
    print typed_record_preview("Config");
    print one_line_helper("id", "value");
}
