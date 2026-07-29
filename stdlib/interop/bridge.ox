use "../strings.ox";
use "c.ox";
use "cpp.ox";
use "java.ox";
use "python.ox";

fn bridge_target(name) {
    return "bridge target: " + name;
}

fn bridge_summary() {
    return join_lines([
        bridge_target("C"),
        bridge_target("C++"),
        bridge_target("Java"),
        bridge_target("Python")
    ], ", ");
}

fn bridge_steps() {
    return join_lines([
        "generate a stub",
        "compile the foreign library or runtime",
        "bind symbols",
        "run a smoke test",
        "call Oxid back from the host"
    ], "
");
}

fn bridge_catalog() {
    return join_lines([
        c_bridge_summary(),
        cpp_bridge_summary(),
        java_bridge_summary(),
        python_bridge_summary()
    ], "
");
}
