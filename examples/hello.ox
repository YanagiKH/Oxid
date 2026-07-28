use "../stdlib/core.ox";
use "../stdlib/math.ox";

fn main() {
    print "Oxid startup confirmed";
    print clamp(15, 0, 10);
    print square(7);
    print cube(3);
    print repeat("ox", 3);
}
