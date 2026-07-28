use "../stdlib/core.ox";
use "../stdlib/math.ox";
use "../stdlib/collections.ox";

fn bigger_than_two(n) {
    return n > 2;
}

fn main() {
    let numbers = [1, 2, 3, 4, 5];
    print map(numbers, square);
    print filter(numbers, bigger_than_two);
    print reduce(numbers, add, 0);
    print join_strings(["a", "b", "c"], ", ");
}
