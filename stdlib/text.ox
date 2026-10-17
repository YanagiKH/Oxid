use "strings.ox";

// Map only ASCII letters; preserve every other character unchanged.
// Split input once and join once; the alphabet lookup is fixed at 26 letters.
fn upper(text) {
    let alphabet = "abcdefghijklmnopqrstuvwxyz";
    let mapped = "ABCDEFGHIJKLMNOPQRSTUVWXYZ";
    let chars = split(text, "");
    let out = [];
    let i = 0;
    while (i < len(chars)) {
        let ch = chars[i];
        let position = index_of(alphabet, ch);
        if (position < 0) { push(out, ch); }
        else { push(out, mapped[position]); }
        i = i + 1;
    }
    return join_text(out, "");
}

fn lower(text) {
    let alphabet = "ABCDEFGHIJKLMNOPQRSTUVWXYZ";
    let mapped = "abcdefghijklmnopqrstuvwxyz";
    let chars = split(text, "");
    let out = [];
    let i = 0;
    while (i < len(chars)) {
        let ch = chars[i];
        let position = index_of(alphabet, ch);
        if (position < 0) { push(out, ch); }
        else { push(out, mapped[position]); }
        i = i + 1;
    }
    return join_text(out, "");
}

fn quote(text) {
    return "\"" + text + "\"";
}

fn surround(text, left, right) {
    return left + text + right;
}
