use "../stdlib/prelude.ox";

fn main() {
    print "Oxid syntax command";
    print exported_fn("run");
    print import_alias("stdlib/frontend/pipeline.ox", "pipeline");
    print match_preview("value");
    print try_preview("expr");
    print defer_preview("cleanup()");
    print pipe_preview("value", "step()");
}
