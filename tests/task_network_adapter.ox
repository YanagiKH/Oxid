use "../stdlib/async.ox";
use "../stdlib/web.ox";

fn increment(value) {
    return value + 1;
}

fn write_created_response(connection) {
    return web_write(connection, web_json_record(201, {created: true}), 1000);
}

fn main() {
    let task = spawn_task(increment, 41);
    assert(status(task) == "pending", "new tasks must begin pending");
    assert(await_task(task) == 42, "task returned the wrong result");
    assert(status(task) == "completed", "joined tasks must be completed");
    assert(await_task(task) == 42, "completed task results must be reusable");

    let listener = web_open("127.0.0.1", 0);
    let address = web_listener_address(listener);
    assert(type_of(address) == "record", "listener address must be a record");
    assert(address.host == "127.0.0.1", "listener must remain loopback-only");
    assert(address.port > 0, "listener must expose its assigned port");
    assert(web_try_accept(listener) == null, "idle listener polling must be nonblocking");
    assert(len(web_serve_requests(listener, [], 0, 10)) == 0,
        "a zero-request server run must terminate without accepting");

    let response = web_json_record(201, {created: true});
    assert(response.status == 201, "structured response status was lost");
    assert(get(response.headers, "Content-Type") == "application/json; charset=utf-8",
        "structured response content type was lost");
    assert(response.body == "{\"created\":true}", "structured response JSON is not canonical");
    assert(type_of(web_write) == "function", "structured HTTP writer wrapper is unavailable");
    web_close(listener);
}
