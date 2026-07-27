fn clamp(value, min, max) {
    if (value < min) {
        return min;
    }
    if (value > max) {
        return max;
    }
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

fn map(xs, f) {
    let out = [];
    let i = 0;
    while (i < len(xs)) {
        push(out, f(xs[i]));
        i = i + 1;
    }
    return out;
}

fn filter(xs, f) {
    let out = [];
    let i = 0;
    while (i < len(xs)) {
        let item = xs[i];
        if (f(item)) {
            push(out, item);
        }
        i = i + 1;
    }
    return out;
}

fn reduce(xs, f, initial) {
    let acc = initial;
    let i = 0;
    while (i < len(xs)) {
        acc = f(acc, xs[i]);
        i = i + 1;
    }
    return acc;
}

fn sum(xs) {
    return reduce(xs, add, 0);
}

fn add(a, b) {
    return a + b;
}
