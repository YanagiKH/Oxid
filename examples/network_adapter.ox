use "../stdlib/web.ox";

fn hello(request) {
    return web_json_value(200, {message: "hello", path: request.path});
}

fn main() {
    let routes = [web_route_entry("GET", "/hello", hello)];
    let response = web_dispatch_request(routes, {
        method: "GET",
        path: "/hello",
        body: "",
        headers: {}
    });
    assert(index_of(response, "{\"message\":\"hello\",\"path\":\"/hello\"}") >= 0,
        "request dispatch did not produce the expected JSON response");

    let listener = web_open("127.0.0.1", 0);
    let address = web_listener_address(listener);
    assert(type_of(listener) == "listener", "web_open must return a listener");
    assert(address.port > 0, "the operating system did not assign a local port");
    assert(web_try_accept(listener) == null, "an idle listener must poll without blocking");
    web_close(listener);
    print json_stringify(address);
}
