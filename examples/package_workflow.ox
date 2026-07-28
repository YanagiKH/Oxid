use "../stdlib/prelude.ox";
use "../stdlib/compiler.ox";

fn main() {
    let manifest = "[project]\nname = \"demo\"\nversion = \"0.5.3\"\nentry = \"src/main.ox\"\n";
    print package_summary(manifest);
    print compile_plan("demo", "0.5.3", "native");
}
