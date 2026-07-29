use "../strings.ox";

fn python_module(name) { return "module " + name; }
fn python_function(name) { return "def " + name + "(): ..."; }
fn python_script(name) { return "python " + name + ".py"; }
fn python_bind(symbol, signature) { return "python bind " + symbol + " : " + signature; }
fn python_export(symbol, signature) { return "python export " + symbol + " : " + signature; }
fn python_virtualenv_hint(name) { return "python -m venv " + name; }

fn python_bridge_summary() {
    return join_lines([
        python_module("oxid"),
        python_function("run"),
        python_script("bridge"),
        python_virtualenv_hint(".venv"),
        python_bind("run", "str(list[str])"),
        python_export("oxid_entry", "str()")
    ], ", ");
}
