fn main() {
    cc::Build::new()
        .file("native/oxid_ffi.c")
        .include("native")
        .warnings(false)
        .compile("oxidffi_c");

    cc::Build::new()
        .cpp(true)
        .file("native/oxid_cpp_bridge.cpp")
        .include("native")
        .flag_if_supported("-std=c++17")
        .warnings(false)
        .compile("oxidffi_cpp");
}
