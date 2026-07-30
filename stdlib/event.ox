use "strings.ox";

fn event_loop_name() {
    return "Oxid event loop";
}

fn event_runtime_note() {
    return "keep message handlers small and isolate I/O behind adapters";
}

fn event_service_summary(service_name, version, entry_point) {
    return join_lines([
        "event-driven service: " + service_name,
        "version: " + version,
        "entry: " + entry_point,
        "model: async tasks, timers, handlers, and adapters"
    ], "\n");
}

fn event_service_plan(service_name, version, entry_point) {
    return join_lines([
        event_service_summary(service_name, version, entry_point),
        event_runtime_note(),
        event_module_list()
    ], "\n");
}

fn event_module_list() {
    return join_lines([
        "event loop",
        "handler dispatch",
        "background task orchestration",
        "timer callbacks",
        "adapter isolation"
    ], ", ");
}
