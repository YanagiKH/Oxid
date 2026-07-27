async fn load_message(name) {
    return "Hello, " + name;
}

fn main() {
    let task = load_message("Oxid");
    print await task;
}
