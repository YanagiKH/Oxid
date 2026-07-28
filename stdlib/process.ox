fn current_dir() {
    return cwd();
}

fn environment(key) {
    return env(key);
}

fn wait_ms(ms) {
    return sleep_ms(ms);
}

fn wait(value) {
    return sleep(value);
}
