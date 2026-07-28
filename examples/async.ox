use "../stdlib/async.ox";

async fn greet(name) {
    return "Hello, " + name;
}

fn main() {
    let task = spawn_task(greet, "Oxid");
    print status(task);
    print await_task(task);
    print yield_control();
}
