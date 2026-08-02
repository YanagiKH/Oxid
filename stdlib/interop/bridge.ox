use "../strings.ox";

fn bridge_stub(name) { return "bridge stub: " + name; }
fn bridge_export(name) { return "export " + name; }
fn bridge_import(name) { return "import " + name; }
fn bridge_roundtrip(name) { return bridge_import(name) + " <-> " + bridge_export(name); }
fn bridge_plan(name) {
    return join_lines([
        bridge_stub(name),
        bridge_roundtrip(name),
        "keep the Oxid side readable",
        "keep the foreign shim minimal"
    ], ", ");
}

fn bridge_summary() { return "Oxid bidirectional process bridge"; }
fn bridge_catalog() { return "Python, Java, Go, C, C++"; }
