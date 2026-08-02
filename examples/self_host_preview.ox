use "../stdlib/prelude.ox";

fn main() {
    print self_host_plan("oxid", "0.8.0", "src/main.ox");
    print self_host_command_list();
}
