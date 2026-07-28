use "../stdlib/prelude.ox";

fn main() {
    print "Oxid frontend command";
    print frontend_pipeline("frontend.ox");
    print exported_fn("compile");
    print local_group("parser");
}
