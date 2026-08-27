fn json_decode(text) {
    return json_parse(text);
}

fn json_encode(value) {
    return json_stringify(value);
}

fn json_clone(value) {
    return json_parse(json_stringify(value));
}

fn json_object(pairs) {
    return record(pairs);
}

fn json_read_file(path) {
    return json_parse(read_text(path));
}

fn json_write_file(path, value) {
    return write_text(path, json_stringify(value) + "\n");
}
