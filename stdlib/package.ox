use "strings.ox";

fn package_name(manifest_text) {
    let marker = "name = \"";
    let start = index_of(manifest_text, marker);
    if (start < 0) { return "unknown"; }
    let rest = slice(manifest_text, start + len(marker));
    return take_until(rest, "\"");
}

fn package_version(manifest_text) {
    let marker = "version = \"";
    let start = index_of(manifest_text, marker);
    if (start < 0) { return "unknown"; }
    let rest = slice(manifest_text, start + len(marker));
    return take_until(rest, "\"");
}

fn package_entry(manifest_text) {
    let marker = "entry = \"";
    let start = index_of(manifest_text, marker);
    if (start < 0) { return "src/main.ox"; }
    let rest = slice(manifest_text, start + len(marker));
    return take_until(rest, "\"");
}

fn package_summary(manifest_text) {
    return package_name(manifest_text) + " " + package_version(manifest_text) + " -> " + package_entry(manifest_text);
}

fn package_manifest_header(name, version, entry) {
    return "[project]\nname = \"" + name + "\"\nversion = \"" + version + "\"\nentry = \"" + entry + "\"\n";
}

fn package_scripts_hint() {
    return "bootstrap, compile, self-compile, frontend, diagnose, lint, emit, module, syntax, self-host";
}

fn package_load_hint(name, entry) {
    return "load " + name + " from " + entry;
}