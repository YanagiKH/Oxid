use "lib.ox";
use "../../stdlib/prelude.ox";

fn main() {
    print package_banner("demo", "0.6.1");
    print greet("world");
    print describe_package("demo", "0.6.1");
    print repeat("-", 12);
}
