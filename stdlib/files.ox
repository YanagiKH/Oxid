
fn file_read(path) {
    return read_text(path);
}

fn file_write(path, text) {
    return write_text(path, text);
}

fn file_exists(path) {
    return exists(path);
}

fn file_lines(path) {
    return split_lines(read_text(path));
}

fn split_lines(text) {
    let out = [];
    let current = "";
    let i = 0;
    while (i < len(text)) {
        let ch = text[i];
        if (ch == "\n") {
            push(out, current);
            current = "";
        } else {
            current = current + ch;
        }
        i = i + 1;
    }
    if (current != "") {
        push(out, current);
    }
    return out;
}
