fn main() {
    cc::Build::new()
        .cpp(true)
        .file("native/oxid_ffi.c")
        .file("native/oxid_cpp_bridge.cpp")
        .include("native")
        .warnings(false)
        .flag_if_supported("-std=c++17")
        .compile("oxidffi");
}
