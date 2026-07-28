use "../strings.ox";
use "../core.ox";

fn module_base(path_text) {
    let idx = index_of(path_text, "/");
    if (idx < 0) {
        return path_text;
    }
    return slice(path_text, idx + 1);
}

fn module_name(path_text) {
    let base = module_base(path_text);
    let dot = index_of(base, ".");
    if (dot < 0) {
        return base;
    }
    return slice(base, 0) + take_until(base, ".");
}

fn module_alias(path_text) {
    return module_name(path_text);
}

fn normalize_import(path_text) {
    return trim(path_text);
}

fn resolve_relative(base_dir, import_path) {
    if (starts_with(import_path, ".")) {
        return base_dir + "/" + import_path;
    }
    return import_path;
}

fn module_cache_key(base_dir, import_path) {
    return base_dir + "::" + normalize_import(import_path);
}

fn module_load_hint(base_dir, import_path) {
    return "resolve " + import_path + " from " + base_dir;
}

fn module_graph_edge(from_name, to_name) {
    return from_name + " -> " + to_name;
}
