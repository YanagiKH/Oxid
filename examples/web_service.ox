use "../stdlib/prelude.ox";
use "../stdlib/web.ox";

fn health(body) {
    return web_json(200, "{\"status\":\"ok\"}");
}

fn echo(body) {
    return web_text(200, body);
}

fn main() {
    let routes = [
        web_route_entry("GET", "/health", health),
        web_route_entry("POST", "/echo", echo)
    ];
    print web_runtime_name();
    print web_service_plan("oxid-web-demo", "0.8.0", "src/main.ox");
    print web_dispatch(routes, "GET", "/health", "");
    print web_dispatch(routes, "POST", "/echo", "hello");
    print web_runtime_note();
}
