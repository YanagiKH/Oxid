use "core.ox";

fn first(xs) {
    return xs[0];
}

fn last(xs) {
    return xs[len(xs) - 1];
}

fn take(xs, count) {
    let out = [];
    let i = 0;
    while (i < count and i < len(xs)) {
        push(out, xs[i]);
        i = i + 1;
    }
    return out;
}

fn drop(xs, count) {
    let out = [];
    let i = count;
    while (i < len(xs)) {
        push(out, xs[i]);
        i = i + 1;
    }
    return out;
}

fn reverse(xs) {
    let out = [];
    let i = len(xs);
    while (i > 0) {
        i = i - 1;
        push(out, xs[i]);
    }
    return out;
}

fn index_of(xs, value) {
    let i = 0;
    while (i < len(xs)) {
        if (xs[i] == value) {
            return i;
        }
        i = i + 1;
    }
    return 0 - 1;
}
