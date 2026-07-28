use "../stdlib/package.ox";
use "../stdlib/cache.ox";

fn main() {
    print package_summary("name = "demo"
version = "0.5.2"
entry = "src/main.ox"");
    print cache_label("demo", "0.5.2");
}
