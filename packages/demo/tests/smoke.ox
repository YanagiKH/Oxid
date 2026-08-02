use "../../../stdlib/core.ox";
use "../src/lib.ox";

fn main() {
    print greet("world");
    print describe_package("demo", "0.5.2");
    print package_banner("demo", "0.5.2");
}
