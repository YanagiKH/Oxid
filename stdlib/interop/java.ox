use "../strings.ox";

fn java_package(name) { return "package " + name + ";"; }
fn java_class(name) { return "class " + name + " { ... }"; }
fn java_method(name) { return "void " + name + "()"; }
fn java_import(name) { return "import " + name + ";"; }
fn java_call(symbol, signature) { return "java call " + symbol + " : " + signature; }
fn java_export(symbol, signature) { return "java export " + symbol + " : " + signature; }

fn java_bridge_summary() {
    return join_lines([
        java_package("oxid.bridge"),
        java_import("java.util.List"),
        java_class("Bridge"),
        java_method("run"),
        java_call("Bridge.run", "String -> int"),
        java_export("Bridge.run", "String -> int")
    ], ", ");
}
