use "../stdlib/prelude.ox";

fn main() {
    print module_graph_edge("src/main.ox", "stdlib/prelude.ox");
    print module_load_hint("src", "frontend/parser.ox");
    print module_cache_key("src", "frontend/parser.ox");
    print resolve_relative("src", "frontend/parser.ox");
}
