fn square(n) { return n * n; }
fn cube(n) { return n * n * n; }
fn abs(n) { if (n < 0) { return 0 - n; } return n; }
fn average(values) {
    if len(values) == 0 { return 0; }
    let total = 0;
    for value in values { total = total + value; }
    return total / len(values);
}
