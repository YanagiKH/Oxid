fn pipe(value, f) {
    return f(value);
}

fn tee(value, f) {
    f(value);
    return value;
}

fn compose_left(a, b, value) {
    return b(a(value));
}

fn identity(value) {
    return value;
}
