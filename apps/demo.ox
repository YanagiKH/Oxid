use "../stdlib/core.ox";
use "../stdlib/math.ox";
use "../stdlib/collections.ox";

fn main() {
    let numbers = range(0, 10);
    print map(numbers, square);
    print filter(numbers, is_large);
    print sum(numbers);
}

fn is_large(n) {
    return n > 4;
}
