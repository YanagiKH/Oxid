// String contract: remove only boundary U+0009..U+000D and U+0020.
// Preserve interior whitespace and all non-ASCII characters (including whitespace).
// Empty/all-whitespace inputs yield "". Legacy source and OXBC use this contract.
fn trim(text) {
    // Decode a fixed constant: the legacy lexer has no \v, \f or \u escapes.
    let whitespace = json_parse("\"\\u0009\\u000a\\u000b\\u000c\\u000d\\u0020\"");
    // Split once and join once: O(n) time and storage, without repeated string indexing.
    let chars = split(text, "");
    let start = 0;
    let end = len(chars);
    while (start < end) {
        if (index_of(whitespace, chars[start]) < 0) { break; }
        start = start + 1;
    }
    while (end > start) {
        if (index_of(whitespace, chars[end - 1]) < 0) { break; }
        end = end - 1;
    }
    let out = [];
    while (start < end) {
        push(out, chars[start]);
        start = start + 1;
    }
    return join_text(out, "");
}

fn starts_with(text, prefix) {
    if (len(prefix) > len(text)) { return false; }
    let i = 0;
    while (i < len(prefix)) {
        if (text[i] != prefix[i]) { return false; }
        i = i + 1;
    }
    return true;
}

fn ends_with(text, suffix) {
    if (len(suffix) > len(text)) { return false; }
    let offset = len(text) - len(suffix);
    let i = 0;
    while (i < len(suffix)) {
        if (text[offset + i] != suffix[i]) { return false; }
        i = i + 1;
    }
    return true;
}

fn index_of(text, needle) {
    if (len(needle) == 0) { return 0; }
    if (len(needle) > len(text)) { return 0 - 1; }
    let i = 0;
    while (i <= len(text) - len(needle)) {
        let j = 0;
        let matched = true;
        while (j < len(needle)) {
            if (text[i + j] != needle[j]) { matched = false; break; }
            j = j + 1;
        }
        if (matched) { return i; }
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
    if (idx < 0) { return text; }
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
    if (len(text) >= width) { return text; }
    return repeat_char(fill, width - len(text)) + text;
}

fn pad_right(text, width, fill) {
    if (len(text) >= width) { return text; }
    return text + repeat_char(fill, width - len(text));
}

fn quote(text) { return "\"" + text + "\""; }

fn join_lines(lines, separator) {
    if (len(lines) == 0) { return ""; }
    let out = lines[0];
    let i = 1;
    while (i < len(lines)) {
        out = out + separator + lines[i];
        i = i + 1;
    }
    return out;
}

fn repeat_line(line, count, separator) {
    let out = "";
    let i = 0;
    while (i < count) {
        if (i > 0) { out = out + separator; }
        out = out + line;
        i = i + 1;
    }
    return out;
}

fn wrap(prefix, body, suffix) { return prefix + body + suffix; }
