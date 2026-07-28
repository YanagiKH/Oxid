fn launch(task_func, value) {
    return spawn(task_func, value);
}

fn wait_for(task) {
    return join(task);
}

fn wait_for_all(tasks) {
    return join_all(tasks);
}
