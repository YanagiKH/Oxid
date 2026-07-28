fn is_even(n) {
    return n == (n / 2) * 2;
}

fn is_odd(n) {
    return !is_even(n);
}

fn clamp_int(value, min_value, max_value) {
    return clamp(value, min_value, max_value);
}

fn sign(n) {
    if (n < 0) {
        return 0 - 1;
    }
    if (n > 0) {
        return 1;
    }
    return 0;
}
