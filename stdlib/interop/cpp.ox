use "../strings.ox";

fn cpp_namespace(name) { return "namespace " + name + " { ... }"; }
fn cpp_class(name) { return "class " + name + " { ... }"; }
fn cpp_method(name) { return "method " + name + "()"; }
fn cpp_constructor(name) { return name + "()"; }
fn cpp_call(symbol, signature) { return "cpp call " + symbol + " : " + signature; }
fn cpp_export(symbol, signature) { return "cpp export " + symbol + " : " + signature; }
fn cpp_bind(symbol, signature) { return cpp_call(symbol, signature) + " | " + cpp_export(symbol, signature); }

fn cpp_bridge_summary() {
    return join_lines([
        cpp_namespace("oxid"),
        cpp_class("Bridge"),
        cpp_method("run"),
        cpp_constructor("Bridge"),
        cpp_bind("Bridge::run", "int(string)")
    ], ", ");
}
