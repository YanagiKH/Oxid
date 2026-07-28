use "../strings.ox";

fn normalize_module_path(path_text) { return trim(path_text); }
fn module_group(name) { return "mod " + name + " { ... }"; }
fn import_alias(path_text, alias) { return "use " + path_text + " as " + alias; }
fn module_key(root, path_text) { return root + "::" + normalize_module_path(path_text); }
fn module_resolve(root, path_text) { return root + "/" + normalize_module_path(path_text); }
fn module_preview(name, path_text) { return module_group(name) + " -> " + module_resolve(".", path_text); }

fn resolve_relative(base_dir, import_path) {
    if (starts_with(import_path, ".")) { return base_dir + "/" + import_path; }
    return import_path;
}

fn module_search_hint(name) {
    return "search module: " + name;
}

fn module_catalog() {
    return join_lines([
        module_group("frontend"),
        import_alias("stdlib/frontend/parser.ox", "parse"),
        module_preview("frontend", "stdlib/frontend/parser.ox"),
        module_key("src", "frontend/parser.ox")
    ], ", ");
}
