use "../stdlib/core.ox";
use "../stdlib/collections.ox";

fn main() {
    let xs = [1, 2, 3, 4, 5];
    print map(xs, add_one);
    print filter(xs, is_big);
    print reduce(xs, add, 0);
    print take(xs, 3);
    print drop(xs, 2);
    print reverse(xs);
}

fn add_one(n) {
    return n + 1;
}

fn is_big(n) {
    return n > 2;
}
