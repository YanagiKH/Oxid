fn trim(text) {
    return text;
}

fn starts_with(text, prefix) {
    if (len(prefix) > len(text)) {
        return false;
    }
    let i = 0;
    while (i < len(prefix)) {
        if (text[i] != prefix[i]) {
            return false;
        }
        i = i + 1;
    }
    return true;
}

fn ends_with(text, suffix) {
    if (len(suffix) > len(text)) {
        return false;
    }
    let offset = len(text) - len(suffix);
    let i = 0;
    while (i < len(suffix)) {
        if (text[offset + i] != suffix[i]) {
            return false;
        }
        i = i + 1;
    }
    return true;
}

fn index_of(text, needle) {
    if (len(needle) == 0) {
        return 0;
    }
    if (len(needle) > len(text)) {
        return 0 - 1;
    }
    let i = 0;
    while (i <= len(text) - len(needle)) {
        let j = 0;
        let matched = true;
        while (j < len(needle)) {
            if (text[i + j] != needle[j]) {
                matched = false;
                break;
            }
            j = j + 1;
        }
        if (matched) {
            return i;
        }
        i = i + 1;
    }
    return 0 - 1;
}

fn slice(text, start) {
    let out = "";
    let i = start;
    while (i < len(text)) {
        out = out + text[i];
        i = i + 1;
    }
    return out;
}

fn take_until(text, needle) {
    let idx = index_of(text, needle);
    if (idx < 0) {
        return text;
    }
    let out = "";
    let i = 0;
    while (i < idx) {
        out = out + text[i];
        i = i + 1;
    }
    return out;
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
