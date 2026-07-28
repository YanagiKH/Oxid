use "../../stdlib/prelude.ox";
use "../src/lib.ox";

fn main() {
    print greet("world");
    print describe_package("demo", "0.7.0");
    print package_banner("demo", "0.7.0");
}
