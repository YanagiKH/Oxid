use "../stdlib/prelude.ox";

fn main() {
    print "Oxid bootstrap command";
    print "This command previews the bootstrap path for the compiler front-end.";
    print frontend_pipeline("bootstrap");
}
