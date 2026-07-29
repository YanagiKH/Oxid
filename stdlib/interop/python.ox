use "../strings.ox";

fn python_module(name) { return "module " + name; }
fn python_function(name) { return "def " + name + "(): ..."; }
fn python_script(name) { return "python " + name + ".py"; }
fn python_import(name) { return "import " + name; }
fn python_call(symbol, signature) { return "python call " + symbol + " : " + signature; }
fn python_export(symbol, signature) { return "python export " + symbol + " : " + signature; }

fn python_bridge_summary() {
    return join_lines([
        python_module("oxid_bridge"),
        python_import("typing"),
        python_function("run"),
        python_script("bridge"),
        python_call("run", "str -> int"),
        python_export("run", "str -> int")
    ], ", ");
}
