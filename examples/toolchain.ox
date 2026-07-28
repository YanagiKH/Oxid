use "../stdlib/cache.ox";

fn main() {
    print cache_key("demo", "0.5.3");
    print cache_path("demo", "abcdef");
    print cache_label("demo", "0.5.3");
}
