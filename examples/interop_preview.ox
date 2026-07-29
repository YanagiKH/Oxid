use "../stdlib/prelude.ox";

fn main() {
    print interop_catalog();
    print c_bridge_summary();
    print cpp_bridge_summary();
    print java_bridge_summary();
    print python_bridge_summary();
    print bridge_steps();
}
