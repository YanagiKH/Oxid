fn square(n) {
    return n * n;
}

fn cube(n) {
    return n * n * n;
}

fn pow4(n) {
    return square(square(n));
}

fn abs(n) {
    if (n < 0) {
        return 0 - n;
    }
    return n;
}

fn max(a, b) {
    if (a > b) {
        return a;
    }
    return b;
}

fn min(a, b) {
    if (a < b) {
        return a;
    }
    return b;
}

fn average(xs) {
    if (len(xs) == 0) {
        return 0;
    }
    return sum(xs) / len(xs);
}
