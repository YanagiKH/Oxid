use "../stdlib/prelude.ox";

fn main() {
    print c_header("stdint.h");
    print cpp_class("Bridge");
    print java_class("Bridge");
    print python_module("oxid");
    print interop_goal();
}
