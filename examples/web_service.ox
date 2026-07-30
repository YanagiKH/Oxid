use "../stdlib/prelude.ox";
use "../stdlib/web.ox";

fn main() {
    print web_runtime_name();
    print web_service_plan("oxid-web-demo", "0.7.0", "src/main.ox");
    print web_route("GET", "/health");
    print web_route("POST", "/events");
    print web_runtime_note();
}
