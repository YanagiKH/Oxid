fn record_new() {
    return {};
}

fn record_from_pairs(pairs) {
    return record(pairs);
}

fn record_keys(value) {
    return keys(value);
}

fn record_has(value, key) {
    return has_key(value, key);
}

fn record_get(value, key, fallback) {
    return get(value, key, fallback);
}

fn record_put(value, key, item) {
    return set(value, key, item);
}

fn record_delete(value, key) {
    return remove(value, key);
}

fn record_merge_into(target, source) {
    for key in keys(source) {
        set(target, key, get(source, key));
    }
    return target;
}

fn record_select(value, selected_keys) {
    let selected = {};
    for key in selected_keys {
        if has_key(value, key) {
            set(selected, key, get(value, key));
        }
    }
    return selected;
}

fn record_json_copy(value) {
    return json_parse(json_stringify(value));
}
