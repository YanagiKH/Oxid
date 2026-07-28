use "../stdlib/prelude.ox";

fn main() {
    print "Oxid lint";
    print lint_suite();
    print lint_preview("source");
    print pipe_preview("source", "check");
}
