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
