fn main() {
    println!("cargo:rerun-if-changed=native");
    let target = std::env::var("TARGET").expect("Cargo target");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("linux")
        && std::env::var("CARGO_CFG_TARGET_POINTER_WIDTH").as_deref() == Ok("64")
        && std::env::var("CARGO_CFG_TARGET_ENDIAN").as_deref() == Ok("little")
        && matches!(
            std::env::var("CARGO_CFG_TARGET_ARCH").as_deref(),
            Ok("x86_64" | "aarch64")
        )
    {
        let output = std::path::PathBuf::from(std::env::var_os("OUT_DIR").expect("Cargo OUT_DIR"));
        let compiler = std::env::var_os("RUSTC").expect("Cargo RUSTC");
        let root = std::env::var("CARGO_MANIFEST_DIR").expect("Cargo manifest path");
        let status = std::process::Command::new(compiler)
            .args([
                "--edition=2021",
                "--crate-name",
                "oxid_float_format",
                "--crate-type=staticlib",
                "--target",
                &target,
                "-Cpanic=abort",
                "-Copt-level=3",
                "-Ccodegen-units=1",
                "-Cembed-bitcode=no",
                &format!("--remap-path-prefix={root}=."),
                "native/float_format.rs",
                "-o",
            ])
            .arg(output.join("float_format.a"))
            .status()
            .expect("build floating-point formatter");
        assert!(status.success(), "floating-point formatter build failed");
    }

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
