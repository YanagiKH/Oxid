fn ensure(condition, message) {
    if (condition) {
        return true;
    }
    print message;
    return false;
}

fn describe(value) {
    return type_of(value);
}

fn require_file(path) {
    return ensure(file_exists(path), "missing file: " + path);
}
