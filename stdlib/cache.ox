fn cache_key(name, version) {
    return name + "@" + version;
}

fn cache_path(project_name, fingerprint) {
    return ".oxid/cache/" + project_name + "/" + fingerprint + ".oxp";
}

fn cache_label(project_name, version) {
    return "cache(" + cache_key(project_name, version) + ")";
}
