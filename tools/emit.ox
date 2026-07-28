use "../stdlib/prelude.ox";

fn main() {
    print "Oxid emit command";
    print "Emit preview only: source -> lowered -> backend";
    print frontend_pipeline("emit");
}
