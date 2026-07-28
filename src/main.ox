use "../stdlib/core.ox";
use "../stdlib/math.ox";

fn main() {
    print "Oxid boot script";
    print repeat(">", 3);
    print clamp(42, 0, 10);
    print square(9);
}
