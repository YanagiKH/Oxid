use "lib.ox";
use "../../stdlib/prelude.ox";

fn main() {
    print package_banner("demo", "0.9.0");
    print greet("world");
    print describe_package("demo", "0.9.0");
    print repeat("-", 12);
    print frontend_pipeline("packages/demo/src/main.ox");
}
