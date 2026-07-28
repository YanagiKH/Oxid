use "../stdlib/package.ox";
use "../stdlib/workflow.ox";

fn main() {
    let manifest = "[project]\nname = \"demo\"\nversion = \"0.5.2\"\nentry = \"src/main.ox\"\n";
    print package_summary(manifest);
    print workflow_preview("demo");
}
