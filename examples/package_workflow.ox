use "../stdlib/prelude.ox";

fn main() {
    let manifest = package_manifest_header("demo", "0.7.0", "src/main.ox");
    print package_summary(manifest);
    print package_scripts_hint();
    print package_load_hint("demo", "src/main.ox");
}
