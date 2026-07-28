fn main() {
    cc::Build::new()
        .file("native/oxid_ffi.c")
        .file("native/oxid_cpp_bridge.cpp")
        .include("native")
        .cpp(true)
        .flag_if_supported("-std=c++17")
        .warnings(false)
        .compile("oxidffi");
}
