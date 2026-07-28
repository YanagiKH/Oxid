fn clamp(value, min, max) {
    if (value < min) { return min; }
    if (value > max) { return max; }
    return value;
}

fn repeat(text, count) {
    let out = "";
    let i = 0;
    while (i < count) {
        out = out + text;
        i = i + 1;
    }
    return out;
}

fn add(a, b) { return a + b; }

fn sum(xs) {
    let total = 0;
    let i = 0;
    while (i < len(xs)) {
        total = total + xs[i];
        i = i + 1;
    }
    return total;
}

fn contains(xs, value) {
    let i = 0;
    while (i < len(xs)) {
        if (xs[i] == value) { return true; }
        i = i + 1;
    }
    return false;
}

fn join_strings(xs, separator) {
    if (len(xs) == 0) { return ""; }
    let out = str(xs[0]);
    let i = 1;
    while (i < len(xs)) {
        out = out + separator + str(xs[i]);
        i = i + 1;
    }
    return out;
}
