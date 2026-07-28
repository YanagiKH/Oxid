fn spawn_task(func, value) {
    return spawn(func, value);
}

fn await_task(task) {
    return join(task);
}

fn await_all(tasks) {
    return join_all(tasks);
}

fn status(task) {
    return task_status(task);
}

fn yield_control() {
    return yield_now();
}
