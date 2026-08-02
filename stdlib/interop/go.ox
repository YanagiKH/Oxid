use "../strings.ox";

fn go_package(name) { return "package " + name; }
fn go_function(name) { return "func " + name + "()"; }
fn go_script(name) { return "go run " + name + ".go"; }
fn go_import(name) { return "import \"" + name + "\""; }
fn go_call(symbol, signature) { return "go call " + symbol + " : " + signature; }
fn go_export(symbol, signature) { return "go export " + symbol + " : " + signature; }

fn go_bridge_summary() {
    return join_lines([
        go_package("oxidbridge"),
        go_import("os/exec"),
        go_function("Run"),
        go_script("bridge"),
        go_call("Run", "string -> (string, error)"),
        go_export("Run", "string -> (string, error)")
    ], ", ");
}
