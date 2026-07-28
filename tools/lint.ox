use "../stdlib/prelude.ox";

fn main() {
    print "Oxid lint command";
    print "Lint rules stay short, explicit, and source-first.";
    print match_preview("node");
    print pipe_preview("source", "check");
}
