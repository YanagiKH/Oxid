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

fn web_service_summary(service_name, version, entry_point) {
    return join_lines([
        "web service: " + service_name,
        "version: " + version,
        "entry: " + entry_point,
        "surface: routing, middleware, async tasks, and adapters"
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
        "routing",
        "request parsing",
        "response building",
        "middleware",
        "adapter isolation"
    ], ", ");
}
