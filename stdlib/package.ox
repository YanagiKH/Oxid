use "core.ox";
use "strings.ox";

fn package_name(manifest_text) {
    let marker = "name = \"";
    let start = index_of(manifest_text, marker);
    if (start < 0) {
        return "unknown";
    }
    let rest = slice(manifest_text, start + len(marker));
    return take_until(rest, "\"");
}

fn package_version(manifest_text) {
    let marker = "version = \"";
    let start = index_of(manifest_text, marker);
    if (start < 0) {
        return "unknown";
    }
    let rest = slice(manifest_text, start + len(marker));
    return take_until(rest, "\"");
}

fn package_entry(manifest_text) {
    let marker = "entry = \"";
    let start = index_of(manifest_text, marker);
    if (start < 0) {
        return "src/main.ox";
    }
    let rest = slice(manifest_text, start + len(marker));
    return take_until(rest, "\"");
}

fn package_summary(manifest_text) {
    return package_name(manifest_text) + " " + package_version(manifest_text) + " -> " + package_entry(manifest_text);
}
