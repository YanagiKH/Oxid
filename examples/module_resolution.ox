use "../stdlib/prelude.ox";

fn main() {
    print module_group("frontend");
    print module_resolve("src", "frontend/parser.ox");
    print module_key("src", "frontend/parser.ox");
    print resolve_relative("src", "./frontend/parser.ox");
}
