use "../stdlib/core.ox";
use "../stdlib/strings.ox";
use "../stdlib/numbers.ox";
use "../stdlib/commands.ox";
use "../stdlib/self_host.ox";

fn main() {
    print repeat("ox", 2);
    print quote("Oxid");
    print pad_left("7", 3, "0");
    print command_catalog();
    print self_host_command_list();
    print is_even(10);
    print is_odd(9);
}
