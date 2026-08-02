use "strings.ox";
use "event.ox";

fn web_runtime_name() {
    return "Oxid web runtime profile";
}

fn web_runtime_note() {
    return "keep HTTP handlers thin and move business logic into reusable Oxid modules";
}

fn web_route(method, path) {
    return method + " " + path;
}

fn web_route_entry(method, path, handler) {
    return [method, path, handler];
}

fn web_text(status, body) {
    return web_response(status, "text/plain; charset=utf-8", body);
}

fn web_json(status, body) {
    return web_response(status, "application/json; charset=utf-8", body);
}

fn web_dispatch(routes, method, path, body) {
    for route in routes {
        if route[0] == method and route[1] == path {
            return route[2](body);
        }
    }
    return web_text(404, "Not Found");
}

fn web_listen_once(host, port, routes, method, path, body) {
    let response = web_dispatch(routes, method, path, body);
    return web_serve_once(host, port, response);
}

fn web_service_summary(service_name, version, entry_point) {
    return join_lines([
        "web service: " + service_name,
        "version: " + version,
        "entry: " + entry_point,
        "surface: routing, dispatch, HTTP responses, one-shot serving, middleware, async tasks, and adapters"
    ], "\n");
}

fn web_service_plan(service_name, version, entry_point) {
    return join_lines([
        web_service_summary(service_name, version, entry_point),
        event_runtime_note(),
        web_module_list()
    ], "\n");
}

fn web_module_list() {
    return join_lines([
        "routing and local dispatch",
        "request parsing",
        "response building",
        "TCP HTTP serving",
        "middleware",
        "adapter isolation"
    ], ", ");
}
