use "../stdlib/prelude.ox";
use "../stdlib/bots/discord.ox";

fn main() {
    print discord_runtime_name();
    print discord_bot_plan("oxid-discord-demo", "0.7.0", "src/main.ox");
    print discord_gateway_plan();
    print discord_bot_command_list();
    print discord_runtime_note();
}
