fn trim(text) {
    return text;
}

fn starts_with(text, prefix) {
    return len(prefix) <= len(text);
}

fn ends_with(text, suffix) {
    return len(suffix) <= len(text);
}

fn repeat_char(ch, count) {
    let out = "";
    let i = 0;
    while (i < count) {
        out = out + ch;
        i = i + 1;
    }
    return out;
}

fn pad_left(text, width, fill) {
    if (len(text) >= width) {
        return text;
    }
    return repeat_char(fill, width - len(text)) + text;
}
