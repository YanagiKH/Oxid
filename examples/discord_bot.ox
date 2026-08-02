use "../stdlib/prelude.ox";
use "../stdlib/bots/discord.ox";

fn ping(payload) {
    return discord_reply("Pong: " + payload);
}

fn about(payload) {
    return discord_reply("Oxid 0.8.0 bot module");
}

fn main() {
    let commands = [
        discord_command("ping", "Reply with pong", ping),
        discord_command("about", "Show runtime information", about)
    ];
    print discord_runtime_name();
    print discord_bot_plan("oxid-discord-demo", "0.8.0", "src/main.ox");
    print discord_dispatch(commands, "ping", "gateway-ready");
    print discord_runtime_note();
}
