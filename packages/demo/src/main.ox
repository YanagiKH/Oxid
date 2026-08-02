use "lib.ox";
use "../../../stdlib/core.ox";

fn main() {
    print package_banner("demo", "0.5.2");
    print greet("world");
    print describe_package("demo", "0.5.2");
    print repeat("-", 12);
}
