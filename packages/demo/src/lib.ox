fn greet(name) {
    return "Hello, " + name;
}

fn describe_package(name, version) {
    return name + " v" + version;
}

fn package_banner(name, version) {
    return "[" + describe_package(name, version) + "]";
}

fn package_summary(name, version) {
    return describe_package(name, version);
}
