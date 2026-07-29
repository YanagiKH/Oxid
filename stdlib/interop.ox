use "strings.ox";
use "interop/c.ox";
use "interop/cpp.ox";
use "interop/java.ox";
use "interop/python.ox";

fn interop_catalog() {
    return join_lines([
        "C: c_call, c_export, c_header",
        "C++: cpp_namespace, cpp_class, cpp_method",
        "Java: java_package, java_class, java_method",
        "Python: python_module, python_function, python_script"
    ], "
");
}

fn interop_bridge_summary() {
    return join_lines([
        c_bridge_summary(),
        cpp_bridge_summary(),
        java_bridge_summary(),
        python_bridge_summary()
    ], "
");
}

fn interop_quickstart() {
    return join_lines([
        "1. declare the foreign target",
        "2. generate a tiny bridge stub",
        "3. link the native library or runtime",
        "4. call it from Oxid",
        "5. expose Oxid back to the foreign host"
    ], "
");
}

fn interop_goal() {
    return "simple steps for C/C++, Java, Python, and Oxid both directions";
}
