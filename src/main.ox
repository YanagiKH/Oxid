use "../stdlib/prelude.ox";

fn main() {
    print "Oxid boot";
    print frontend_bootstrap("oxid", "src/main.ox");
    print frontend_pipeline("src/main.ox");
    print repeat(">", 3);
    print clamp(42, 0, 10);
}
