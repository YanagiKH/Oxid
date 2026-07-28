use "../stdlib/package.ox";
use "../stdlib/workflow.ox";

fn main() {
    let manifest = "[project]\nname = \"demo\"\nversion = \"0.5.3\"\nentry = \"src/main.ox\"\n\n[scripts]\nrun = \"oxid run src/main.ox\"\ntest = \"oxid test\"\n";
    print package_summary(manifest);
    print workflow_preview("demo");
}
