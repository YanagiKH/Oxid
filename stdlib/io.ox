fn write_line(path, text) {
    return write_text(path, text + "
");
}

fn read_lines(path) {
    return split_lines(read_text(path));
}

fn safe_exists(path) {
    return exists(path);
}
