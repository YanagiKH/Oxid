use "../stdlib/core.ox";
use "../stdlib/math.ox";

fn greater_than_two(n) {
    return n > 2;
}

fn main() {
    let numbers = [1, 2, 3, 4, 5];
    print map(numbers, square);
    print filter(numbers, greater_than_two);
    print reduce(numbers, add, 0);
    print clamp(15, 0, 10);
    print repeat("ox", 3);
    print abs(0 - 7);
    print average(numbers);
}
