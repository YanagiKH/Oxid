use "../stdlib/prelude.ox";

fn main() {
    print "Oxid self-host";
    print self_host_command_list();
    print self_host_boundary_note();
    print self_host_plan("oxid", "0.8.0", "src/main.ox");
}