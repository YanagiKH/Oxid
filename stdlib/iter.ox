fn for_each(xs, f) {
    let i = 0;
    while (i < len(xs)) {
        f(xs[i]);
        i = i + 1;
    }
    return true;
}

fn count_if(xs, f) {
    let count = 0;
    let i = 0;
    while (i < len(xs)) {
        if (f(xs[i])) {
            count = count + 1;
        }
        i = i + 1;
    }
    return count;
}
