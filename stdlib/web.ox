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

fn web_json_value(status, value) {
    return web_json(status, json_stringify(value));
}

fn web_response_record(status, content_type, body) {
    return {
        status: status,
        headers: {"Content-Type": content_type},
        body: body
    };
}

fn web_text_record(status, body) {
    return web_response_record(status, "text/plain; charset=utf-8", body);
}

fn web_json_record(status, value) {
    return web_response_record(status, "application/json; charset=utf-8", json_stringify(value));
}

fn web_dispatch(routes, method, path, body) {
    for route in routes {
        if route[0] == method and route[1] == path {
            return route[2](body);
        }
    }
    return web_text(404, "Not Found");
}

fn web_dispatch_request(routes, request) {
    for route in routes {
        if route[0] == request.method and route[1] == request.path {
            return route[2](request);
        }
    }
    return web_text(404, "Not Found");
}

fn web_open(host, port) {
    return net_listen(host, port);
}

fn web_listener_address(listener) {
    return net_local_addr(listener);
}

fn web_accept(listener, timeout_ms) {
    return net_accept(listener, timeout_ms);
}

fn web_try_accept(listener) {
    return net_try_accept(listener);
}

fn web_read_request(connection, timeout_ms) {
    return http_read_request(connection, timeout_ms);
}

fn web_write(connection, response, timeout_ms) {
    return http_write_response(connection, response, timeout_ms);
}

fn web_close(adapter) {
    return net_close(adapter);
}

fn web_serve_connection(connection, routes, timeout_ms) {
    let request = web_read_request(connection, timeout_ms);
    let response = web_dispatch_request(routes, request);
    let bytes_written = web_write(connection, response, timeout_ms);
    web_close(connection);
    return {request: request, bytes_written: bytes_written};
}

fn web_accept_and_serve(listener, routes, timeout_ms) {
    let connection = web_accept(listener, timeout_ms);
    return web_serve_connection(connection, routes, timeout_ms);
}

fn web_serve_requests(listener, routes, request_limit, timeout_ms) {
    let served = [];
    let index = 0;
    while index < request_limit {
        push(served, web_accept_and_serve(listener, routes, timeout_ms));
        index = index + 1;
    }
    return served;
}

fn web_serve_forever(listener, routes, timeout_ms) {
    while true {
        let connection = web_try_accept(listener);
        if connection == null {
            sleep_ms(2);
            continue;
        }
        web_serve_connection(connection, routes, timeout_ms);
    }
}

fn web_serve_response_once(listener, response, timeout_ms) {
    return web_serve_once(listener, response, timeout_ms);
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
        "surface: routing, dispatch, structured responses, reusable listeners, bounded or long-running serving, and adapters"
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
