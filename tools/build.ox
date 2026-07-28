use "../stdlib/prelude.ox";

fn main() {
    print "Oxid build";
    print command_catalog();
    print frontend_compile_plan("oxid", "0.7.0", "src/main.ox");
    print self_host_command_list();
}
