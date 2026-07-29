use "../strings.ox";

fn java_package(name) { return "package " + name + ";"; }
fn java_class(name) { return "public class " + name + " { ... }"; }
fn java_method(name) { return "public static void " + name + "()"; }
fn java_signature(name) { return "java signature " + name; }
fn java_bridge_class(name) { return "JNI bridge " + name; }
fn java_bind(symbol, signature) { return "java bind " + symbol + " : " + signature; }
fn java_export(symbol, signature) { return "java export " + symbol + " : " + signature; }

fn java_bridge_summary() {
    return join_lines([
        java_package("io.oxid"),
        java_class("Bridge"),
        java_method("main"),
        java_signature("main(String[])"),
        java_bind("Bridge.main", "void(String[])"),
        java_export("oxidEntry", "void()")
    ], ", ");
}
