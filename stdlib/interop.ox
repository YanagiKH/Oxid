use "strings.ox";
use "interop/c.ox";
use "interop/cpp.ox";
use "interop/java.ox";
use "interop/python.ox";
use "interop/go.ox";

fn interop_catalog() {
    return join_lines([
        "C: c_call, c_export, c_header",
        "C++: cpp_namespace, cpp_class, cpp_method",
        "Java: java_package, java_class, java_method",
        "Python: python_module, python_function, python_script",
        "Go: go_package, go_function, go_script"
    ], "\n");
}

fn interop_bridge_summary() {
    return join_lines([
        c_bridge_summary(),
        cpp_bridge_summary(),
        java_bridge_summary(),
        python_bridge_summary(),
        go_bridge_summary()
    ], "\n");
}

fn interop_quickstart() {
    return join_lines([
        "1. declare the foreign target",
        "2. generate a tiny bridge stub",
        "3. link the native library or runtime",
        "4. call it from Oxid",
        "5. expose Oxid back to the foreign host"
    ], "\n");
}

fn bridge_steps() { return interop_quickstart(); }

fn interop_goal() {
    return "simple steps for C/C++, Java, Python, Go, and Oxid both directions";
}
